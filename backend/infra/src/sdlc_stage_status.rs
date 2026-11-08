use super::*;
use domain::sdlc_lifecycle::LifecycleStatus;
use domain::sdlc_stage_status::{StageStartFence, StageStartReceipt, StartStage};

impl PostgresSdlcRepository {
    pub(super) async fn record_pm_stage_completed(
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
    ) -> Result<TaskState, AppError> {
        let mut next = state.clone();
        next.status = Some(LifecycleStatus::Completed);
        if state.retained_fields.contains_key("lifecycle") {
            let mut current = lifecycle::current(tx, state.task_id)
                .await?
                .ok_or_else(|| AppError::conflict("PM review projection missing"))?;
            current.task.status = LifecycleStatus::Completed;
            current.task.version = current
                .task
                .version
                .checked_add(1)
                .filter(|v| *v <= MAX_SAFE_VERSION)
                .ok_or_else(|| AppError::conflict("lifecycle version exhausted"))?;
            Self::save_delivery_projection(tx, actor, &current, false).await?;
            next = serde_json::from_value(app::sdlc_lifecycle::canonical_projection(
                &serde_json::to_value(state).map_err(AppError::internal)?,
                &current.task,
            )?)
            .map_err(AppError::internal)?;
        } else {
            let raw = json_value(&next)?;
            exec(
                tx,
                "INSERT INTO sdlc_pm_state_write_permits(task_id,transaction_id,next_state)
                VALUES($1,pg_current_xact_id(),$2)",
                vec![state.task_id.into(), raw.clone()],
            )
            .await?;
            exec(
                tx,
                "UPDATE sdlc_tasks SET state=$2 WHERE task_id=$1",
                vec![state.task_id.into(), raw],
            )
            .await?;
            let status =
                lifecycle::mapped_issue_status(tx, Stage::Draft, LifecycleStatus::Completed)
                    .await?;
            lifecycle::project_issue_status(tx, state.task_id, actor, status).await?;
        }
        Ok(next)
    }

    async fn saved_stage_start(
        tx: &DatabaseTransaction,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<Option<(String, StageStartReceipt)>, AppError> {
        let Some(row) = tx
            .query_one(statement(
                "SELECT payload_hash,result FROM sdlc_idempotency
             WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
                vec![task.into(), actor.subject.clone().into(), key.into()],
            ))
            .await
            .map_err(map_db)?
        else {
            return Ok(None);
        };
        let result: Json = row.try_get("", "result").map_err(map_db)?;
        let receipt: StageStartReceipt = serde_json::from_value(result["stage_start"].clone())
            .map_err(|_| AppError::conflict("operation key is not a stage start"))?;
        if receipt.task_id != task
            || receipt.idempotency_key != key
            || receipt.status != LifecycleStatus::InProgress
        {
            return Err(AppError::conflict("saved stage start differs"));
        }
        Ok(Some((
            row.try_get("", "payload_hash").map_err(map_db)?,
            receipt,
        )))
    }

    pub(super) async fn agent_stage_start(
        &self,
        task: Uuid,
        actor: &Principal,
        command: StartStage,
    ) -> Result<(StageStartReceipt, bool), AppError> {
        app::sdlc::validate_key(&command.idempotency_key)?;
        app::sdlc_configured_cli::authorize_configured_identity(actor)?;
        let hash =
            app::sdlc::canonical_hash(&json!({"operation":"start_stage","command":command}))?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let mut state = Self::load_binding(&tx, &self.config, task, actor).await?;
        if let Some((saved_hash, saved)) =
            Self::saved_stage_start(&tx, task, actor, &command.idempotency_key).await?
        {
            if saved_hash != hash {
                return Err(AppError::conflict("stage start key payload conflict"));
            }
            spaces::bind_retained_task(&tx, &self.config, &state, actor).await?;
            tx.commit().await.map_err(map_db)?;
            return Ok((saved, true));
        }
        Self::bind_current_task(&tx, &self.config, &state, actor).await?;
        if matches!(&command.fence, StageStartFence::Pm(_))
            && state.retained_fields.contains_key("lifecycle")
        {
            self.configured_review_context(&tx, &state)
                .await?
                .ok_or_else(|| AppError::conflict("current PM review missing"))?;
        }
        let mut delivery = match &command.fence {
            StageStartFence::Pm(_) => None,
            StageStartFence::Delivery(_) => Some(
                lifecycle::current(&tx, task)
                    .await?
                    .ok_or_else(|| AppError::conflict("delivery custody missing"))?,
            ),
        };
        let at = execution_lease::clock(&tx).await?;
        if let (Some(current), StageStartFence::Delivery(fence)) = (&delivery, &command.fence) {
            lifecycle::exact_revision(&state, current)?;
            lifecycle::exact_fence(current, fence, at, false)?;
            super::critical_blockers::ensure_progress_allowed(&tx, current).await?;
            lifecycle::producer_grant(&tx, &state, actor, true).await?;
        }
        let receipt = app::sdlc_stage_status::prepare_stage_start(
            &state,
            actor,
            &command,
            delivery.as_ref(),
            at,
        )?;
        // Persist the typed operation before its exact status projection in this same transaction.
        exec(&tx,"INSERT INTO sdlc_idempotency(task_id,actor_subject,idempotency_key,payload_hash,result)
            VALUES($1,$2,$3,$4,$5)",vec![task.into(),actor.subject.clone().into(),
                command.idempotency_key.clone().into(),hash.into(),json!({"stage_start":receipt,"command":command}).into()]).await?;
        if delivery.is_none() && state.retained_fields.contains_key("lifecycle") {
            delivery = lifecycle::current(&tx, task).await?;
        }
        if let Some(current) = delivery.as_mut() {
            current.task.status = LifecycleStatus::InProgress;
            current.task.version = current
                .task
                .version
                .checked_add(1)
                .filter(|v| *v <= MAX_SAFE_VERSION)
                .ok_or_else(|| AppError::conflict("lifecycle version exhausted"))?;
            Self::save_delivery_projection(&tx, actor, current, false).await?;
        } else {
            state.status = Some(LifecycleStatus::InProgress);
            let next = json_value(&state)?;
            exec(
                &tx,
                "INSERT INTO sdlc_pm_state_write_permits(task_id,transaction_id,next_state)
                VALUES($1,pg_current_xact_id(),$2)",
                vec![task.into(), next.clone()],
            )
            .await?;
            exec(
                &tx,
                "UPDATE sdlc_tasks SET state=$2 WHERE task_id=$1",
                vec![task.into(), next],
            )
            .await?;
            let status =
                lifecycle::mapped_issue_status(&tx, state.stage, LifecycleStatus::InProgress)
                    .await?;
            lifecycle::project_issue_status(&tx, task, actor, status).await?;
        }
        tx.commit().await.map_err(map_db)?;
        Ok((receipt, false))
    }

    pub(super) async fn agent_stage_start_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<StageStartReceipt, AppError> {
        app::sdlc::validate_key(key)?;
        app::sdlc_configured_cli::authorize_configured_identity(actor)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = Self::load_binding(&tx, &self.config, task, actor).await?;
        let (_, receipt) = Self::saved_stage_start(&tx, task, actor, key)
            .await?
            .ok_or_else(|| AppError::not_found("stage start operation", key))?;
        spaces::bind_retained_task(&tx, &self.config, &state, actor).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(receipt)
    }
}
