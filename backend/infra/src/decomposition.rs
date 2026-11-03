use super::*;
use domain::decomposition::*;
use domain::sdlc_lifecycle::{DeliveryRole, LifecycleStatus, RuntimeState};

struct Access {
    user: Uuid,
    source: Option<PlanSource>,
    restriction: Option<String>,
    state: Option<TaskState>,
}

impl PostgresSdlcRepository {
    async fn plan_access(
        &self,
        tx: &DatabaseTransaction,
        task: Uuid,
        actor: &Principal,
    ) -> Result<Access, AppError> {
        app::decomposition::authorize(actor, task)?;
        let (project, _) = Self::issue_access(tx, task, actor).await?;
        let (user, _) = Self::project_access(tx, project, actor).await?;
        let state = tx
            .query_one(statement(
                "SELECT state FROM sdlc_tasks WHERE task_id=$1 FOR UPDATE",
                vec![task.into()],
            ))
            .await
            .map_err(map_db)?
            .map(state_from)
            .transpose()?;
        if let Some(state) = state.as_ref() {
            if state.task_id != task
                || state.project_id != project
                || state.tracker_instance_id != self.config.instance_id
            {
                return Err(AppError::conflict("plan SDLC binding mismatch"));
            }
            state.require_root()?;
            state.interaction_protocol()?;
        }
        if !actor.trusted_human {
            let state = state.as_ref().ok_or(AppError::Forbidden)?;
            if !state.configured_cli() {
                return Err(AppError::Forbidden);
            }
            lifecycle::configured_assignment(tx, state, actor)
                .await?
                .ok_or(AppError::Forbidden)?;
            Self::bind_current_task(tx, &self.config, state, actor).await?;
        }
        let source = state
            .as_ref()
            .and_then(|s| s.revisions.last())
            .map(|r| PlanSource {
                requirements_revision: r.revision,
                requirements_hash: r.content_hash.clone(),
                analysis_result_id: None,
            });
        let mut restriction = None;
        if actor.trusted_human {
            if !actor.human_session && !actor.scopes.contains("task-tracker:write") {
                restriction = Some("Токен доступен только для чтения".into());
            }
            let row = tx.query_one(statement("SELECT archived_at IS NOT NULL AS archived,owner_id=$2 OR EXISTS(SELECT 1 FROM project_members WHERE project_id=$1 AND user_id=$2) AS editable FROM projects WHERE id=$1", vec![project.into(),user.into()]))
                .await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
            if row.try_get::<bool>("", "archived").map_err(map_db)? {
                restriction = Some("Проект архивирован".into());
            } else if !row.try_get::<bool>("", "editable").map_err(map_db)? {
                restriction = Some("Нет права редактировать задачу".into());
            }
            if let Some(frame) = spaces::request_scope(tx, actor).await? {
                let links =
                    crate::project_spaces::get(tx, shared::ProjectId::from_uuid(project)).await?;
                if frame
                    .spaces()
                    .ok_or(AppError::Forbidden)?
                    .require_project_write(&links)
                    .is_err()
                {
                    restriction = Some("Пространство доступно только для чтения".into());
                }
            }
        }
        if let Some(state) = state.as_ref() {
            if let Some(current) = lifecycle::current(tx, task).await? {
                if current.assignment.is_some()
                    && matches!(
                        current.task.runtime,
                        RuntimeState::Reserved | RuntimeState::Running | RuntimeState::Unknown
                    )
                {
                    if actor.trusted_human {
                        restriction =
                            Some("Агент изменяет задачу. Дождитесь завершения назначения".into());
                    } else if current
                        .assignment
                        .as_ref()
                        .is_none_or(|a| a.role != DeliveryRole::Architect)
                        || current.task.status != LifecycleStatus::InProgress
                        || current.task.runtime != RuntimeState::Running
                    {
                        restriction =
                            Some("Запись доступна только текущему работающему Architect".into());
                    }
                } else if !actor.trusted_human {
                    restriction = Some("Нет активного назначения Architect".into());
                }
            } else if !actor.trusted_human {
                restriction = Some("Нет активного назначения Architect".into());
            }
            if actor.trusted_human
                && state.assignment.is_some()
                && matches!(state.stage, Stage::Draft | Stage::Backlog)
            {
                let busy = tx.query_one(statement("SELECT pm_admission_state='reserved' AS busy FROM sdlc_tasks WHERE task_id=$1", vec![task.into()]))
                    .await.map_err(map_db)?.ok_or(AppError::Forbidden)?.try_get::<Option<bool>>("", "busy").map_err(map_db)?.unwrap_or(false);
                if busy && Self::configured_saved(tx, state, None).await?.is_none() {
                    restriction =
                        Some("PM изменяет задачу. Дождитесь завершения назначения".into());
                }
            }
        }
        Ok(Access {
            user,
            source,
            restriction,
            state,
        })
    }
}

fn decode<T: serde::de::DeserializeOwned>(row: QueryResult, column: &str) -> Result<T, AppError> {
    serde_json::from_value(row.try_get::<Json>("", column).map_err(map_db)?)
        .map_err(AppError::internal)
}
async fn latest(tx: &DatabaseTransaction, task: Uuid) -> Result<Option<PlanRevision>, AppError> {
    tx.query_one(statement(
        "SELECT payload FROM issue_plan_revisions WHERE issue_id=$1 ORDER BY revision DESC LIMIT 1",
        vec![task.into()],
    ))
    .await
    .map_err(map_db)?
    .map(|r| decode(r, "payload"))
    .transpose()
}
async fn operation(
    tx: &DatabaseTransaction,
    task: Uuid,
    actor: &Principal,
    key: &str,
) -> Result<Option<PlanOperation>, AppError> {
    tx.query_one(statement("SELECT result FROM issue_plan_operations WHERE issue_id=$1 AND actor_subject=$2 AND operation_key=$3", vec![task.into(),actor.subject.clone().into(),key.into()]))
        .await.map_err(map_db)?.map(|r| decode(r, "result")).transpose()
}

#[async_trait]
impl DecompositionRepository for PostgresSdlcRepository {
    async fn read_plan(&self, task: Uuid, actor: &Principal) -> Result<PlanReadback, AppError> {
        let tx = self.db.begin().await.map_err(map_db)?;
        let access = self.plan_access(&tx, task, actor).await?;
        let result = PlanReadback {
            plan: latest(&tx, task).await?,
            current_source: access.source,
            can_edit: access.restriction.is_none(),
            edit_restriction: access.restriction,
        };
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }
    async fn plan_history(
        &self,
        task: Uuid,
        actor: &Principal,
        query: PlanHistoryQuery,
    ) -> Result<PlanHistory, AppError> {
        let limit = app::decomposition::history_limit(&query)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        self.plan_access(&tx, task, actor).await?;
        let mut revisions: Vec<PlanRevisionSummary> = tx.query_all(statement("SELECT payload-'content' AS payload FROM issue_plan_revisions WHERE issue_id=$1 AND ($2::bigint IS NULL OR revision<$2) ORDER BY revision DESC LIMIT $3", vec![task.into(),query.before.into(),(i64::from(limit)+1).into()]))
            .await.map_err(map_db)?.into_iter().map(|r| decode(r,"payload")).collect::<Result<_,_>>()?;
        let next_before = if revisions.len() > usize::from(limit) {
            revisions.truncate(usize::from(limit));
            revisions.last().map(|r| r.revision)
        } else {
            None
        };
        tx.commit().await.map_err(map_db)?;
        Ok(PlanHistory {
            revisions,
            next_before,
        })
    }
    async fn plan_revision(
        &self,
        task: Uuid,
        actor: &Principal,
        revision: i64,
    ) -> Result<PlanRevision, AppError> {
        if !(1..=MAX_SAFE_VERSION).contains(&revision) {
            return Err(AppError::validation("invalid plan revision"));
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        self.plan_access(&tx, task, actor).await?;
        let result = decode(
            tx.query_one(statement(
                "SELECT payload FROM issue_plan_revisions WHERE issue_id=$1 AND revision=$2",
                vec![task.into(), revision.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::not_found("plan revision", revision))?,
            "payload",
        )?;
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }
    async fn save_plan(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SavePlan,
    ) -> Result<(PlanOperation, bool), AppError> {
        app::decomposition::validate(&command)?;
        let hash = app::sdlc::canonical_hash(&command)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let access = self.plan_access(&tx, task, actor).await?;
        // Replay is a read of the original operation, never a fresh mutation.
        // Current access is still required; a later plan never replaces its result.
        if let Some(saved) = operation(&tx, task, actor, &command.idempotency_key).await? {
            if saved.request_sha256 != hash {
                return Err(AppError::conflict(
                    "plan operation key reused with different payload",
                ));
            }
            tx.commit().await.map_err(map_db)?;
            return Ok((saved, true));
        }
        if let Some(reason) = access.restriction {
            return Err(AppError::conflict(reason));
        }
        if actor.trusted_human {
            if command.fence.is_some() {
                return Err(AppError::Forbidden);
            }
        } else {
            let state = access.state.as_ref().ok_or(AppError::Forbidden)?;
            let assignment = lifecycle::configured_assignment(&tx, state, actor)
                .await?
                .ok_or(AppError::Forbidden)?;
            let fence = MachineFence {
                assignment_id: assignment.assignment_id,
                execution_id: assignment.execution_id,
                agent_id: assignment.agent_id,
                assignment_version: assignment.version,
            };
            if command.fence.as_ref() != Some(&fence) || state.stage != Stage::Architecture {
                return Err(AppError::Forbidden);
            }
        }
        if command
            .content
            .source
            .as_ref()
            .map(|s| (s.requirements_revision, &s.requirements_hash))
            != access
                .source
                .as_ref()
                .map(|s| (s.requirements_revision, &s.requirements_hash))
        {
            return Err(AppError::conflict(
                "requirements changed; review the current requirements",
            ));
        }
        if let Some(source) = command.content.source.as_ref() {
            if let Some(id) = source.analysis_result_id {
                let exists = tx.query_one(statement("SELECT r.result_id FROM sdlc_delivery_results r JOIN sdlc_delivery_assignments a USING(assignment_id) WHERE r.task_id=$1 AND r.result_id=$2 AND (a.payload->'routing'->>'requirement_revision')::bigint=$3 AND a.payload->'routing'->>'content_hash'=$4 AND r.payload->'output'->>'role'='analyst' UNION ALL SELECT result_id FROM sdlc_analysis_results WHERE task_id=$1 AND result_id=$2 AND (payload->>'requirement_revision')::bigint=$3 AND payload->>'content_hash'=$4", vec![task.into(),id.into(),source.requirements_revision.into(),source.requirements_hash.clone().into()]))
                    .await.map_err(map_db)?;
                if exists.is_none() {
                    return Err(AppError::conflict(
                        "analysis result does not match this task and requirements",
                    ));
                }
            }
        }
        let current = latest(&tx, task).await?;
        if command.expected_revision != current.as_ref().map_or(0, |r| r.revision) {
            return Err(AppError::conflict("plan revision changed"));
        }
        let changed = current
            .as_ref()
            .is_none_or(|r| r.content != command.content);
        let result = if changed {
            if current.is_some() && command.reason.trim().is_empty() {
                return Err(AppError::validation("reason required for a changed plan"));
            }
            let current_ids: std::collections::HashSet<_> = current
                .as_ref()
                .map(|r| r.content.sections.iter().map(|s| s.id).collect())
                .unwrap_or_default();
            let new_ids: Vec<_> = command
                .content
                .sections
                .iter()
                .filter(|s| !current_ids.contains(&s.id))
                .map(|s| s.id.to_string())
                .collect();
            if !new_ids.is_empty() {
                let reused: bool = tx.query_one(statement(
                    "SELECT EXISTS(SELECT 1 FROM issue_plan_revisions r CROSS JOIN LATERAL jsonb_array_elements(r.payload->'content'->'sections') section WHERE r.issue_id=$1 AND $2::jsonb ? (section->>'id')) AS reused",
                    vec![task.into(),json!(new_ids).into()],
                )).await.map_err(map_db)?.ok_or_else(|| AppError::internal("section history missing"))?.try_get("", "reused").map_err(map_db)?;
                if reused {
                    return Err(AppError::conflict("removed section IDs cannot be reused"));
                }
            }
            let row = tx
                .query_one(statement(
                    "SELECT display_name,clock_timestamp() AS stamp FROM users WHERE id=$1",
                    vec![access.user.into()],
                ))
                .await
                .map_err(map_db)?
                .ok_or(AppError::Forbidden)?;
            let result = PlanRevision {
                task_id: task,
                revision: command.expected_revision + 1,
                content_sha256: app::sdlc::canonical_hash(&command.content)?,
                content: command.content,
                author_subject: actor.subject.clone(),
                author_name: row.try_get("", "display_name").map_err(map_db)?,
                reason: command.reason.trim().into(),
                created_at: row.try_get("", "stamp").map_err(map_db)?,
            };
            exec(&tx,"INSERT INTO issue_plan_revisions(issue_id,revision,author_id,payload) VALUES($1,$2,$3,$4)",vec![task.into(),result.revision.into(),access.user.into(),json_value(&result)?]).await?;
            result
        } else {
            current.ok_or_else(|| AppError::internal("missing unchanged plan"))?
        };
        let saved = PlanOperation {
            request_sha256: hash.clone(),
            result,
            changed,
        };
        exec(&tx,"INSERT INTO issue_plan_operations(issue_id,actor_subject,operation_key,author_id,payload_hash,result) VALUES($1,$2,$3,$4,$5,$6)",vec![task.into(),actor.subject.clone().into(),command.idempotency_key.into(),access.user.into(),hash.into(),json_value(&saved)?]).await?;
        tx.commit().await.map_err(map_db)?;
        Ok((saved, false))
    }
    async fn plan_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<PlanOperation, AppError> {
        app::sdlc_discussion::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        self.plan_access(&tx, task, actor).await?;
        let result = operation(&tx, task, actor, key)
            .await?
            .ok_or_else(|| AppError::not_found("plan operation", key))?;
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }
}
