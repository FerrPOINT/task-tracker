//! Pure planning under the existing task lock/transaction; no persistence or runtime proof service.
use domain::sdlc::{MAX_SAFE_VERSION, Principal, SdlcConfig};
use domain::sdlc_critical_blockers::*;
use domain::sdlc_lifecycle::*;
use shared::AppError;
use uuid::Uuid;

impl crate::sdlc::SdlcService {
    pub async fn submit_critical_blocker(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SubmitCriticalBlocker,
    ) -> Result<(CriticalBlockerReceipt, bool), AppError> {
        crate::sdlc::validate_key(&command.idempotency_key)?;
        self.lifecycle_repository()?
            .submit_critical_blocker(task, actor, command)
            .await
    }

    pub async fn critical_blocker(
        &self,
        task: Uuid,
        actor: &Principal,
        blocker_id: Uuid,
    ) -> Result<CriticalBlockerReceipt, AppError> {
        if blocker_id.is_nil() {
            return Err(AppError::validation("invalid critical blocker ID"));
        }
        self.lifecycle_repository()?
            .critical_blocker(task, actor, blocker_id)
            .await
    }

    pub async fn critical_blocker_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<CriticalBlockerReceipt, AppError> {
        crate::sdlc::validate_key(key)?;
        self.lifecycle_repository()?
            .critical_blocker_operation(task, actor, key)
            .await
    }
}

/// Server allocation, never a request DTO. Generation comes from persisted current selection.
pub struct BlockerAllocation {
    pub blocker_id: Uuid,
    pub routing_generation: i64,
    pub observed_at: chrono::DateTime<chrono::Utc>,
}

/// Persist both projections and the operation journal atomically; archive the old head on return.
#[derive(Clone, Debug, PartialEq)]
pub struct CriticalPolicyChange {
    pub lifecycle: LifecycleReceipt,
    pub blocker: CriticalBlockerReceipt,
}

fn conflict() -> AppError {
    AppError::conflict("critical blocker does not match current lifecycle")
}

fn positive(value: i64) -> bool {
    (1..=MAX_SAFE_VERSION).contains(&value)
}

fn next_version(value: i64) -> Result<i64, AppError> {
    if !positive(value) {
        return Err(conflict());
    }
    value
        .checked_add(1)
        .filter(|v| positive(*v))
        .ok_or_else(conflict)
}

fn configured(current: &LifecycleReceipt, config: &SdlcConfig) -> Result<(), AppError> {
    let task = &current.task;
    let routing = &task.routing;
    crate::sdlc_routing::validate_routes(&routing.policy.routes)?;
    if task.task_id.is_nil()
        || routing.task_id != task.task_id
        || routing.root_task_id != task.task_id
        || routing.project_id.is_nil()
        || routing.snapshot_id.is_nil()
        || routing.confirmation_id.is_nil()
        || routing.tracker_instance_id != config.instance_id
        || !config.single_task_project_ids.contains(&routing.project_id)
        || routing.contract_version != 2
        || routing.policy.contract_version != 2
        || routing.policy.project_id != routing.project_id
        || routing.policy.tracker_instance_id != routing.tracker_instance_id
        || routing.policy.routing_hash != crate::sdlc::canonical_hash(&routing.policy.routes)?
        || routing
            .policy
            .routes
            .entries()
            .iter()
            .any(|(_, route)| route.instruction_profile.as_deref() != Some("single-task-v1"))
        || task.owner_subject.trim().is_empty()
        || config.reservation_scheduler_subject.trim().is_empty()
        || !positive(task.version)
        || !positive(routing.requirement_revision)
        || !sha256(&routing.content_hash)
    {
        return Err(conflict());
    }
    Ok(())
}

/// Expiry blocks new agent proposals/holds. Terminal reconciliation still needs exact custody,
/// but may happen after expiry; expiry never supplies terminal evidence.
fn current_custody<'a>(
    current: &'a LifecycleReceipt,
    fence: &DeliveryFence,
    holder: &str,
    live_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<(&'a DeliveryAssignment, &'a RunBinding), AppError> {
    crate::sdlc_lifecycle::validate_fence(fence)?;
    let task = &current.task;
    let assignment = current.assignment.as_ref().ok_or_else(conflict)?;
    let lease = current.lease.as_ref().ok_or_else(conflict)?;
    let binding = current.binding.as_ref().ok_or_else(conflict)?;
    let route = task
        .routing
        .policy
        .routes
        .entries()
        .into_iter()
        .find(|(role, _)| *role == assignment.role.key())
        .map(|(_, route)| route)
        .ok_or_else(conflict)?;
    if !matches!(task.runtime, RuntimeState::Running | RuntimeState::Unknown)
        || task.status != LifecycleStatus::InProgress
        || task.stage.role() != Some(assignment.role)
        || assignment.task_id != task.task_id
        || assignment.owner_subject != task.owner_subject
        || assignment.stage != task.stage
        || assignment.cycle != task.cycle
        || assignment.routing != task.routing
        || assignment.agent_id.is_nil()
        || assignment.agent_id != route.agent_id
        || assignment.assignment_id != fence.assignment_id
        || assignment.execution_id != fence.execution_id
        || assignment.fencing_token != fence.fencing_token
        || assignment.lease_id != fence.lease_id
        || lease.lease_id != fence.lease_id
        || lease.version != fence.expected_lease_version
        || lease.holder_subject != holder
        || lease.claimed_at > lease.heartbeat_at
        || lease.heartbeat_at >= lease.expires_at
        || live_at.is_some_and(|now| now < lease.heartbeat_at || now >= lease.expires_at)
        || binding.chat_id.is_nil()
        || binding.run_id.is_nil()
        || binding.machine_subject.trim().is_empty()
        || assignment.assignment_hash != crate::sdlc_lifecycle::assignment_hash(assignment)?
    {
        return Err(conflict());
    }
    crate::sdlc_lifecycle::validate_artifact(&binding.dispatch_receipt)?;
    Ok((assignment, binding))
}

fn prepare_blocker(
    current: &LifecycleReceipt,
    actor: &Principal,
    config: &SdlcConfig,
    proposal: &SubmitCriticalBlocker,
    allocation: BlockerAllocation,
) -> Result<CriticalBlockerReceipt, AppError> {
    configured(current, config)?;
    let grant = crate::sdlc_lifecycle::producer(actor, true)?;
    let (assignment, binding) = current_custody(
        current,
        &proposal.fence,
        &config.reservation_scheduler_subject,
        Some(allocation.observed_at),
    )?;
    crate::sdlc_lifecycle::authorize_delivery_assignment(&grant, assignment)?;
    if actor.subject != binding.machine_subject {
        return Err(AppError::Forbidden);
    }
    if current.task.runtime != RuntimeState::Running {
        return Err(conflict());
    }
    ensure_technical_progress_allowed(current, None)?;
    crate::sdlc::validate_key(&proposal.idempotency_key)?;
    proposal.validate(assignment.role)?;
    let routing = &current.task.routing;
    if allocation.blocker_id.is_nil()
        || !positive(allocation.routing_generation)
        || proposal.requirement_revision != routing.requirement_revision
        || proposal.content_hash != routing.content_hash
    {
        return Err(conflict());
    }
    Ok(CriticalBlockerReceipt {
        blocker_id: allocation.blocker_id,
        version: 1,
        tracker_instance_id: routing.tracker_instance_id.clone(),
        project_id: routing.project_id,
        task_id: current.task.task_id,
        root_task_id: current.task.task_id,
        owner_subject: current.task.owner_subject.clone(),
        producer_subject: actor.subject.clone(),
        agent_id: assignment.agent_id,
        role: assignment.role,
        assignment_hash: assignment.assignment_hash.clone(),
        chat_id: binding.chat_id,
        run_id: binding.run_id,
        routing_snapshot_id: routing.snapshot_id,
        routing_generation: allocation.routing_generation,
        proposal: proposal.clone(),
        blocker_sha256: crate::sdlc::canonical_hash(proposal)?,
        state: CriticalBlockerState::Proposed,
        pm_review: None,
        resolution: None,
        technical_terminal: None,
        created_at: allocation.observed_at,
    })
}

/// The current technical grant requests waiting, not scheduler impersonation or custody release.
/// Persist the proposal, held projection and ORIGINAL held receipt in ONE task transaction.
pub fn prepare_and_hold(
    current: &LifecycleReceipt,
    actor: &Principal,
    config: &SdlcConfig,
    proposal: &SubmitCriticalBlocker,
    allocation: BlockerAllocation,
) -> Result<CriticalPolicyChange, AppError> {
    let blocker = prepare_blocker(current, actor, config, proposal, allocation)?;
    hold_prepared(current, blocker)
}

fn hold_prepared(
    current: &LifecycleReceipt,
    mut blocker: CriticalBlockerReceipt,
) -> Result<CriticalPolicyChange, AppError> {
    let mut lifecycle = current.clone();
    lifecycle.task.version = next_version(current.task.version)?;
    lifecycle.task.waiting_reason = Some(HOLD_REASON.into());
    // Proposed is an in-memory preparation only: the first persisted/returned state is Held v1.
    blocker.state = CriticalBlockerState::Held;
    Ok(CriticalPolicyChange { lifecycle, blocker })
}

/// Invoke only after fresh repository ACL/operation authorization, before fresh-write guards.
/// `saved` is the immutable journal result, not the mutable detail projection.
pub fn replay_blocker(
    saved: &CriticalBlockerReceipt,
    proposal: &SubmitCriticalBlocker,
) -> Result<CriticalBlockerReceipt, AppError> {
    if saved.state != CriticalBlockerState::Held
        || saved.version != 1
        || saved.pm_review.is_some()
        || saved.resolution.is_some()
        || saved.technical_terminal.is_some()
        || saved.proposal != *proposal
        || saved.blocker_sha256 != crate::sdlc::canonical_hash(proposal)?
    {
        return Err(AppError::conflict(
            "critical blocker idempotency payload mismatch",
        ));
    }
    Ok(saved.clone())
}

/// Required inside all existing Complete/Reserve/EnsureQueued/Reconcile-progress transactions.
/// Heartbeat and exact terminal handoff remain available while held.
pub fn ensure_technical_progress_allowed(
    current: &LifecycleReceipt,
    blocker: Option<&CriticalBlockerReceipt>,
) -> Result<(), AppError> {
    if current
        .task
        .waiting_reason
        .as_deref()
        .is_some_and(|reason| reason == HOLD_REASON || reason == PM_REVIEW_REASON)
        || blocker.is_some_and(|b| {
            matches!(
                b.state,
                CriticalBlockerState::Held
                    | CriticalBlockerState::AwaitingPmReview
                    | CriticalBlockerState::InPmReview
            )
        })
    {
        return Err(AppError::conflict(
            "critical PM review holds technical progress",
        ));
    }
    Ok(())
}

fn match_blocker(
    current: &LifecycleReceipt,
    blocker: &CriticalBlockerReceipt,
    command: &CriticalPmReviewGuard,
) -> Result<(), AppError> {
    crate::sdlc::validate_key(&command.idempotency_key)?;
    let assignment = current.assignment.as_ref().ok_or_else(conflict)?;
    let binding = current.binding.as_ref().ok_or_else(conflict)?;
    let source = &blocker.proposal.fence;
    let routing = &current.task.routing;
    blocker.proposal.validate(blocker.role)?;
    crate::sdlc_lifecycle::validate_fence(source)?;
    if blocker.blocker_id.is_nil()
        || command.blocker_id != blocker.blocker_id
        || command.blocker_sha256 != blocker.blocker_sha256
        || !positive(command.expected_blocker_version)
        || command.expected_blocker_version != blocker.version
        || !positive(command.expected_task_version)
        || command.expected_task_version != current.task.version
        || blocker.blocker_sha256 != crate::sdlc::canonical_hash(&blocker.proposal)?
        || blocker.tracker_instance_id != routing.tracker_instance_id
        || blocker.project_id != routing.project_id
        || blocker.task_id != current.task.task_id
        || blocker.root_task_id != current.task.task_id
        || blocker.owner_subject != current.task.owner_subject
        || blocker.producer_subject != binding.machine_subject
        || blocker.chat_id != binding.chat_id
        || blocker.run_id != binding.run_id
        || blocker.role != assignment.role
        || blocker.agent_id != assignment.agent_id
        || blocker.assignment_hash != assignment.assignment_hash
        || blocker.routing_snapshot_id != routing.snapshot_id
        || !positive(blocker.routing_generation)
        || blocker.proposal.requirement_revision != routing.requirement_revision
        || blocker.proposal.content_hash != routing.content_hash
        || source.assignment_id != assignment.assignment_id
        || source.execution_id != assignment.execution_id
        || source.fencing_token != assignment.fencing_token
        || source.lease_id != assignment.lease_id
        || blocker.pm_review.is_some()
        || blocker.resolution.is_some()
        || blocker.technical_terminal.is_some()
    {
        return Err(conflict());
    }
    Ok(())
}

/// The existing trusted scheduler must first read actual native terminal bytes. This checks
/// exact readback identity; a receipt-shaped string alone is NOT independent terminal proof.
pub fn return_to_pm(
    current: &LifecycleReceipt,
    blocker: &CriticalBlockerReceipt,
    actor: &Principal,
    config: &SdlcConfig,
    command: &ReturnCriticalToPm,
) -> Result<CriticalPolicyChange, AppError> {
    crate::sdlc_reservation::scheduler(actor, config)?;
    configured(current, config)?;
    current_custody(current, &command.review.fence, &actor.subject, None)?;
    match_blocker(current, blocker, &command.review)?;
    crate::sdlc_lifecycle::validate_artifact(&command.terminal.receipt)?;
    if blocker.state != CriticalBlockerState::Held
        || current.task.waiting_reason.as_deref() != Some(HOLD_REASON)
        || command.terminal.assignment_hash != blocker.assignment_hash
        || command.terminal.chat_id != blocker.chat_id
        || command.terminal.run_id != blocker.run_id
    {
        return Err(conflict());
    }
    let mut next = CriticalPolicyChange {
        lifecycle: current.clone(),
        blocker: blocker.clone(),
    };
    next.lifecycle.task.version = next_version(current.task.version)?;
    next.lifecycle.task.stage = LifecycleStage::Draft;
    next.lifecycle.task.status = LifecycleStatus::Ready;
    // PM work is discovered through the existing PM journal, never the technical queue.
    next.lifecycle.task.runtime = RuntimeState::Terminal;
    next.lifecycle.task.waiting_reason = Some(PM_REVIEW_REASON.into());
    next.lifecycle.task.previous_result_id = None;
    next.lifecycle.task.resume_chat_id = None;
    next.lifecycle.assignment = None;
    next.lifecycle.lease = None;
    next.lifecycle.binding = None;
    next.lifecycle.result = None;
    next.blocker.version = next_version(blocker.version)?;
    next.blocker.state = CriticalBlockerState::AwaitingPmReview;
    next.blocker.technical_terminal = Some(command.terminal.clone());
    Ok(next)
}

#[cfg(test)]
#[path = "sdlc_critical_blockers_tests.rs"]
mod tests;
