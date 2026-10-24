//! Existing task/lease locks and idempotency ledger; no lease or business mutation.
use super::execution_lease::{check_fence, clock};
use super::*;
use chrono::Duration;
use domain::sdlc_execution_lease::ExecutionLeaseReceipt;
use domain::sdlc_lifecycle::PmFeedbackReceipt;
use domain::sdlc_terminal_idle_recovery::*;

fn invalid() -> AppError {
    AppError::conflict("terminal-idle PM checkpoint source inconsistent")
}

impl PostgresSdlcRepository {
    async fn terminal_idle_operation_source(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
        key: &str,
    ) -> Result<Option<PermitPmIdleCheckpointResponse>, AppError> {
        // No query-order choice across independent ledgers or actors.
        let rows = tx.query_all(statement(
            "SELECT 'generic' AS source,actor_subject,payload_hash,result FROM sdlc_idempotency WHERE task_id=$1 AND idempotency_key=$2
             UNION ALL SELECT 'input',actor_subject,payload_hash,result FROM sdlc_pm_resume_inputs WHERE task_id=$1 AND idempotency_key=$2
             UNION ALL SELECT 'lease',o.actor_subject,o.payload_hash,o.result FROM sdlc_pm_lease_operations o
             JOIN sdlc_pm_executions e USING(execution_id) WHERE e.task_id=$1 AND o.idempotency_key=$2",
            vec![state.task_id.into(),key.into()])).await.map_err(map_db)?;
        if rows.is_empty() {
            return Ok(None);
        }
        if rows.len() != 1 {
            return Err(AppError::conflict("ambiguous PM resume operation key"));
        }
        let row = &rows[0];
        if row.try_get::<String>("", "actor_subject").map_err(map_db)? != actor.subject {
            return Err(AppError::Forbidden);
        }
        if row.try_get::<String>("", "source").map_err(map_db)? != "generic" {
            return Err(invalid());
        }
        let raw: Json = row.try_get("", "result").map_err(map_db)?;
        let mut value: PermitPmIdleCheckpointResponse =
            serde_json::from_value(raw.clone()).map_err(|_| invalid())?;
        let p = &value.permit;
        if serde_json::to_value(&value).map_err(AppError::internal)? != raw
            || value.contract_version != 1
            || value.idempotency_key != key
            || value.request_sha256 != row.try_get::<String>("", "payload_hash").map_err(map_db)?
            || !domain::sdlc_native_admission::hash(&value.request_sha256)
            || key != format!("fleet-pm-terminal-idle:{}:{}", p.session_run_id, p.input_id)
            || p.permit_id.is_nil()
            || p.barrier_id.is_nil()
            || p.session_run_id.is_nil()
            || p.input_id.is_nil()
            || !domain::sdlc_native_admission::hash(&p.checkpoint_sha256)
            || p.expires_at <= p.issued_at
            || p.expires_at - p.issued_at > Duration::seconds(30)
            || value.observed_at < p.issued_at
        {
            return Err(invalid());
        }
        app::sdlc::validate_key(&p.checkpoint_operation_key)?;
        value.observed_at = clock(tx).await?;
        if value.observed_at < value.permit.issued_at {
            return Err(invalid());
        }
        // Expired permits are historical evidence; replay never extends their core.
        Ok(Some(value))
    }

    async fn terminal_idle_context(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
        c: &PermitPmIdleCheckpointRequest,
    ) -> Result<(PmDraftReservation, PmFeedbackReceipt, ExecutionLeaseReceipt), AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        let reservation = Self::reservation(tx, state, c.fence.execution_id).await?;
        app::sdlc_terminal_idle_recovery::validate_context(state, &reservation, c)?;
        check_fence(&reservation, c.expected_owner_version, &c.fence)?;
        let row = tx.query_one(statement(
            "SELECT pm_execution_id,pm_owner_version,pm_admission_state FROM sdlc_tasks WHERE task_id=$1",
            vec![state.task_id.into()])).await.map_err(map_db)?.ok_or_else(invalid)?;
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
            return Err(invalid());
        }
        let holder = Principal {
            subject: reservation.assignment.machine_subject.clone(),
            human_session: false,
            trusted_human: false,
            scopes: Default::default(),
        };
        Self::project_access(tx, state.project_id, &holder).await?;
        let row = tx.query_one(statement(
            "SELECT payload FROM sdlc_assignments WHERE task_id=$1 ORDER BY version DESC LIMIT 1", vec![state.task_id.into()]))
            .await.map_err(map_db)?.ok_or_else(invalid)?;
        if row.try_get::<Json>("", "payload").map_err(map_db)?
            != serde_json::to_value(&reservation.assignment).map_err(AppError::internal)?
        {
            return Err(invalid());
        }
        let row = tx.query_one(statement(
            "SELECT actor_subject,payload FROM sdlc_pm_feedback WHERE task_id=$1 AND message_id=$2",
            vec![state.task_id.into(),c.input.input_id().into()])).await.map_err(map_db)?.ok_or_else(invalid)?;
        let raw: Json = row.try_get("", "payload").map_err(map_db)?;
        let feedback: PmFeedbackReceipt =
            serde_json::from_value(raw.clone()).map_err(|_| invalid())?;
        if row.try_get::<String>("", "actor_subject").map_err(map_db)? != state.owner_subject
            || serde_json::to_value(&feedback).map_err(AppError::internal)? != raw
        {
            return Err(invalid());
        }
        app::sdlc_terminal_idle_recovery::validate_feedback(state, c, &feedback)?;
        let row = tx
            .query_one(statement(
                "SELECT EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_pm_completions WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_requirements WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_pm_resume_inputs WHERE task_id=$1 AND
                (previous_run_id=$2 OR (input_kind='feedback' AND input_id=$3))) AS busy",
                vec![
                    state.task_id.into(),
                    c.previous_run_id.into(),
                    c.input.input_id().into(),
                ],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(invalid)?;
        if row.try_get::<bool>("", "busy").map_err(map_db)? {
            return Err(invalid());
        }
        let current = Self::lease_current(tx, &reservation)
            .await?
            .ok_or_else(invalid)?;
        if current.lease.lease_id != c.lease_id
            || current.lease.version != c.expected_lease_version
            || current.lease.expires_at > clock(tx).await?
        {
            return Err(AppError::conflict(
                "terminal-idle permit requires exact expired canonical lease",
            ));
        }
        Ok((reservation, feedback, current))
    }

    pub(super) async fn preflight_terminal_idle(
        &self,
        task: Uuid,
        actor: &Principal,
        c: &PermitPmIdleCheckpointRequest,
    ) -> Result<Option<PermitPmIdleCheckpointResponse>, AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        app::sdlc_terminal_idle_recovery::validate_command(c)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        let saved = self
            .terminal_idle_operation_source(&tx, &state, actor, &c.idempotency_key)
            .await?;
        if let Some(value) = &saved {
            app::sdlc_terminal_idle_recovery::validate_response(c, value)?;
        } else {
            self.terminal_idle_context(&tx, &state, actor, c).await?;
        }
        tx.commit().await.map_err(map_db)?;
        Ok(saved)
    }

    pub(super) async fn permit_terminal_idle(
        &self,
        task: Uuid,
        actor: &Principal,
        c: PermitPmIdleCheckpointRequest,
        w: PmTerminalIdleRecoveryWitness,
    ) -> Result<(PermitPmIdleCheckpointResponse, bool), AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        app::sdlc_terminal_idle_recovery::validate_command(&c)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        if let Some(value) = self
            .terminal_idle_operation_source(&tx, &state, actor, &c.idempotency_key)
            .await?
        {
            app::sdlc_terminal_idle_recovery::validate_response(&c, &value)?;
            tx.commit().await.map_err(map_db)?;
            return Ok((value, true));
        }
        let (reservation, feedback, _unchanged_lease) =
            self.terminal_idle_context(&tx, &state, actor, &c).await?;
        app::sdlc_terminal_idle_recovery::validate_witness(
            &w,
            &state,
            &reservation,
            &c,
            &feedback,
        )?;
        let now = clock(&tx).await?;
        let value = PermitPmIdleCheckpointResponse {
            contract_version: 1,
            operation: IdleCheckpointOperation::PermitPmIdleCheckpoint,
            idempotency_key: c.idempotency_key.clone(),
            request_sha256: app::sdlc_terminal_idle_recovery::request_hash(&c)?,
            permit: PmIdleCheckpointPermit {
                permit_id: Uuid::new_v4(),
                scope: IdleCheckpointScope::IdleCheckpoint,
                barrier_id: c.terminal_idle_recovery.barrier_id,
                session_run_id: c.previous_run_id,
                input_id: c.input.input_id(),
                checkpoint_operation_key: c.terminal_idle_recovery.checkpoint_operation_key.clone(),
                checkpoint_sha256: c.terminal_idle_recovery.checkpoint_sha256.clone(),
                issued_at: now,
                expires_at: now + Duration::seconds(30),
            },
            observed_at: now,
        };
        app::sdlc_terminal_idle_recovery::validate_response(&c, &value)?;
        let result = serde_json::to_value(&value).map_err(AppError::internal)?;
        exec(&tx, "INSERT INTO sdlc_idempotency(task_id,actor_subject,idempotency_key,payload_hash,result) VALUES($1,$2,$3,$4,$5)",
            vec![task.into(),actor.subject.clone().into(),c.idempotency_key.into(),value.request_sha256.clone().into(),result.into()]).await?;
        tx.commit().await.map_err(map_db)?;
        Ok((value, false))
    }

    pub(super) async fn terminal_idle_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<PermitPmIdleCheckpointResponse, AppError> {
        app::sdlc_lifecycle::scheduler_read(actor, &self.config)?;
        app::sdlc::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        let value = self
            .terminal_idle_operation_source(&tx, &state, actor, key)
            .await?
            .ok_or_else(|| AppError::not_found("PM idle checkpoint operation", key))?;
        tx.commit().await.map_err(map_db)?;
        Ok(value)
    }
}
