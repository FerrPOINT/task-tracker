//! Genuine human entry into the existing technical execution ledger.
use super::*;
use app::task_handoff::PendingHandoffMaterials;
use domain::{
    current_stage_execution::{CurrentStageAssignment, CurrentStageExecution},
    sdlc_lifecycle::{LifecycleReceipt, RuntimeState},
    task_handoff::{PreparedCurrentStageHandoff, ReserveTaskHandoffCommand, TaskExecutionRole},
};

impl PostgresSdlcRepository {
    async fn current_handoff_owner(
        &self,
        task: Uuid,
        actor: &Principal,
        require_write: bool,
    ) -> Result<(), AppError> {
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = Self::load_binding(&tx, &self.config, task, actor).await?;
        tx.commit().await.map_err(map_db)?;
        if app::native_space_access::application_policy_required().await? {
            let native = app::native_space_access::current()?.ok_or(AppError::Forbidden)?;
            if native.identity().subject != actor.subject || (require_write && !native.may_write())
            {
                return Err(AppError::Forbidden);
            }
            native
                .require_owner(
                    Uuid::parse_str(&state.owner_subject).map_err(|_| AppError::Forbidden)?,
                    &["task-tracker", "fleet-control", "project-workflow", "ci-cd"],
                )
                .await?;
        } else if let Some(native) = app::native_space_access::current()? {
            if native.identity().subject != actor.subject || (require_write && !native.may_write())
            {
                return Err(AppError::Forbidden);
            }
            native
                .require_known_owner(
                    Uuid::parse_str(&state.owner_subject).map_err(|_| AppError::Forbidden)?,
                )
                .await?;
        }
        Ok(())
    }
    pub(super) async fn handoff_reservation_read(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
        mutation: bool,
    ) -> Result<Option<app::task_handoff::SavedHandoffReservation>, AppError> {
        if mutation {
            app::task_handoff::scheduler_writer(actor, &self.config)?
        } else {
            app::task_handoff::scheduler_reader(actor, &self.config)?
        }
        domain::task_lifecycle::validate_command_fields(1, key)?;
        self.current_handoff_owner(task, actor, mutation).await?;
        let tx = self.db.begin().await.map_err(map_db)?;
        Self::load_binding(&tx, &self.config, task, actor).await?;
        let row=tx.query_one(statement("SELECT a.command_hash,o.result,m.source_binding FROM task_handoff_delivery_adoptions a JOIN task_handoff_delivery_outcomes o USING(handoff_id) JOIN task_handoff_materials m USING(task_id,handoff_id) WHERE a.task_id=$1 AND a.actor_subject=$2 AND a.idempotency_key=$3",vec![task.into(),actor.subject.clone().into(),key.into()])).await.map_err(map_db)?;
        let Some(row) = row else {
            tx.commit().await.map_err(map_db)?;
            // A missing key does not bypass the data boundary or expose which
            // original operations exist. No owner HTTP may hold this lock open.
            let owner_data = self.owner_data_admission(task, actor, None).await?;
            let tx = self.db.begin().await.map_err(map_db)?;
            let state = Self::load_binding(&tx, &self.config, task, actor).await?;
            Self::bind_owner_data(&tx, &state, owner_data.as_ref(), None, false).await?;
            tx.commit().await.map_err(map_db)?;
            if let Some(owner_data) = owner_data {
                owner_data.current()?;
            }
            return Ok(None);
        };
        let source: domain::sdlc_source::TaskSourceBinding =
            serde_json::from_value(row.try_get::<Json>("", "source_binding").map_err(map_db)?)
                .map_err(AppError::internal)?;
        tx.commit().await.map_err(map_db)?;
        let owner_data = self
            .owner_data_admission(task, actor, Some(&source.namespace))
            .await?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = Self::load_binding(&tx, &self.config, task, actor).await?;
        Self::bind_owner_data(
            &tx,
            &state,
            owner_data.as_ref(),
            Some(&source.namespace),
            false,
        )
        .await?;
        let enforced=tx.query_one(statement("SELECT active OR EXISTS(SELECT 1 FROM tracker_space_catalog WHERE metadata IS NOT NULL) AS enforced FROM tracker_space_admission_state WHERE singleton",vec![])).await.map_err(map_db)?.ok_or(AppError::Forbidden)?.try_get::<bool>("","enforced").map_err(map_db)?;
        if enforced {
            let native = app::native_space_access::current()?.ok_or(AppError::Forbidden)?;
            if native.identity().subject != actor.subject {
                return Err(AppError::Forbidden);
            }
            let source: domain::sdlc_source::TaskSourceBinding =
                serde_json::from_value(row.try_get::<Json>("", "source_binding").map_err(map_db)?)
                    .map_err(AppError::internal)?;
            let catalog=tx.query_one(statement("SELECT state FROM tracker_space_catalog WHERE registry_instance_id=$1 AND space_id=$2 AND admin_revision IS NOT NULL FOR SHARE",vec![source.namespace.registry_instance_id.into(),source.namespace.namespace_id.into()])).await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
            if !matches!(
                catalog
                    .try_get::<String>("", "state")
                    .map_err(map_db)?
                    .as_str(),
                "active" | "archiving" | "archived"
            ) {
                return Err(AppError::Forbidden);
            }
            crate::guarded_db::bind_commit_clock(&tx, native.validity()?.valid_until)
                .await
                .map_err(map_db)?;
        }
        let saved = app::task_handoff::SavedHandoffReservation {
            command_hash: row.try_get("", "command_hash").map_err(map_db)?,
            result: serde_json::from_value(row.try_get::<Json>("", "result").map_err(map_db)?)
                .map_err(AppError::internal)?,
        };
        tx.commit().await.map_err(map_db)?;
        if let Some(owner_data) = owner_data {
            owner_data.current()?;
        }
        Ok(Some(saved))
    }
    pub(super) async fn prepare_handoff_delivery(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<PreparedCurrentStageHandoff, AppError> {
        app::task_handoff::scheduler_writer(actor, &self.config)?;
        self.current_handoff_owner(task, actor, true).await?;
        let owner_data = self.owner_data_admission(task, actor, None).await?;
        let tx = self.db.begin().await.map_err(map_db)?;
        self.pending_handoff_locked(&tx, task, actor, true).await?;
        let (checked_state, prepared) = self.prepare_handoff_locked(&tx, task, actor, true).await?;
        Self::bind_owner_data(&tx, &checked_state, owner_data.as_ref(), None, true).await?;
        if let Some(native) = app::native_space_access::current()? {
            if !native.is_current() {
                return Err(AppError::Forbidden);
            }
        }
        tx.commit().await.map_err(map_db)?;
        Ok(prepared)
    }
    async fn bind_handoff_native_scope(
        tx: &DatabaseTransaction,
        pending: &PendingHandoffMaterials,
        actor: &Principal,
    ) -> Result<(), AppError> {
        let row=tx.query_one(statement("SELECT active OR EXISTS(SELECT 1 FROM tracker_space_catalog WHERE metadata IS NOT NULL) AS enforced FROM tracker_space_admission_state WHERE singleton",vec![])).await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
        let space = row.try_get::<bool>("", "enforced").map_err(map_db)?;
        let apps = app::native_space_access::application_policy_required().await?;
        if !space && !apps {
            if let Some(native) = app::native_space_access::current()? {
                if native.identity().subject != actor.subject || !native.may_write() {
                    return Err(AppError::Forbidden);
                }
                native.confirmed_owner(
                    Uuid::parse_str(&pending.receipt.actor_subject)
                        .map_err(|_| AppError::Forbidden)?,
                )?;
                crate::guarded_db::bind_commit_clock(tx, native.validity()?.valid_until)
                    .await
                    .map_err(map_db)?;
            }
            return Ok(());
        }
        let native = app::native_space_access::current()?.ok_or(AppError::Forbidden)?;
        if native.identity().subject != actor.subject || !native.may_write() {
            return Err(AppError::Forbidden);
        }
        // require_owner was resolved before this transaction, preserving its original clock.
        crate::guarded_db::bind_commit_clock(tx, native.validity()?.valid_until)
            .await
            .map_err(map_db)?;
        if space {
            let source = &pending.source.namespace;
            let row=tx.query_one(statement("SELECT state FROM tracker_space_catalog WHERE registry_instance_id=$1 AND space_id=$2 AND admin_revision IS NOT NULL FOR SHARE",vec![source.registry_instance_id.into(),source.namespace_id.into()])).await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
            if row.try_get::<String>("", "state").map_err(map_db)? != "active" {
                return Err(AppError::Forbidden);
            }
            exec(
                tx,
                "SELECT set_config('pdlc.tracker.current_handoff',$1,true)",
                vec![pending.receipt.handoff_id.to_string().into()],
            )
            .await?;
            let scope = json!({"schema_version":1,"task_id":pending.receipt.task_id,"project_id":pending.receipt.project_id,"identity":native.identity(),"validity":native.validity()?,"writable":true});
            exec(
                tx,
                "SELECT tracker_bind_native_task($1::jsonb)",
                vec![scope.into()],
            )
            .await?;
        }
        Ok(())
    }
    pub(super) async fn adopt_handoff_delivery(
        &self,
        task: Uuid,
        actor: &Principal,
        command: &ReserveTaskHandoffCommand,
        observed: &app::task_handoff_observations::MatchedHandoffInputs,
    ) -> Result<(CurrentStageExecution, bool), AppError> {
        app::task_handoff::scheduler_writer(actor, &self.config)?;
        command.validate()?;
        self.current_handoff_owner(task, actor, true).await?;
        let owner_data = self.owner_data_admission(task, actor, None).await?;
        let hash = app::sdlc::canonical_hash(command)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = Self::load_binding(&tx, &self.config, task, actor).await?;
        if let Some(row)=tx.query_one(statement("SELECT a.command_hash,o.result FROM task_handoff_delivery_adoptions a JOIN task_handoff_delivery_outcomes o USING(handoff_id) WHERE a.task_id=$1 AND a.actor_subject=$2 AND a.idempotency_key=$3",vec![task.into(),actor.subject.clone().into(),command.idempotency_key.clone().into()])).await.map_err(map_db)?{
            if row.try_get::<String>("","command_hash").map_err(map_db)?!=hash{return Err(AppError::conflict("current_stage_reservation_key_conflict"))}
            let result=serde_json::from_value(row.try_get::<Json>("","result").map_err(map_db)?).map_err(AppError::internal)?;
            tx.commit().await.map_err(map_db)?;return Ok((result,true))
        }
        exec(
            &tx,
            "SELECT project_id FROM sdlc_project_routing_heads WHERE project_id=$1 FOR SHARE",
            vec![state.project_id.into()],
        )
        .await?;
        exec(
            &tx,
            "SELECT project_id FROM sdlc_source_policy_heads WHERE project_id=$1 FOR SHARE",
            vec![state.project_id.into()],
        )
        .await?;
        let pending = self.pending_handoff_locked(&tx, task, actor, true).await?;
        let (checked_state, prepared) = self.prepare_handoff_locked(&tx, task, actor, true).await?;
        Self::bind_owner_data(&tx, &checked_state, owner_data.as_ref(), None, true).await?;
        observed.ensure_for(&prepared)?;
        if pending.receipt.handoff_id != command.handoff_id
            || prepared.state.revision != command.expected_revision
        {
            return Err(AppError::conflict("current_stage_reservation_changed"));
        }
        if prepared.state.stage.role().is_none() {
            return Err(AppError::conflict(
                "current_stage_project_manager_admission_required",
            ));
        }
        if command.machine_subject == state.owner_subject
            || command.machine_subject == actor.subject
            || command.machine_subject == self.config.orchestrator_subject
            || command.machine_subject == self.config.verifier_subject
        {
            return Err(AppError::Forbidden);
        }
        let producer = Principal {
            subject: command.machine_subject.clone(),
            trusted_human: false,
            human_session: false,
            scopes: Default::default(),
        };
        Self::project_access(&tx, state.project_id, &producer).await?;
        Self::bind_handoff_native_scope(&tx, &pending, actor).await?;
        Self::ensure_initial_delivery_idle(&tx, task).await?;
        let allocation = Self::allocate_delivery_reservation(&tx).await?;
        let ordinal = allocation
            .execution
            .ordinal
            .parse::<i64>()
            .map_err(AppError::internal)?;
        exec(&tx,"INSERT INTO task_handoff_delivery_adoptions(handoff_id,task_id,actor_subject,idempotency_key,command_hash,expected_revision,assignment_id,execution_id,execution_ordinal,observations) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",vec![command.handoff_id.into(),task.into(),actor.subject.clone().into(),command.idempotency_key.clone().into(),hash.into(),command.expected_revision.into(),allocation.assignment_id.into(),allocation.execution_id.into(),ordinal.into(),observed.audit().into()]).await?;
        let mut current = app::task_handoff::seed_current_stage(&pending)?;
        let mut receipt = LifecycleReceipt {
            task: current.task.clone(),
            assignment: None,
            lease: None,
            binding: None,
            result: None,
        };
        let row=tx.query_one(statement("SELECT state,EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=$1) AS has_head FROM sdlc_tasks WHERE task_id=$1",vec![task.into()])).await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
        let previous: Json = row.try_get("", "state").map_err(map_db)?;
        let mut next = app::sdlc_lifecycle::canonical_projection(&previous, &receipt.task)?;
        let object = next
            .as_object_mut()
            .ok_or_else(|| AppError::conflict("invalid task snapshot"))?;
        object.remove("human_handoff_id");
        object.remove("draft_submode");
        object.insert("execution_handoff_id".into(), json!(command.handoff_id));
        object.insert("interaction_protocol".into(), json!("configured-cli-v1"));
        if object
            .get("workflow_task_key")
            .and_then(Json::as_str)
            .is_none()
        {
            object.insert("workflow_task_key".into(), json!(allocation.execution.key));
        }
        let cursor = json!({"assignment":null,"lease":null,"binding":null,"result":null});
        exec(&tx,"INSERT INTO task_handoff_delivery_seed_permits(task_id,transaction_id,handoff_id,previous_state,next_state) VALUES($1,pg_current_xact_id(),$2,$3,$4)",vec![task.into(),command.handoff_id.into(),previous.into(),next.clone().into()]).await?;
        exec(&tx,"INSERT INTO sdlc_delivery_projection_permits(task_id,transaction_id,next_state,next_execution) VALUES($1,pg_current_xact_id(),$2,$3)",vec![task.into(),next.clone().into(),cursor.clone().into()]).await?;
        exec(
            &tx,
            "UPDATE sdlc_tasks SET state=$2,manual_control=false WHERE task_id=$1",
            vec![task.into(), next.into()],
        )
        .await?;
        let sql = if row.try_get::<bool>("", "has_head").map_err(map_db)? {
            "UPDATE sdlc_delivery_heads SET execution=$2 WHERE task_id=$1"
        } else {
            "INSERT INTO sdlc_delivery_heads(task_id,execution) VALUES($1,$2)"
        };
        exec(&tx, sql, vec![task.into(), cursor.into()]).await?;
        self.delivery_reserve(
            &tx,
            actor,
            &mut receipt,
            1,
            Some(&command.machine_subject),
            Some(allocation),
        )
        .await?;
        Self::save_delivery_projection(&tx, actor, &receipt, false).await?;
        let assignment = receipt
            .assignment
            .as_ref()
            .ok_or_else(|| AppError::conflict("actual assignment missing"))?;
        Self::freeze_handoff_dependencies(
            &tx,
            &pending,
            assignment.assignment_id,
            actor,
            owner_data.as_ref(),
            observed,
        )
        .await?;
        let role: TaskExecutionRole = serde_json::from_value(
            serde_json::to_value(assignment.role).map_err(AppError::internal)?,
        )
        .map_err(AppError::internal)?;
        current.task = receipt.task.clone();
        current.assignment = Some(CurrentStageAssignment {
            handoff_id: command.handoff_id,
            assignment_id: assignment.assignment_id,
            execution_id: assignment.execution_id,
            execution: assignment
                .execution
                .clone()
                .ok_or_else(|| AppError::conflict("actual execution ordinal missing"))?,
            task_id: task,
            owner_subject: assignment.owner_subject.clone(),
            role,
            stage: assignment.stage,
            cycle: assignment.cycle,
            attempt: assignment.attempt,
            agent_id: assignment.agent_id,
            machine_subject: command.machine_subject.clone(),
            routing: assignment.routing.clone(),
            previous_result_id: assignment.previous_result_id,
            fencing_token: assignment.fencing_token,
            lease_id: assignment.lease_id,
            assignment_hash: assignment.assignment_hash.clone(),
        });
        current.lease = receipt.lease.clone();
        current.binding = receipt.binding.clone();
        debug_assert_eq!(current.task.runtime, RuntimeState::Reserved);
        exec(
            &tx,
            "INSERT INTO task_handoff_delivery_outcomes(handoff_id,result) VALUES($1,$2)",
            vec![command.handoff_id.into(), json_value(&current)?],
        )
        .await?;
        let payload = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,"project_id":state.project_id,"task_id":task,"root_task_id":task,"owner_subject":state.owner_subject,"stage":receipt.task.stage,"requirement_revision":receipt.task.routing.requirement_revision,"result":receipt});
        exec(&tx,"INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,'delivery.assignment_reserved',$3)",vec![Uuid::now_v7().into(),task.into(),payload.into()]).await?;
        exec(&tx,"DELETE FROM task_handoff_delivery_seed_permits WHERE task_id=$1 AND transaction_id=pg_current_xact_id()",vec![task.into()]).await?;
        tx.commit().await.map_err(map_db)?;
        Ok((current, false))
    }
}
