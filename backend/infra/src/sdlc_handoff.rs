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
        let (_, prepared) = self.prepare_handoff_locked(&tx, task, actor, false).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(prepared)
    }

    pub(super) async fn prepare_handoff_locked(
        &self,
        tx: &DatabaseTransaction,
        task: Uuid,
        actor: &Principal,
        pending_request_read: bool,
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
            if app::task_handoff::scheduler_writer(actor, &self.config).is_err() {
                app::sdlc_reservation::reader(actor)?;
            }
        }
        let state = Self::load_binding(tx, &self.config, task, actor).await?;
        if actor.trusted_human && actor.subject != state.owner_subject {
            return Err(AppError::Forbidden);
        }
        // Match the current reporter as well as the retained owner identity.
        let current=tx.query_one(statement("SELECT u.id,u.central_sub,p.owner_id,(p.archived_at IS NOT NULL) AS project_read_only FROM issues i JOIN users u ON u.id=i.reporter_id JOIN projects p ON p.id=i.project_id WHERE i.id=$1 AND u.is_active FOR SHARE OF u,p",vec![task.into()]))
            .await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
        if current
            .try_get::<Option<String>>("", "central_sub")
            .map_err(map_db)?
            .as_deref()
            != Some(state.owner_subject.as_str())
        {
            return Err(AppError::Forbidden);
        }
        let owner_id: Uuid = current.try_get("", "id").map_err(map_db)?;
        if current.try_get::<Uuid>("", "owner_id").map_err(map_db)? != owner_id {
            tx.query_one(statement(
                "SELECT user_id FROM project_members WHERE project_id=$1 AND user_id=$2 FOR SHARE",
                vec![state.project_id.into(), owner_id.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or(AppError::Forbidden)?;
        }
        let row=tx.query_one(statement("SELECT state,core_revision,CASE WHEN $2 THEN task_lifecycle_runtime_busy(task_id) ELSE task_lifecycle_execution_busy(task_id) END AS busy,
            (SELECT enabled FROM task_lifecycle_control WHERE singleton) AS enabled FROM sdlc_tasks WHERE task_id=$1",vec![task.into(),pending_request_read.into()]))
            .await.map_err(map_db)?.ok_or_else(||AppError::Unavailable("task_lifecycle_import_required".into()))?;
        let raw: Json = row.try_get("", "state").map_err(map_db)?;
        let core = task_lifecycle::effective_state(
            &raw,
            row.try_get("", "core_revision").map_err(map_db)?,
        )?;
        let busy = row.try_get::<bool>("", "busy").map_err(map_db)?;
        let mut blockers = Vec::new();
        if current
            .try_get::<bool>("", "project_read_only")
            .map_err(map_db)?
        {
            blockers.push(Blocker::ProjectReadOnly)
        }
        if !row.try_get::<bool>("", "enabled").map_err(map_db)? {
            blockers.push(Blocker::ModelNotActive);
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
        let default_repository=source.as_ref().map(|policy|json!({
            "namespace":policy.verification.namespace,"forge_instance_id":policy.source.forge_instance_id,
            "repository_id":policy.source.repository_id,"kind":"hosted"}));
        // Repository identity includes the selected project source before its link
        // is projected at explicit queueing. Display names and relation authorship
        // are not task materials; unrelated explicit repository identities remain.
        // Hash inputs that can change without a lifecycle transition too. Do not hash
        // navigation position, user display names or unrelated catalogue activity.
        let row=tx.query_one(statement("SELECT jsonb_build_object(
            'issue',to_jsonb(i)-ARRAY['position','created_at','updated_at','deleted_at','time_spent_seconds','remaining_estimate_seconds','tsv_search','status_id'],
            'labels',COALESCE((SELECT jsonb_agg(to_jsonb(label)-'created_at' ORDER BY label.id) FROM issue_labels relation JOIN labels label ON label.id=relation.label_id WHERE relation.issue_id=i.id),'[]'::jsonb),
            'custom_fields',COALESCE((SELECT jsonb_agg(jsonb_build_object('field',to_jsonb(field)-'created_at','value',value.value) ORDER BY field.id) FROM custom_fields field LEFT JOIN issue_custom_field_values value ON value.field_id=field.id AND value.issue_id=i.id WHERE field.project_id=i.project_id),'[]'::jsonb),
            'attachments',COALESCE((SELECT jsonb_agg(to_jsonb(a)-ARRAY['storage_key','created_at'] ORDER BY a.id) FROM attachments a WHERE a.issue_id=i.id),'[]'::jsonb),
            'comments_sha256',(SELECT encode(sha256(convert_to(COALESCE(string_agg(c.id::text||'/'||c.author_id::text||'/'||encode(sha256(convert_to(c.body,'UTF8')),'hex'),',' ORDER BY c.id),''),'UTF8')),'hex') FROM comments c WHERE c.issue_id=i.id),
            'links',COALESCE((SELECT jsonb_agg(to_jsonb(link) ORDER BY link.id) FROM issue_links link WHERE link.source_id=i.id OR link.target_id=i.id),'[]'::jsonb),
            'repositories',COALESCE((SELECT jsonb_agg(repo ORDER BY repo->>'forge_instance_id',repo->>'repository_id',repo->'namespace',repo->>'kind') FROM (
                SELECT jsonb_build_object('namespace',link.snapshot->'namespace','forge_instance_id',link.forge_instance_id,'repository_id',link.repository_id,'kind',link.snapshot->'kind') AS repo FROM task_repository_links link WHERE link.issue_id=i.id
                UNION SELECT $2::jsonb WHERE $2::jsonb IS NOT NULL
            ) effective),'[]'::jsonb)) AS material
            FROM issues i WHERE i.id=$1 AND i.deleted_at IS NULL",vec![task.into(),default_repository.into()]))
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
    async fn prepare_requested_handoff(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<PreparedCurrentStageHandoff, AppError> {
        self.prepare_handoff_delivery(task, actor).await
    }
    async fn reserve_current_stage_handoff(
        &self,
        task: Uuid,
        actor: &Principal,
        command: &domain::task_handoff::ReserveTaskHandoffCommand,
        observed: &app::task_handoff_observations::MatchedHandoffInputs,
    ) -> Result<(domain::current_stage_execution::CurrentStageExecution, bool), AppError> {
        self.adopt_handoff_delivery(task, actor, command, observed)
            .await
    }

    async fn handoff_reservation_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
        mutation: bool,
    ) -> Result<Option<app::task_handoff::SavedHandoffReservation>, AppError> {
        self.handoff_reservation_read(task, actor, key, mutation)
            .await
    }

    async fn current_owner_handoff(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<Option<domain::task_handoff::CurrentHandoffStatus>, AppError> {
        if !actor.trusted_human
            || app::sdlc::has_pm_grant(&actor.scopes)
            || app::sdlc_lifecycle::has_delivery_grant(&actor.scopes)
        {
            return Err(AppError::Forbidden);
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = Self::load_binding(&tx, &self.config, task, actor).await?;
        if state.owner_subject != actor.subject {
            return Err(AppError::Forbidden);
        }
        let reporter=tx.query_one(statement("SELECT u.central_sub FROM issues i JOIN users u ON u.id=i.reporter_id WHERE i.id=$1 AND u.is_active FOR SHARE OF u",vec![task.into()])).await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
        if reporter
            .try_get::<Option<String>>("", "central_sub")
            .map_err(map_db)?
            .as_deref()
            != Some(actor.subject.as_str())
        {
            return Err(AppError::Forbidden);
        }
        let row=tx.query_one(statement("SELECT m.receipt,m.execution_requested,c.receipt AS cancellation,t.core_revision,
            t.manual_control AND NOT task_lifecycle_runtime_busy(t.task_id) AND p.archived_at IS NULL AND c.handoff_id IS NULL AS can_cancel
            FROM sdlc_tasks t JOIN projects p ON p.id=t.project_id
            JOIN task_handoff_materials m ON m.task_id=t.task_id AND m.handoff_id::text=t.state->>'human_handoff_id' AND m.execution_requested
            LEFT JOIN task_handoff_cancellations c ON c.task_id=m.task_id AND c.handoff_id=m.handoff_id
            WHERE t.task_id=$1 AND m.actor_subject=$2",vec![task.into(),actor.subject.clone().into()])).await.map_err(map_db)?;
        let current = row
            .map(|row| -> Result<_, AppError> {
                Ok(domain::task_handoff::CurrentHandoffStatus {
                    core_revision: row.try_get("", "core_revision").map_err(map_db)?,
                    can_cancel: row.try_get("", "can_cancel").map_err(map_db)?,
                    operation: domain::task_handoff::TaskHandoffOperation {
                        receipt: serde_json::from_value(
                            row.try_get::<Json>("", "receipt").map_err(map_db)?,
                        )
                        .map_err(AppError::internal)?,
                        execution_requested: row
                            .try_get("", "execution_requested")
                            .map_err(map_db)?,
                        cancellation: row
                            .try_get::<Option<Json>>("", "cancellation")
                            .map_err(map_db)?
                            .map(serde_json::from_value)
                            .transpose()
                            .map_err(AppError::internal)?,
                    },
                })
            })
            .transpose()?;
        tx.commit().await.map_err(map_db)?;
        Ok(current)
    }

    async fn handoff_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<Option<app::task_handoff::SavedHandoffOperation>, AppError> {
        task_lifecycle::validate_command_fields(1, key)?;
        if !actor.trusted_human
            || app::sdlc::has_pm_grant(&actor.scopes)
            || app::sdlc_lifecycle::has_delivery_grant(&actor.scopes)
        {
            return Err(AppError::Forbidden);
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = Self::load_binding(&tx, &self.config, task, actor).await?;
        if state.owner_subject != actor.subject {
            return Err(AppError::Forbidden);
        }
        tx.query_one(statement("SELECT u.id FROM issues i JOIN users u ON u.id=i.reporter_id WHERE i.id=$1 AND u.central_sub=$2 AND u.is_active FOR SHARE OF u",
            vec![task.into(),actor.subject.clone().into()])).await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
        let row=tx.query_one(statement("SELECT m.command_hash,m.receipt,m.execution_requested,c.receipt AS cancellation FROM task_handoff_materials m LEFT JOIN task_handoff_cancellations c ON c.task_id=m.task_id AND c.handoff_id=m.handoff_id WHERE m.task_id=$1 AND m.actor_subject=$2 AND m.idempotency_key=$3",
            vec![task.into(),actor.subject.clone().into(),key.to_owned().into()])).await.map_err(map_db)?;
        let saved = row
            .map(|row| -> Result<_, AppError> {
                Ok(app::task_handoff::SavedHandoffOperation {
                    command_hash: row.try_get("", "command_hash").map_err(map_db)?,
                    operation: domain::task_handoff::TaskHandoffOperation {
                        receipt: serde_json::from_value(
                            row.try_get::<Json>("", "receipt").map_err(map_db)?,
                        )
                        .map_err(AppError::internal)?,
                        execution_requested: row
                            .try_get("", "execution_requested")
                            .map_err(map_db)?,
                        cancellation: row
                            .try_get::<Option<Json>>("", "cancellation")
                            .map_err(map_db)?
                            .map(serde_json::from_value)
                            .transpose()
                            .map_err(AppError::internal)?,
                    },
                })
            })
            .transpose()?;
        tx.commit().await.map_err(map_db)?;
        Ok(saved)
    }

    async fn pending_handoff_materials(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<app::task_handoff::PendingHandoffMaterials, AppError> {
        let tx = self.db.begin().await.map_err(map_db)?;
        let pending = self.pending_handoff_locked(&tx, task, actor, false).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(pending)
    }

    async fn requested_handoff_execution(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<domain::current_stage_execution::CurrentStageExecution, AppError> {
        app::task_handoff::scheduler_reader(actor, &self.config)?;
        // Read the genuine owner under ACL first, resolve its app proof outside
        // a transaction, then re-read the same basis before exposing context.
        let tx = self.db.begin().await.map_err(map_db)?;
        let first = self.pending_handoff_locked(&tx, task, actor, true).await?;
        tx.commit().await.map_err(map_db)?;
        if app::native_space_access::application_policy_required().await? {
            let native = app::native_space_access::current()?.ok_or(AppError::Forbidden)?;
            let owner =
                Uuid::parse_str(&first.receipt.actor_subject).map_err(|_| AppError::Forbidden)?;
            native
                .require_owner(
                    owner,
                    &["task-tracker", "fleet-control", "project-workflow", "ci-cd"],
                )
                .await?;
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        let pending = self.pending_handoff_locked(&tx, task, actor, true).await?;
        if pending.receipt.handoff_id != first.receipt.handoff_id
            || pending.receipt.actor_subject != first.receipt.actor_subject
        {
            return Err(AppError::conflict(
                "current_stage_handoff_inputs_changed_or_blocked",
            ));
        }
        // This is metadata for an unclaimed human request. Do not bind an old
        // PM/delivery task scope: it can select a historical source namespace.
        // No write-capable native_task_scope is installed by this read.
        let policy=tx.query_one(statement("SELECT active OR EXISTS(SELECT 1 FROM tracker_space_catalog WHERE metadata IS NOT NULL) AS enforced FROM tracker_space_admission_state WHERE singleton",vec![])).await.map_err(map_db)?.ok_or_else(||AppError::Unavailable("space_admission_state_missing".into()))?;
        if policy.try_get::<bool>("", "enforced").map_err(map_db)? {
            let native = app::native_space_access::current()?.ok_or(AppError::Forbidden)?;
            if native.identity().subject != actor.subject || native.may_write() {
                return Err(AppError::Forbidden);
            }
            let namespace = &pending.source.namespace;
            let catalog=tx.query_one(statement("SELECT state FROM tracker_space_catalog WHERE registry_instance_id=$1 AND space_id=$2 AND admin_revision IS NOT NULL FOR SHARE",vec![namespace.registry_instance_id.into(),namespace.namespace_id.into()])).await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
            let state: String = catalog.try_get("", "state").map_err(map_db)?;
            if !matches!(state.as_str(), "active" | "archiving" | "archived") {
                return Err(AppError::Forbidden);
            }
            crate::guarded_db::bind_commit_clock(&tx, native.validity()?.valid_until)
                .await
                .map_err(map_db)?;
        }
        let execution = app::task_handoff::seed_current_stage(&pending)?;
        tx.commit().await.map_err(map_db)?;
        Ok(execution)
    }

    async fn requested_handoffs(
        &self,
        actor: &Principal,
        query: domain::sdlc_candidates::DeliveryCandidatesQuery,
    ) -> Result<domain::task_handoff::RequestedHandoffs, AppError> {
        app::task_handoff::scheduler_reader(actor, &self.config)?;
        query.validate()?;
        let tx = self.db.begin().await.map_err(map_db)?;
        Self::project_access(&tx, query.project_id, actor).await?;
        let rows=tx.query_all(statement("SELECT m.receipt FROM sdlc_tasks t
            JOIN task_handoff_materials m ON m.task_id=t.task_id AND m.handoff_id::text=t.state->>'human_handoff_id'
            JOIN issues i ON i.id=t.task_id AND i.project_id=t.project_id AND i.deleted_at IS NULL
            JOIN projects p ON p.id=t.project_id AND p.archived_at IS NULL
            JOIN users owner ON owner.id=i.reporter_id AND owner.is_active AND owner.central_sub=m.actor_subject
            WHERE t.project_id=$1 AND t.tracker_instance_id=$2 AND t.manual_control
            AND t.root_task_id=t.task_id AND m.execution_requested
            AND (SELECT enabled FROM task_lifecycle_control WHERE singleton)
            AND m.actor_subject=t.owner_subject AND m.resulting_revision=t.core_revision
            AND (p.owner_id=owner.id OR EXISTS(SELECT 1 FROM project_members member WHERE member.project_id=p.id AND member.user_id=owner.id))
            AND m.receipt->'entry'->'state'->>'stage'=CASE WHEN t.state->>'stage'='Clarification' THEN 'Draft' ELSE t.state->>'stage' END
            AND m.receipt->'entry'->'state'->>'status'=COALESCE(t.state->>'status','Ready')
            AND NOT task_lifecycle_runtime_busy(t.task_id)
            AND NOT EXISTS(SELECT 1 FROM task_handoff_cancellations c WHERE c.handoff_id=m.handoff_id)
            AND ($3::uuid IS NULL OR t.task_id>$3)
            ORDER BY t.task_id LIMIT $4",vec![query.project_id.into(),self.config.instance_id.clone().into(),query.after.into(),(i64::from(query.limit)+1).into()])).await.map_err(map_db)?;
        let more = rows.len() > usize::from(query.limit);
        let mut items = Vec::new();
        for row in rows.into_iter().take(usize::from(query.limit)) {
            let receipt: domain::task_handoff::TaskHandoffReceipt =
                serde_json::from_value(row.try_get::<Json>("", "receipt").map_err(map_db)?)
                    .map_err(AppError::internal)?;
            items.push(domain::task_handoff::RequestedHandoff {
                handoff_id: receipt.handoff_id,
                task_id: receipt.task_id,
                owner_subject: receipt.actor_subject,
                entry: receipt.entry,
                core_revision: receipt.resulting_revision,
            });
        }
        let next_cursor = if more {
            items.last().map(|item| item.task_id)
        } else {
            None
        };
        let observed_at = lifecycle::now(&tx).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(domain::task_handoff::RequestedHandoffs {
            contract_version: 1,
            tracker_instance_id: self.config.instance_id.clone(),
            project_id: query.project_id,
            observed_at,
            items,
            next_cursor,
        })
    }

    async fn cancel_current_stage_handoff(
        &self,
        task: Uuid,
        handoff: Uuid,
        actor: &Principal,
        command: &domain::task_handoff::CancelTaskHandoffCommand,
    ) -> Result<(domain::task_handoff::CancelTaskHandoffReceipt, bool), AppError> {
        command.validate()?;
        if handoff.is_nil() {
            return Err(AppError::validation("invalid handoff reference"));
        }
        if !actor.trusted_human
            || app::sdlc::has_pm_grant(&actor.scopes)
            || app::sdlc_lifecycle::has_delivery_grant(&actor.scopes)
        {
            return Err(AppError::Forbidden);
        }
        let hash = app::sdlc::canonical_hash(&json!({"handoff_id":handoff,"command":command}))?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = Self::load_binding(&tx, &self.config, task, actor).await?;
        if state.owner_subject != actor.subject {
            return Err(AppError::Forbidden);
        }
        let reporter=tx.query_one(statement("SELECT u.id FROM issues i JOIN users u ON u.id=i.reporter_id WHERE i.id=$1 AND u.central_sub=$2 AND u.is_active FOR SHARE OF u",
            vec![task.into(),actor.subject.clone().into()])).await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
        let actor_id: Uuid = reporter.try_get("", "id").map_err(map_db)?;
        if let Some(saved)=tx.query_one(statement("SELECT command_hash,receipt FROM task_handoff_cancellations WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![task.into(),actor.subject.clone().into(),command.idempotency_key.clone().into()])).await.map_err(map_db)? {
            if saved.try_get::<String>("","command_hash").map_err(map_db)?!=hash {return Err(AppError::conflict("task_handoff_idempotency_conflict"));}
            let receipt=serde_json::from_value(saved.try_get::<Json>("","receipt").map_err(map_db)?).map_err(AppError::internal)?;
            tx.commit().await.map_err(map_db)?;return Ok((receipt,true));
        }
        let row=tx.query_one(statement("SELECT state,core_revision,manual_control,task_lifecycle_runtime_busy(task_id) AS busy FROM sdlc_tasks WHERE task_id=$1",vec![task.into()]))
            .await.map_err(map_db)?.ok_or_else(||AppError::not_found("SDLC binding",task))?;
        let raw: Json = row.try_get("", "state").map_err(map_db)?;
        let revision: i64 = row.try_get("", "core_revision").map_err(map_db)?;
        if revision != command.expected_revision {
            return Err(AppError::conflict("task_lifecycle_revision_conflict"));
        }
        if !row.try_get::<bool>("", "manual_control").map_err(map_db)?
            || row.try_get::<bool>("", "busy").map_err(map_db)?
            || raw.get("human_handoff_id").and_then(Json::as_str)
                != Some(handoff.to_string().as_str())
        {
            return Err(AppError::conflict(
                "current_stage_handoff_request_already_claimed_or_changed",
            ));
        }
        let queued=tx.query_one(statement("SELECT execution_requested FROM task_handoff_materials WHERE task_id=$1 AND handoff_id=$2",
            vec![task.into(),handoff.into()])).await.map_err(map_db)?.ok_or_else(||AppError::not_found("handoff",handoff))?;
        if !queued
            .try_get::<bool>("", "execution_requested")
            .map_err(map_db)?
        {
            return Err(AppError::conflict("current_stage_handoff_not_requested"));
        }
        let next_revision = revision
            .checked_add(1)
            .filter(|r| *r <= MAX_SAFE_VERSION)
            .ok_or_else(|| AppError::conflict("task_lifecycle_revision_exhausted"))?;
        let receipt = domain::task_handoff::CancelTaskHandoffReceipt {
            task_id: task,
            handoff_id: handoff,
            actor_subject: actor.subject.clone(),
            resulting_revision: next_revision,
            created_at: chrono::Utc::now(),
        };
        let mut next = raw;
        next.as_object_mut()
            .ok_or_else(|| AppError::conflict("invalid task state"))?
            .remove("human_handoff_id");
        exec(&tx,"INSERT INTO task_handoff_cancellations(task_id,handoff_id,actor_id,actor_subject,idempotency_key,command_hash,previous_revision,resulting_revision,receipt) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",
            vec![task.into(),handoff.into(),actor_id.into(),actor.subject.clone().into(),command.idempotency_key.clone().into(),hash.into(),revision.into(),next_revision.into(),json_value(&receipt)?]).await?;
        exec(&tx,"INSERT INTO task_handoff_cancel_write_permits(task_id,transaction_id,handoff_id,next_state) VALUES($1,pg_current_xact_id(),$2,$3)",
            vec![task.into(),handoff.into(),next.clone().into()]).await?;
        exec(
            &tx,
            "UPDATE sdlc_tasks SET state=$2 WHERE task_id=$1",
            vec![task.into(), next.into()],
        )
        .await?;
        exec(&tx,"DELETE FROM task_handoff_cancel_write_permits WHERE task_id=$1 AND transaction_id=pg_current_xact_id()",vec![task.into()]).await?;
        tx.commit().await.map_err(map_db)?;
        Ok((receipt, false))
    }

    async fn request_current_stage_handoff(
        &self,
        task: Uuid,
        actor: &Principal,
        command: &domain::task_handoff::TaskHandoffCommand,
        observed: &app::task_handoff_observations::MatchedHandoffInputs,
    ) -> Result<(domain::task_handoff::TaskHandoffReceipt, bool), AppError> {
        self.commit_handoff_materials(task, actor, command, Some(observed))
            .await
    }

    async fn save_handoff_materials(
        &self,
        task: Uuid,
        actor: &Principal,
        command: &domain::task_handoff::TaskHandoffCommand,
    ) -> Result<(domain::task_handoff::TaskHandoffReceipt, bool), AppError> {
        self.commit_handoff_materials(task, actor, command, None)
            .await
    }
}

impl PostgresSdlcRepository {
    async fn commit_handoff_materials(
        &self,
        task: Uuid,
        actor: &Principal,
        command: &domain::task_handoff::TaskHandoffCommand,
        observed: Option<&app::task_handoff_observations::MatchedHandoffInputs>,
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
            "SELECT command_hash,receipt,execution_requested FROM task_handoff_materials WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![task.into(), actor.subject.clone().into(), command.idempotency_key.clone().into()])).await.map_err(map_db)? {
            if saved.try_get::<String>("", "command_hash").map_err(map_db)? != hash {
                return Err(AppError::conflict("task_handoff_idempotency_conflict"));
            }
            if observed.is_some() && !saved.try_get::<bool>("","execution_requested").map_err(map_db)? {
                return Err(AppError::conflict("current_stage_handoff_key_operation_conflict"));
            }
            let receipt = serde_json::from_value(saved.try_get::<Json>("", "receipt").map_err(map_db)?).map_err(AppError::internal)?;
            tx.commit().await.map_err(map_db)?;
            return Ok((receipt, true));
        }
        // Lock current routing/source heads as well as their immutable revisions.
        // Every authoring path takes the same project row before updating these heads.
        let (state, prepared) = self.prepare_handoff_locked(&tx, task, actor, false).await?;
        if let Some(observed) = observed {
            observed.ensure_for(&prepared)?;
        }
        if let Some(source) = &prepared.source {
            if Self::source_namespace(&tx, state.project_id).await? != source.verification.namespace
            {
                return Err(AppError::conflict("source policy namespace changed"));
            }
        }
        if observed.is_some() {
            Self::ensure_handoff_repository_link(
                &tx,
                &state,
                actor_id,
                prepared
                    .source
                    .as_ref()
                    .ok_or_else(|| AppError::Unavailable("source_not_configured".into()))?,
            )
            .await?;
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
        exec(&tx,"INSERT INTO task_handoff_materials(handoff_id,task_id,actor_id,actor_subject,idempotency_key,command_hash,previous_revision,resulting_revision,confirmation_id,requirement_revision,requirements_sha256,receipt,routing,source_binding,execution_requested,queue_observations)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)",
            vec![receipt.handoff_id.into(),task.into(),actor_id.into(),actor.subject.clone().into(),command.idempotency_key.clone().into(),hash.into(),
                prepared.state.revision.into(),receipt.resulting_revision.into(),receipt.confirmation_id.into(),receipt.requirement_revision.into(),receipt.requirements_sha256.clone().into(),
                json_value(receipt)?,json_value(&frozen.routing)?,json_value(&frozen.source)?,observed.is_some().into(),observed.map(|o|o.audit()).into()]).await?;
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

impl PostgresSdlcRepository {
    async fn ensure_handoff_repository_link(
        tx: &DatabaseTransaction,
        state: &TaskState,
        creator: Uuid,
        policy: &domain::sdlc_source::ProjectSourcePolicy,
    ) -> Result<(), AppError> {
        let expected = domain::task_repositories::TaskRepositoryLink {
            namespace: policy.verification.namespace.clone(),
            forge_instance_id: policy.source.forge_instance_id,
            repository_id: policy.source.repository_id,
            public_name: policy.verification.public_name.clone(),
            kind: "hosted".into(),
        };
        exec(tx,"INSERT INTO task_repository_links(issue_id,forge_instance_id,repository_id,snapshot,created_by) VALUES($1,$2,$3,$4,$5) ON CONFLICT(issue_id,forge_instance_id,repository_id) DO NOTHING",vec![state.task_id.into(),expected.forge_instance_id.into(),expected.repository_id.into(),json_value(&expected)?,creator.into()]).await?;
        if let Some(row)=tx.query_one(statement("SELECT snapshot FROM task_repository_links WHERE issue_id=$1 AND forge_instance_id=$2 AND repository_id=$3 FOR SHARE",vec![state.task_id.into(),expected.forge_instance_id.into(),expected.repository_id.into()])).await.map_err(map_db)?{
            let link:domain::task_repositories::TaskRepositoryLink=serde_json::from_value(row.try_get::<Json>("","snapshot").map_err(map_db)?).map_err(AppError::internal)?;
            if link.namespace!=expected.namespace || link.forge_instance_id!=expected.forge_instance_id || link.repository_id!=expected.repository_id || link.kind!=expected.kind {
                return Err(AppError::conflict("task source repository link differs"));
            }
        } else {
            return Err(AppError::conflict("task source repository link missing"));
        }
        Ok(())
    }

    pub(super) async fn pending_handoff_locked(
        &self,
        tx: &DatabaseTransaction,
        task: Uuid,
        actor: &Principal,
        require_requested: bool,
    ) -> Result<app::task_handoff::PendingHandoffMaterials, AppError> {
        let (state, prepared) = self.prepare_handoff_locked(tx, task, actor, true).await?;
        let handoff = state
            .retained_fields
            .get("human_handoff_id")
            .and_then(Json::as_str)
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| AppError::conflict("current_stage_handoff_consent_required"))?;
        let row=tx.query_one(statement("SELECT receipt,routing,source_binding,execution_requested FROM task_handoff_materials WHERE task_id=$1 AND handoff_id=$2",
            vec![task.into(),handoff.into()])).await.map_err(map_db)?.ok_or_else(||AppError::conflict("current_stage_handoff_consent_missing"))?;
        if require_requested
            && !row
                .try_get::<bool>("", "execution_requested")
                .map_err(map_db)?
        {
            return Err(AppError::conflict("current_stage_handoff_not_requested"));
        }
        let receipt: domain::task_handoff::TaskHandoffReceipt =
            serde_json::from_value(row.try_get::<Json>("", "receipt").map_err(map_db)?)
                .map_err(AppError::internal)?;
        let routing: domain::sdlc_routing::TaskRoutingSnapshot =
            serde_json::from_value(row.try_get::<Json>("", "routing").map_err(map_db)?)
                .map_err(AppError::internal)?;
        let source: domain::sdlc_source::TaskSourceBinding =
            serde_json::from_value(row.try_get::<Json>("", "source_binding").map_err(map_db)?)
                .map_err(AppError::internal)?;
        if !Self::manual_controlled(tx, task).await?
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
            || Self::source_namespace(tx, state.project_id).await? != source.namespace
        {
            return Err(AppError::conflict(
                "current_stage_handoff_inputs_changed_or_blocked",
            ));
        }
        Ok(app::task_handoff::PendingHandoffMaterials {
            receipt,
            routing,
            source,
        })
    }
}
