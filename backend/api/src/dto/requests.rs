use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateIssueRequest {
    pub summary: Option<String>,
    #[serde(default, deserialize_with = "super::deserialize_optional_nullable")]
    pub description: Option<Option<String>>,
    pub priority: Option<String>,
    pub status_id: Option<String>,
    pub assignee_id: Option<String>,
    #[serde(default, deserialize_with = "super::deserialize_optional_nullable")]
    pub sprint_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "super::deserialize_optional_nullable")]
    pub component_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "super::deserialize_optional_nullable")]
    pub affected_version_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "super::deserialize_optional_nullable")]
    pub fix_version_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MoveIssueRequest {
    pub issue_id: String,
    pub status_id: String,
}
