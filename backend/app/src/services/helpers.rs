use std::{collections::HashMap, sync::Arc};

use domain::{BoardColumn, Issue, ProjectRepository, StatusCategory};
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

pub fn build_issue_dto_from_lookups(
    issue: Issue,
    project_names: &HashMap<ProjectId, String>,
    user_names: &HashMap<shared::UserId, String>,
) -> crate::dto::IssueDto {
    let status_id = issue.status_id;
    let assignee_name = issue
        .assignee_id
        .and_then(|id| user_names.get(&id).cloned());
    let reporter_name = user_names.get(&issue.reporter_id).cloned();
    let project_name = project_names
        .get(&issue.project_id)
        .cloned()
        .unwrap_or_default();
    crate::dto::IssueDto::from_issue(
        issue,
        project_name,
        issue_status_column(status_id),
        assignee_name,
        reporter_name,
    )
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

async fn issue_user_name_lookup(
    users: Arc<dyn domain::UserRepository>,
) -> HashMap<shared::UserId, String> {
    users
        .list()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|user| (user.id, user.display_name.to_string()))
        .collect()
}

pub async fn build_issue_dtos(
    users: Arc<dyn domain::UserRepository>,
    issues: Vec<Issue>,
    project_name: &str,
) -> Result<Vec<crate::dto::IssueDto>, AppError> {
    let user_names = issue_user_name_lookup(users).await;
    let project_names = issues
        .iter()
        .map(|issue| (issue.project_id, project_name.to_string()))
        .collect::<HashMap<_, _>>();
    Ok(issues
        .into_iter()
        .map(|issue| build_issue_dto_from_lookups(issue, &project_names, &user_names))
        .collect())
}

async fn build_issue_dtos_prefetched(
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
    issues: Vec<Issue>,
) -> Result<Vec<crate::dto::IssueDto>, AppError> {
    let project_names = projects
        .list(domain::ProjectQuery::default())
        .await?
        .into_iter()
        .map(|project| (project.id, project.name.to_string()))
        .collect::<HashMap<_, _>>();
    let user_names = issue_user_name_lookup(users).await;
    if let Some(missing) = issues
        .iter()
        .find(|issue| !project_names.contains_key(&issue.project_id))
    {
        return Err(AppError::not_found("project", missing.project_id));
    }
    Ok(issues
        .into_iter()
        .map(|issue| build_issue_dto_from_lookups(issue, &project_names, &user_names))
        .collect())
}

pub async fn build_issue_dtos_with_projects(
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
    issues: Vec<Issue>,
) -> Result<Vec<crate::dto::IssueDto>, AppError> {
    build_issue_dtos_prefetched(projects, users, issues).await
}

pub async fn build_issue_dtos_for_dashboard(
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
    issues: Vec<Issue>,
) -> Result<Vec<crate::dto::IssueDto>, AppError> {
    build_issue_dtos_prefetched(projects, users, issues).await
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
            position: 3,
        },
        BoardColumn {
            id: done_status(),
            name: "Done".into(),
            category: StatusCategory::Done,
            wip_limit: None,
            position: 4,
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

        let dto = build_issue_dto_from_lookups(issue, &projects, &users);
        assert_eq!(dto.project_name, "Project TT");
        assert_eq!(dto.reporter_name.as_deref(), Some("Reporter"));
    }
}
