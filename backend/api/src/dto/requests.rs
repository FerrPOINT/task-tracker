use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateIssueRequest {
    pub summary: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<String>,
    pub status_id: Option<String>,
    pub assignee_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MoveIssueRequest {
    pub issue_id: String,
    pub status_id: String,
}
