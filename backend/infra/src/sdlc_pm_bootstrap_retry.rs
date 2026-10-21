use super::execution_lease::{clock, valid_receipt};
use super::*;
use app::sdlc_pm_bootstrap_retry::{BootstrapRetryService, BootstrapRetrySource, request_hash};
use domain::sdlc_execution_lease::*;
use domain::sdlc_pm_bootstrap_retry::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetryEvidence {
    kind: String,
    command: BootstrapRetryCommand,
    witness: BootstrapFailureReadback,
    before: ExecutionLeaseReceipt,
    reservation: PmDraftReservation,
    transaction_id: String,
}

fn inconsistent() -> AppError {
    AppError::conflict("PM bootstrap retry source inconsistent")
}

impl PostgresSdlcRepository {
    // History replay precedes fresh context/witness checks, including after later heartbeats.
    async fn bootstrap_retry_saved(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
        key: &str,
    ) -> Result<Option<(BootstrapRetryCommand, ExecutionLeaseOperation)>, AppError> {
        let rows = tx.query_all(statement(
            "SELECT o.payload_hash,o.lease_version,o.result,o.recovery FROM sdlc_pm_lease_operations o
             JOIN sdlc_pm_executions e USING(execution_id)
             WHERE e.task_id=$1 AND o.actor_subject=$2 AND o.idempotency_key=$3",
            vec![state.task_id.into(),actor.subject.clone().into(),key.into()])).await.map_err(map_db)?;
        if rows.is_empty() {
            return Ok(None);
        }
        if rows.len() != 1 {
            return Err(inconsistent());
        }
        let row = &rows[0];
        let raw: Json = row.try_get("", "result").map_err(map_db)?;
        let evidence: RetryEvidence = serde_json::from_value(
            row.try_get::<Option<Json>>("", "recovery")
                .map_err(map_db)?
                .ok_or_else(inconsistent)?,
        )
        .map_err(|_| inconsistent())?;
        let result: ExecutionLeaseReceipt =
            serde_json::from_value(raw.clone()).map_err(|_| inconsistent())?;
        let operation = ExecutionLeaseOperation {
            idempotency_key: key.into(),
            request_sha256: row.try_get("", "payload_hash").map_err(map_db)?,
            result,
        };
        if evidence.kind != "bootstrap_retry"
            || evidence.transaction_id.is_empty()
            || evidence.reservation.binding.task_id != state.task_id
            || evidence.reservation.binding.project_id != state.project_id
            || evidence.reservation.binding.owner_subject != state.owner_subject
            || operation.result.lease.version
                != row.try_get::<i64>("", "lease_version").map_err(map_db)?
            || serde_json::to_value(&operation.result).map_err(AppError::internal)? != raw
        {
            return Err(inconsistent());
        }
        BootstrapRetryService::replay(
            actor,
            &self.config,
            &evidence.command,
            (&evidence.reservation, &evidence.before),
            (&actor.subject, &operation),
        )?;
        // Revalidate retained proof at its original DB timestamp, not today's liveness.
        app::sdlc_pm_bootstrap_retry::validate_witness(
            &evidence.reservation,
            &evidence.command,
            &evidence.witness,
            operation.result.lease.heartbeat_at,
        )?;
        let input = PmDraftInputResponse {
            contract_version: 1,
            tracker_instance_id: state.tracker_instance_id.clone(),
            project_id: state.project_id,
            task_id: state.task_id,
            root_task_id: state.root_task_id,
            owner_subject: state.owner_subject.clone(),
            input: Self::original_input(tx, state).await?,
        };
        // The immutable ledger/SQL gate binds these rows. Fresh stage changes must not
        // invalidate an exact historical replay, so only the retained proof is checked here.
        if input.input.snapshot_ref != evidence.reservation.input.snapshot_ref
            || input.input.sha256 != evidence.reservation.input.sha256
        {
            return Err(inconsistent());
        }
        let retained = Self::lease_current(tx, &evidence.reservation)
            .await?
            .ok_or_else(inconsistent)?;
        if retained.lease.lease_id != operation.result.lease.lease_id
            || retained.lease.claimed_at != operation.result.lease.claimed_at
            || retained.lease.version < operation.result.lease.version
            || retained.lease.heartbeat_at < operation.result.lease.heartbeat_at
        {
            return Err(inconsistent());
        }
        Ok(Some((evidence.command, operation)))
    }

    async fn bootstrap_retry_context(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        c: &BootstrapRetryCommand,
    ) -> Result<
        (
            PmDraftReservation,
            ExecutionLeaseReceipt,
            PmDraftInputResponse,
        ),
        AppError,
    > {
        let row=tx.query_one(statement("SELECT pm_owner_version,pm_admission_state,pm_execution_id FROM sdlc_tasks WHERE task_id=$1",
            vec![state.task_id.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if row.try_get::<i64>("", "pm_owner_version").map_err(map_db)? != c.expected_owner_version
            || row
                .try_get::<Option<Uuid>>("", "pm_execution_id")
                .map_err(map_db)?
                != Some(c.fence.execution_id)
            || row
                .try_get::<Option<String>>("", "pm_admission_state")
                .map_err(map_db)?
                .as_deref()
                != Some("reserved")
        {
            return Err(inconsistent());
        }
        let r = Self::reservation(tx, state, c.fence.execution_id).await?;
        app::sdlc_pm_unsubmitted::validate_context(
            state,
            &r,
            &RecoverUnsubmittedPmExecutionLease {
                expected_owner_version: c.expected_owner_version,
                fence: c.fence.clone(),
                lease_id: c.lease_id,
                expected_lease_version: c.expected_lease_version,
                session_run_id: c.previous_session_run_id,
                barrier_id: c.barrier_id,
                idempotency_key: c.idempotency_key.clone(),
            },
        )?;
        let holder = Principal {
            subject: r.assignment.machine_subject.clone(),
            human_session: false,
            trusted_human: false,
            scopes: Default::default(),
        };
        Self::project_access(tx, state.project_id, &holder).await?;
        let latest=tx.query_one(statement("SELECT payload FROM sdlc_assignments WHERE task_id=$1 ORDER BY version DESC LIMIT 1",
            vec![state.task_id.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if latest.try_get::<Json>("", "payload").map_err(map_db)?
            != serde_json::to_value(&r.assignment).map_err(AppError::internal)?
        {
            return Err(inconsistent());
        }
        let row=tx.query_one(statement("SELECT
            EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=$1)
            OR EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=$1)
            OR EXISTS(SELECT 1 FROM sdlc_pm_completions WHERE task_id=$1)
            OR EXISTS(SELECT 1 FROM sdlc_pm_native_admissions WHERE execution_id=$2)
            OR EXISTS(SELECT 1 FROM sdlc_pm_resume_inputs WHERE execution_id=$2)
            OR EXISTS(SELECT 1 FROM sdlc_pm_lease_operations WHERE execution_id=$2 AND recovery->>'kind'='bootstrap_retry') AS busy",
            vec![state.task_id.into(),c.fence.execution_id.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if row.try_get::<bool>("", "busy").map_err(map_db)? {
            return Err(inconsistent());
        }
        let l = Self::lease_current(tx, &r)
            .await?
            .ok_or_else(inconsistent)?;
        valid_receipt(&l, &r)?;
        if l.lease.lease_id != c.lease_id
            || l.lease.version != c.expected_lease_version
            || l.lease.expires_at > clock(tx).await?
        {
            return Err(inconsistent());
        }
        let input = PmDraftInputResponse {
            contract_version: 1,
            tracker_instance_id: state.tracker_instance_id.clone(),
            project_id: state.project_id,
            task_id: state.task_id,
            root_task_id: state.root_task_id,
            owner_subject: state.owner_subject.clone(),
            input: Self::original_input(tx, state).await?,
        };
        Ok((r, l, input))
    }

    pub(super) async fn preflight_bootstrap_retry(
        &self,
        task: Uuid,
        actor: &Principal,
        c: &BootstrapRetryCommand,
    ) -> Result<Option<ExecutionLeaseReceipt>, AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        let hash = request_hash(c)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        let result = if let Some((_, op)) = self
            .bootstrap_retry_saved(&tx, &state, actor, &c.idempotency_key)
            .await?
        {
            if op.request_sha256 != hash {
                return Err(inconsistent());
            }
            Some(op.result)
        } else {
            self.bootstrap_retry_context(&tx, &state, c).await?;
            None
        };
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }

    pub(super) async fn commit_bootstrap_retry(
        &self,
        task: Uuid,
        actor: &Principal,
        c: BootstrapRetryCommand,
        w: BootstrapFailureReadback,
    ) -> Result<(ExecutionLeaseReceipt, bool), AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        let hash = request_hash(&c)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        if let Some((_, op)) = self
            .bootstrap_retry_saved(&tx, &state, actor, &c.idempotency_key)
            .await?
        {
            if op.request_sha256 != hash {
                return Err(inconsistent());
            }
            tx.commit().await.map_err(map_db)?;
            return Ok((op.result, true));
        }
        let (r, l, input) = self.bootstrap_retry_context(&tx, &state, &c).await?;
        let now = clock(&tx).await?;
        let observation = BootstrapRetryService::trusted_observation(&c, w.clone())?;
        let plan = BootstrapRetryService::prepare(
            BootstrapRetrySource {
                state: &state,
                reservation: &r,
                original_input: &input,
                lease: &l,
                owner_version: r.owner_cas.version,
                admission_state: "reserved",
                predecessor_consumed: false,
                delivery_owned: false,
                now,
            },
            actor,
            &self.config,
            &c,
            &observation,
        )?;
        let evidence = RetryEvidence {
            kind: "bootstrap_retry".into(),
            command: c.clone(),
            witness: w,
            before: l.clone(),
            reservation: r,
            transaction_id: tx
                .query_one(statement("SELECT pg_current_xact_id()::text AS id", vec![]))
                .await
                .map_err(map_db)?
                .ok_or_else(inconsistent)?
                .try_get("", "id")
                .map_err(map_db)?,
        };
        let result = plan.operation.result;
        exec(&tx,"INSERT INTO sdlc_pm_lease_operations(execution_id,actor_subject,idempotency_key,payload_hash,lease_version,result,recovery)
            VALUES($1,$2,$3,$4,$5,$6,$7)",vec![c.fence.execution_id.into(),actor.subject.clone().into(),c.idempotency_key.into(),
                hash.into(),result.lease.version.into(),json_value(&result)?,json_value(&evidence)?]).await?;
        let updated=tx.execute(statement("UPDATE sdlc_pm_execution_leases SET version=$2,heartbeat_at=$3,expires_at=$4,result=$5
            WHERE execution_id=$1 AND version=$6 AND lease_id=$7 AND result=$8 AND expires_at<=clock_timestamp()",
            vec![c.fence.execution_id.into(),result.lease.version.into(),now.into(),result.lease.expires_at.into(),json_value(&result)?,
                c.expected_lease_version.into(),c.lease_id.into(),json_value(&l)?])).await.map_err(map_db)?;
        if updated.rows_affected() != 1 {
            return Err(inconsistent());
        }
        tx.commit().await.map_err(map_db)?;
        Ok((result, false))
    }

    pub(super) async fn read_bootstrap_retry(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<ExecutionLeaseOperation, AppError> {
        app::sdlc_lifecycle::scheduler_read(actor, &self.config)?;
        app::sdlc::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        let (_, op) = self
            .bootstrap_retry_saved(&tx, &state, actor, key)
            .await?
            .ok_or_else(|| AppError::not_found("PM bootstrap retry operation", key))?;
        tx.commit().await.map_err(map_db)?;
        Ok(op)
    }
}
