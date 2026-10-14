use super::execution_lease::{check_fence, clock, valid_receipt};
use super::*;
use chrono::Duration;
use domain::sdlc_execution_lease::*;

fn inconsistent() -> AppError {
    AppError::conflict("PM resume source inconsistent")
}

impl PostgresSdlcRepository {
    async fn resume_operation_source(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
        key: &str,
    ) -> Result<Option<ExecutionLeaseOperation>, AppError> {
        let row=tx.query_one(statement("SELECT execution_id,lease_version,payload_hash,command,result FROM sdlc_pm_resume_inputs
            WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",vec![state.task_id.into(),actor.subject.clone().into(),key.into()]))
            .await.map_err(map_db)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let raw: Json = row.try_get("", "result").map_err(map_db)?;
        let result: ExecutionLeaseReceipt =
            serde_json::from_value(raw.clone()).map_err(|_| inconsistent())?;
        let command: ResumePmExecutionLease =
            serde_json::from_value(row.try_get::<Json>("", "command").map_err(map_db)?)
                .map_err(|_| inconsistent())?;
        let request_sha256: String = row.try_get("", "payload_hash").map_err(map_db)?;
        app::sdlc_pm_resume::validate_command(&command)?;
        let reservation = Self::reservation(tx, state, result.fence.execution_id).await?;
        valid_receipt(&result, &reservation)?;
        if command.idempotency_key != key
            || request_sha256 != app::sdlc_pm_resume::request_hash(&command)?
            || result.fence != command.checkpoint.fence
            || result.owner_version != command.expected_owner_version
            || result.lease.lease_id != command.lease_id
            || result.lease.version != command.expected_lease_version + 1
            || result.fence.execution_id
                != row.try_get::<Uuid>("", "execution_id").map_err(map_db)?
            || result.lease.version != row.try_get::<i64>("", "lease_version").map_err(map_db)?
            || serde_json::to_value(&result).map_err(AppError::internal)? != raw
        {
            return Err(inconsistent());
        }
        Ok(Some(ExecutionLeaseOperation {
            idempotency_key: key.into(),
            request_sha256,
            result,
        }))
    }

    async fn resume_input(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        c: &ResumePmExecutionLease,
    ) -> Result<(), AppError> {
        if let Some(revision) = c.checkpoint.requirement_revision {
            let row = tx
                .query_one(statement(
                    "SELECT content_hash FROM sdlc_requirements WHERE task_id=$1 AND revision=$2",
                    vec![state.task_id.into(), revision.into()],
                ))
                .await
                .map_err(map_db)?
                .ok_or_else(inconsistent)?;
            if Some(row.try_get::<String>("", "content_hash").map_err(map_db)?)
                != c.checkpoint.content_hash
            {
                return Err(inconsistent());
            }
        }
        let id = c.input.input_id();
        match c.input {
            PmResumeInput::Answer { .. } => {
                let row=tx.query_one(statement("SELECT question_id,question_version,requirement_revision,payload FROM sdlc_answers WHERE task_id=$1 AND id=$2",
                    vec![state.task_id.into(),id.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
                let raw: Json = row.try_get("", "payload").map_err(map_db)?;
                let answer: Answer =
                    serde_json::from_value(raw.clone()).map_err(|_| inconsistent())?;
                if answer.question_id != row.try_get::<Uuid>("", "question_id").map_err(map_db)?
                    || answer.question_version
                        != row.try_get::<i64>("", "question_version").map_err(map_db)?
                    || answer.requirement_revision
                        != row
                            .try_get::<i64>("", "requirement_revision")
                            .map_err(map_db)?
                    || serde_json::to_value(&answer).map_err(AppError::internal)? != raw
                {
                    return Err(inconsistent());
                }
                app::sdlc_pm_resume::validate_answer(state, c, &answer)?;
            }
            PmResumeInput::Feedback { .. } => {
                let row=tx.query_one(statement("SELECT actor_subject,payload FROM sdlc_pm_feedback WHERE task_id=$1 AND message_id=$2",
                    vec![state.task_id.into(),id.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
                let raw: Json = row.try_get("", "payload").map_err(map_db)?;
                let f: domain::sdlc_lifecycle::PmFeedbackReceipt =
                    serde_json::from_value(raw.clone()).map_err(|_| inconsistent())?;
                if f.message_id != id
                    || f.task_id != state.task_id
                    || f.root_task_id != state.task_id
                    || f.project_id != state.project_id
                    || f.tracker_instance_id != state.tracker_instance_id
                    || f.owner_subject != state.owner_subject
                    || f.owner_subject
                        != row.try_get::<String>("", "actor_subject").map_err(map_db)?
                    || f.fence != c.checkpoint.fence
                    || f.requirement_revision != c.checkpoint.requirement_revision
                    || f.content_hash != app::sdlc::canonical_hash(&json!({"text":f.text}))?
                    || serde_json::to_value(&f).map_err(AppError::internal)? != raw
                {
                    return Err(inconsistent());
                }
            }
            PmResumeInput::Confirmation { .. } => {
                let revision = c.checkpoint.requirement_revision.ok_or_else(inconsistent)?;
                let row=tx.query_one(statement("SELECT revision,content_hash,payload FROM sdlc_confirmations WHERE task_id=$1 AND id=$2",
                    vec![state.task_id.into(),id.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
                let raw: Json = row.try_get("", "payload").map_err(map_db)?;
                let confirmation: Confirmation =
                    serde_json::from_value(raw.clone()).map_err(|_| inconsistent())?;
                let hash: String = row.try_get("", "content_hash").map_err(map_db)?;
                if confirmation.id != id
                    || row.try_get::<i64>("", "revision").map_err(map_db)? != revision
                    || serde_json::to_value(&confirmation).map_err(AppError::internal)? != raw
                {
                    return Err(inconsistent());
                }
                app::sdlc_confirmation::exact_receipt(
                    state,
                    revision,
                    Some(&confirmation),
                    Some(&hash),
                )?;
            }
        }
        Ok(())
    }

    pub(super) async fn resume_pm_lease(
        &self,
        task: Uuid,
        actor: &Principal,
        command: ResumePmExecutionLease,
    ) -> Result<(ExecutionLeaseReceipt, bool), AppError> {
        // Fleet owns actual terminal/checkpoint observations, just as for Lifecycle Complete.
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        app::sdlc_pm_resume::validate_command(&command)?;
        let hash = app::sdlc_pm_resume::request_hash(&command)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let mut state = self.load(&tx, task, actor).await?;
        if let Some(saved) = self
            .resume_operation_source(&tx, &state, actor, &command.idempotency_key)
            .await?
        {
            if saved.request_sha256 != hash {
                return Err(AppError::conflict("PM resume key payload conflict"));
            }
            tx.commit().await.map_err(map_db)?;
            return Ok((saved.result, true));
        }
        let f = &command.checkpoint.fence;
        let row=tx.query_one(statement("SELECT pm_execution_id,pm_owner_version,pm_admission_state FROM sdlc_tasks WHERE task_id=$1",
            vec![task.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
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
            return Err(AppError::conflict("current reserved PM execution required"));
        }
        let reservation = Self::reservation(&tx, &state, f.execution_id).await?;
        app::sdlc_pm_resume::validate_context(&state, &reservation, &command)?;
        check_fence(&reservation, command.expected_owner_version, f)?;
        let holder = Principal {
            subject: reservation.assignment.machine_subject.clone(),
            human_session: false,
            trusted_human: false,
            scopes: Default::default(),
        };
        Self::project_access(&tx, state.project_id, &holder).await?;
        let row=tx.query_one(statement("SELECT payload FROM sdlc_assignments WHERE task_id=$1 ORDER BY version DESC LIMIT 1",vec![task.into()]))
            .await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if row.try_get::<Json>("", "payload").map_err(map_db)?
            != serde_json::to_value(&reservation.assignment).map_err(AppError::internal)?
        {
            return Err(inconsistent());
        }
        let overlap = tx
            .query_one(statement(
                "SELECT EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=$1)
            OR EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=$1) AS busy",
                vec![task.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(inconsistent)?;
        if overlap.try_get::<bool>("", "busy").map_err(map_db)? {
            return Err(AppError::conflict("delivery already owns lifecycle"));
        }
        let mut current = Self::lease_current(&tx, &reservation)
            .await?
            .ok_or_else(inconsistent)?;
        if current.lease.lease_id != command.lease_id
            || current.lease.version != command.expected_lease_version
            || current.lease.version >= MAX_SAFE_VERSION
        {
            return Err(AppError::conflict("stale PM resume lease CAS"));
        }
        self.resume_input(&tx, &state, &command).await?;
        let duplicate=tx.query_one(statement("SELECT EXISTS(SELECT 1 FROM sdlc_pm_resume_inputs
            WHERE (task_id=$1 AND input_kind=$2 AND input_id=$3)
                OR (execution_id=$4 AND previous_run_id=$5)
                OR (execution_id=$4 AND checkpoint_id=$6 AND checkpoint_revision>=$7)) AS duplicate",
            vec![task.into(),command.input.kind().into(),command.input.input_id().into(),f.execution_id.into(),command.previous_run_id.into(),
                command.checkpoint.checkpoint_id.into(),command.checkpoint.checkpoint_revision.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if duplicate.try_get::<bool>("", "duplicate").map_err(map_db)? {
            return Err(AppError::conflict(
                "PM resume input/run/checkpoint already consumed",
            ));
        }
        let now = clock(&tx).await?;
        current.lease.version += 1;
        current.lease.heartbeat_at = now;
        current.lease.expires_at = now + Duration::seconds(LEASE_TTL_SECONDS);
        exec(&tx,"INSERT INTO sdlc_pm_resume_inputs(task_id,execution_id,actor_subject,idempotency_key,payload_hash,input_kind,input_id,
            checkpoint_id,checkpoint_revision,previous_run_id,lease_version,transaction_id,command,result)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,pg_current_xact_id(),$12,$13)",
            vec![task.into(),f.execution_id.into(),actor.subject.clone().into(),command.idempotency_key.clone().into(),hash.into(),command.input.kind().into(),
                command.input.input_id().into(),command.checkpoint.checkpoint_id.into(),command.checkpoint.checkpoint_revision.into(),command.previous_run_id.into(),
                current.lease.version.into(),json_value(&command)?,json_value(&current)?]).await?;
        exec(&tx,"UPDATE sdlc_pm_execution_leases SET version=$2,heartbeat_at=$3,expires_at=$4,result=$5 WHERE execution_id=$1 AND version=$6",
            vec![f.execution_id.into(),current.lease.version.into(),now.into(),current.lease.expires_at.into(),json_value(&current)?,command.expected_lease_version.into()]).await?;
        if state.status.is_some() && matches!(state.stage, Stage::Draft | Stage::Clarification) {
            state.status = Some(domain::sdlc_lifecycle::LifecycleStatus::InProgress);
            let next = json_value(&state)?;
            exec(&tx,"INSERT INTO sdlc_pm_state_write_permits(task_id,transaction_id,next_state)
                SELECT $1,pg_current_xact_id(),$2 FROM sdlc_tasks WHERE task_id=$1 AND state IS DISTINCT FROM $2",vec![task.into(),next.clone()]).await?;
            exec(
                &tx,
                "UPDATE sdlc_tasks SET state=$2 WHERE task_id=$1",
                vec![task.into(), next],
            )
            .await?;
            let status_id = lifecycle::mapped_issue_status(
                &tx,
                state.stage,
                domain::sdlc_lifecycle::LifecycleStatus::InProgress,
            )
            .await?;
            lifecycle::project_issue_status(&tx, task, actor, status_id).await?;
        }
        // Input events already exist; renewal/consumption creates no duplicate business message.
        tx.commit().await.map_err(map_db)?;
        Ok((current, false))
    }

    pub(super) async fn read_pm_resume_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<ExecutionLeaseOperation, AppError> {
        app::sdlc_lifecycle::scheduler_read(actor, &self.config)?;
        app::sdlc::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        let operation = self
            .resume_operation_source(&tx, &state, actor, key)
            .await?
            .ok_or_else(|| AppError::not_found("PM resume operation", key))?;
        tx.commit().await.map_err(map_db)?;
        Ok(operation)
    }
}
