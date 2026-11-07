//! A versioned plan inside one issue. Sections have no independent lifecycle.
use crate::sdlc::{MachineFence, Principal};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::AppError;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanSection {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub criteria: String,
    pub requirements: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanSource {
    pub requirements_revision: i64,
    pub requirements_hash: String,
    pub analysis_result_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanContent {
    pub overview: String,
    pub source: Option<PlanSource>,
    /// Array order is the section order. IDs survive rename and reorder.
    pub sections: Vec<PlanSection>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SavePlan {
    /// Zero creates the first revision; all edits compare the current revision.
    pub expected_revision: i64,
    pub idempotency_key: String,
    pub reason: String,
    pub content: PlanContent,
    /// Required only for the current configured Architect, forbidden for humans.
    pub fence: Option<MachineFence>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PlanRevision {
    pub task_id: Uuid,
    pub revision: i64,
    pub content: PlanContent,
    pub content_sha256: String,
    pub author_subject: String,
    pub author_name: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PlanReadback {
    pub plan: Option<PlanRevision>,
    pub current_source: Option<PlanSource>,
    pub can_edit: bool,
    pub edit_restriction: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PlanOperation {
    pub request_sha256: String,
    pub result: PlanRevision,
    pub changed: bool,
}

#[derive(Clone, Debug, Default, Deserialize, utoipa::IntoParams)]
#[serde(deny_unknown_fields)]
pub struct PlanHistoryQuery {
    pub before: Option<i64>,
    pub limit: Option<u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PlanHistory {
    pub revisions: Vec<PlanRevisionSummary>,
    pub next_before: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PlanRevisionSummary {
    pub revision: i64,
    pub author_name: String,
    pub author_subject: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub content_sha256: String,
}

#[async_trait]
pub trait DecompositionRepository: Send + Sync {
    async fn read_plan(&self, task: Uuid, actor: &Principal) -> Result<PlanReadback, AppError>;
    async fn plan_history(
        &self,
        task: Uuid,
        actor: &Principal,
        query: PlanHistoryQuery,
    ) -> Result<PlanHistory, AppError>;
    async fn plan_revision(
        &self,
        task: Uuid,
        actor: &Principal,
        revision: i64,
    ) -> Result<PlanRevision, AppError>;
    async fn save_plan(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SavePlan,
    ) -> Result<(PlanOperation, bool), AppError>;
    async fn plan_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<PlanOperation, AppError>;
}
