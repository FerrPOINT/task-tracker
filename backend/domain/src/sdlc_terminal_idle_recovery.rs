//! A permit for one frozen checkpoint, never a live execution lease.
use crate::sdlc::{MachineFence, safe_version};
use crate::sdlc_execution_lease::{
    PmExecutionIdentity, PmResumeInput, canonical_fence, canonical_uuid, utc_nanos,
};
use crate::sdlc_lifecycle::ArtifactReceipt;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, de::Error};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum PmLeaseResumeRequest {
    Ordinary(crate::sdlc_execution_lease::ResumePmExecutionLease),
    TerminalIdle(PermitPmIdleCheckpointRequest),
}

#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum PmLeaseResumeResponse {
    Ordinary(crate::sdlc_execution_lease::ExecutionLeaseReceipt),
    TerminalIdle(PermitPmIdleCheckpointResponse),
}

#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum PmLeaseResumeOperationResponse {
    Ordinary(crate::sdlc_execution_lease::ExecutionLeaseOperation),
    TerminalIdle(PermitPmIdleCheckpointResponse),
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PermitPmIdleCheckpointRequest {
    #[serde(deserialize_with = "safe_version")]
    pub expected_owner_version: i64,
    #[serde(deserialize_with = "canonical_uuid")]
    pub lease_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    pub expected_lease_version: i64,
    #[serde(deserialize_with = "feedback_input")]
    #[schema(value_type = PmIdleFeedbackInput)]
    pub input: PmResumeInput,
    #[serde(deserialize_with = "canonical_uuid")]
    pub previous_run_id: Uuid,
    pub previous_run_terminal: ArtifactReceipt,
    #[serde(deserialize_with = "canonical_fence")]
    pub fence: MachineFence,
    pub terminal_idle_recovery: PmTerminalIdleRecovery,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(tag = "input_kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum PmIdleFeedbackInput {
    Feedback {
        #[serde(deserialize_with = "canonical_uuid")]
        input_id: Uuid,
    },
}

fn feedback_input<'de, D: Deserializer<'de>>(d: D) -> Result<PmResumeInput, D::Error> {
    let PmIdleFeedbackInput::Feedback { input_id } = PmIdleFeedbackInput::deserialize(d)?;
    Ok(PmResumeInput::Feedback { input_id })
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PmTerminalIdleRecovery {
    pub contract_version: u8,
    #[serde(deserialize_with = "canonical_uuid")]
    pub barrier_id: Uuid,
    pub checkpoint_operation_key: String,
    pub checkpoint_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, utoipa::ToSchema)]
pub enum IdleCheckpointOperation {
    #[serde(rename = "permit_pm_idle_checkpoint")]
    PermitPmIdleCheckpoint,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, utoipa::ToSchema)]
pub enum IdleCheckpointScope {
    #[serde(rename = "idle_checkpoint")]
    IdleCheckpoint,
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PmIdleCheckpointPermit {
    #[serde(deserialize_with = "canonical_uuid")]
    pub permit_id: Uuid,
    pub scope: IdleCheckpointScope,
    #[serde(deserialize_with = "canonical_uuid")]
    pub barrier_id: Uuid,
    #[serde(deserialize_with = "canonical_uuid")]
    pub session_run_id: Uuid,
    #[serde(deserialize_with = "canonical_uuid")]
    pub input_id: Uuid,
    pub checkpoint_operation_key: String,
    pub checkpoint_sha256: String,
    #[serde(serialize_with = "utc_nanos", deserialize_with = "utc_time")]
    pub issued_at: DateTime<Utc>,
    #[serde(serialize_with = "utc_nanos", deserialize_with = "utc_time")]
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PermitPmIdleCheckpointResponse {
    pub contract_version: u8,
    pub operation: IdleCheckpointOperation,
    pub idempotency_key: String,
    pub request_sha256: String,
    pub permit: PmIdleCheckpointPermit,
    #[serde(serialize_with = "utc_nanos", deserialize_with = "utc_time")]
    pub observed_at: DateTime<Utc>,
}

fn utc_time<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
    let raw = String::deserialize(d)?;
    if !raw.ends_with('Z') {
        return Err(D::Error::custom("UTC timestamp required"));
    }
    raw.parse().map_err(D::Error::custom)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PmTerminalIdleRecoveryWitness {
    pub contract_version: u8,
    #[serde(deserialize_with = "canonical_uuid")]
    pub barrier_id: Uuid,
    pub owner_subject: String,
    pub identity: PmExecutionIdentity,
    pub request_sha256: String,
    pub command: PermitPmIdleCheckpointRequest,
    #[serde(deserialize_with = "closed_native")]
    pub native_terminal: PmIdleNativeObservation,
    pub feedback_receipt: ArtifactReceipt,
    pub instruction_operation_key: String,
    pub instruction_request_sha256: String,
    pub instruction_result_sha256: String,
    pub checkpoint_request: PmIdleCheckpointRequest,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PmIdleCheckpointRequest {
    pub operation_key: String,
    pub body: Value,
}

/// Exact Fleet PmRuntimeObservation wire; no native authority is constructed here.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PmIdleNativeObservation {
    #[serde(flatten)]
    pub identity: PmExecutionIdentity,
    pub observation_ref: Uuid,
    pub binding_ref: String,
    pub hermes_run_ref: String,
    pub session_run_id: Uuid,
    pub status: PmIdleNativeStatus,
    pub dispatch_operation_key: String,
    pub checkpoint_ref: Option<String>,
    pub fence: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PmIdleNativeStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
    Stopped,
}

fn closed_native<'de, D: Deserializer<'de>>(d: D) -> Result<PmIdleNativeObservation, D::Error> {
    let raw = Value::deserialize(d)?;
    let fields = raw
        .as_object()
        .ok_or_else(|| D::Error::custom("native observation object required"))?;
    let keys = [
        "task",
        "execution_ref",
        "tracker_instance_ref",
        "tracker_project_ref",
        "task_ref",
        "root_ref",
        "agent_ref",
        "assignment_operation_key",
        "assignment_ref",
        "assignment_revision",
        "observation_ref",
        "binding_ref",
        "hermes_run_ref",
        "session_run_id",
        "status",
        "dispatch_operation_key",
        "checkpoint_ref",
        "fence",
    ];
    if fields.len() != keys.len() || !keys.iter().all(|key| fields.contains_key(*key)) {
        return Err(D::Error::custom("closed Fleet native observation required"));
    }
    serde_json::from_value(raw).map_err(D::Error::custom)
}
