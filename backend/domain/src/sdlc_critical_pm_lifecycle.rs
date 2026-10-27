//! Internal planning inputs, not another public command or terminal proof protocol.
use crate::sdlc::{PmAssignment, TaskState};
use crate::sdlc_critical_blockers::CriticalBlockerReceipt;
use crate::sdlc_critical_pm_reservation::CriticalPmReservationResult;
use crate::sdlc_lifecycle::LifecycleReceipt;
use crate::sdlc_routing::TaskRoutingSnapshot;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub const PM_FINALIZATION_REASON: &str = "critical_pm_awaiting_finalization";

pub struct CriticalPmLifecycleSource<'a> {
    pub state: &'a TaskState,
    pub lifecycle: &'a LifecycleReceipt,
    pub review: &'a CriticalPmReservationResult,
    pub blocker: &'a CriticalBlockerReceipt,
    pub initial_pm_assignment: &'a PmAssignment,
    pub initial_routing: &'a TaskRoutingSnapshot,
    pub observed_at: DateTime<Utc>,
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

#[derive(Clone, Debug)]
pub struct CriticalPmRoutingGeneration {
    pub generation: i64,
    pub previous_snapshot_id: Uuid,
    pub snapshot: TaskRoutingSnapshot,
}

/// Business completion has no native receipt and does not enqueue Analyst.
pub struct ConfiguredCriticalPmCompletionPlan {
    pub before: CriticalPmProjection,
    pub after: CriticalPmProjection,
    pub routing: CriticalPmRoutingGeneration,
}
