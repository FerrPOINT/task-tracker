//! Append-only lease recovery for an already consumed input. No Workflow/native effects.
use super::execution_lease::{check_fence, clock, valid_receipt};
use super::*;
use domain::sdlc_execution_lease::*;
use serde::{Deserialize, Serialize};

const KIND: &str = "settled_input_expiry";

fn inconsistent() -> AppError {
    AppError::conflict("settled PM input recovery source inconsistent")
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    kind: PmSettledInputRecoveryKind,
    command: ResumePmExecutionLease,
    witness: PmSettledInputRecoveryWitness,
    before: ExecutionLeaseReceipt,
    transaction_id: String,
}

fn original_body(command: &ResumePmExecutionLease) -> Result<Json, AppError> {
    let mut value = serde_json::to_value(command).map_err(AppError::internal)?;
    let fields = value.as_object_mut().ok_or_else(inconsistent)?;
    for key in [
        "idempotency_key",
        "lease_id",
        "expected_lease_version",
        "recovery",
    ] {
        fields.remove(key);
    }
    Ok(value)
}

impl PostgresSdlcRepository {
    // One logical route namespace. Never choose a cross-ledger result by query order.
    pub(super) async fn resume_route_operation_source(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
        key: &str,
    ) -> Result<Option<ExecutionLeaseOperation>, AppError> {
        let rows = tx.query_all(statement(
            "SELECT 'input' AS source,actor_subject FROM sdlc_pm_resume_inputs WHERE task_id=$1 AND idempotency_key=$2
             UNION ALL SELECT 'lease',o.actor_subject FROM sdlc_pm_lease_operations o
             JOIN sdlc_pm_executions e USING(execution_id) WHERE e.task_id=$1 AND o.idempotency_key=$2",
            vec![state.task_id.into(), key.into()],
        )).await.map_err(map_db)?;
        if rows.is_empty() {
            return Ok(None);
        }
        if rows.len() != 1 {
            return Err(AppError::conflict("ambiguous PM resume operation key"));
        }
        if rows[0]
            .try_get::<String>("", "actor_subject")
            .map_err(map_db)?
            != actor.subject
        {
            return Err(AppError::Forbidden);
        }
        if rows[0].try_get::<String>("", "source").map_err(map_db)? == "input" {
            return self.resume_operation_source(tx, state, actor, key).await;
        }
        let row = tx.query_one(statement(
            "SELECT o.execution_id,o.lease_version,o.payload_hash,o.result,o.recovery FROM sdlc_pm_lease_operations o
             JOIN sdlc_pm_executions e USING(execution_id)
             WHERE e.task_id=$1 AND o.actor_subject=$2 AND o.idempotency_key=$3",
            vec![state.task_id.into(), actor.subject.clone().into(), key.into()],
        )).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        let evidence: Evidence = serde_json::from_value(
            row.try_get::<Option<Json>>("", "recovery")
                .map_err(map_db)?
                .ok_or_else(inconsistent)?,
        )
        .map_err(|_| inconsistent())?;
        let raw: Json = row.try_get("", "result").map_err(map_db)?;
        let result: ExecutionLeaseReceipt =
            serde_json::from_value(raw.clone()).map_err(|_| inconsistent())?;
        let reservation = Self::reservation(tx, state, result.fence.execution_id).await?;
        valid_receipt(&result, &reservation)?;
        valid_receipt(&evidence.before, &reservation)?;
        app::sdlc_pm_resume::validate_settled_witness(
            &evidence.witness,
            &reservation,
            &evidence.command,
        )?;
        let c = &evidence.command;
        let hash = app::sdlc_pm_resume::request_hash(c)?;
        if c.idempotency_key != key
            || hash != row.try_get::<String>("", "payload_hash").map_err(map_db)?
            || result.fence.execution_id
                != row.try_get::<Uuid>("", "execution_id").map_err(map_db)?
            || result.lease.version != row.try_get::<i64>("", "lease_version").map_err(map_db)?
            || result.lease.version != c.expected_lease_version + 1
            || evidence.before.lease.version != c.expected_lease_version
            || evidence.before.lease.lease_id != c.lease_id
            || result.lease.lease_id != c.lease_id
            || result.lease.claimed_at != evidence.before.lease.claimed_at
            || result.lease.holder_subject != evidence.before.lease.holder_subject
            || result.lease.heartbeat_at < evidence.before.lease.expires_at
            || serde_json::to_value(&result).map_err(AppError::internal)? != raw
            || serde_json::to_value(&result).map_err(AppError::internal)?["binding"]
                != serde_json::to_value(&evidence.before).map_err(AppError::internal)?["binding"]
            || result.owner_version != c.expected_owner_version
            || result.fence != c.checkpoint.fence
        {
            return Err(inconsistent());
        }
        // Operation GET/replay remains historical, including after a lost ACK/expiry.
        Ok(Some(ExecutionLeaseOperation {
            idempotency_key: key.into(),
            request_sha256: hash,
            result,
        }))
    }

    async fn settled_input_context(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
        command: &ResumePmExecutionLease,
    ) -> Result<(PmDraftReservation, ExecutionLeaseReceipt), AppError> {
        let recovery = command.recovery.as_ref().ok_or_else(inconsistent)?;
        let original = self
            .resume_operation_source(tx, state, actor, &recovery.original_key)
            .await?
            .ok_or_else(inconsistent)?;
        if original.request_sha256 != recovery.original_request_sha256 {
            return Err(inconsistent());
        }
        let row = tx.query_one(statement(
            "SELECT command FROM sdlc_pm_resume_inputs WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![state.task_id.into(), actor.subject.clone().into(), recovery.original_key.clone().into()],
        )).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        let original_command: ResumePmExecutionLease =
            serde_json::from_value(row.try_get::<Json>("", "command").map_err(map_db)?)
                .map_err(|_| inconsistent())?;
        if original_command.recovery.is_some()
            || original_body(&original_command)? != original_body(command)?
            || command.lease_id != original.result.lease.lease_id
        {
            return Err(inconsistent());
        }
        let f = &command.checkpoint.fence;
        let row = tx.query_one(statement(
            "SELECT pm_execution_id,pm_owner_version,pm_admission_state FROM sdlc_tasks WHERE task_id=$1",
            vec![state.task_id.into()],
        )).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if row
            .try_get::<Option<Uuid>>("", "pm_execution_id")
            .map_err(map_db)?
            != Some(f.execution_id)
            || row.try_get::<i64>("", "pm_owner_version").map_err(map_db)?
                != command.expected_owner_version
            || row
                .try_get::<Option<String>>("", "pm_admission_state")
                .map_err(map_db)?
                .as_deref()
                != Some("reserved")
        {
            return Err(inconsistent());
        }
        let reservation = Self::reservation(tx, state, f.execution_id).await?;
        app::sdlc_pm_resume::validate_context(state, &reservation, command)?;
        check_fence(&reservation, command.expected_owner_version, f)?;
        let holder = Principal {
            subject: reservation.assignment.machine_subject.clone(),
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
            != serde_json::to_value(&reservation.assignment).map_err(AppError::internal)?
        {
            return Err(inconsistent());
        }
        self.resume_input(tx, state, command).await?;
        let row = tx.query_one(statement(
            "SELECT EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_pm_completions WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_pm_resume_inputs WHERE execution_id=$2 AND lease_version>$3) AS busy",
            vec![state.task_id.into(), f.execution_id.into(), original.result.lease.version.into()],
        )).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if row.try_get::<bool>("", "busy").map_err(map_db)? {
            return Err(AppError::conflict(
                "settled PM input is superseded or completed",
            ));
        }
        let chain = tx.query_all(statement(
            "SELECT idempotency_key,payload_hash FROM sdlc_pm_lease_operations WHERE execution_id=$1
             AND recovery->>'kind'=$2 AND recovery#>>'{command,recovery,original_key}'=$3
             AND recovery#>>'{command,recovery,original_request_sha256}'=$4 ORDER BY lease_version DESC",
            vec![f.execution_id.into(), KIND.into(), recovery.original_key.clone().into(), recovery.original_request_sha256.clone().into()],
        )).await.map_err(map_db)?;
        if chain.len() >= 3 {
            return Err(AppError::conflict(
                "settled PM recovery attempt limit reached",
            ));
        }
        let (key, hash) = match chain.first() {
            Some(row) => (
                row.try_get::<String>("", "idempotency_key")
                    .map_err(map_db)?,
                row.try_get::<String>("", "payload_hash").map_err(map_db)?,
            ),
            None => (
                original.idempotency_key.clone(),
                original.request_sha256.clone(),
            ),
        };
        if recovery.predecessor_key != key || recovery.predecessor_request_sha256 != hash {
            return Err(AppError::conflict(
                "settled PM recovery predecessor changed",
            ));
        }
        let predecessor = self
            .resume_route_operation_source(tx, state, actor, &key)
            .await?
            .ok_or_else(inconsistent)?;
        let current = Self::lease_current(tx, &reservation)
            .await?
            .ok_or_else(inconsistent)?;
        if current.lease.lease_id != command.lease_id
            || current.lease.version != command.expected_lease_version
            || current.lease.version >= MAX_SAFE_VERSION
            || current.lease.expires_at > clock(tx).await?
            || current.lease.claimed_at != predecessor.result.lease.claimed_at
            || current.lease.holder_subject != predecessor.result.lease.holder_subject
            || current.lease.version < predecessor.result.lease.version
            || current.lease.heartbeat_at < predecessor.result.lease.heartbeat_at
        {
            return Err(AppError::conflict(
                "settled PM recovery requires exact current expired lease",
            ));
        }
        Ok((reservation, current))
    }

    pub(super) async fn preflight_settled_input_recovery(
        &self,
        task: Uuid,
        actor: &Principal,
        command: &ResumePmExecutionLease,
    ) -> Result<Option<ExecutionLeaseReceipt>, AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        app::sdlc_pm_resume::validate_command(command)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        let saved = self
            .resume_route_operation_source(&tx, &state, actor, &command.idempotency_key)
            .await?;
        let result = if let Some(saved) = saved {
            if saved.request_sha256 != app::sdlc_pm_resume::request_hash(command)? {
                return Err(inconsistent());
            }
            Some(saved.result)
        } else {
            self.settled_input_context(&tx, &state, actor, command)
                .await?;
            None
        };
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }

    pub(super) async fn recover_settled_input_lease(
        &self,
        task: Uuid,
        actor: &Principal,
        command: ResumePmExecutionLease,
        witness: PmSettledInputRecoveryWitness,
    ) -> Result<(ExecutionLeaseReceipt, bool), AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        app::sdlc_pm_resume::validate_command(&command)?;
        let hash = app::sdlc_pm_resume::request_hash(&command)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        if let Some(saved) = self
            .resume_route_operation_source(&tx, &state, actor, &command.idempotency_key)
            .await?
        {
            if saved.request_sha256 != hash {
                return Err(inconsistent());
            }
            tx.commit().await.map_err(map_db)?;
            return Ok((saved.result, true));
        }
        let (reservation, mut current) = self
            .settled_input_context(&tx, &state, actor, &command)
            .await?;
        app::sdlc_pm_resume::validate_settled_witness(&witness, &reservation, &command)?;
        let before = current.clone();
        let now = clock(&tx).await?;
        current.lease.version += 1;
        current.lease.heartbeat_at = now;
        current.lease.expires_at = now + chrono::Duration::seconds(LEASE_TTL_SECONDS);
        exec(&tx,"INSERT INTO sdlc_pm_lease_operations(execution_id,actor_subject,idempotency_key,payload_hash,lease_version,result,recovery)
            VALUES($1,$2,$3,$4,$5,$6,jsonb_build_object('kind',$7::text,'command',$8::jsonb,'witness',$9::jsonb,'before',$10::jsonb,'transaction_id',pg_current_xact_id()::text))",
            vec![current.fence.execution_id.into(),actor.subject.clone().into(),command.idempotency_key.clone().into(),hash.into(),
                current.lease.version.into(),json_value(&current)?,KIND.into(),json_value(&command)?,json_value(&witness)?,json_value(&before)?]).await?;
        exec(&tx,"UPDATE sdlc_pm_execution_leases SET version=$2,heartbeat_at=$3,expires_at=$4,result=$5 WHERE execution_id=$1 AND version=$6",
            vec![current.fence.execution_id.into(),current.lease.version.into(),now.into(),current.lease.expires_at.into(),json_value(&current)?,command.expected_lease_version.into()]).await?;
        tx.commit().await.map_err(map_db)?;
        Ok((current, false))
    }
}
