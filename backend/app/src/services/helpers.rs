use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use domain::{BoardColumn, Issue, LabelRepository, ProjectRepository, StatusCategory};
use shared::{AppError, ProjectId, StatusId, UserId};

pub async fn resolve_names(
    users: Arc<dyn domain::UserRepository>,
    issue: &Issue,
) -> Result<(Option<String>, Option<String>), AppError> {
    let assignee_name = if let Some(id) = issue.assignee_id {
        Some(
            users
                .get_by_id(id)
                .await
                .map(|u| u.display_name.as_ref().to_string())?,
        )
    } else {
        None
    };
    let reporter_name = Some(
        users
            .get_by_id(issue.reporter_id)
            .await
            .map(|u| u.display_name.as_ref().to_string())?,
    );
    Ok((assignee_name, reporter_name))
}

pub fn issue_status_column(status_id: StatusId) -> String {
    default_board_columns()
        .into_iter()
        .find(|c| c.id == status_id)
        .map(|c| c.name.as_ref().to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

pub async fn project_name(
    projects: Arc<dyn ProjectRepository>,
    project_id: ProjectId,
) -> Result<String, AppError> {
    projects
        .get_by_id(project_id)
        .await
        .map(|p| p.name.as_ref().to_string())
}

pub async fn issue_notification_recipients(
    watchers: &Arc<dyn domain::WatcherRepository>,
    issue: &Issue,
) -> Vec<UserId> {
    let mut recipients = vec![issue.reporter_id];
    if let Some(assignee_id) = issue.assignee_id {
        recipients.push(assignee_id);
    }
    if let Ok(watchers) = watchers.list_by_issue(issue.id).await {
        recipients.extend(watchers.into_iter().map(|watcher| watcher.user_id));
    }
    recipients
}

async fn create_notification_if_allowed(
    notifications: &Arc<dyn domain::NotificationRepository>,
    notification_settings: &Arc<dyn domain::UserNotificationSettingsRepository>,
    events: &crate::context::EventBus,
    notification: domain::Notification,
) {
    let recipient_id = notification.recipient_id;
    let event_type = notification.event_type.as_ref();
    let actor_is_recipient = notification.actor_id == Some(recipient_id);
    let allowed = match notification_settings.get_settings(recipient_id).await {
        Ok(settings) => {
            (settings.notify_own_changes || !actor_is_recipient)
                && !settings
                    .disabled_event_types
                    .iter()
                    .any(|value| value.as_ref() == event_type)
        }
        // Missing settings preserve the existing default delivery behavior.
        Err(AppError::NotFound(_)) => !actor_is_recipient,
        Err(_) => return,
    };
    if allowed && notifications.save(&notification).await.is_ok() {
        events.publish(shared::TrackerEvent::NotificationCreated {
            recipient_id: recipient_id.to_string(),
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn notify_recipients(
    notifications: &Arc<dyn domain::NotificationRepository>,
    notification_settings: &Arc<dyn domain::UserNotificationSettingsRepository>,
    events: &crate::context::EventBus,
    recipients: Vec<UserId>,
    issue: &Issue,
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
        create_notification_if_allowed(
            notifications,
            notification_settings,
            events,
            domain::Notification {
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
            },
        )
        .await;
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn notify_issue_recipients(
    watchers: &Arc<dyn domain::WatcherRepository>,
    notifications: &Arc<dyn domain::NotificationRepository>,
    notification_settings: &Arc<dyn domain::UserNotificationSettingsRepository>,
    events: &crate::context::EventBus,
    issue: &Issue,
    actor_id: UserId,
    event_type: &str,
    title: String,
    body: Option<String>,
    metadata: serde_json::Value,
) {
    notify_recipients(
        notifications,
        notification_settings,
        events,
        issue_notification_recipients(watchers, issue).await,
        issue,
        actor_id,
        event_type,
        title,
        body,
        metadata,
    )
    .await;
}

pub fn build_issue_dto_from_lookups(
    issue: Issue,
    project_names: &HashMap<ProjectId, String>,
    user_names: &HashMap<shared::UserId, String>,
    label_names: &HashMap<shared::IssueId, Vec<String>>,
) -> crate::dto::IssueDto {
    let issue_id = issue.id;
    let status_id = issue.status_id;
    let assignee_name = issue
        .assignee_id
        .and_then(|id| user_names.get(&id).cloned());
    let reporter_name = user_names.get(&issue.reporter_id).cloned();
    let project_name = project_names
        .get(&issue.project_id)
        .cloned()
        .unwrap_or_default();
    let mut dto = crate::dto::IssueDto::from_issue(
        issue,
        project_name,
        issue_status_column(status_id),
        assignee_name,
        reporter_name,
    );
    if let Some(labels) = label_names.get(&issue_id) {
        dto.labels = labels.clone();
    }
    dto
}

pub async fn build_issue_dto(
    users: Arc<dyn domain::UserRepository>,
    labels: Arc<dyn LabelRepository>,
    issue: Issue,
    project_name: &str,
) -> Result<crate::dto::IssueDto, AppError> {
    let label_names = issue_label_name_lookup(labels, &[issue.id]).await?;
    let (assignee_name, reporter_name) = resolve_names(users, &issue).await?;
    let project_names = HashMap::from([(issue.project_id, project_name.to_string())]);
    let user_names = [
        issue.assignee_id.map(|id| (id, assignee_name.clone())),
        Some((issue.reporter_id, reporter_name.clone())),
    ]
    .into_iter()
    .flatten()
    .filter_map(|(id, name)| name.map(|name| (id, name)))
    .collect::<HashMap<_, _>>();
    Ok(build_issue_dto_from_lookups(
        issue,
        &project_names,
        &user_names,
        &label_names,
    ))
}

async fn issue_user_name_lookup(
    users: Arc<dyn domain::UserRepository>,
) -> Result<HashMap<shared::UserId, String>, AppError> {
    Ok(users
        .list()
        .await?
        .into_iter()
        .map(|user| (user.id, user.display_name.to_string()))
        .collect())
}

async fn issue_label_name_lookup(
    labels: Arc<dyn LabelRepository>,
    issue_ids: &[shared::IssueId],
) -> Result<HashMap<shared::IssueId, Vec<String>>, AppError> {
    Ok(labels
        .list_by_issues(issue_ids)
        .await?
        .into_iter()
        .map(|(issue_id, labels)| {
            (
                issue_id,
                labels
                    .into_iter()
                    .map(|label| label.name.as_ref().to_string())
                    .collect(),
            )
        })
        .collect())
}

pub async fn build_issue_dtos(
    users: Arc<dyn domain::UserRepository>,
    labels: Arc<dyn LabelRepository>,
    issues: Vec<Issue>,
    project_name: &str,
) -> Result<Vec<crate::dto::IssueDto>, AppError> {
    let user_names = issue_user_name_lookup(users).await?;
    let issue_ids = issues.iter().map(|issue| issue.id).collect::<Vec<_>>();
    let label_names = issue_label_name_lookup(labels, &issue_ids).await?;
    let project_names = issues
        .iter()
        .map(|issue| (issue.project_id, project_name.to_string()))
        .collect::<HashMap<_, _>>();
    Ok(issues
        .into_iter()
        .map(|issue| build_issue_dto_from_lookups(issue, &project_names, &user_names, &label_names))
        .collect())
}

async fn build_issue_dtos_prefetched(
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
    labels: Arc<dyn LabelRepository>,
    issues: Vec<Issue>,
) -> Result<Vec<crate::dto::IssueDto>, AppError> {
    let project_names = projects
        .list(domain::ProjectQuery::default())
        .await?
        .into_iter()
        .map(|project| (project.id, project.name.to_string()))
        .collect::<HashMap<_, _>>();
    let user_names = issue_user_name_lookup(users).await?;
    if let Some(missing) = issues
        .iter()
        .find(|issue| !project_names.contains_key(&issue.project_id))
    {
        return Err(AppError::not_found("project", missing.project_id));
    }
    let issue_ids = issues.iter().map(|issue| issue.id).collect::<Vec<_>>();
    let label_names = issue_label_name_lookup(labels, &issue_ids).await?;
    Ok(issues
        .into_iter()
        .map(|issue| build_issue_dto_from_lookups(issue, &project_names, &user_names, &label_names))
        .collect())
}

pub async fn build_issue_dtos_with_projects(
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
    labels: Arc<dyn LabelRepository>,
    issues: Vec<Issue>,
) -> Result<Vec<crate::dto::IssueDto>, AppError> {
    build_issue_dtos_prefetched(projects, users, labels, issues).await
}

pub async fn build_issue_dtos_for_dashboard(
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
    labels: Arc<dyn LabelRepository>,
    issues: Vec<Issue>,
) -> Result<Vec<crate::dto::IssueDto>, AppError> {
    build_issue_dtos_prefetched(projects, users, labels, issues).await
}

fn todo_status() -> StatusId {
    StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap())
}
fn in_progress_status() -> StatusId {
    StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap())
}
fn review_status() -> StatusId {
    StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000004").unwrap())
}
fn done_status() -> StatusId {
    StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap())
}

pub fn default_board_columns() -> Vec<BoardColumn> {
    vec![
        BoardColumn {
            id: todo_status(),
            name: "To Do".into(),
            category: StatusCategory::Todo,
            wip_limit: None,
            position: 0,
        },
        BoardColumn {
            id: in_progress_status(),
            name: "In Progress".into(),
            category: StatusCategory::InProgress,
            wip_limit: Some(5),
            position: 1,
        },
        BoardColumn {
            id: review_status(),
            name: "Review".into(),
            category: StatusCategory::InProgress,
            wip_limit: None,
            position: 2,
        },
        BoardColumn {
            id: done_status(),
            name: "Done".into(),
            category: StatusCategory::Done,
            wip_limit: None,
            position: 3,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn issue_dto_uses_prefetched_user_and_project_names() {
        let reporter_id = shared::UserId::new();
        let project = domain::Project {
            id: ProjectId::new(),
            key: shared::ProjectKey::new("TT"),
            name: "Project TT".into(),
            description: None,
            owner_id: reporter_id,
            default_board_id: shared::BoardId::new(),
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        let issue = Issue::create(
            &project,
            1,
            shared::IssueType::Task,
            todo_status(),
            "Prefetch check",
            None,
            reporter_id,
            shared::Priority::Medium,
        );
        let mut projects = HashMap::new();
        projects.insert(issue.project_id, "Project TT".to_string());
        let mut users = HashMap::new();
        users.insert(issue.reporter_id, "Reporter".to_string());

        let labels = HashMap::new();
        let dto = build_issue_dto_from_lookups(issue, &projects, &users, &labels);
        assert_eq!(dto.project_name, "Project TT");
        assert_eq!(dto.reporter_name.as_deref(), Some("Reporter"));
        assert_eq!(dto.status, "To Do");
    }
}
