pub mod requests;

pub use requests::*;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegisterRequest {
    pub email: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub user_id: String,
    pub email: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserListResponse {
    pub users: Vec<UserResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub username: String,
    pub display_name: String,
    /// Whether the user may access admin endpoints.
    pub is_system_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectResponse {
    pub id: String,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: String,
    pub todo_count: u32,
    pub in_progress_count: u32,
    pub done_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateProjectRequest {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IssueResponse {
    pub id: String,
    pub key: String,
    pub summary: String,
    pub description: String,
    pub issue_type: String,
    pub project_key: String,
    pub status: String,
    pub status_id: String,
    pub priority: String,
    pub labels: Vec<String>,
    pub assignee_id: Option<String>,
    pub assignee_name: Option<String>,
    pub reporter_id: String,
    pub reporter_name: Option<String>,
    pub project_name: String,
    pub sprint_id: Option<String>,
    /// Original estimate in seconds. Null when no estimate was set.
    pub original_estimate_seconds: Option<i64>,
    /// Remaining estimate in seconds, derived from the latest worklog.
    pub remaining_estimate_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IssueListResponse {
    pub issues: Vec<IssueResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateIssueRequest {
    pub project_key: String,
    pub issue_type: String,
    pub summary: String,
    pub description: Option<String>,
    pub priority: String,
    pub status_id: Option<String>,
    pub assignee_id: Option<String>,
    pub reporter_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BoardColumnResponse {
    pub id: String,
    pub name: String,
    pub wip_limit: Option<u32>,
    pub issue_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SprintResponse {
    pub id: String,
    pub name: String,
    pub goal: String,
    pub state: String,
    pub velocity: i64,
    pub remaining_days: Option<i64>,
    pub issue_ids: Vec<String>,
    /** Format: date-time */
    pub start_date: Option<DateTime<FixedOffset>>,
    /** Format: date-time */
    pub end_date: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BoardResponse {
    pub project_id: String,
    pub project_key: String,
    pub columns: Vec<BoardColumnResponse>,
    pub issues: Vec<IssueResponse>,
    pub sprint: SprintResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BacklogResponse {
    pub project_id: String,
    pub project_key: String,
    pub backlog_total: usize,
    pub sprint: SprintResponse,
    pub sprint_issues: Vec<IssueResponse>,
    pub backlog_issues: Vec<IssueResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DashboardResponse {
    pub assigned_issues: Vec<IssueResponse>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub project_key: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub assignee_id: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub jql: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CommentResponse {
    pub id: String,
    pub issue_id: String,
    pub author_id: String,
    pub author_name: Option<String>,
    pub body: String,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CommentListResponse {
    pub comments: Vec<CommentResponse>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateCommentRequest {
    pub body: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateCommentRequest {
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorklogResponse {
    pub id: String,
    pub issue_id: String,
    pub author_id: String,
    pub author_name: Option<String>,
    pub started_at: DateTime<FixedOffset>,
    pub duration_seconds: i64,
    pub description: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorklogListResponse {
    pub worklogs: Vec<WorklogResponse>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateWorklogRequest {
    pub started_at: DateTime<FixedOffset>,
    pub duration_seconds: i64,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateWorklogRequest {
    pub started_at: Option<DateTime<FixedOffset>>,
    pub duration_seconds: Option<i64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectMemberResponse {
    pub project_id: String,
    pub user_id: String,
    pub role: String,
    pub joined_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectMemberListResponse {
    pub members: Vec<ProjectMemberResponse>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct AddProjectMemberRequest {
    pub user_id: String,
    pub role: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TransitionIssueRequest {
    pub target_status_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SprintListResponse {
    pub sprints: Vec<SprintResponse>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateSprintRequest {
    pub name: String,
    pub goal: Option<String>,
    pub start_date: Option<DateTime<FixedOffset>>,
    pub end_date: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateSprintRequest {
    pub name: Option<String>,
    pub goal: Option<Option<String>>,
    pub start_date: Option<Option<DateTime<FixedOffset>>>,
    pub end_date: Option<Option<DateTime<FixedOffset>>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct MoveIssueToSprintRequest {
    pub issue_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StatusResponse {
    pub id: String,
    pub name: String,
    pub category: String,
    pub position: i32,
    pub is_default: bool,
    pub is_closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TransitionResponse {
    pub id: String,
    pub name: String,
    pub from_status_id: String,
    pub to_status_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IssueTypeResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub is_subtask: bool,
    pub hierarchy_level: i16,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AttachmentResponse {
    pub id: String,
    pub issue_id: String,
    pub author_id: String,
    pub file_name: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AttachmentListResponse {
    pub attachments: Vec<AttachmentResponse>,
}
