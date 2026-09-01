use async_trait::async_trait;
use std::{collections::HashSet, sync::Arc};

use crate::authz::Authz;
use crate::commands::{CreateIssueCommand, UpdateIssueCommand};
use crate::dto::IssueDto;
use domain::{
    AttachmentRepository, BoardRepository, FileStorage, Issue, IssueQuery, IssueRepository,
    ProjectRepository, StatusRepository, WorkflowTransitionRepository,
};
use shared::{AppError, IssueId, ProjectKey, StatusId, UserId};

pub struct IssueServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    boards: Arc<dyn BoardRepository>,
    users: Arc<dyn domain::UserRepository>,
    statuses: Arc<dyn StatusRepository>,
    transitions: Arc<dyn WorkflowTransitionRepository>,
    sprints: Arc<dyn domain::SprintRepository>,
    components: Arc<dyn domain::ProjectComponentRepository>,
    versions: Arc<dyn domain::ProjectVersionRepository>,
    custom_fields: Arc<dyn domain::CustomFieldRepository>,
    labels: Arc<dyn domain::LabelRepository>,
    attachments: Arc<dyn AttachmentRepository>,
    storage: Arc<dyn FileStorage>,
    watchers: Arc<dyn domain::WatcherRepository>,
    events: crate::context::EventBus,
    notifications: Arc<dyn domain::NotificationRepository>,
    notification_settings: Arc<dyn domain::UserNotificationSettingsRepository>,
    authz: Authz,
}

impl IssueServiceImpl {
    /// WIP capacity snapshot for any status-changing path. The count is
    /// re-validated inside `change_status_atomic`; this only carries the
    /// limit and column name into the critical section.
    async fn build_wip_guard(
        &self,
        project_id: domain::ProjectId,
        target: shared::StatusId,
    ) -> Result<domain::TransitionGuard, AppError> {
        let board = self.boards.get_default_by_project(project_id).await?;
        let column = board
            .columns
            .iter()
            .find(|c| c.id == target)
            .ok_or_else(|| AppError::invalid_input("unknown target column"))?;
        let target_count = self
            .issues
            .count_by_project_status(project_id, target)
            .await?;
        Ok(domain::TransitionGuard {
            wip_limit: column.wip_limit.map(|v| v as u32),
            target_count,
            column_name: column.name.as_ref().to_string(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        boards: Arc<dyn BoardRepository>,
        users: Arc<dyn domain::UserRepository>,
        statuses: Arc<dyn StatusRepository>,
        transitions: Arc<dyn WorkflowTransitionRepository>,

        sprints: Arc<dyn domain::SprintRepository>,
        components: Arc<dyn domain::ProjectComponentRepository>,
        versions: Arc<dyn domain::ProjectVersionRepository>,
        custom_fields: Arc<dyn domain::CustomFieldRepository>,
        labels: Arc<dyn domain::LabelRepository>,
        attachments: Arc<dyn AttachmentRepository>,
        storage: Arc<dyn FileStorage>,
        watchers: Arc<dyn domain::WatcherRepository>,
        events: crate::context::EventBus,
        notifications: Arc<dyn domain::NotificationRepository>,
        notification_settings: Arc<dyn domain::UserNotificationSettingsRepository>,
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
            sprints,
            components,
            versions,
            custom_fields,
            labels,
            attachments,
            storage,
            watchers,
            notifications,
            notification_settings,
            authz,
        }
    }

    /// Create a notification and publish a real-time SSE event.
    async fn create_notification(&self, notification: domain::Notification) {
        let recipient_id = notification.recipient_id;
        let event_type = notification.event_type.as_ref();
        let actor_is_recipient = notification.actor_id == Some(recipient_id);
        let allowed = match self.notification_settings.get_settings(recipient_id).await {
            Ok(settings) => {
                (settings.notify_own_changes || !actor_is_recipient)
                    && !settings
                        .disabled_event_types
                        .iter()
                        .any(|value| value.as_ref() == event_type)
            }
            // Missing settings preserve the existing default delivery behavior.
            Err(shared::AppError::NotFound(_)) => !actor_is_recipient,
            Err(_) => return,
        };
        if allowed && self.notifications.save(&notification).await.is_ok() {
            self.events
                .publish(shared::TrackerEvent::NotificationCreated {
                    recipient_id: recipient_id.to_string(),
                });
        }
    }

    async fn issue_recipients(&self, issue: &Issue) -> Vec<UserId> {
        let mut recipients = vec![issue.reporter_id];
        if let Some(assignee_id) = issue.assignee_id {
            recipients.push(assignee_id);
        }
        if let Ok(watchers) = self.watchers.list_by_issue(issue.id).await {
            recipients.extend(watchers.into_iter().map(|watcher| watcher.user_id));
        }
        recipients
    }

    #[allow(clippy::too_many_arguments)]
    async fn notify_issue_recipients(
        &self,
        recipients: Vec<UserId>,
        issue: &Issue,
        _project: &domain::Project,
        actor_id: UserId,
        event_type: &str,
        title: String,
        body: Option<String>,
        metadata: serde_json::Value,
    ) {
        let mut seen = HashSet::new();
        let action_url = format!("/issues/{}", issue.id);
        for recipient_id in recipients {
            if !seen.insert(recipient_id) {
                continue;
            }
            self.create_notification(domain::Notification {
                id: shared::NotificationId::new(),
                recipient_id,
                event_type: event_type.into(),
                entity_type: "issue".into(),
                entity_id: Some(issue.id.as_uuid()),
                actor_id: Some(actor_id),
                title: title.clone().into(),
                body: body.clone().map(Into::into),
                is_read: false,
                read_at: None,
                action_url: Some(action_url.clone().into()),
                metadata: metadata.clone(),
                created_at: shared::now(),
            })
            .await;
        }
    }

    async fn require_active_user(&self, user_id: UserId, field: &str) -> Result<(), AppError> {
        let user = self
            .users
            .get_by_id(user_id)
            .await
            .map_err(|error| match error {
                AppError::NotFound(_) => AppError::invalid_input(field),
                error => error,
            })?;
        if user.is_active {
            Ok(())
        } else {
            Err(AppError::invalid_input(field))
        }
    }

    async fn require_project_user(
        &self,
        project_id: shared::ProjectId,
        user_id: UserId,
        field: &str,
    ) -> Result<(), AppError> {
        self.require_active_user(user_id, field).await?;
        self.authz.require_project_access(project_id, user_id).await
    }

    async fn normalize_custom_fields_for_create(
        &self,
        project_id: shared::ProjectId,
        values: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Vec<(shared::CustomFieldId, serde_json::Value)>, AppError> {
        let fields = self.custom_fields.list_by_project(project_id).await?;
        let mut provided = HashSet::new();
        let mut normalized_values = Vec::new();

        for (raw_field_id, value) in values {
            let field_id = raw_field_id
                .parse::<shared::CustomFieldId>()
                .map_err(|_| AppError::invalid_input("custom_fields key"))?;
            let field = fields
                .iter()
                .find(|field| field.id == field_id)
                .ok_or_else(|| AppError::invalid_input("custom field"))?;
            if super::custom_field::is_empty_custom_field_value(value) {
                if field.is_required {
                    return Err(AppError::validation(format!(
                        "required custom field {} cannot be empty",
                        field.name.as_ref()
                    )));
                }
                continue;
            }
            let normalized = super::custom_field::normalize_custom_field_value(field, value)?;
            provided.insert(field_id);
            normalized_values.push((field_id, normalized));
        }

        for field in fields.iter().filter(|field| field.is_required) {
            if !provided.contains(&field.id) {
                return Err(AppError::validation(format!(
                    "required custom field {} is missing",
                    field.name.as_ref()
                )));
            }
        }

        Ok(normalized_values)
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
        match self.statuses.get_by_id(status_id).await {
            Ok(_) => {}
            Err(AppError::NotFound(_)) => return Err(AppError::invalid_input("status_id")),
            Err(err) => return Err(err),
        }
        self.require_project_user(project.id, cmd.reporter_id, "reporter_id")
            .await?;
        if let Some(assignee_id) = cmd.assignee_id {
            self.require_project_user(project.id, assignee_id, "assignee_id")
                .await?;
        }
        let custom_field_values = self
            .normalize_custom_fields_for_create(project.id, &cmd.custom_fields)
            .await?;
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
            let initial_status_history = domain::IssueStatusHistory {
                id: shared::IssueStatusHistoryId::new(),
                issue_id: candidate.id,
                from_status_id: None,
                to_status_id: status_id,
                changed_by_id: cmd.actor_id,
                changed_at: candidate.created_at,
            };
            match self
                .issues
                .create_with_initial_data(&candidate, &initial_status_history, &custom_field_values)
                .await
            {
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
        self.events.publish(shared::TrackerEvent::IssueCreated {
            issue_id: issue.id.to_string(),
            project_key: project.key.to_string(),
        });
        if let Some(assignee_id) = issue.assignee_id {
            let key = issue.key.to_string();
            self.notify_issue_recipients(
                vec![assignee_id],
                &issue,
                &project,
                cmd.actor_id,
                "issue_assigned",
                format!("You were assigned to {}", key),
                Some(issue.summary.as_ref().to_string()),
                serde_json::json!({"issue_key": key}),
            )
            .await;
        }
        super::helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            Arc::clone(&self.labels),
            vec![issue],
        )
        .await
        .map(|mut issues| issues.remove(0))
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
        let statuses = self.statuses.list_all().await?;
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
        // WIP is a domain invariant: enforce it on every status-changing
        // path, not only on board drag-and-drop.
        let guard = self
            .build_wip_guard(issue.project_id, cmd.target_status_id)
            .await?;
        self.issues
            .change_status_atomic(
                issue.id,
                issue.project_id,
                issue.status_id,
                cmd.target_status_id,
                cmd.actor_id,
                &guard,
            )
            .await?;
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
        self.events.publish(shared::TrackerEvent::IssueMoved {
            issue_id: updated.id.to_string(),
            project_key: project.key.to_string(),
        });
        let key = updated.key.to_string();
        self.notify_issue_recipients(
            self.issue_recipients(&updated).await,
            &updated,
            &project,
            cmd.actor_id,
            "issue_moved",
            format!("{} moved to {}", key, status),
            None,
            serde_json::json!({"issue_key": key, "status": status}),
        )
        .await;
        super::helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            Arc::clone(&self.labels),
            vec![updated],
        )
        .await
        .map(|mut issues| issues.remove(0))
    }

    async fn get_by_id(&self, id: IssueId, requester: UserId) -> Result<IssueDto, AppError> {
        let issue = self.issues.get_by_id(id).await?;
        self.authz
            .require_project_access(issue.project_id, requester)
            .await?;
        let name = super::helpers::project_name(self.projects.clone(), issue.project_id).await?;
        super::helpers::build_issue_dto(
            self.users.clone(),
            self.labels.clone(),
            issue,
            name.as_str(),
        )
        .await
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

        let status_change = if let Some(status_id) = cmd.status_id.as_deref() {
            let sid = status_id
                .parse()
                .map_err(|_| AppError::invalid_input("status_id"))?;
            let target = StatusId::from_uuid(sid);
            let allowed = self.transitions.is_allowed(issue.status_id, target).await?;
            if !allowed {
                return Err(AppError::invalid_input("workflow transition not allowed"));
            }
            let from_status = issue.status_id;
            let guard = self.build_wip_guard(issue.project_id, target).await?;
            Some((from_status, target, guard))
        } else {
            None
        };

        if let Some(Some(assignee_id)) = cmd.assignee_id {
            self.require_project_user(issue.project_id, assignee_id, "assignee_id")
                .await?;
        }
        // Cross-project references corrupt project-scoped reports/metadata:
        // every sprint/component/version must belong to the issue's project.
        if let Some(Some(sid)) = cmd.sprint_id {
            let sprint = self.sprints.get_by_id(sid).await?;
            if sprint.project_id != issue.project_id {
                return Err(AppError::invalid_input(
                    "sprint belongs to a different project",
                ));
            }
        }
        if let Some(Some(cid)) = cmd.component_id {
            let component = self.components.get_by_id(cid).await?;
            if component.project_id != issue.project_id {
                return Err(AppError::invalid_input(
                    "component belongs to a different project",
                ));
            }
        }
        if let Some(Some(vid)) = cmd.affected_version_id {
            let version = self.versions.get_by_id(vid).await?;
            if version.project_id != issue.project_id {
                return Err(AppError::invalid_input(
                    "version belongs to a different project",
                ));
            }
        }
        if let Some(Some(vid)) = cmd.fix_version_id {
            let version = self.versions.get_by_id(vid).await?;
            if version.project_id != issue.project_id {
                return Err(AppError::invalid_input(
                    "version belongs to a different project",
                ));
            }
        }

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
        if let Some((from_status, target, guard)) = status_change {
            self.issues
                .change_status_atomic(
                    issue.id,
                    issue.project_id,
                    from_status,
                    target,
                    cmd.actor_id,
                    &guard,
                )
                .await?;
            issue.change_status(target);
        }
        if let Some(assignee_id) = cmd.assignee_id {
            issue.assign(assignee_id);
        }
        if let Some(sprint_id) = cmd.sprint_id {
            issue.sprint_id = sprint_id;
            issue.updated_at = shared::now();
        }
        if let Some(component_id) = cmd.component_id {
            issue.component_id = component_id;
            issue.updated_at = shared::now();
        }
        if let Some(affected_version_id) = cmd.affected_version_id {
            issue.affected_version_id = affected_version_id;
            issue.updated_at = shared::now();
        }
        if let Some(fix_version_id) = cmd.fix_version_id {
            issue.fix_version_id = fix_version_id;
            issue.updated_at = shared::now();
        }

        self.issues.save(&issue).await?;
        self.events.publish(shared::TrackerEvent::IssueUpdated {
            issue_id: issue.id.to_string(),
            project_key: project.key.to_string(),
        });
        let key = issue.key.to_string();
        let assigned_recipient = cmd.assignee_id.flatten();
        let update_recipients = self
            .issue_recipients(&issue)
            .await
            .into_iter()
            .filter(|recipient_id| Some(*recipient_id) != assigned_recipient)
            .collect();
        self.notify_issue_recipients(
            update_recipients,
            &issue,
            &project,
            cmd.actor_id,
            "issue_updated",
            format!("{} updated", key),
            Some(issue.summary.as_ref().to_string()),
            serde_json::json!({"issue_key": key}),
        )
        .await;
        if let Some(new_assignee) = assigned_recipient {
            self.notify_issue_recipients(
                vec![new_assignee],
                &issue,
                &project,
                cmd.actor_id,
                "issue_assigned",
                format!("You were assigned to {}", key),
                Some(issue.summary.as_ref().to_string()),
                serde_json::json!({"issue_key": key}),
            )
            .await;
        }
        super::helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            Arc::clone(&self.labels),
            vec![issue],
        )
        .await
        .map(|mut issues| issues.remove(0))
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
        // Keep `/api/v1/issues` aligned with `/api/v1/search`: list-style
        // search defaults to deterministic recency ordering so the first page
        // contains the newest matching issues.
        query.sort_by = Some(
            filters
                .sort_by
                .clone()
                .unwrap_or_else(|| "created".to_string()),
        );
        query.sort_order = Some(
            filters
                .sort_order
                .clone()
                .unwrap_or_else(|| "desc".to_string()),
        );
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
        if let Some(status) = filters.status.as_deref().filter(|s| !s.is_empty()) {
            // The UI filters by status name; issues store status ids.
            // Status names are human-cased ("To Do"); URL filters use
            // snake_case ("to_do") — normalize both sides.
            let norm = |v: &str| {
                v.replace('_', " ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join("")
                    .to_lowercase()
            };
            let wanted = norm(status);
            let all = self.statuses.list_all().await?;
            let matched = all.iter().find(|s| norm(s.name.as_ref()) == wanted);
            match matched {
                Some(s) => query.status_id = Some(s.id),
                None => return Ok(Vec::new()),
            }
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
        if let Some(jql_str) = filters.jql.as_deref().filter(|s| !s.is_empty()) {
            let expr =
                domain::jql::parse(jql_str).map_err(|e| AppError::invalid_input(e.to_string()))?;
            query.jql = Some(expr);
            if let Some(uid_str) = filters.user_id.as_deref().filter(|s| !s.is_empty()) {
                let uuid = uuid::Uuid::parse_str(uid_str)
                    .map_err(|e| AppError::invalid_input(e.to_string()))?;
                query.jql_user_id = Some(UserId::from_uuid(uuid));
            }
        }
        let issues = self.issues.list(query).await?;
        super::helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            Arc::clone(&self.labels),
            issues,
        )
        .await
    }

    async fn delete(&self, id: IssueId, actor_id: UserId) -> Result<(), AppError> {
        let issue = self.issues.get_by_id(id).await?;
        self.authz
            .require_project_edit(issue.project_id, actor_id)
            .await?;
        self.issues.delete(id).await?;
        let project = self.projects.get_by_id(issue.project_id).await?;
        self.events.publish(shared::TrackerEvent::IssueDeleted {
            issue_id: id.to_string(),
            project_key: project.key.to_string(),
        });
        Ok(())
    }

    async fn restore(&self, id: IssueId, actor_id: UserId) -> Result<IssueDto, AppError> {
        let issue = self.issues.get_by_id_include_deleted(id).await?;
        self.authz
            .require_project_edit(issue.project_id, actor_id)
            .await?;
        self.issues.restore(id).await?;
        let issue = self.issues.get_by_id(id).await?;
        let project = self.projects.get_by_id(issue.project_id).await?;
        self.events.publish(shared::TrackerEvent::IssueUpdated {
            issue_id: id.to_string(),
            project_key: project.key.to_string(),
        });
        super::helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            Arc::clone(&self.labels),
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
        let attachment_keys = self
            .attachments
            .list_by_issue(id)
            .await?
            .into_iter()
            .map(|attachment| attachment.storage_key.as_ref().to_string())
            .collect::<Vec<_>>();
        self.issues.purge(id).await?;
        for storage_key in attachment_keys {
            if let Err(error) = self.storage.delete(&id.to_string(), &storage_key).await {
                tracing::warn!(
                    issue_id = %id,
                    storage_key = %storage_key,
                    error = %error,
                    "failed to delete purged issue attachment"
                );
            }
        }
        Ok(())
    }

    async fn list_trash(
        &self,
        project_key: &ProjectKey,
        requester: UserId,
        offset: u32,
        limit: u32,
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
            offset: offset as u64,
            limit: limit.clamp(1, 100) as u64,
            ..Default::default()
        };
        let issues = self.issues.list(query).await?;
        super::helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            Arc::clone(&self.labels),
            issues,
        )
        .await
    }
}
