//! Lease recovery for a linked, pre-enrollment bootstrap attempt, never dispatch authority.
use crate::sdlc::MachineFence;
use crate::sdlc_execution_lease::PmExecutionIdentity;
use crate::sdlc_lifecycle::ArtifactReceipt;
use crate::sdlc_metadata::MetadataInputRef;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FinalizedBootstrapBinding {
    pub bind_operation_key: String,
    pub bind_request_sha256: String,
    pub binding_ref: String,
    #[serde(deserialize_with = "crate::sdlc_execution_lease::canonical_uuid")]
    pub session_run_id: Uuid,
    pub native_run_ref: String,
}

/// Workflow's immutable local cursor, explicitly distinct from canonical revision 1.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowBootstrapPredecessor {
    pub task_id: i64,
    pub assignment_operation_key: String,
    pub assignment_revision: i64,
    pub attempt: i64,
    // Required nullable: absent is an authoritative unbound cursor, not unknown ACK.
    #[serde(deserialize_with = "required_binding")]
    #[schema(required = true, nullable = true)]
    pub finalized_binding: Option<FinalizedBootstrapBinding>,
}
fn required_binding<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<FinalizedBootstrapBinding>, D::Error> {
    Option::deserialize(d)
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BootstrapRetryCommand {
    #[serde(deserialize_with = "crate::sdlc::safe_version")]
    pub expected_owner_version: i64,
    #[serde(deserialize_with = "crate::sdlc_execution_lease::canonical_fence")]
    pub fence: MachineFence,
    #[serde(deserialize_with = "crate::sdlc_execution_lease::canonical_uuid")]
    pub lease_id: Uuid,
    #[serde(deserialize_with = "crate::sdlc::safe_version")]
    pub expected_lease_version: i64,
    #[serde(deserialize_with = "crate::sdlc_execution_lease::canonical_uuid")]
    pub creation_operation_id: Uuid,
    #[serde(deserialize_with = "crate::sdlc_execution_lease::canonical_uuid")]
    pub previous_session_run_id: Uuid,
    #[serde(deserialize_with = "crate::sdlc_execution_lease::canonical_uuid")]
    pub next_session_run_id: Uuid,
    pub previous_native_run_ref: String,
    #[serde(deserialize_with = "crate::sdlc_execution_lease::canonical_uuid")]
    pub chat_id: Uuid,
    #[serde(deserialize_with = "crate::sdlc_execution_lease::canonical_uuid")]
    pub barrier_id: Uuid,
    pub predecessor: WorkflowBootstrapPredecessor,
    pub idempotency_key: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapReadbackState {
    Absent,
    Present,
    Pending,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapNativeTerminal {
    Completed,
    Failed,
    Cancelled,
    Stopped,
}

/// Composite authoritative Fleet readback. Never accepted in the scheduler POST.
/// Fleet verifies native custody and queries Workflow with its assignment credential.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapFailureReadback {
    pub contract_version: u8,
    pub observed_at: DateTime<Utc>,
    pub identity: PmExecutionIdentity,
    pub owner_subject: String,
    pub input: MetadataInputRef,
    pub owner_version: i64,
    pub lease_id: Uuid,
    pub lease_version: i64,
    pub creation_operation_id: Uuid,
    pub session_run_id: Uuid,
    pub native_run_ref: String,
    pub chat_id: Uuid,
    pub barrier_id: Uuid,
    pub next_session_run_id: Uuid,
    pub terminal: Option<BootstrapNativeTerminal>,
    pub predecessor: WorkflowBootstrapPredecessor,
    pub initial_phase_id: i64,
    pub current_phase_id: i64,
    pub workflow_status: String,
    pub first_instruction: BootstrapReadbackState,
    pub pm_enrollment: BootstrapReadbackState,
    pub generic_binding: BootstrapReadbackState,
    pub business_result: BootstrapReadbackState,
    pub inflight_work: BootstrapReadbackState,
    /// Canonical references/hashes of existing trusted readbacks, not agent uploads.
    pub terminal_receipt: ArtifactReceipt,
    pub bootstrap_receipt: ArtifactReceipt,
}
