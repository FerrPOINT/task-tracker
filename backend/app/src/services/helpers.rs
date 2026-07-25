use std::sync::Arc;

use domain::{BoardColumn, ColumnCategory, Issue, ProjectRepository};
use shared::{AppError, ProjectId, StatusId};

pub async fn resolve_names(
    users: Arc<dyn domain::UserRepository>,
    issue: &Issue,
) -> (Option<String>, Option<String>) {
    let assignee_name = if let Some(id) = issue.assignee_id {
        users
            .get_by_id(id)
            .await
            .map(|u| u.display_name.as_ref().to_string())
            .ok()
    } else {
        None
    };
    let reporter_name = users
        .get_by_id(issue.reporter_id)
        .await
        .map(|u| u.display_name.as_ref().to_string())
        .ok();
    (assignee_name, reporter_name)
}

pub fn issue_status_column(status_id: StatusId) -> String {
    default_board_columns()
        .into_iter()
        .find(|c| c.id == status_id)
        .map(|c| c.name.as_ref().to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

pub fn count_by_status(issues: &[Issue]) -> (i64, i64, i64) {
    let todo = issues
        .iter()
        .filter(|i| i.status_id == todo_status())
        .count() as i64;
    let in_progress = issues
        .iter()
        .filter(|i| i.status_id == in_progress_status() || i.status_id == review_status())
        .count() as i64;
    let done = issues
        .iter()
        .filter(|i| i.status_id == done_status())
        .count() as i64;
    (todo, in_progress, done)
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

pub async fn build_issue_dto(
    users: Arc<dyn domain::UserRepository>,
    issue: Issue,
    project_name: &str,
) -> crate::dto::IssueDto {
    let status_id = issue.status_id;
    let (assignee_name, reporter_name) = resolve_names(users, &issue).await;
    crate::dto::IssueDto::from_issue(
        issue,
        project_name.to_string(),
        issue_status_column(status_id),
        assignee_name,
        reporter_name,
    )
}

pub async fn build_issue_dtos(
    users: Arc<dyn domain::UserRepository>,
    issues: Vec<Issue>,
    project_name: &str,
) -> Result<Vec<crate::dto::IssueDto>, AppError> {
    let mut dtos = Vec::new();
    for issue in issues {
        dtos.push(build_issue_dto(Arc::clone(&users), issue, project_name).await);
    }
    Ok(dtos)
}

pub async fn build_issue_dtos_with_projects(
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
    issues: Vec<Issue>,
) -> Result<Vec<crate::dto::IssueDto>, AppError> {
    let mut dtos = Vec::new();
    for issue in issues {
        let name = project_name(Arc::clone(&projects), issue.project_id).await?;
        dtos.push(build_issue_dto(Arc::clone(&users), issue, name.as_str()).await);
    }
    Ok(dtos)
}

pub async fn build_issue_dtos_for_dashboard(
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
    issues: Vec<Issue>,
) -> Result<Vec<crate::dto::IssueDto>, AppError> {
    let mut dtos = Vec::new();
    for issue in issues {
        let name = project_name(Arc::clone(&projects), issue.project_id).await?;
        dtos.push(build_issue_dto(Arc::clone(&users), issue, name.as_str()).await);
    }
    Ok(dtos)
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
            name: "Todo".into(),
            category: ColumnCategory::Todo,
            wip_limit: None,
            position: 0,
        },
        BoardColumn {
            id: in_progress_status(),
            name: "In Progress".into(),
            category: ColumnCategory::InProgress,
            wip_limit: Some(5),
            position: 1,
        },
        BoardColumn {
            id: review_status(),
            name: "Review".into(),
            category: ColumnCategory::InProgress,
            wip_limit: None,
            position: 3,
        },
        BoardColumn {
            id: done_status(),
            name: "Done".into(),
            category: ColumnCategory::Done,
            wip_limit: None,
            position: 4,
        },
    ]
}
