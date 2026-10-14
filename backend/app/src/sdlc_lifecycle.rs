use domain::sdlc::{MAX_SAFE_VERSION, Principal};
use domain::sdlc_lifecycle::*;
use shared::AppError;
use uuid::Uuid;

impl crate::sdlc::SdlcService {
    pub async fn submit_pm_feedback(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SubmitPmFeedback,
    ) -> Result<(PmFeedbackReceipt, bool), AppError> {
        self.lifecycle_repository()?
            .submit_pm_feedback(task, actor, command)
            .await
    }

    pub async fn pm_feedback_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<PmFeedbackReceipt, AppError> {
        self.lifecycle_repository()?
            .pm_feedback_operation(task, actor, key)
            .await
    }

    pub async fn pm_feedback_message(
        &self,
        task: Uuid,
        actor: &Principal,
        message_id: Uuid,
    ) -> Result<PmFeedbackReceipt, AppError> {
        authorize_pm_feedback_message_read(task, actor)?;
        if message_id.is_nil() {
            return Err(AppError::validation("invalid PM feedback message ID"));
        }
        self.lifecycle_repository()?
            .pm_feedback_message(task, actor, message_id)
            .await
    }

    fn lifecycle_repository(&self) -> Result<&dyn LifecycleRepository, AppError> {
        self.repository.lifecycle_repository().ok_or_else(|| {
            AppError::Unavailable("single-task lifecycle repository not configured".into())
        })
    }

    pub async fn lifecycle_command(
        &self,
        task: Uuid,
        actor: &Principal,
        command: LifecycleCommand,
    ) -> Result<(LifecycleReceipt, bool), AppError> {
        validate_command(&command)?;
        self.lifecycle_repository()?
            .lifecycle_command(task, actor, command)
            .await
    }

    pub async fn submit_delivery_proposal(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SubmitDeliveryProposal,
    ) -> Result<(DeliveryProposalReceipt, bool), AppError> {
        self.lifecycle_repository()?
            .submit_delivery_proposal(task, actor, command)
            .await
    }

    pub async fn delivery_proposal_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<DeliveryProposalReceipt, AppError> {
        self.lifecycle_repository()?
            .delivery_proposal_operation(task, actor, key)
            .await
    }

    pub async fn lifecycle_read(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<LifecycleReadback, AppError> {
        self.lifecycle_repository()?
            .lifecycle_read(task, actor)
            .await
    }

    pub async fn lifecycle_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<LifecycleReceipt, AppError> {
        self.lifecycle_repository()?
            .lifecycle_operation(task, actor, key)
            .await
    }

    pub async fn lifecycle_result(
        &self,
        task: Uuid,
        actor: &Principal,
        result: Uuid,
    ) -> Result<DeliveryInput, AppError> {
        self.lifecycle_repository()?
            .lifecycle_result(task, actor, result)
            .await
    }
}

pub fn validate_artifact(receipt: &ArtifactReceipt) -> Result<(), AppError> {
    if receipt.reference.trim().is_empty()
        || receipt.reference.len() > 512
        || receipt.reference.chars().any(char::is_control)
        || !sha256(&receipt.sha256)
    {
        return Err(AppError::validation("invalid delivery artifact receipt"));
    }
    Ok(())
}

fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

pub fn validate_command(command: &LifecycleCommand) -> Result<(), AppError> {
    crate::sdlc::validate_key(command.key())?;
    let fence = match command {
        LifecycleCommand::Reconcile {
            fence,
            terminal,
            recovery,
            ..
        } => {
            if terminal.chat_id.is_nil()
                || terminal.run_id.is_nil()
                || !sha256(&terminal.assignment_hash)
            {
                return Err(AppError::validation(
                    "exact actual terminal readback required",
                ));
            }
            validate_artifact(&terminal.receipt)?;
            if let RecoveryDisposition::RetryTransient {
                replay_safety_receipt,
            } = recovery
            {
                if terminal.state != RuntimeTerminalState::Failed {
                    return Err(AppError::conflict(
                        "only replayable transient failure can retry",
                    ));
                }
                validate_artifact(replay_safety_receipt)?;
            }
            fence
        }
        LifecycleCommand::EnsureQueued { .. } => return Ok(()),
        LifecycleCommand::Reserve {
            expected_task_version,
            ..
        } => {
            if !(1..=MAX_SAFE_VERSION).contains(expected_task_version) {
                return Err(AppError::validation("invalid lifecycle CAS"));
            }
            return Ok(());
        }
        LifecycleCommand::Heartbeat { fence, .. } => fence,
        LifecycleCommand::BindRun { fence, binding, .. } => {
            if binding.chat_id.is_nil()
                || binding.run_id.is_nil()
                || binding.machine_subject.trim().is_empty()
                || binding.machine_subject.len() > 256
                || binding.machine_subject.chars().any(char::is_control)
            {
                return Err(AppError::validation("actual chat and run UUID required"));
            }
            validate_artifact(&binding.dispatch_receipt)?;
            fence
        }
        LifecycleCommand::Complete {
            fence,
            terminal,
            result,
            ..
        } => {
            if terminal.chat_id.is_nil() || terminal.run_id.is_nil() {
                return Err(AppError::validation(
                    "actual terminal run identity required",
                ));
            }
            validate_artifact(&terminal.fleet_receipt)?;
            validate_artifact(&terminal.workflow_receipt)?;
            if terminal.output_sha256 != crate::sdlc::canonical_hash(result)? {
                return Err(AppError::validation("terminal output hash mismatch"));
            }
            validate_result(result)?;
            fence
        }
    };
    validate_fence(fence)
}

pub fn validate_fence(fence: &DeliveryFence) -> Result<(), AppError> {
    if fence.assignment_id.is_nil()
        || fence.execution_id.is_nil()
        || fence.lease_id.is_nil()
        || !(1..=MAX_SAFE_VERSION).contains(&fence.fencing_token)
        || !(1..=MAX_SAFE_VERSION).contains(&fence.expected_lease_version)
    {
        return Err(AppError::validation("invalid delivery fence"));
    }
    Ok(())
}

pub fn validate_result(result: &DeliveryResult) -> Result<(), AppError> {
    for artifact in result.artifacts() {
        validate_artifact(artifact)?;
    }
    if let DeliveryResult::Developer {
        commit_sha, checks, ..
    } = result
    {
        if commit_sha.len() != 40
            || !commit_sha
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || checks.is_empty()
            || checks.len() > 100
        {
            return Err(AppError::validation(
                "commit and executed check receipts required",
            ));
        }
    }
    Ok(())
}

pub fn has_delivery_grant(scopes: &std::collections::HashSet<String>) -> bool {
    let prefix = DELIVERY_GRANT_PREFIX.trim_end_matches(':');
    scopes.iter().any(|scope| {
        scope
            .get(..prefix.len())
            .is_some_and(|raw| raw.eq_ignore_ascii_case(prefix))
    })
}

pub fn delivery_grant(actor: &Principal) -> Result<Option<DeliveryGrant>, AppError> {
    if !has_delivery_grant(&actor.scopes) {
        return Ok(None);
    }
    let mut grants = actor
        .scopes
        .iter()
        .filter(|scope| scope.starts_with("task-tracker:sdlc:delivery"));
    let raw = grants.next().ok_or(AppError::Forbidden)?;
    if grants.next().is_some()
        || actor.human_session
        || actor.scopes.iter().any(|scope| {
            scope != raw && scope != "task-tracker:read" && scope != "task-tracker:write"
        })
    {
        return Err(AppError::Forbidden);
    }
    let parts: Vec<_> = raw
        .strip_prefix(DELIVERY_GRANT_PREFIX)
        .ok_or(AppError::Forbidden)?
        .split(':')
        .collect();
    if parts.len() != 5
        || parts[..4]
            .iter()
            .any(|part| crate::sdlc::canonical_pm_uuid(part).is_none())
        || !crate::sdlc::canonical_pm_version(parts[4])
    {
        return Err(AppError::Forbidden);
    }
    Ok(Some(DeliveryGrant {
        task_id: parts[0].parse().map_err(|_| AppError::Forbidden)?,
        assignment_id: parts[1].parse().map_err(|_| AppError::Forbidden)?,
        execution_id: parts[2].parse().map_err(|_| AppError::Forbidden)?,
        agent_id: parts[3].parse().map_err(|_| AppError::Forbidden)?,
        fencing_token: parts[4].parse().map_err(|_| AppError::Forbidden)?,
    }))
}

pub fn authorize_delivery_assignment(
    grant: &DeliveryGrant,
    assignment: &DeliveryAssignment,
) -> Result<(), AppError> {
    if grant.task_id != assignment.task_id
        || grant.assignment_id != assignment.assignment_id
        || grant.execution_id != assignment.execution_id
        || grant.agent_id != assignment.agent_id
        || grant.fencing_token != assignment.fencing_token
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub fn delivery_request_policy(
    actor: &Principal,
    method: &str,
    path: &str,
) -> Result<(), AppError> {
    let Some(grant) = delivery_grant(actor)? else {
        return Ok(());
    };
    let prefix = format!("/api/v1/issues/{}/sdlc/", grant.task_id);
    let resource = path.strip_prefix(&prefix).ok_or(AppError::Forbidden)?;
    let parts: Vec<_> = resource.split('/').collect();
    let allowed = match (method, parts.as_slice()) {
        ("GET", ["lifecycle" | "context" | "requirements" | "events"]) => true,
        ("GET", ["lifecycle", "results", id]) => {
            Uuid::parse_str(id).is_ok_and(|value| !value.is_nil() && value.to_string() == *id)
        }
        ("GET", ["requirements", revision]) => crate::sdlc::canonical_pm_version(revision),
        ("GET", ["delivery-proposals", "operations", key]) => !key.is_empty(),
        ("POST", ["delivery-proposals"]) => true,
        _ => false,
    };
    if allowed {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

pub fn producer(actor: &Principal, write: bool) -> Result<DeliveryGrant, AppError> {
    let required = if write {
        "task-tracker:write"
    } else {
        "task-tracker:read"
    };
    if !actor.scopes.contains(required) {
        return Err(AppError::Forbidden);
    }
    delivery_grant(actor)?.ok_or(AppError::Forbidden)
}

pub fn scheduler_read(
    actor: &Principal,
    config: &domain::sdlc::SdlcConfig,
) -> Result<(), AppError> {
    crate::sdlc_reservation::reader(actor)?;
    if config.reservation_scheduler_subject.trim().is_empty() {
        return Err(AppError::Unavailable(
            "reservation scheduler identity not configured".into(),
        ));
    }
    if actor.human_session
        || actor.subject != config.reservation_scheduler_subject
        || actor.subject == config.orchestrator_subject
        || actor.subject == config.verifier_subject
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub fn assignment_hash(assignment: &DeliveryAssignment) -> Result<String, AppError> {
    if let Some(execution) = &assignment.execution {
        execution.validate()?;
    }
    let mut value = serde_json::to_value(assignment).map_err(AppError::internal)?;
    value
        .as_object_mut()
        .ok_or_else(|| AppError::internal("assignment object absent"))?
        .remove("assignment_hash");
    crate::sdlc::canonical_hash(&value)
}

pub fn canonical_projection(
    state: &serde_json::Value,
    task: &LifecycleTask,
) -> Result<serde_json::Value, AppError> {
    let mut next = state.clone();
    let object = next
        .as_object_mut()
        .ok_or_else(|| AppError::internal("canonical task object absent"))?;
    object.insert(
        "stage".into(),
        serde_json::to_value(task.stage.canonical()).map_err(AppError::internal)?,
    );
    object.insert(
        "status".into(),
        serde_json::to_value(task.status).map_err(AppError::internal)?,
    );
    object.insert(
        "waiting_reason".into(),
        serde_json::to_value(&task.waiting_reason).map_err(AppError::internal)?,
    );
    object.insert(
        "runtime_state".into(),
        serde_json::to_value(task.runtime).map_err(AppError::internal)?,
    );
    object.insert(
        "lifecycle".into(),
        serde_json::to_value(task).map_err(AppError::internal)?,
    );
    Ok(next)
}

pub fn issue_mapping_keys(task: &LifecycleTask) -> Result<(String, String), AppError> {
    let stage = serde_json::to_value(task.stage).map_err(AppError::internal)?;
    let status = serde_json::to_value(task.status).map_err(AppError::internal)?;
    Ok((
        stage
            .as_str()
            .ok_or_else(|| AppError::internal("stage key absent"))?
            .into(),
        status
            .as_str()
            .ok_or_else(|| AppError::internal("status key absent"))?
            .into(),
    ))
}

/// Apply only after the repository accepts the exact live fence and terminal run.
pub fn accept_result(task: &mut LifecycleTask, result: &DeliveryResult) -> Result<(), AppError> {
    if task.status != LifecycleStatus::InProgress
        || task.runtime != RuntimeState::Running
        || task.cycle > MAX_REWORK_CYCLES
        || task.stage.role() != Some(result.role())
    {
        return Err(AppError::conflict(
            "result does not match running task role",
        ));
    }
    let next = match result {
        DeliveryResult::Analyst { .. } => LifecycleStage::Architecture,
        DeliveryResult::Architect { .. } => LifecycleStage::Development,
        DeliveryResult::Developer { .. } => LifecycleStage::Review,
        DeliveryResult::Reviewer {
            verdict: ReviewVerdict::Approved,
            ..
        } => LifecycleStage::Testing,
        DeliveryResult::Tester {
            verdict: TestVerdict::Passed,
            ..
        } => LifecycleStage::Deployment,
        DeliveryResult::Reviewer {
            verdict: ReviewVerdict::Rework,
            ..
        }
        | DeliveryResult::Tester {
            verdict: TestVerdict::Rework,
            ..
        } => LifecycleStage::Rework,
        DeliveryResult::DevOps { .. } => LifecycleStage::Deployment,
    };
    let version = task
        .version
        .checked_add(1)
        .filter(|v| *v <= MAX_SAFE_VERSION)
        .ok_or_else(|| AppError::conflict("lifecycle version exhausted"))?;
    task.version = version;
    task.runtime = RuntimeState::Terminal;
    task.transient_retries = 0;
    task.resume_chat_id = None;
    if next == LifecycleStage::Rework && task.cycle == MAX_REWORK_CYCLES {
        // Keep the rejected stage; never silently mark a failed task completed.
        task.status = LifecycleStatus::Ready;
        task.waiting_reason = Some("rework_limit_reached".into());
        return Ok(());
    }
    if next == LifecycleStage::Rework {
        task.cycle += 1;
    }
    task.stage = next;
    if matches!(result, DeliveryResult::DevOps { .. }) {
        task.status = LifecycleStatus::Completed;
        task.waiting_reason = None;
    } else {
        task.status = LifecycleStatus::Ready;
        task.runtime = RuntimeState::Queued;
        task.waiting_reason = Some("queued_for_assignment".into());
    }
    Ok(())
}

/// A transport error after dispatch is unknown acceptance, never a retry permit.
pub fn transient_retry_allowed(retries: u8, safely_replayable: bool) -> bool {
    safely_replayable && retries < MAX_TRANSIENT_RETRIES
}

pub fn reconcile_terminal(
    task: &mut LifecycleTask,
    terminal: &RuntimeTerminalObservation,
    recovery: &RecoveryDisposition,
) -> Result<(), AppError> {
    if !matches!(
        task.runtime,
        RuntimeState::Running | RuntimeState::Reserved | RuntimeState::Unknown
    ) {
        return Err(AppError::conflict("current unresolved execution required"));
    }
    let version = task
        .version
        .checked_add(1)
        .filter(|v| *v <= MAX_SAFE_VERSION)
        .ok_or_else(|| AppError::conflict("task version exhausted"))?;
    if let RecoveryDisposition::RetryTransient { .. } = recovery {
        if terminal.state != RuntimeTerminalState::Failed {
            return Err(AppError::conflict("transient failure readback required"));
        }
    }
    task.version = version;
    task.status = LifecycleStatus::Ready;
    task.resume_chat_id = Some(terminal.chat_id);
    if matches!(recovery, RecoveryDisposition::RetryTransient { .. })
        && task.transient_retries < MAX_TRANSIENT_RETRIES
    {
        task.transient_retries += 1;
        task.runtime = RuntimeState::Queued;
        task.waiting_reason = Some("transient_retry_queued".into());
    } else {
        task.runtime = RuntimeState::Terminal;
        task.waiting_reason = Some(
            if matches!(recovery, RecoveryDisposition::RetryTransient { .. }) {
                "transient_retry_limit_reached"
            } else {
                "runtime_terminal_requires_intervention"
            }
            .into(),
        );
    }
    Ok(())
}

/// Called only after the PM grant's own-task prefix has been validated.
pub fn is_pm_feedback_message_read(method: &str, resource: &[&str]) -> bool {
    matches!((method, resource), ("GET", ["pm-feedback", "messages", id])
        if crate::sdlc::canonical_pm_uuid(id).is_some())
}

pub fn authorize_pm_feedback_message_read(task: Uuid, actor: &Principal) -> Result<(), AppError> {
    if crate::sdlc::pm_grant_task(actor)? != Some(task)
        || !actor.scopes.contains("task-tracker:read")
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub fn exact_pm_feedback_message(
    state: &domain::sdlc::TaskState,
    message_id: Uuid,
    saved_owner: &str,
    receipt: &PmFeedbackReceipt,
) -> Result<PmFeedbackReceipt, AppError> {
    state.require_root()?;
    let assignment = state.assignment.as_ref().ok_or(AppError::Forbidden)?;
    let fence = domain::sdlc::MachineFence {
        assignment_id: assignment.assignment_id,
        execution_id: assignment.execution_id,
        agent_id: assignment.agent_id,
        assignment_version: assignment.version,
    };
    if message_id.is_nil()
        || receipt.message_id != message_id
        || receipt.tracker_instance_id != state.tracker_instance_id
        || receipt.project_id != state.project_id
        || receipt.task_id != state.task_id
        || receipt.root_task_id != state.root_task_id
        || receipt.owner_subject != state.owner_subject
        || saved_owner != state.owner_subject
        || receipt.fence != fence
        || receipt.text.trim().is_empty()
        || receipt.text.len() > 100_000
        || receipt.text.contains('\0')
        || receipt.content_hash
            != crate::sdlc::canonical_hash(&serde_json::json!({"text":receipt.text}))?
        || receipt.requirement_revision.is_some_and(|revision| {
            !state
                .revisions
                .iter()
                .any(|saved| saved.revision == revision)
        })
    {
        return Err(AppError::conflict("pm_feedback_history_inconsistent"));
    }
    Ok(receipt.clone())
}

/// New input only: historical operation replay must run before this gate.
pub fn validate_new_pm_feedback(state: &domain::sdlc::TaskState) -> Result<(), AppError> {
    use domain::sdlc::{DraftSubmode, QuestionState, Stage};
    state.require_root()?;
    let marked = state.draft_flow()?.is_some();
    let stage_allowed =
        state.stage == Stage::Draft || (!marked && state.stage == Stage::Clarification);
    let status_allowed = matches!(
        state.status,
        Some(LifecycleStatus::Ready | LifecycleStatus::InProgress)
    ) || (!marked && state.status.is_none());
    let mode_allowed = matches!(
        state.draft_submode,
        Some(DraftSubmode::Brainstorming | DraftSubmode::AwaitingConfirmation)
    ) || (!marked && state.draft_submode.is_none());
    if !stage_allowed
        || !status_allowed
        || !mode_allowed
        || state
            .questions
            .iter()
            .any(|q| q.state == QuestionState::Open)
    {
        return Err(AppError::conflict(
            "pm_feedback_not_available_in_current_draft_mode",
        ));
    }
    Ok(())
}

pub fn pm_feedback_resume_contract(
    state: &domain::sdlc::TaskState,
    feedback: &PmFeedbackReceipt,
    checkpoint: &PmFeedbackCheckpoint,
    lease: &domain::sdlc_execution_lease::ExecutionLeaseReadback,
) -> Result<PmFeedbackResumeContract, AppError> {
    state.require_root()?;
    let assignment = state
        .assignment
        .as_ref()
        .ok_or_else(|| AppError::conflict("current PM assignment required"))?;
    if !matches!(
        state.stage,
        domain::sdlc::Stage::Draft | domain::sdlc::Stage::Clarification
    ) || feedback.task_id != state.task_id
        || feedback.project_id != state.project_id
        || feedback.root_task_id != state.task_id
        || feedback.owner_subject != state.owner_subject
        || feedback.tracker_instance_id != state.tracker_instance_id
        || feedback.fence != checkpoint.fence
        || lease.fence != checkpoint.fence
        || lease.binding.task_id != state.task_id
        || lease.binding.root_task_id != state.task_id
        || lease.binding.project_id != state.project_id
        || lease.binding.tracker_instance_id != state.tracker_instance_id
        || lease.binding.owner_subject != state.owner_subject
        || checkpoint.fence.assignment_id != assignment.assignment_id
        || checkpoint.fence.execution_id != assignment.execution_id
        || checkpoint.fence.agent_id != assignment.agent_id
        || checkpoint.fence.assignment_version != assignment.version
        || checkpoint.checkpoint_id.is_nil()
        || checkpoint.previous_run_id.is_nil()
        || !(1..=MAX_SAFE_VERSION).contains(&checkpoint.checkpoint_version)
    {
        return Err(AppError::conflict(
            "PM feedback checkpoint/current execution mismatch or business lifecycle frozen",
        ));
    }
    validate_artifact(&checkpoint.previous_run_terminal)?;
    let current = lease
        .current
        .as_ref()
        .ok_or_else(|| AppError::conflict("PM feedback lease readback absent"))?;
    if current.holder_subject != assignment.machine_subject {
        return Err(AppError::Forbidden);
    }
    if current.expires_at <= lease.observed_at
        || !matches!(
            lease.state,
            domain::sdlc_execution_lease::ExecutionLeaseState::Active
        )
    {
        return Err(AppError::conflict(
            "pm_feedback_expired_lease_requires_guarded_resume_reconciliation",
        ));
    }
    Ok(PmFeedbackResumeContract {
        feedback_message_id: feedback.message_id,
        fence: checkpoint.fence.clone(),
        checkpoint_id: checkpoint.checkpoint_id,
        checkpoint_version: checkpoint.checkpoint_version,
        previous_run_id: checkpoint.previous_run_id,
        lease_id: current.lease_id,
        lease_version: current.version,
        resume_operation_key: format!("pm-feedback:{}", feedback.message_id),
    })
}

#[cfg(test)]
#[path = "sdlc_lifecycle_tests.rs"]
mod tests;
