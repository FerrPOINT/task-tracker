//! Internal planning inputs, not another public command or terminal proof protocol.
use crate::sdlc::{AnalysisIntent, PmAssignment, TaskState};
use crate::sdlc_critical_blockers::CriticalBlockerReceipt;
use crate::sdlc_critical_pm_reservation::CriticalPmReservationResult;
use crate::sdlc_execution_lease::ExecutionLeaseReceipt;
use crate::sdlc_lifecycle::LifecycleReceipt;
use crate::sdlc_reservation::PmDraftCompletionReceipt;
use crate::sdlc_routing::TaskRoutingSnapshot;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub const PM_FINALIZATION_REASON: &str = "critical_pm_awaiting_finalization";

#[derive(Clone, Copy)]
pub enum CriticalPmExecutionContext<'a> {
    /// Current configured assignment is the authority. Native leases belong to
    /// Fleet and are not manufactured for business commands or owner answers.
    Configured { observed_at: DateTime<Utc> },
    /// Actual current lease readback; this is not proof of native running.
    Open {
        lease: &'a ExecutionLeaseReceipt,
        observed_at: DateTime<Utc>,
    },
    /// Server-prepared from this CompletePmDraft after parent terminal verification.
    /// Not previously committed: insert it WITH the resolution/generation/projection.
    Finalizing {
        lease: &'a ExecutionLeaseReceipt,
        completion: &'a PmDraftCompletionReceipt,
    },
}

pub struct CriticalPmLifecycleSource<'a> {
    pub state: &'a TaskState,
    pub lifecycle: &'a LifecycleReceipt,
    pub review: &'a CriticalPmReservationResult,
    pub blocker: &'a CriticalBlockerReceipt,
    pub initial_pm_assignment: &'a PmAssignment,
    pub initial_routing: &'a TaskRoutingSnapshot,
    pub execution: CriticalPmExecutionContext<'a>,
}

#[derive(Clone, Debug)]
pub struct CriticalPmProjection {
    pub state: TaskState,
    pub lifecycle: LifecycleReceipt,
    pub blocker: CriticalBlockerReceipt,
}

pub struct CriticalPmCommandAllocation {
    pub now: DateTime<Utc>,
    /// Fresh server allocation ONLY for Answer/Confirm; never an invented input receipt.
    pub generated_receipt_id: Option<Uuid>,
}

pub struct CriticalPmCommandPlan {
    pub before: CriticalPmProjection,
    pub after: CriticalPmProjection,
    pub result: serde_json::Value,
}

pub struct CriticalPmCompletionAllocation {
    pub snapshot_id: Uuid,
    pub analysis_intent_id: Uuid,
}

#[derive(Clone, Debug)]
pub struct CriticalPmRoutingGeneration {
    pub generation: i64,
    pub previous_snapshot_id: Uuid,
    pub snapshot: TaskRoutingSnapshot,
}

/// All fields are prospective effects of ONE transaction, never an early release permit.
pub struct CriticalPmCompletionPlan {
    pub before: CriticalPmProjection,
    pub after: CriticalPmProjection,
    pub completion: PmDraftCompletionReceipt,
    pub routing: CriticalPmRoutingGeneration,
    pub analysis_intent: AnalysisIntent,
}

/// Business completion has no native receipt and does not enqueue Analyst.
pub struct ConfiguredCriticalPmCompletionPlan {
    pub before: CriticalPmProjection,
    pub after: CriticalPmProjection,
    pub routing: CriticalPmRoutingGeneration,
}
