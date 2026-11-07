//! Explicit agent-owned start of the current stage; routing remains server-owned.
use crate::sdlc::{MachineFence, Stage};
use crate::sdlc_lifecycle::{DeliveryFence, LifecycleStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StageStartFence {
    Pm(MachineFence),
    Delivery(DeliveryFence),
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StartStage {
    pub fence: StageStartFence,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StageStartReceipt {
    pub task_id: Uuid,
    pub stage: Stage,
    pub status: LifecycleStatus,
    pub execution_id: Uuid,
    pub idempotency_key: String,
    pub started_at: DateTime<Utc>,
}
