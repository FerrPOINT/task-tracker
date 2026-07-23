pub mod requests;

pub use requests::*;

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegisterRequest {
    pub email: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthResponse {
    pub access_token: String,
    pub token_type: String,
    pub user_id: String,
    pub email: String,
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
pub struct IssueResponse {
    pub id: String,
    pub key: String,
    pub summary: String,
    pub description: String,
    pub issue_type: String,
    pub status: String,
    pub priority: String,
    pub labels: Vec<String>,
    pub assignee_id: Option<String>,
    pub assignee_name: Option<String>,
    pub reporter_id: String,
    pub reporter_name: Option<String>,
    pub project_name: String,
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
    pub status_id: String,
    pub assignee_id: Option<String>,
    pub reporter_id: String,
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
pub struct DashboardResponse {
    pub assigned_issues: Vec<IssueResponse>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct SearchQuery {
    pub q: String,
}
