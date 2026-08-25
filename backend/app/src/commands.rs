use serde::{Deserialize, Serialize};
use shared::{IssueId, ProjectId, SprintId, StatusId, UserId};

#[derive(Debug, Clone, Deserialize)]
pub struct CreateSprintCommand {
    pub project_id: ProjectId,
    pub name: String,
    pub goal: Option<String>,
    pub start_date: Option<chrono::DateTime<chrono::FixedOffset>>,
    pub end_date: Option<chrono::DateTime<chrono::FixedOffset>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateSprintCommand {
    pub name: Option<String>,
    pub goal: Option<Option<String>>,
    pub start_date: Option<Option<chrono::DateTime<chrono::FixedOffset>>>,
    pub end_date: Option<Option<chrono::DateTime<chrono::FixedOffset>>>,
}

#[derive(Debug, Clone)]
pub struct MoveIssueToSprintCommand {
    pub issue_id: IssueId,
    pub sprint_id: Option<SprintId>,
}

#[derive(Debug, Clone)]
pub struct StartSprintCommand {
    pub sprint_id: SprintId,
}

#[derive(Debug, Clone)]
pub struct CloseSprintCommand {
    pub sprint_id: SprintId,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterCommand {
    pub email: String,
    pub username: String,
    pub name: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginCommand {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProjectCommand {
    pub key: shared::ProjectKey,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: shared::UserId,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectQueryDto {
    pub limit: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateIssueCommand {
    pub project_key: shared::ProjectKey,
    pub issue_type: shared::IssueType,
    pub status_id: String,
    pub summary: String,
    pub description: Option<String>,
    pub reporter_id: shared::UserId,
    pub priority: shared::Priority,
    pub assignee_id: Option<shared::UserId>,
    pub actor_id: shared::UserId,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateIssueCommand {
    pub summary: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<shared::Priority>,
    pub status_id: Option<String>,
    pub assignee_id: Option<Option<shared::UserId>>,
    pub sprint_id: Option<Option<shared::SprintId>>,
    pub component_id: Option<Option<shared::ProjectComponentId>>,
    pub affected_version_id: Option<Option<shared::ProjectVersionId>>,
    pub fix_version_id: Option<Option<shared::ProjectVersionId>>,
    pub actor_id: shared::UserId,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateCommentCommand {
    pub issue_id: shared::IssueId,
    pub author_id: shared::UserId,
    pub body: String,
    pub actor_id: shared::UserId,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateCommentCommand {
    pub body: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateWorklogCommand {
    pub issue_id: shared::IssueId,
    pub author_id: shared::UserId,
    pub started_at: chrono::DateTime<chrono::FixedOffset>,
    pub duration_seconds: i64,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateWorklogCommand {
    pub started_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    pub duration_seconds: Option<i64>,
    pub description: Option<Option<String>>,
}

#[derive(Debug, Clone)]
pub struct AddProjectMemberCommand {
    pub project_id: ProjectId,
    pub user_id: UserId,
    pub role: String,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateProjectCommand {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
}

#[derive(Debug, Clone)]
pub struct TransitionIssueCommand {
    pub issue_id: IssueId,
    pub target_status_id: StatusId,
    pub actor_id: shared::UserId,
}

#[derive(Debug, Clone)]
pub struct UpdateNotificationSettingsCommand {
    pub email_frequency: domain::ArcStr,
    pub disabled_event_types: Vec<domain::ArcStr>,
    pub notify_own_changes: bool,
}
