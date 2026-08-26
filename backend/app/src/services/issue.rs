use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use crate::commands::{CreateIssueCommand, UpdateIssueCommand};
use crate::dto::IssueDto;
use domain::{
    BoardRepository, Issue, IssueQuery, IssueRepository, ProjectRepository, StatusRepository,
    WorkflowTransitionRepository,
};
use shared::{AppError, IssueId, ProjectKey, StatusId, UserId};

pub struct IssueServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    boards: Arc<dyn BoardRepository>,
    users: Arc<dyn domain::UserRepository>,
    statuses: Arc<dyn StatusRepository>,
    transitions: Arc<dyn WorkflowTransitionRepository>,
    status_history: Arc<dyn domain::IssueStatusHistoryRepository>,
    sprints: Arc<dyn domain::SprintRepository>,
    components: Arc<dyn domain::ProjectComponentRepository>,
    versions: Arc<dyn domain::ProjectVersionRepository>,
    events: crate::context::EventBus,
    notifications: Arc<dyn domain::NotificationRepository>,
    authz: Authz,
}

impl IssueServiceImpl {
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        boards: Arc<dyn BoardRepository>,
        users: Arc<dyn domain::UserRepository>,
        statuses: Arc<dyn StatusRepository>,
        transitions: Arc<dyn WorkflowTransitionRepository>,
        status_history: Arc<dyn domain::IssueStatusHistoryRepository>,
        sprints: Arc<dyn domain::SprintRepository>,
        components: Arc<dyn domain::ProjectComponentRepository>,
        versions: Arc<dyn domain::ProjectVersionRepository>,
        events: crate::context::EventBus,
        notifications: Arc<dyn domain::NotificationRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            issues,
            projects,
            events,
            boards,
            users,
            statuses,
            transitions,
            status_history,
            sprints,
            components,
            versions,
            notifications,
            authz,
        }
    }

    /// Create a notification and publish a real-time SSE event.
    async fn create_notification(&self, notification: domain::Notification) {
        let recipient_id = notification.recipient_id;
        if let Ok(_id) = self.notifications.save(&notification).await {
            self.events
                .publish(shared::TrackerEvent::NotificationCreated {
                    recipient_id: recipient_id.to_string(),
                });
        }
    }
}

#[async_trait]
impl crate::context::IssueService for IssueServiceImpl {
    async fn create(
        &self,
        cmd: CreateIssueCommand,
        requester: UserId,
    ) -> Result<IssueDto, AppError> {
        let project = self.projects.get_by_key(&cmd.project_key).await?;
        self.authz
            .require_project_edit(project.id, requester)
            .await?;
        if cmd.summary.trim().is_empty() || cmd.summary.chars().count() > 500 {
            return Err(AppError::invalid_input(
                "summary must be between 1 and 500 characters",
            ));
        }
        if cmd
            .description
            .as_deref()
            .is_some_and(|description| description.chars().count() > 100_000)
        {
            return Err(AppError::invalid_input(
                "description must not exceed 100000 characters",
            ));
        }
        let status_id = StatusId::from_uuid(
            cmd.status_id
                .parse()
                .map_err(|_| AppError::invalid_input("status_id"))?,
        );
        // Retry on key conflicts: concurrent creators may compute the same next number.
        let mut issue = None;
        for _ in 0..5 {
            let number = self.projects.next_issue_number(project.id).await?;
            let mut candidate = Issue::create(
                &project,
                number,
                cmd.issue_type,
                status_id,
                cmd.summary.clone(),
                cmd.description.clone().map(domain::RichText::from),
                cmd.reporter_id,
                cmd.priority,
            );
            if let Some(assignee_id) = cmd.assignee_id {
                candidate.assign(Some(assignee_id));
            }
            match self.issues.save(&candidate).await {
                Ok(_) => {
                    issue = Some(candidate);
                    break;
                }
                // Key collisions arrive either as a raw DB error naming the
                // constraint or as the sanitized unique-violation Conflict.
                // `issues.key` is the only unique constraint on INSERT here,
                // so any duplicate-entry conflict is de facto a key collision.
                Err(AppError::Database(msg)) if msg.contains("issues_key_key") => continue,
                Err(AppError::Conflict(ref msg)) if msg == "duplicate entry" => continue,
                Err(e) => return Err(e),
            }
        }
        let issue = issue.ok_or_else(|| {
            AppError::conflict("could not allocate a unique issue key, try again")
        })?;
        let statuses = self.statuses.list_all().await.unwrap_or_default();
        let column = statuses
            .iter()
            .find(|s| s.id == issue.status_id)
            .map(|s| s.name.as_ref().to_string())
            .unwrap_or_else(|| super::helpers::issue_status_column(issue.status_id));
        let (assignee_name, reporter_name) =
            super::helpers::resolve_names(self.users.clone(), &issue).await;
        self.events.publish(shared::TrackerEvent::IssueCreated {
            issue_id: issue.id.to_string(),
            project_key: project.key.to_string(),
        });
        // Notify assignee if assigned and not the reporter
        if let Some(assignee_id) = issue.assignee_id {
            if assignee_id != cmd.reporter_id {
                let key = issue.key.to_string();
                self.create_notification(domain::Notification {
                    id: shared::NotificationId::new(),
                    recipient_id: assignee_id,
                    event_type: "issue_assigned".into(),
                    entity_type: "issue".into(),
                    entity_id: Some(issue.id.as_uuid()),
                    actor_id: Some(cmd.reporter_id),
                    title: format!("You were assigned to {}", key).into(),
                    body: Some(issue.summary.as_ref().to_string().into()),
                    is_read: false,
                    read_at: None,
                    action_url: Some(
                        format!("/projects/{}/issues/{}", project.key, issue.id).into(),
                    ),
                    metadata: serde_json::json!({"issue_key": key}),
                    created_at: shared::now(),
                })
                .await;
            }
        }
        Ok(IssueDto::from_issue(
            issue,
            project.name.as_ref().to_string(),
            column,
            assignee_name,
            reporter_name,
        ))
    }

    async fn transition(
        &self,
        cmd: crate::commands::TransitionIssueCommand,
    ) -> Result<IssueDto, AppError> {
        let issue = self.issues.get_by_id(cmd.issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, cmd.actor_id)
            .await?;
        let board = self.boards.get_default_by_project(issue.project_id).await?;
        let statuses = self.statuses.list_all().await.unwrap_or_default();
        let valid = statuses.iter().any(|s| s.id == cmd.target_status_id)
            || board.columns.iter().any(|c| c.id == cmd.target_status_id);
        if !valid {
            return Err(AppError::invalid_input("invalid target status"));
        }
        let allowed = self
            .transitions
            .is_allowed(issue.status_id, cmd.target_status_id)
            .await?;
        if !allowed {
            return Err(AppError::invalid_input("workflow transition not allowed"));
        }
        let mut updated = issue.clone();
        updated.status_id = cmd.target_status_id;
        updated.updated_at = shared::now();
        self.issues.save(&updated).await?;
        let project = self.projects.get_by_id(updated.project_id).await?;
        let status = statuses
            .iter()
            .find(|s| s.id == updated.status_id)
            .map(|s| s.name.as_ref().to_string())
            .unwrap_or_else(|| {
                board
                    .columns
                    .iter()
                    .find(|c| c.id == updated.status_id)
                    .map(|c| c.name.as_ref().to_string())
                    .unwrap_or_default()
            });
        let (assignee_name, reporter_name) =
            super::helpers::resolve_names(self.users.clone(), &updated).await;
        self.events.publish(shared::TrackerEvent::IssueMoved {
            issue_id: updated.id.to_string(),
            project_key: project.key.to_string(),
        });
        // Notify reporter of status change
        if updated.reporter_id != cmd.actor_id {
            let key = updated.key.to_string();
            self.create_notification(domain::Notification {
                id: shared::NotificationId::new(),
                recipient_id: updated.reporter_id,
                event_type: "issue_moved".into(),
                entity_type: "issue".into(),
                entity_id: Some(updated.id.as_uuid()),
                actor_id: Some(cmd.actor_id),
                title: format!("{} moved to {}", key, status).into(),
                body: None,
                is_read: false,
                read_at: None,
                action_url: Some(format!("/projects/{}/issues/{}", project.key, updated.id).into()),
                metadata: serde_json::json!({"issue_key": key, "status": status}),
                created_at: shared::now(),
            })
            .await;
        }
        Ok(IssueDto::from_issue(
            updated,
            project.name.as_ref().to_string(),
            status,
            assignee_name,
            reporter_name,
        ))
    }

    async fn get_by_id(&self, id: IssueId, requester: UserId) -> Result<IssueDto, AppError> {
        let issue = self.issues.get_by_id(id).await?;
        self.authz
            .require_project_access(issue.project_id, requester)
            .await?;
        let name = super::helpers::project_name(self.projects.clone(), issue.project_id).await?;
        Ok(super::helpers::build_issue_dto(self.users.clone(), issue, name.as_str()).await)
    }

    async fn update(
        &self,
        id: IssueId,
        cmd: UpdateIssueCommand,
        requester: UserId,
    ) -> Result<IssueDto, AppError> {
        let mut issue = self.issues.get_by_id(id).await?;
        self.authz
            .require_project_edit(issue.project_id, requester)
            .await?;
        let project = self.projects.get_by_id(issue.project_id).await?;

        if let Some(summary) = cmd.summary {
            if summary.trim().is_empty() || summary.chars().count() > 500 {
                return Err(AppError::invalid_input(
                    "summary must be between 1 and 500 characters",
                ));
            }
            issue.summary = summary.into();
            issue.updated_at = shared::now();
        }
        if let Some(description) = cmd.description {
            if description
                .as_deref()
                .is_some_and(|value| value.chars().count() > 100_000)
            {
                return Err(AppError::invalid_input(
                    "description must not exceed 100000 characters",
                ));
            }
            issue.description = description.map(domain::RichText::from);
            issue.updated_at = shared::now();
        }
        if let Some(priority) = cmd.priority {
            issue.priority = priority;
            issue.updated_at = shared::now();
        }
        if let Some(status_id) = cmd.status_id {
            let sid = status_id
                .parse()
                .map_err(|_| AppError::invalid_input("status_id"))?;
            let target = StatusId::from_uuid(sid);
            let allowed = self.transitions.is_allowed(issue.status_id, target).await?;
            if !allowed {
                return Err(AppError::invalid_input("workflow transition not allowed"));
            }
            let from_status = issue.status_id;
            let actor = cmd.actor_id;
            issue.change_status(target);
            self.status_history
                .save_for_project(
                    &domain::IssueStatusHistory {
                        id: shared::IssueStatusHistoryId::new(),
                        issue_id: issue.id,
                        from_status_id: Some(from_status),
                        to_status_id: target,
                        changed_by_id: actor,
                        changed_at: shared::now(),
                    },
                    issue.project_id,
                )
                .await?;
        }
        if let Some(assignee_id) = cmd.assignee_id {
            issue.assign(assignee_id);
        }
        // Cross-project references corrupt project-scoped reports/metadata:
        // every sprint/component/version must belong to the issue's project.
        if let Some(sprint_id) = cmd.sprint_id {
            if let Some(sid) = sprint_id {
                let sprint = self.sprints.get_by_id(sid).await?;
                if sprint.project_id != issue.project_id {
                    return Err(AppError::invalid_input(
                        "sprint belongs to a different project",
                    ));
                }
            }
            issue.sprint_id = sprint_id;
        }
        if let Some(component_id) = cmd.component_id {
            if let Some(cid) = component_id {
                let component = self.components.get_by_id(cid).await?;
                if component.project_id != issue.project_id {
                    return Err(AppError::invalid_input(
                        "component belongs to a different project",
                    ));
                }
            }
            issue.component_id = component_id;
            issue.updated_at = shared::now();
        }
        if let Some(affected_version_id) = cmd.affected_version_id {
            if let Some(vid) = affected_version_id {
                let version = self.versions.get_by_id(vid).await?;
                if version.project_id != issue.project_id {
                    return Err(AppError::invalid_input(
                        "version belongs to a different project",
                    ));
                }
            }
            issue.affected_version_id = affected_version_id;
            issue.updated_at = shared::now();
        }
        if let Some(fix_version_id) = cmd.fix_version_id {
            if let Some(vid) = fix_version_id {
                let version = self.versions.get_by_id(vid).await?;
                if version.project_id != issue.project_id {
                    return Err(AppError::invalid_input(
                        "version belongs to a different project",
                    ));
                }
            }
            issue.fix_version_id = fix_version_id;
            issue.updated_at = shared::now();
        }

        self.issues.save(&issue).await?;
        let statuses = self.statuses.list_all().await.unwrap_or_default();
        let column = statuses
            .iter()
            .find(|s| s.id == issue.status_id)
            .map(|s| s.name.as_ref().to_string())
            .unwrap_or_else(|| super::helpers::issue_status_column(issue.status_id));
        let (assignee_name, reporter_name) =
            super::helpers::resolve_names(self.users.clone(), &issue).await;
        self.events.publish(shared::TrackerEvent::IssueUpdated {
            issue_id: issue.id.to_string(),
            project_key: project.key.to_string(),
        });
        // Notify assignee if assignment changed
        if let Some(new_assignee) = cmd.assignee_id.flatten() {
            if new_assignee != issue.reporter_id {
                let key = issue.key.to_string();
                self.create_notification(domain::Notification {
                    id: shared::NotificationId::new(),
                    recipient_id: new_assignee,
                    event_type: "issue_assigned".into(),
                    entity_type: "issue".into(),
                    entity_id: Some(issue.id.as_uuid()),
                    actor_id: Some(cmd.actor_id),
                    title: format!("You were assigned to {}", key).into(),
                    body: Some(issue.summary.as_ref().to_string().into()),
                    is_read: false,
                    read_at: None,
                    action_url: Some(
                        format!("/projects/{}/issues/{}", project.key, issue.id).into(),
                    ),
                    metadata: serde_json::json!({"issue_key": key}),
                    created_at: shared::now(),
                })
                .await;
            }
        }
        Ok(IssueDto::from_issue(
            issue,
            project.name.as_ref().to_string(),
            column,
            assignee_name,
            reporter_name,
        ))
    }

    async fn search(
        &self,
        filters: crate::context::SearchFilters,
        requester: UserId,
    ) -> Result<Vec<IssueDto>, AppError> {
        let mut query = IssueQuery::default();
        // Search is a list endpoint: keep responses bounded and reject a
        // zero/oversized page instead of silently loading every issue.
        if let Some(limit) = filters.limit {
            if !(1..=100).contains(&limit) {
                return Err(AppError::invalid_input("limit must be between 1 and 100"));
            }
            query.limit = limit;
        } else {
            query.limit = 50;
        }
        query.offset = filters.offset.unwrap_or(0);
        if let Some(q) = filters.q.as_deref().filter(|s| !s.is_empty()) {
            query.search_text = Some(q.to_string());
        }
        if let Some(priority) = filters.priority.as_deref().filter(|s| !s.is_empty()) {
            // DB stores canonical Title-Case values; accept any casing.
            let canonical = ["lowest", "low", "medium", "high", "highest"]
                .iter()
                .find(|p| p.eq_ignore_ascii_case(priority))
                .map(|p| {
                    let mut c = p.chars();
                    match c.next() {
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        None => String::new(),
                    }
                });
            match canonical {
                Some(p) => query.priority = Some(p),
                None => return Ok(Vec::new()),
            }
        }
        if let Some(sort_by) = filters.sort_by.as_deref() {
            query.sort_by = Some(sort_by.to_string());
            query.sort_order = filters.sort_order.clone();
        }
        if let Some(project_key) = filters.project_key.as_deref().filter(|s| !s.is_empty()) {
            let key: ProjectKey = project_key
                .parse()
                .map_err(|e: String| AppError::invalid_input(e))?;
            let project = self.projects.get_by_key(&key).await?;
            self.authz
                .require_project_access(project.id, requester)
                .await?;
            query.project_id = Some(project.id);
        } else {
            // Cross-project search must never leak issues from projects the
            // requester does not own or hold membership in.
            query.accessible_project_ids =
                Some(self.authz.accessible_project_ids(requester).await?);
        }
        if let Some(assignee_id) = filters.assignee_id.as_deref().filter(|s| !s.is_empty()) {
            let uuid = uuid::Uuid::parse_str(assignee_id)
                .map_err(|e| AppError::invalid_input(e.to_string()))?;
            query.assignee_id = Some(UserId::from_uuid(uuid));
        }
        let issues = self.issues.list(query).await?;
        super::helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            issues,
        )
        .await
    }

    async fn delete(&self, id: IssueId, actor_id: UserId) -> Result<(), AppError> {
        let issue = self.issues.get_by_id(id).await?;
        self.authz
            .require_project_edit(issue.project_id, actor_id)
            .await?;
        self.issues.delete(id).await
    }

    async fn restore(&self, id: IssueId, actor_id: UserId) -> Result<IssueDto, AppError> {
        let issue = self.issues.get_by_id_include_deleted(id).await?;
        self.authz
            .require_project_edit(issue.project_id, actor_id)
            .await?;
        self.issues.restore(id).await?;
        let issue = self.issues.get_by_id(id).await?;
        super::helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            vec![issue],
        )
        .await
        .map(|mut v| v.remove(0))
    }

    async fn purge(&self, id: IssueId, actor_id: UserId) -> Result<(), AppError> {
        let issue = self.issues.get_by_id_include_deleted(id).await?;
        self.authz
            .require_project_edit(issue.project_id, actor_id)
            .await?;
        self.issues.purge(id).await
    }

    async fn list_trash(
        &self,
        project_key: &ProjectKey,
        requester: UserId,
    ) -> Result<Vec<IssueDto>, AppError> {
        let project = self
            .projects
            .get_by_key(project_key)
            .await
            .map_err(|_| AppError::not_found("project", project_key))?;
        self.authz
            .require_project_access(project.id, requester)
            .await?;
        let query = IssueQuery {
            project_id: Some(project.id),
            deleted_only: true,
            ..Default::default()
        };
        let issues = self.issues.list(query).await?;
        super::helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            issues,
        )
        .await
    }
}
