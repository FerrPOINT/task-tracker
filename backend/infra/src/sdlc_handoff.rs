//! Genuine current-stage consent and exact input revalidation on the owner boundary.
//! Material persistence is separate from fresh owner observations and native execution admission.
use super::*;
use domain::sdlc_lifecycle::LifecycleStatus;
use domain::task_handoff::{
    CurrentStageEntry, PreparedCurrentStageHandoff, TaskExecutionBlocker as Blocker,
};
use domain::task_lifecycle;

impl PostgresSdlcRepository {
    pub(super) async fn prepare_handoff(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<PreparedCurrentStageHandoff, AppError> {
        if !actor.trusted_human
            || app::sdlc::has_pm_grant(&actor.scopes)
            || app::sdlc_lifecycle::has_delivery_grant(&actor.scopes)
        {
            return Err(AppError::Forbidden);
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        let (_, prepared) = self.prepare_handoff_locked(&tx, task, actor).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(prepared)
    }

    async fn prepare_handoff_locked(
        &self,
        tx: &DatabaseTransaction,
        task: Uuid,
        actor: &Principal,
    ) -> Result<(TaskState, PreparedCurrentStageHandoff), AppError> {
        if actor.trusted_human {
            if app::sdlc::has_pm_grant(&actor.scopes)
                || app::sdlc_lifecycle::has_delivery_grant(&actor.scopes)
            {
                return Err(AppError::Forbidden);
            }
        } else {
            if actor.human_session {
                return Err(AppError::Forbidden);
            }
            app::sdlc_reservation::reader(actor)?;
        }
        let state = Self::load_binding(tx, &self.config, task, actor).await?;
        if actor.trusted_human && actor.subject != state.owner_subject {
            return Err(AppError::Forbidden);
        }
        // Match the current reporter as well as the retained owner identity.
        let current=tx.query_one(statement("SELECT u.central_sub FROM issues i JOIN users u ON u.id=i.reporter_id WHERE i.id=$1 AND u.is_active FOR SHARE OF u",vec![task.into()]))
            .await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
        if current
            .try_get::<Option<String>>("", "central_sub")
            .map_err(map_db)?
            .as_deref()
            != Some(state.owner_subject.as_str())
        {
            return Err(AppError::Forbidden);
        }
        let row=tx.query_one(statement("SELECT state,core_revision,task_lifecycle_execution_busy(task_id) AS busy,
            (SELECT enabled FROM task_lifecycle_control WHERE singleton) AS enabled FROM sdlc_tasks WHERE task_id=$1",vec![task.into()]))
            .await.map_err(map_db)?.ok_or_else(||AppError::Unavailable("task_lifecycle_import_required".into()))?;
        let raw: Json = row.try_get("", "state").map_err(map_db)?;
        let core = task_lifecycle::effective_state(
            &raw,
            row.try_get("", "core_revision").map_err(map_db)?,
        )?;
        let busy = row.try_get::<bool>("", "busy").map_err(map_db)?;
        let mut blockers = Vec::new();
        if !row.try_get::<bool>("", "enabled").map_err(map_db)? {
            blockers.push(Blocker::ModelNotActive);
        }
        if !self
            .config
            .single_task_project_ids
            .contains(&state.project_id)
        {
            blockers.push(Blocker::ExecutionNotConfigured);
        }
        if busy {
            blockers.push(Blocker::ExecutionBusy);
        }
        if core.status == LifecycleStatus::Completed {
            blockers.push(Blocker::AlreadyCompleted);
        }
        let entry = if busy || core.status == LifecycleStatus::Completed {
            None
        } else {
            Some(CurrentStageEntry::derive(core, false)?)
        };
        let routing = match Self::policy(tx, state.project_id, None).await {
            Ok(policy) => {
                app::sdlc_draft_flow::validate_policy(
                    &policy,
                    state.project_id,
                    &state.tracker_instance_id,
                )?;
                Some(policy)
            }
            Err(AppError::NotFound(_)) => {
                blockers.push(Blocker::RoutingNotConfigured);
                None
            }
            Err(error) => return Err(error),
        };
        let source = match Self::retained_source_policy(tx, state.project_id, None).await {
            Ok(policy) => Some(policy),
            Err(AppError::NotFound(_)) => {
                blockers.push(Blocker::SourceUnavailable);
                None
            }
            Err(error) => return Err(error),
        };
        // Hash inputs that can change without a lifecycle transition too. Do not hash
        // navigation position, user display names or unrelated catalogue activity.
        let row=tx.query_one(statement("SELECT jsonb_build_object(
            'issue',to_jsonb(i)-ARRAY['position','created_at','updated_at','deleted_at','time_spent_seconds','remaining_estimate_seconds','tsv_search','status_id'],
            'labels',COALESCE((SELECT jsonb_agg(to_jsonb(label)-'created_at' ORDER BY label.id) FROM issue_labels relation JOIN labels label ON label.id=relation.label_id WHERE relation.issue_id=i.id),'[]'::jsonb),
            'custom_fields',COALESCE((SELECT jsonb_agg(jsonb_build_object('field',to_jsonb(field)-'created_at','value',value.value) ORDER BY field.id) FROM custom_fields field LEFT JOIN issue_custom_field_values value ON value.field_id=field.id AND value.issue_id=i.id WHERE field.project_id=i.project_id),'[]'::jsonb),
            'attachments',COALESCE((SELECT jsonb_agg(to_jsonb(a)-ARRAY['storage_key','created_at'] ORDER BY a.id) FROM attachments a WHERE a.issue_id=i.id),'[]'::jsonb),
            'links',COALESCE((SELECT jsonb_agg(to_jsonb(link) ORDER BY link.id) FROM issue_links link WHERE link.source_id=i.id OR link.target_id=i.id),'[]'::jsonb),
            'repositories',COALESCE((SELECT jsonb_agg(to_jsonb(link)-'created_at' ORDER BY link.forge_instance_id,link.repository_id) FROM task_repository_links link WHERE link.issue_id=i.id),'[]'::jsonb)) AS material
            FROM issues i WHERE i.id=$1 AND i.deleted_at IS NULL",vec![task.into()]))
            .await.map_err(map_db)?.ok_or_else(||AppError::not_found("issue",task))?;
        let material: Json = row.try_get("", "material").map_err(map_db)?;
        let issue_materials_sha256 = app::sdlc::canonical_hash(&material)?;
        let issue_sha256 = app::sdlc::canonical_hash(&json!({"issue":material,"state":core,
            "routing":routing.as_ref().map(|p|json!({"version":p.version,"hash":p.routing_hash})),
            "source":source.as_ref().map(|p|json!({"revision":p.source_revision,"hash":p.source_hash}))}))?;
        let prepared = PreparedCurrentStageHandoff {
            task_id: task,
            project_id: state.project_id,
            owner_subject: state.owner_subject.clone(),
            state: core,
            issue_sha256,
            issue_materials_sha256,
            entry,
            routing,
            source,
            blockers,
        };
        Ok((state, prepared))
    }
}

#[async_trait]
impl app::task_handoff::CurrentStageHandoffRepository for PostgresSdlcRepository {
    async fn pending_handoff_materials(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<app::task_handoff::PendingHandoffMaterials, AppError> {
        let tx = self.db.begin().await.map_err(map_db)?;
        let (state, prepared) = self.prepare_handoff_locked(&tx, task, actor).await?;
        let handoff = state
            .retained_fields
            .get("human_handoff_id")
            .and_then(Json::as_str)
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| AppError::conflict("current_stage_handoff_consent_required"))?;
        let row=tx.query_one(statement("SELECT receipt,routing,source_binding FROM task_handoff_materials WHERE task_id=$1 AND handoff_id=$2",
            vec![task.into(),handoff.into()])).await.map_err(map_db)?.ok_or_else(||AppError::conflict("current_stage_handoff_consent_missing"))?;
        let receipt: domain::task_handoff::TaskHandoffReceipt =
            serde_json::from_value(row.try_get::<Json>("", "receipt").map_err(map_db)?)
                .map_err(AppError::internal)?;
        let routing: domain::sdlc_routing::TaskRoutingSnapshot =
            serde_json::from_value(row.try_get::<Json>("", "routing").map_err(map_db)?)
                .map_err(AppError::internal)?;
        let source: domain::sdlc_source::TaskSourceBinding =
            serde_json::from_value(row.try_get::<Json>("", "source_binding").map_err(map_db)?)
                .map_err(AppError::internal)?;
        if !Self::manual_controlled(&tx, task).await?
            || !prepared.blockers.is_empty()
            || prepared.state.revision != receipt.resulting_revision
            || prepared.state.stage != receipt.entry.state.stage
            || prepared.state.status != receipt.entry.state.status
            || prepared.issue_materials_sha256 != receipt.issue_materials_sha256
            || prepared.owner_subject != receipt.actor_subject
            || prepared.routing.as_ref() != Some(&routing.policy)
            || prepared.source.as_ref().is_none_or(|p| {
                p.source_revision != source.source_revision || p.source_hash != source.policy_hash
            })
            || Self::source_namespace(&tx, state.project_id).await? != source.namespace
        {
            return Err(AppError::conflict(
                "current_stage_handoff_inputs_changed_or_blocked",
            ));
        }
        tx.commit().await.map_err(map_db)?;
        Ok(app::task_handoff::PendingHandoffMaterials {
            receipt,
            routing,
            source,
        })
    }

    async fn save_handoff_materials(
        &self,
        task: Uuid,
        actor: &Principal,
        command: &domain::task_handoff::TaskHandoffCommand,
    ) -> Result<(domain::task_handoff::TaskHandoffReceipt, bool), AppError> {
        command.validate()?;
        if !actor.trusted_human
            || app::sdlc::has_pm_grant(&actor.scopes)
            || app::sdlc_lifecycle::has_delivery_grant(&actor.scopes)
        {
            return Err(AppError::Forbidden);
        }
        let hash = app::sdlc::canonical_hash(command)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        // Re-check the current account, project, reporter and aggregate even on replay.
        let state = Self::load_binding(&tx, &self.config, task, actor).await?;
        if actor.subject != state.owner_subject {
            return Err(AppError::Forbidden);
        }
        let reporter = tx.query_one(statement(
            "SELECT u.id FROM issues i JOIN users u ON u.id=i.reporter_id WHERE i.id=$1 AND u.central_sub=$2 AND u.is_active FOR SHARE OF u",
            vec![task.into(),actor.subject.clone().into()])).await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
        let actor_id: Uuid = reporter.try_get("", "id").map_err(map_db)?;
        if let Some(saved) = tx.query_one(statement(
            "SELECT command_hash,receipt FROM task_handoff_materials WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![task.into(), actor.subject.clone().into(), command.idempotency_key.clone().into()])).await.map_err(map_db)? {
            if saved.try_get::<String>("", "command_hash").map_err(map_db)? != hash {
                return Err(AppError::conflict("task_handoff_idempotency_conflict"));
            }
            let receipt = serde_json::from_value(saved.try_get::<Json>("", "receipt").map_err(map_db)?).map_err(AppError::internal)?;
            tx.commit().await.map_err(map_db)?;
            return Ok((receipt, true));
        }
        // Lock current routing/source heads as well as their immutable revisions.
        // Every authoring path takes the same project row before updating these heads.
        let (state, prepared) = self.prepare_handoff_locked(&tx, task, actor).await?;
        if let Some(source) = &prepared.source {
            if Self::source_namespace(&tx, state.project_id).await? != source.verification.namespace
            {
                return Err(AppError::conflict("source policy namespace changed"));
            }
        }
        let frozen = app::task_handoff::prepare_materials(
            &state,
            actor,
            &prepared,
            command,
            app::task_handoff::HandoffAllocation {
                handoff_id: Uuid::new_v4(),
                confirmation_id: Uuid::new_v4(),
                routing_snapshot_id: Uuid::new_v4(),
                source_binding_id: Uuid::new_v4(),
                observed_at: chrono::Utc::now(),
            },
        )?;
        let receipt = &frozen.receipt;
        // Preserve the original JSON byte values (including historical timestamps
        // and absent optional keys); append only this real human consent.
        let raw = tx
            .query_one(statement(
                "SELECT state FROM sdlc_tasks WHERE task_id=$1",
                vec![task.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::not_found("SDLC binding", task))?;
        let previous: Json = raw.try_get("", "state").map_err(map_db)?;
        let mut next = previous.clone();
        next["revisions"]
            .as_array_mut()
            .ok_or_else(|| AppError::conflict("invalid requirements history"))?
            .push(serde_json::to_value(&frozen.requirement).map_err(AppError::internal)?);
        next["confirmations"]
            .as_array_mut()
            .ok_or_else(|| AppError::conflict("invalid confirmation history"))?
            .push(serde_json::to_value(&frozen.confirmation).map_err(AppError::internal)?);
        next["confirmation_revision"] = json!(receipt.requirement_revision);
        next["human_handoff_id"] = json!(receipt.handoff_id);
        exec(&tx,"INSERT INTO sdlc_requirements(task_id,revision,content_hash,payload) VALUES($1,$2,$3,$4)",
            vec![task.into(),frozen.requirement.revision.into(),frozen.requirement.content_hash.clone().into(),json_value(&frozen.requirement)?]).await?;
        exec(&tx,"INSERT INTO sdlc_confirmations(id,task_id,revision,content_hash,payload) VALUES($1,$2,$3,$4,$5)",
            vec![frozen.confirmation.id.into(),task.into(),frozen.confirmation.revision.into(),frozen.confirmation.content_hash.clone().into(),json_value(&frozen.confirmation)?]).await?;
        exec(&tx,"INSERT INTO task_handoff_materials(handoff_id,task_id,actor_id,actor_subject,idempotency_key,command_hash,previous_revision,resulting_revision,confirmation_id,requirement_revision,requirements_sha256,receipt,routing,source_binding)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
            vec![receipt.handoff_id.into(),task.into(),actor_id.into(),actor.subject.clone().into(),command.idempotency_key.clone().into(),hash.into(),
                prepared.state.revision.into(),receipt.resulting_revision.into(),receipt.confirmation_id.into(),receipt.requirement_revision.into(),receipt.requirements_sha256.clone().into(),
                json_value(receipt)?,json_value(&frozen.routing)?,json_value(&frozen.source)?]).await?;
        exec(&tx,"INSERT INTO task_handoff_material_write_permits(task_id,transaction_id,handoff_id,previous_state,next_state) VALUES($1,pg_current_xact_id(),$2,$3,$4)",
            vec![task.into(),receipt.handoff_id.into(),previous.into(),next.clone().into()]).await?;
        exec(
            &tx,
            "UPDATE sdlc_tasks SET state=$2,manual_control=true WHERE task_id=$1",
            vec![task.into(), next.into()],
        )
        .await?;
        exec(&tx,"DELETE FROM task_handoff_material_write_permits WHERE task_id=$1 AND transaction_id=pg_current_xact_id()",vec![task.into()]).await?;
        tx.commit().await.map_err(map_db)?;
        Ok((frozen.receipt, false))
    }
}
