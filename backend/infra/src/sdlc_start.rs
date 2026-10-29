//! One explicit owner command on the existing lifecycle/permit/operation boundary.
use super::*;
use domain::sdlc_lifecycle::*;
use domain::sdlc_routing::TaskRoutingSnapshot;

impl PostgresSdlcRepository {
    pub(super) async fn configured_start_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<LifecycleReceipt, AppError> {
        app::sdlc::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = Self::load_binding(&tx, &self.config, task, actor).await?;
        app::sdlc_lifecycle::start_owner(&state, actor)?;
        Self::start_current_owner(&tx, task, actor).await?;
        let row = tx
            .query_one(statement(
                "SELECT result FROM sdlc_delivery_operations WHERE task_id=$1 AND actor_subject=$2
             AND idempotency_key=$3 AND command->>'operation'='start_analysis'",
                vec![task.into(), actor.subject.clone().into(), key.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::not_found("start operation", key))?;
        let receipt = serde_json::from_value(row.try_get::<Json>("", "result").map_err(map_db)?)
            .map_err(AppError::internal)?;
        spaces::bind_retained_task(&tx, &self.config, &state, actor).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(receipt)
    }

    async fn start_current_owner(
        tx: &DatabaseTransaction,
        task: Uuid,
        actor: &Principal,
    ) -> Result<(), AppError> {
        let row = tx
            .query_one(statement(
                "SELECT u.central_sub AS owner FROM issues i JOIN users u ON u.id=i.reporter_id
             WHERE i.id=$1 AND u.is_active FOR SHARE OF u",
                vec![task.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or(AppError::Forbidden)?;
        if row
            .try_get::<Option<String>>("", "owner")
            .map_err(map_db)?
            .as_deref()
            != Some(&actor.subject)
        {
            return Err(AppError::Forbidden);
        }
        Ok(())
    }

    pub(super) async fn start_configured_analysis(
        &self,
        task: Uuid,
        actor: &Principal,
        command: LifecycleCommand,
    ) -> Result<(LifecycleReceipt, bool), AppError> {
        app::sdlc_lifecycle::validate_command(&command)?;
        let LifecycleCommand::StartAnalysis {
            pm_completion_id, ..
        } = &command
        else {
            unreachable!()
        };
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = Self::load_binding(&tx, &self.config, task, actor).await?;
        app::sdlc_lifecycle::start_owner(&state, actor)?;
        Self::start_current_owner(&tx, task, actor).await?;
        let hash = app::sdlc::canonical_hash(&command)?;
        if let Some(row) = tx
            .query_one(statement(
                "SELECT command,payload_hash,result FROM sdlc_delivery_operations
             WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
                vec![
                    task.into(),
                    actor.subject.clone().into(),
                    command.key().into(),
                ],
            ))
            .await
            .map_err(map_db)?
        {
            if row.try_get::<String>("", "payload_hash").map_err(map_db)? != hash
                || row
                    .try_get::<Option<Json>>("", "command")
                    .map_err(map_db)?
                    .as_ref()
                    != Some(&serde_json::to_value(&command).map_err(AppError::internal)?)
            {
                return Err(AppError::conflict("delivery key payload conflict"));
            }
            let receipt =
                serde_json::from_value(row.try_get::<Json>("", "result").map_err(map_db)?)
                    .map_err(AppError::internal)?;
            spaces::bind_retained_task(&tx, &self.config, &state, actor).await?;
            tx.commit().await.map_err(map_db)?;
            return Ok((receipt, true));
        }
        if !self
            .config
            .single_task_project_ids
            .contains(&state.project_id)
        {
            return Err(AppError::Unavailable(
                "single_task_lifecycle_project_not_enabled".into(),
            ));
        }
        Self::bind_current_task(&tx, &self.config, &state, actor).await?;
        let completed = Self::configured_saved(&tx, &state, None)
            .await?
            .ok_or_else(|| AppError::conflict("current PM completion required"))?
            .result;
        if completed.completion_id != *pm_completion_id {
            return Err(AppError::conflict("PM completion selector is stale"));
        }
        app::sdlc_lifecycle::validate_start(&state, &completed)?;
        let row = tx.query_one(statement(
            "SELECT pm_owner_version,sdlc_current_configured_pm_completed(task_id,pm_execution_id,state,pm_admission_state) AS completed
             FROM sdlc_tasks WHERE task_id=$1", vec![task.into()],
        )).await.map_err(map_db)?.ok_or_else(|| AppError::conflict("PM cursor absent"))?;
        if row.try_get::<i64>("", "pm_owner_version").map_err(map_db)? != completed.owner_version
            || !row.try_get::<bool>("", "completed").map_err(map_db)?
        {
            return Err(AppError::conflict("PM completion cursor changed"));
        }
        // Use only the owner's frozen confirmation/source generation, never today's project policy.
        let row = tx.query_one(statement(
            "SELECT s.payload FROM sdlc_task_routing_snapshots s JOIN sdlc_task_source_bindings b
             ON b.task_id=s.task_id AND b.confirmation_id=s.confirmation_id AND b.routing_snapshot_id=s.snapshot_id
             AND b.requirement_revision=s.requirement_revision AND b.content_hash=s.content_hash
             WHERE s.task_id=$1 AND s.confirmation_id=$2",
            vec![task.into(), completed.confirmation_id.into()],
        )).await.map_err(map_db)?.ok_or_else(|| AppError::conflict("frozen routing/source binding absent"))?;
        let routing: TaskRoutingSnapshot =
            serde_json::from_value(row.try_get::<Json>("", "payload").map_err(map_db)?)
                .map_err(AppError::internal)?;
        app::sdlc_draft_flow::validate_policy(
            &routing.policy,
            state.project_id,
            &state.tracker_instance_id,
        )?;
        super::lifecycle::exact_snapshot(&state, &routing)?;
        let receipt = LifecycleReceipt {
            task: LifecycleTask {
                task_id: task,
                owner_subject: state.owner_subject.clone(),
                version: 1,
                stage: LifecycleStage::Analysis,
                status: LifecycleStatus::Ready,
                cycle: 0,
                transient_retries: 0,
                waiting_reason: Some("queued_for_assignment".into()),
                runtime: RuntimeState::Queued,
                routing,
                previous_result_id: None,
                resume_chat_id: None,
            },
            assignment: None,
            lease: None,
            binding: None,
            result: None,
        };
        let mut intent =
            app::sdlc_lifecycle::configured_start_intent(&state, &completed, Uuid::now_v7())?;
        intent.workflow = receipt
            .task
            .routing
            .policy
            .routes
            .analyst
            .workflow_key
            .clone();
        let event = Uuid::now_v7();
        Self::ensure_initial_delivery_idle(&tx, task).await?;
        // Journal precedes projection so the SQL initial-transition guard can require
        // this exact command in the same transaction as its existing permit.
        exec(&tx, "INSERT INTO sdlc_delivery_operations(task_id,actor_subject,idempotency_key,payload_hash,result,event_id,command,transaction_id)
            VALUES($1,$2,$3,$4,$5,$6,$7,pg_current_xact_id())",
            vec![task.into(), actor.subject.clone().into(), command.key().into(), hash.into(), json_value(&receipt)?, event.into(), json_value(&command)?]).await?;
        exec(&tx, "INSERT INTO sdlc_analysis_intents(intent_id,task_id,requirement_revision,content_hash,confirmation_id,operation_key,payload,routing_snapshot_id)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8)", vec![intent.intent_id.into(), task.into(), intent.requirement_revision.into(),
                intent.content_hash.clone().into(), intent.confirmation_id.into(), intent.operation_key.clone().into(), json_value(&intent)?, receipt.task.routing.snapshot_id.into()]).await?;
        Self::save_delivery_projection(&tx, actor, &receipt, true).await?;
        for (event_id, kind, result) in [
            (
                intent.intent_id,
                "analysis.intent_created",
                serde_json::to_value(&intent).map_err(AppError::internal)?,
            ),
            (
                event,
                "delivery.queued",
                serde_json::to_value(&receipt).map_err(AppError::internal)?,
            ),
        ] {
            let payload = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
                "project_id":state.project_id,"task_id":task,"root_task_id":task,"owner_subject":state.owner_subject,
                "stage":"Analysis","requirement_revision":completed.requirement_revision,"result":result});
            exec(
                &tx,
                "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,$3,$4)",
                vec![event_id.into(), task.into(), kind.into(), payload.into()],
            )
            .await?;
        }
        tx.commit().await.map_err(map_db)?;
        Ok((receipt, false))
    }
}
