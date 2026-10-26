use super::*;
use domain::sdlc_configured_cli::*;

impl PostgresSdlcRepository {
    async fn configured_saved(
        tx: &DatabaseTransaction,
        state: &TaskState,
        key: Option<&str>,
    ) -> Result<Option<ConfiguredPmCompletionOperation>, AppError> {
        let row=tx.query_one(statement("SELECT completion_id,actor_subject,idempotency_key,payload_hash,command,workflow_receipt,result,created_at
            FROM sdlc_configured_pm_completions WHERE task_id=$1 AND ($2::text IS NULL OR idempotency_key=$2)",
            vec![state.task_id.into(),key.map(str::to_owned).into()])).await.map_err(map_db)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let command: CompleteConfiguredPm =
            serde_json::from_value(row.try_get::<Json>("", "command").map_err(map_db)?)
                .map_err(AppError::internal)?;
        let workflow: ConfiguredWorkflowReceipt = serde_json::from_value(
            row.try_get::<Json>("", "workflow_receipt")
                .map_err(map_db)?,
        )
        .map_err(AppError::internal)?;
        let result: ConfiguredPmCompletionReceipt =
            serde_json::from_value(row.try_get::<Json>("", "result").map_err(map_db)?)
                .map_err(AppError::internal)?;
        let assignment = state.assignment.as_ref().ok_or(AppError::Forbidden)?;
        let reservation = Self::reservation(tx, state, assignment.execution_id).await?;
        let expected = app::sdlc_configured_cli::completion_result(
            state,
            &command,
            workflow,
            reservation.owner_cas.version,
            row.try_get("", "completion_id").map_err(map_db)?,
            row.try_get("", "created_at").map_err(map_db)?,
        )?;
        let hash: String = row.try_get("", "payload_hash").map_err(map_db)?;
        let saved_key: String = row.try_get("", "idempotency_key").map_err(map_db)?;
        if result != expected
            || saved_key != command.idempotency_key
            || hash != app::sdlc::canonical_hash(&command)?
            || row.try_get::<String>("", "actor_subject").map_err(map_db)?
                != assignment.machine_subject
        {
            return Err(AppError::conflict("configured completion ledger mismatch"));
        }
        Ok(Some(ConfiguredPmCompletionOperation {
            idempotency_key: saved_key,
            request_sha256: hash,
            result,
        }))
    }

    pub(super) async fn configured_completion_read(
        &self,
        task: Uuid,
        actor: &Principal,
        key: Option<&str>,
    ) -> Result<Option<ConfiguredPmCompletionOperation>, AppError> {
        if let Some(key) = key {
            app::sdlc::validate_key(key)?;
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        if !state.configured_cli() {
            return Err(AppError::conflict(
                "configured completion cannot read legacy task",
            ));
        }
        if !state.owner(actor) {
            app::sdlc_configured_cli::authorize_configured_pm(&state, actor)?;
        }
        let saved = Self::configured_saved(&tx, &state, key).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(saved)
    }

    pub(super) async fn configured_completion_commit(
        &self,
        task: Uuid,
        actor: &Principal,
        command: CompleteConfiguredPm,
        receipt: ConfiguredWorkflowReceipt,
    ) -> Result<(ConfiguredPmCompletionReceipt, bool), AppError> {
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        app::sdlc_configured_cli::validate_completion(&state, actor, &command)?;
        let hash = app::sdlc::canonical_hash(&command)?;
        if let Some(saved) =
            Self::configured_saved(&tx, &state, Some(&command.idempotency_key)).await?
        {
            if saved.request_sha256 != hash {
                return Err(AppError::conflict("PM completion key payload conflict"));
            }
            tx.commit().await.map_err(map_db)?;
            return Ok((saved.result, true));
        }
        app::sdlc_configured_cli::validate_receipt(&state, &command, &receipt)?;
        let confirmation = state
            .current_confirmation()
            .ok_or_else(|| AppError::conflict("owner confirmation missing"))?;
        let saved=tx.query_one(statement("SELECT payload FROM sdlc_confirmations WHERE task_id=$1 AND id=$2 AND revision=$3 AND content_hash=$4",
            vec![task.into(),confirmation.id.into(),command.requirement_revision.into(),command.content_hash.clone().into()])).await.map_err(map_db)?
            .ok_or_else(||AppError::conflict("owner confirmation ledger missing"))?;
        if saved.try_get::<Json>("", "payload").map_err(map_db)?
            != serde_json::to_value(confirmation).map_err(AppError::internal)?
        {
            return Err(AppError::conflict("owner confirmation ledger mismatch"));
        }
        let row = tx
            .query_one(statement(
                "SELECT pm_owner_version,pm_admission_state FROM sdlc_tasks WHERE task_id=$1",
                vec![task.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::conflict("PM cursor absent"))?;
        let version: i64 = row.try_get("", "pm_owner_version").map_err(map_db)?;
        if state.stage != Stage::Draft
            || row
                .try_get::<Option<String>>("", "pm_admission_state")
                .map_err(map_db)?
                .as_deref()
                != Some("reserved")
            || Self::configured_saved(&tx, &state, None).await?.is_some()
        {
            return Err(AppError::conflict(
                "configured PM completion is no longer current",
            ));
        }
        let assignment = state.assignment.as_ref().ok_or(AppError::Forbidden)?;
        let reservation = Self::reservation(&tx, &state, assignment.execution_id).await?;
        if reservation.assignment != *assignment || reservation.owner_cas.version != version {
            return Err(AppError::conflict("configured PM reservation changed"));
        }
        let now = execution_lease::clock(&tx).await?;
        let result = app::sdlc_configured_cli::completion_result(
            &state,
            &command,
            receipt,
            version,
            Uuid::now_v7(),
            now,
        )?;
        exec(&tx,"INSERT INTO sdlc_configured_pm_completions(completion_id,task_id,execution_id,confirmation_id,requirement_revision,content_hash,
            actor_subject,idempotency_key,payload_hash,command,workflow_receipt,result,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
            vec![result.completion_id.into(),task.into(),assignment.execution_id.into(),confirmation.id.into(),command.requirement_revision.into(),
                command.content_hash.clone().into(),actor.subject.clone().into(),command.idempotency_key.clone().into(),hash.into(),
                json_value(&command)?,json_value(&result.workflow_receipt)?,json_value(&result)?,now.into()]).await?;
        let mut next = state.clone();
        next.stage = Stage::Backlog;
        next.status = Some(domain::sdlc_lifecycle::LifecycleStatus::Ready);
        next.draft_submode = None;
        let next_state = json_value(&next)?;
        exec(
            &tx,
            "INSERT INTO sdlc_pm_state_write_permits(task_id,transaction_id,next_state)
            VALUES($1,pg_current_xact_id(),$2)",
            vec![task.into(), next_state.clone()],
        )
        .await?;
        exec(
            &tx,
            "UPDATE sdlc_tasks SET state=$2,pm_admission_state='completed' WHERE task_id=$1",
            vec![task.into(), next_state],
        )
        .await?;
        let status_id = lifecycle::mapped_issue_status(
            &tx,
            Stage::Backlog,
            domain::sdlc_lifecycle::LifecycleStatus::Ready,
        )
        .await?;
        lifecycle::project_issue_status(&tx, task, actor, status_id).await?;
        exec(&tx,"INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,'pm.completed',$3)",
            vec![result.completion_id.into(),task.into(),json!({"contract_version":1,"interaction_protocol":"configured-cli-v1",
                "tracker_instance_id":state.tracker_instance_id,"project_id":state.project_id,"task_id":task,"root_task_id":state.root_task_id,
                "owner_subject":state.owner_subject,"stage":"Backlog","requirement_revision":command.requirement_revision,"result":result}).into()]).await?;
        tx.commit().await.map_err(map_db)?;
        Ok((result, false))
    }
}
