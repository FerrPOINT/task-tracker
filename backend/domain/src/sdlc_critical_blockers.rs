//! Proposed critical-business-question contract. Registration belongs to integration.
use crate::attachment_artifact::AttachmentArtifact;
use crate::sdlc::safe_version;
use crate::sdlc_lifecycle::{DeliveryFence, DeliveryRole, RuntimeTerminalObservation};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::AppError;
use std::collections::HashSet;
use uuid::Uuid;

pub const MAX_OPTIONS: usize = 8;
pub const MAX_EVIDENCE: usize = 16;
pub const HOLD_REASON: &str = "critical_business_question_held";
pub const PM_REVIEW_REASON: &str = "critical_pm_review_queued";

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CriticalBlockerKind {
    RequirementsContradiction,
    MissingBusinessDecision,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CriticalBlockerOption {
    pub option_id: Uuid,
    pub label: String,
    pub consequences: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmitCriticalBlocker {
    pub fence: DeliveryFence,
    #[serde(deserialize_with = "safe_version")]
    pub requirement_revision: i64,
    pub content_hash: String,
    pub kind: CriticalBlockerKind,
    pub question: String,
    pub rationale: String,
    #[serde(default)]
    pub options: Vec<CriticalBlockerOption>,
    pub recommendation: Option<Uuid>,
    pub evidence: Vec<AttachmentArtifact>,
    pub idempotency_key: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CriticalBlockerState {
    Proposed,
    Held,
    AwaitingPmReview,
    InPmReview,
    Resolved,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CriticalPmReviewRef {
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CriticalResolutionRef {
    #[serde(deserialize_with = "safe_version")]
    pub requirement_revision: i64,
    pub content_hash: String,
    #[serde(deserialize_with = "safe_version")]
    pub routing_generation: i64,
}

/// Detail may evolve; each operation journal must retain its original receipt verbatim.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CriticalBlockerReceipt {
    pub blocker_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    pub version: i64,
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub root_task_id: Uuid,
    pub owner_subject: String,
    pub producer_subject: String,
    pub agent_id: Uuid,
    pub role: DeliveryRole,
    pub assignment_hash: String,
    pub chat_id: Uuid,
    pub run_id: Uuid,
    pub routing_snapshot_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    pub routing_generation: i64,
    pub proposal: SubmitCriticalBlocker,
    pub blocker_sha256: String,
    pub state: CriticalBlockerState,
    pub pm_review: Option<CriticalPmReviewRef>,
    pub resolution: Option<CriticalResolutionRef>,
    pub technical_terminal: Option<RuntimeTerminalObservation>,
    #[serde(serialize_with = "crate::sdlc_execution_lease::utc_nanos")]
    pub created_at: DateTime<Utc>,
}

/// CAS fields for the scheduler-only return, not a standalone public hold operation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CriticalPmReviewGuard {
    pub blocker_id: Uuid,
    pub blocker_sha256: String,
    #[serde(deserialize_with = "safe_version")]
    pub expected_blocker_version: i64,
    #[serde(deserialize_with = "safe_version")]
    pub expected_task_version: i64,
    pub fence: DeliveryFence,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReturnCriticalToPm {
    pub review: CriticalPmReviewGuard,
    pub terminal: RuntimeTerminalObservation,
}

pub fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= limit
        && !value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t')
}

impl SubmitCriticalBlocker {
    /// Shape/provenance only. The existing attachment service must verify actual sealed bytes.
    pub fn validate(&self, role: DeliveryRole) -> Result<(), AppError> {
        let invalid = || AppError::validation("invalid critical business blocker");
        if self.fence.assignment_id.is_nil()
            || self.fence.execution_id.is_nil()
            || self.fence.lease_id.is_nil()
            || !(1..=crate::sdlc::MAX_SAFE_VERSION).contains(&self.fence.fencing_token)
            || !(1..=crate::sdlc::MAX_SAFE_VERSION).contains(&self.fence.expected_lease_version)
            || self.idempotency_key.is_empty()
            || self.idempotency_key.len() > 128
            || matches!(self.idempotency_key.as_str(), "." | "..")
            || self
                .idempotency_key
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
            || !(1..=crate::sdlc::MAX_SAFE_VERSION).contains(&self.requirement_revision)
            || !sha256(&self.content_hash)
            || !text(&self.question, 4096)
            || !text(&self.rationale, 8192)
            || self.options.len() > MAX_OPTIONS
            || !(1..=MAX_EVIDENCE).contains(&self.evidence.len())
        {
            return Err(invalid());
        }
        let mut options = HashSet::new();
        for option in &self.options {
            if option.option_id.is_nil()
                || !options.insert(option.option_id)
                || !text(&option.label, 256)
                || !text(&option.consequences, 4096)
            {
                return Err(invalid());
            }
        }
        if self.recommendation.is_some_and(|id| !options.contains(&id)) {
            return Err(invalid());
        }
        let mut references = HashSet::new();
        for artifact in &self.evidence {
            let raw = artifact
                .receipt
                .reference
                .strip_prefix("tracker-attachment:")
                .ok_or_else(invalid)?;
            let id: Uuid = raw.parse().map_err(|_| invalid())?;
            if id.is_nil()
                || id.to_string() != raw
                || !references.insert(id)
                || !sha256(&artifact.receipt.sha256)
                || artifact.binding.assignment_id != self.fence.assignment_id
                || artifact.binding.execution_id != self.fence.execution_id
                || artifact.binding.role != role
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "sdlc_critical_blockers_tests.rs"]
mod tests;
