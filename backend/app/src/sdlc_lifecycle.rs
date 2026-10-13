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

pub fn producer(actor: &Principal, write: bool) -> Result<(), AppError> {
    let required = if write {
        "task-tracker:write"
    } else {
        "task-tracker:read"
    };
    if actor.human_session
        || !actor.scopes.contains(required)
        || actor
            .scopes
            .iter()
            .any(|scope| scope != "task-tracker:read" && scope != "task-tracker:write")
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub fn assignment_hash(assignment: &DeliveryAssignment) -> Result<String, AppError> {
    let mut value = serde_json::to_value(assignment).map_err(AppError::internal)?;
    value
        .as_object_mut()
        .ok_or_else(|| AppError::internal("assignment object absent"))?
        .remove("assignment_hash");
    crate::sdlc::canonical_hash(&value)
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

#[cfg(test)]
#[path = "sdlc_lifecycle_tests.rs"]
mod tests;
