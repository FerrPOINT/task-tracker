use crate::sdlc::{InteractionProtocol, MachineFence, Stage, safe_version};
use crate::sdlc_lifecycle::LifecycleStatus;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, de::Error};
use uuid::Uuid;

fn canonical_uuid<'de, D: Deserializer<'de>>(d: D) -> Result<Uuid, D::Error> {
    let raw = String::deserialize(d)?;
    let id = Uuid::parse_str(&raw).map_err(D::Error::custom)?;
    if id.is_nil() || id.to_string() != raw {
        return Err(D::Error::custom("canonical nonnil UUID required"));
    }
    Ok(id)
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CompleteConfiguredPm {
    pub interaction_protocol: InteractionProtocol,
    pub fence: MachineFence,
    #[serde(deserialize_with = "safe_version")]
    pub requirement_revision: i64,
    pub content_hash: String,
    #[serde(deserialize_with = "safe_version")]
    pub namespace_id: i64,
    #[serde(deserialize_with = "canonical_uuid")]
    pub workflow_execution_ref: Uuid,
    pub workflow_operation_key: String,
    #[serde(deserialize_with = "safe_version")]
    pub workflow_result_version: i64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConfiguredWorkflowReceipt {
    pub interaction_protocol: InteractionProtocol,
    #[serde(deserialize_with = "canonical_uuid")]
    pub execution_ref: Uuid,
    pub task: String,
    #[serde(deserialize_with = "canonical_uuid")]
    pub tracker_task_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    pub namespace_id: i64,
    #[serde(deserialize_with = "canonical_uuid")]
    pub concrete_agent_ref: Uuid,
    #[serde(deserialize_with = "canonical_uuid")]
    pub assignment_ref: Uuid,
    #[serde(deserialize_with = "canonical_uuid")]
    pub tracker_execution_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    pub assignment_version: i64,
    pub machine_subject: String,
    pub operation_key: String,
    pub request_sha256: String,
    #[serde(deserialize_with = "safe_version")]
    pub result_version: i64,
    #[serde(deserialize_with = "safe_version")]
    pub requirements_revision: i64,
    pub requirements_sha256: String,
    pub owner_subject: String,
    #[serde(deserialize_with = "canonical_uuid")]
    pub confirmation_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    pub history_id: i64,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConfiguredPmCompletionReceipt {
    pub interaction_protocol: InteractionProtocol,
    pub completion_id: Uuid,
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub root_task_id: Uuid,
    pub owner_subject: String,
    pub fence: MachineFence,
    pub owner_version: i64,
    pub requirement_revision: i64,
    pub content_hash: String,
    pub confirmation_id: Uuid,
    pub workflow_receipt: ConfiguredWorkflowReceipt,
    pub workflow_receipt_sha256: String,
    pub stage: Stage,
    pub status: LifecycleStatus,
    pub previous_pm_admission_state: String,
    pub next_pm_admission_state: String,
    pub idempotency_key: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfiguredPmCompletionOperation {
    pub idempotency_key: String,
    pub request_sha256: String,
    pub result: ConfiguredPmCompletionReceipt,
}

#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
pub struct ConfiguredPmCompletionReadback {
    pub interaction_protocol: InteractionProtocol,
    pub current: Option<ConfiguredPmCompletionReceipt>,
}

#[derive(Clone, Debug, Deserialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum PmCompletionCommand {
    Configured(CompleteConfiguredPm),
    Legacy(crate::sdlc_reservation::CompletePmDraft),
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum PmCompletionResponse {
    Configured(ConfiguredPmCompletionReceipt),
    Legacy(crate::sdlc_reservation::PmDraftCompletionReceipt),
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum PmCompletionReadbackResponse {
    Configured(ConfiguredPmCompletionReadback),
    Legacy(crate::sdlc_reservation::PmDraftCompletionReadback),
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum PmCompletionOperationResponse {
    Configured(ConfiguredPmCompletionOperation),
    Legacy(crate::sdlc_reservation::PmDraftCompletionOperation),
}
