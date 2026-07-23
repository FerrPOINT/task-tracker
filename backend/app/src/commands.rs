use serde::{Deserialize, Serialize};

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
pub struct CreateIssueCommand {
    pub project_key: shared::ProjectKey,
    pub issue_type: shared::IssueType,
    pub status_id: String,
    pub summary: String,
    pub description: Option<String>,
    pub reporter_id: shared::UserId,
    pub priority: shared::Priority,
    pub assignee_id: Option<shared::UserId>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectQueryDto {
    pub limit: u64,
    pub offset: u64,
}
