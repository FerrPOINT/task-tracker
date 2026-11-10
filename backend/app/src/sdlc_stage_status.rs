use chrono::{DateTime, Utc};
use domain::sdlc::{Principal, Stage, TaskState};
use domain::sdlc_lifecycle::{LifecycleReceipt, LifecycleStatus, RuntimeState};
use domain::sdlc_stage_status::{StageStartFence, StageStartReceipt, StartStage};
use shared::AppError;
use uuid::Uuid;

impl crate::sdlc::SdlcService {
    pub async fn start_stage(
        &self,
        task: Uuid,
        actor: &Principal,
        command: StartStage,
    ) -> Result<(StageStartReceipt, bool), AppError> {
        crate::sdlc::validate_key(&command.idempotency_key)?;
        self.repository.start_stage(task, actor, command).await
    }

    pub async fn stage_start_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<StageStartReceipt, AppError> {
        crate::sdlc::validate_key(key)?;
        self.repository
            .stage_start_operation(task, actor, key)
            .await
    }
}

/// Only a new start uses current custody; exact-key replay reads the saved receipt.
pub fn prepare_stage_start(
    state: &TaskState,
    actor: &Principal,
    command: &StartStage,
    delivery: Option<&LifecycleReceipt>,
    observed_at: DateTime<Utc>,
) -> Result<StageStartReceipt, AppError> {
    crate::sdlc::validate_key(&command.idempotency_key)?;
    crate::sdlc_configured_cli::authorize_configured_identity(actor)?;
    state.require_root()?;
    if !state.configured_cli() || state.status != Some(LifecycleStatus::Ready) {
        return Err(AppError::conflict(
            "stage start requires the current Ready stage",
        ));
    }
    let execution_id = match &command.fence {
        StageStartFence::Pm(fence) => {
            crate::sdlc_configured_cli::authorize_configured_pm(state, actor)?;
            let assignment = state.assignment.as_ref().ok_or(AppError::Forbidden)?;
            if state.stage != Stage::Draft
                || delivery.is_some()
                || fence.assignment_id.is_nil()
                || fence.execution_id.is_nil()
                || fence.agent_id.is_nil()
                || !(1..=domain::sdlc::MAX_SAFE_VERSION).contains(&fence.assignment_version)
                || fence.assignment_id != assignment.assignment_id
                || fence.execution_id != assignment.execution_id
                || fence.agent_id != assignment.agent_id
                || fence.assignment_version != assignment.version
            {
                return Err(AppError::conflict("PM stage start fence differs"));
            }
            assignment.execution_id
        }
        StageStartFence::Delivery(fence) => {
            crate::sdlc_lifecycle::validate_fence(fence)?;
            let receipt = delivery.ok_or_else(|| AppError::conflict("delivery custody missing"))?;
            let assignment = receipt.assignment.as_ref().ok_or(AppError::Forbidden)?;
            let lease = receipt.lease.as_ref().ok_or(AppError::Forbidden)?;
            let binding = receipt.binding.as_ref().ok_or(AppError::Forbidden)?;
            if receipt.task.task_id != state.task_id
                || receipt.task.owner_subject != state.owner_subject
                || receipt.task.stage.canonical() != state.stage
                || receipt.task.status != LifecycleStatus::Ready
                || receipt.task.runtime != RuntimeState::Running
                || assignment.task_id != state.task_id
                || assignment.owner_subject != state.owner_subject
                || assignment.cycle != receipt.task.cycle
                || assignment.routing != receipt.task.routing
                || assignment.assignment_hash != crate::sdlc_lifecycle::assignment_hash(assignment)?
                || assignment.stage != receipt.task.stage
                || assignment.role != receipt.task.stage.role().ok_or(AppError::Forbidden)?
                || assignment.assignment_id != fence.assignment_id
                || assignment.execution_id != fence.execution_id
                || assignment.fencing_token != fence.fencing_token
                || assignment.lease_id != fence.lease_id
                || lease.lease_id != fence.lease_id
                || lease.version != fence.expected_lease_version
                || binding.machine_subject != actor.subject
                || assignment.machine_subject.as_deref() != Some(actor.subject.as_str())
                || actor.subject == state.owner_subject
                || binding.run_id.is_nil()
                || binding.chat_id.is_nil()
            {
                return Err(AppError::conflict("delivery stage start custody differs"));
            }
            assignment.execution_id
        }
    };
    Ok(StageStartReceipt {
        task_id: state.task_id,
        stage: state.stage,
        status: LifecycleStatus::InProgress,
        execution_id,
        idempotency_key: command.idempotency_key.clone(),
        started_at: observed_at,
    })
}

#[cfg(test)]
#[path = "agent_stage_status_tests.rs"]
mod tests;
