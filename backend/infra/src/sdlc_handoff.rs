//! Current-stage human preparation on the existing owner/project/aggregate boundary.
//! This is a read plan. Fresh configuration/source observations and atomic commit follow separately.
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
        if !actor.human_session
            || !actor.trusted_human
            || app::sdlc::has_pm_grant(&actor.scopes)
            || app::sdlc_lifecycle::has_delivery_grant(&actor.scopes)
        {
            return Err(AppError::Forbidden);
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = Self::load_binding(&tx, &self.config, task, actor).await?;
        if !state.owner(actor) {
            return Err(AppError::Forbidden);
        }
        // Match the current reporter as well as the retained owner identity.
        let current=tx.query_one(statement("SELECT u.central_sub FROM issues i JOIN users u ON u.id=i.reporter_id WHERE i.id=$1 AND u.is_active FOR SHARE OF u",vec![task.into()]))
            .await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
        if current
            .try_get::<Option<String>>("", "central_sub")
            .map_err(map_db)?
            .as_deref()
            != Some(actor.subject.as_str())
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
        let routing = match Self::policy(&tx, state.project_id, None).await {
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
        let source = match Self::retained_source_policy(&tx, state.project_id, None).await {
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
        let issue_sha256 = app::sdlc::canonical_hash(&json!({"issue":material,"state":core,
            "routing":routing.as_ref().map(|p|json!({"version":p.version,"hash":p.routing_hash})),
            "source":source.as_ref().map(|p|json!({"revision":p.source_revision,"hash":p.source_hash}))}))?;
        tx.commit().await.map_err(map_db)?;
        Ok(PreparedCurrentStageHandoff {
            task_id: task,
            project_id: state.project_id,
            owner_subject: state.owner_subject,
            state: core,
            issue_sha256,
            entry,
            routing,
            source,
            blockers,
        })
    }
}
