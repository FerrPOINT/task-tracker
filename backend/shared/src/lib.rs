use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Health {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectResponse {
    pub id: String,
    pub key: String,
    pub name: String,
    pub description: String,
    pub owner_id: String,
    pub created_at: String,
    pub todo_count: i64,
    pub in_progress_count: i64,
    pub done_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectListResponse {
    pub items: Vec<ProjectResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IssueResponse {
    pub id: String,
    pub key: String,
    pub summary: String,
    pub description: String,
    pub project_key: String,
    pub project_name: String,
    pub status: String,
    pub assignee_id: Option<String>,
    pub assignee_name: Option<String>,
    pub reporter_id: String,
    pub reporter_name: String,
    pub priority: String,
    pub labels: Vec<String>,
    pub due_date: Option<String>,
    pub original_estimate_seconds: Option<i64>,
    pub remaining_estimate_seconds: Option<i64>,
    pub time_spent_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BoardColumnResponse {
    pub id: String,
    pub name: String,
    pub wip_limit: Option<i64>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BoardResponse {
    pub columns: Vec<BoardColumnResponse>,
    pub issues: Vec<IssueResponse>,
    pub sprint: SprintResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BacklogResponse {
    pub sprint: SprintResponse,
    pub sprint_issues: Vec<IssueResponse>,
    pub backlog_issues: Vec<IssueResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SearchResultResponse {
    pub issues: Vec<IssueResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateIssueRequest {
    pub project_key: String,
    pub issue_type: String,
    pub summary: String,
    pub description: String,
    pub priority: String,
    pub assignee_id: Option<String>,
    pub due_date: Option<String>,
    pub labels: Vec<String>,
    pub original_estimate_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateIssueResponse {
    pub issue: IssueResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub name: String,
}
