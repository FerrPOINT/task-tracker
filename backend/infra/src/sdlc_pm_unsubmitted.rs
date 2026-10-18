use super::execution_lease::{check_fence, clock, valid_receipt};
use super::*;
use domain::sdlc_execution_lease::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RecoveryEvidence {
    command: RecoverUnsubmittedPmExecutionLease,
    witness: UnsubmittedPmWitness,
    transaction_id: String,
}

fn inconsistent() -> AppError {
    AppError::conflict("unsubmitted PM recovery source inconsistent")
}

impl PostgresSdlcRepository {
    async fn unsubmitted_recovery_source(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
        key: &str,
    ) -> Result<Option<ExecutionLeaseOperation>, AppError> {
        let rows = tx
            .query_all(statement(
                "SELECT o.execution_id,o.payload_hash,o.lease_version,o.result,o.recovery
             FROM sdlc_pm_lease_operations o JOIN sdlc_pm_executions e USING(execution_id)
             WHERE e.task_id=$1 AND o.actor_subject=$2 AND o.idempotency_key=$3",
                vec![
                    state.task_id.into(),
                    actor.subject.clone().into(),
                    key.into(),
                ],
            ))
            .await
            .map_err(map_db)?;
        if rows.is_empty() {
            return Ok(None);
        }
        if rows.len() != 1 {
            return Err(inconsistent());
        }
        let row = &rows[0];
        let evidence: RecoveryEvidence = serde_json::from_value(
            row.try_get::<Option<Json>>("", "recovery")
                .map_err(map_db)?
                .ok_or_else(inconsistent)?,
        )
        .map_err(|_| inconsistent())?;
        let raw: Json = row.try_get("", "result").map_err(map_db)?;
        let result: ExecutionLeaseReceipt =
            serde_json::from_value(raw.clone()).map_err(|_| inconsistent())?;
        let r = Self::reservation(tx, state, result.fence.execution_id).await?;
        valid_receipt(&result, &r)?;
        let c = &evidence.command;
        app::sdlc_pm_unsubmitted::validate_command(c)?;
        app::sdlc_pm_unsubmitted::validate_witness(&evidence.witness, &r, c)?;
        let hash = app::sdlc_pm_unsubmitted::request_hash(c)?;
        if c.idempotency_key != key
            || hash != row.try_get::<String>("", "payload_hash").map_err(map_db)?
            || result.fence != c.fence
            || result.owner_version != c.expected_owner_version
            || result.lease.lease_id != c.lease_id
            || result.lease.version != c.expected_lease_version + 1
            || result.lease.version != row.try_get::<i64>("", "lease_version").map_err(map_db)?
            || result.fence.execution_id
                != row.try_get::<Uuid>("", "execution_id").map_err(map_db)?
            || r.binding.owner_subject != state.owner_subject
            || r.binding.project_id != state.project_id
            || serde_json::to_value(&result).map_err(AppError::internal)? != raw
        {
            return Err(inconsistent());
        }
        let current = Self::lease_current(tx, &r)
            .await?
            .ok_or_else(inconsistent)?;
        if result.lease.lease_id != current.lease.lease_id
            || result.lease.claimed_at != current.lease.claimed_at
            || result.lease.version > current.lease.version
            || result.lease.heartbeat_at > current.lease.heartbeat_at
        {
            return Err(inconsistent());
        }
        Ok(Some(ExecutionLeaseOperation {
            idempotency_key: key.into(),
            request_sha256: hash,
            result,
        }))
    }

    async fn unsubmitted_recovery_context(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        c: &RecoverUnsubmittedPmExecutionLease,
    ) -> Result<(PmDraftReservation, ExecutionLeaseReceipt), AppError> {
        let row = tx.query_one(statement(
            "SELECT pm_execution_id,pm_owner_version,pm_admission_state FROM sdlc_tasks WHERE task_id=$1",
            vec![state.task_id.into()],
        )).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if row
            .try_get::<Option<Uuid>>("", "pm_execution_id")
            .map_err(map_db)?
            != Some(c.fence.execution_id)
            || row.try_get::<i64>("", "pm_owner_version").map_err(map_db)?
                != c.expected_owner_version
            || row
                .try_get::<Option<String>>("", "pm_admission_state")
                .map_err(map_db)?
                .as_deref()
                != Some("reserved")
        {
            return Err(AppError::conflict(
                "current initial reserved PM execution required",
            ));
        }
        let r = Self::reservation(tx, state, c.fence.execution_id).await?;
        app::sdlc_pm_unsubmitted::validate_context(state, &r, c)?;
        check_fence(&r, c.expected_owner_version, &c.fence)?;
        let holder = Principal {
            subject: r.assignment.machine_subject.clone(),
            human_session: false,
            trusted_human: false,
            scopes: Default::default(),
        };
        Self::project_access(tx, state.project_id, &holder).await?;
        let row = tx.query_one(statement(
            "SELECT payload FROM sdlc_assignments WHERE task_id=$1 ORDER BY version DESC LIMIT 1",
            vec![state.task_id.into()],
        )).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if row.try_get::<Json>("", "payload").map_err(map_db)?
            != serde_json::to_value(&r.assignment).map_err(AppError::internal)?
        {
            return Err(inconsistent());
        }
        let row = tx
            .query_one(statement(
                "SELECT EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_pm_completions WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_pm_native_admissions WHERE execution_id=$2)
             OR EXISTS(SELECT 1 FROM sdlc_pm_resume_inputs WHERE execution_id=$2) AS busy",
                vec![state.task_id.into(), c.fence.execution_id.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(inconsistent)?;
        if row.try_get::<bool>("", "busy").map_err(map_db)? {
            return Err(AppError::conflict(
                "PM execution is no longer initially unsubmitted",
            ));
        }
        let current = Self::lease_current(tx, &r)
            .await?
            .ok_or_else(inconsistent)?;
        if current.lease.lease_id != c.lease_id
            || current.lease.version != c.expected_lease_version
            || current.lease.version >= MAX_SAFE_VERSION
            || current.lease.expires_at > clock(tx).await?
        {
            return Err(AppError::conflict(
                "PM recovery requires exact expired lease; active leases use heartbeat",
            ));
        }
        Ok((r, current))
    }

    pub(super) async fn preflight_unsubmitted_recovery(
        &self,
        task: Uuid,
        actor: &Principal,
        c: &RecoverUnsubmittedPmExecutionLease,
    ) -> Result<Option<ExecutionLeaseReceipt>, AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        app::sdlc_pm_unsubmitted::validate_command(c)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        let replay = self
            .unsubmitted_recovery_source(&tx, &state, actor, &c.idempotency_key)
            .await?;
        let result = if let Some(saved) = replay {
            if saved.request_sha256 != app::sdlc_pm_unsubmitted::request_hash(c)? {
                return Err(AppError::conflict("PM recovery key payload conflict"));
            }
            Some(saved.result)
        } else {
            self.unsubmitted_recovery_context(&tx, &state, c).await?;
            None
        };
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }

    pub(super) async fn recover_unsubmitted_pm_lease(
        &self,
        task: Uuid,
        actor: &Principal,
        c: RecoverUnsubmittedPmExecutionLease,
        w: UnsubmittedPmWitness,
    ) -> Result<(ExecutionLeaseReceipt, bool), AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        app::sdlc_pm_unsubmitted::validate_command(&c)?;
        let hash = app::sdlc_pm_unsubmitted::request_hash(&c)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        if let Some(saved) = self
            .unsubmitted_recovery_source(&tx, &state, actor, &c.idempotency_key)
            .await?
        {
            if saved.request_sha256 != hash {
                return Err(AppError::conflict("PM recovery key payload conflict"));
            }
            tx.commit().await.map_err(map_db)?;
            return Ok((saved.result, true));
        }
        let (r, mut current) = self.unsubmitted_recovery_context(&tx, &state, &c).await?;
        app::sdlc_pm_unsubmitted::validate_witness(&w, &r, &c)?;
        let now = clock(&tx).await?;
        current.lease.version += 1;
        current.lease.heartbeat_at = now;
        current.lease.expires_at = now + chrono::Duration::seconds(LEASE_TTL_SECONDS);
        exec(&tx,"INSERT INTO sdlc_pm_lease_operations(execution_id,actor_subject,idempotency_key,payload_hash,lease_version,result,recovery)
            VALUES($1,$2,$3,$4,$5,$6,jsonb_build_object('command',$7::jsonb,'witness',$8::jsonb,'transaction_id',pg_current_xact_id()::text))",
            vec![c.fence.execution_id.into(),actor.subject.clone().into(),c.idempotency_key.clone().into(),hash.into(),current.lease.version.into(),
                json_value(&current)?,json_value(&c)?,json_value(&w)?]).await?;
        let updated = tx.execute(statement("UPDATE sdlc_pm_execution_leases SET version=$2,heartbeat_at=$3,expires_at=$4,result=$5
            WHERE execution_id=$1 AND version=$6 AND lease_id=$7 AND expires_at<=clock_timestamp()",
            vec![c.fence.execution_id.into(),current.lease.version.into(),now.into(),current.lease.expires_at.into(),json_value(&current)?,
                c.expected_lease_version.into(),c.lease_id.into()])).await.map_err(map_db)?;
        if updated.rows_affected() != 1 {
            return Err(inconsistent());
        }
        tx.commit().await.map_err(map_db)?;
        Ok((current, false))
    }

    pub(super) async fn read_unsubmitted_recovery(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<ExecutionLeaseOperation, AppError> {
        app::sdlc_lifecycle::scheduler_read(actor, &self.config)?;
        app::sdlc::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        let result = self
            .unsubmitted_recovery_source(&tx, &state, actor, key)
            .await?
            .ok_or_else(|| AppError::not_found("unsubmitted PM recovery operation", key))?;
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }
}
