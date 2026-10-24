//! Terminal-idle recovery grants only one exact Workflow checkpoint operation.
use domain::sdlc::*;
use domain::sdlc_execution_lease::PmResumeInput;
use domain::sdlc_lifecycle::PmFeedbackReceipt;
use domain::sdlc_pm_draft::PmDraftReservation;
use domain::sdlc_terminal_idle_recovery::*;
use serde_json::{Value, json};
use shared::AppError;
use uuid::Uuid;

fn invalid() -> AppError {
    AppError::conflict("terminal-idle PM checkpoint custody changed")
}

pub fn request_hash(c: &PermitPmIdleCheckpointRequest) -> Result<String, AppError> {
    crate::sdlc::canonical_hash(&json!({"operation":"permit_pm_idle_checkpoint","payload":c}))
}

pub fn validate_command(c: &PermitPmIdleCheckpointRequest) -> Result<(), AppError> {
    crate::sdlc::validate_key(&c.idempotency_key)?;
    crate::sdlc::validate_key(&c.terminal_idle_recovery.checkpoint_operation_key)?;
    crate::sdlc_lifecycle::validate_artifact(&c.previous_run_terminal)?;
    if !matches!(c.input, PmResumeInput::Feedback { .. })
        || [
            c.lease_id,
            c.previous_run_id,
            c.input.input_id(),
            c.fence.assignment_id,
            c.fence.execution_id,
            c.fence.agent_id,
            c.terminal_idle_recovery.barrier_id,
        ]
        .iter()
        .any(Uuid::is_nil)
        || [
            c.expected_owner_version,
            c.expected_lease_version,
            c.fence.assignment_version,
        ]
        .iter()
        .any(|v| !(1..=MAX_SAFE_VERSION).contains(v))
        || c.terminal_idle_recovery.contract_version != 1
        || !domain::sdlc_native_admission::hash(&c.terminal_idle_recovery.checkpoint_sha256)
        || c.idempotency_key
            != format!(
                "fleet-pm-terminal-idle:{}:{}",
                c.previous_run_id,
                c.input.input_id()
            )
        || c.previous_run_terminal.reference
            != format!("/internal/runtime/v1/pm/runs/{}", c.previous_run_id)
    {
        return Err(invalid());
    }
    Ok(())
}

pub fn validate_context(
    state: &TaskState,
    r: &PmDraftReservation,
    c: &PermitPmIdleCheckpointRequest,
) -> Result<(), AppError> {
    validate_command(c)?;
    state.require_root()?;
    if state.stage != Stage::Draft
        || state.draft_submode != Some(DraftSubmode::Brainstorming)
        || state.draft_flow()? != Some(DraftFlow::SingleTaskV1)
        || !state.revisions.is_empty()
        || state.confirmation_revision.is_some()
        || !state.confirmations.is_empty()
        || state
            .questions
            .iter()
            .any(|q| q.state == QuestionState::Open)
        || state.assignment.as_ref() != Some(&r.assignment)
        || r.binding.task_id != state.task_id
        || r.binding.root_task_id != state.root_task_id
        || r.binding.owner_subject != state.owner_subject
        || r.binding.project_id != state.project_id
        || r.binding.tracker_instance_id != state.tracker_instance_id
        || r.contract_version != 1
        || r.dispatch_allowed
        || r.owner_cas.version != c.expected_owner_version
        || r.assignment.assignment_id != c.fence.assignment_id
        || r.assignment.execution_id != c.fence.execution_id
        || r.assignment.agent_id != c.fence.agent_id
        || r.assignment.version != c.fence.assignment_version
    {
        return Err(invalid());
    }
    Ok(())
}

pub fn validate_feedback(
    state: &TaskState,
    c: &PermitPmIdleCheckpointRequest,
    f: &PmFeedbackReceipt,
) -> Result<(), AppError> {
    if f.message_id != c.input.input_id()
        || f.tracker_instance_id != state.tracker_instance_id
        || f.project_id != state.project_id
        || f.task_id != state.task_id
        || f.root_task_id != state.task_id
        || f.owner_subject != state.owner_subject
        || f.fence != c.fence
        || f.requirement_revision.is_some()
        || f.content_hash != crate::sdlc::canonical_hash(&json!({"text":f.text}))?
    {
        return Err(invalid());
    }
    Ok(())
}

pub fn validate_witness(
    w: &PmTerminalIdleRecoveryWitness,
    state: &TaskState,
    r: &PmDraftReservation,
    c: &PermitPmIdleCheckpointRequest,
    feedback: &PmFeedbackReceipt,
) -> Result<(), AppError> {
    validate_context(state, r, c)?;
    validate_feedback(state, c, feedback)?;
    crate::sdlc_lifecycle::validate_artifact(&w.feedback_receipt)?;
    let identity = &w.identity;
    let native = &w.native_terminal;
    let body = &w.checkpoint_request.body;
    let typed = &c.terminal_idle_recovery;
    if w.contract_version != 1
        || w.barrier_id != typed.barrier_id
        || w.owner_subject != state.owner_subject
        || w.request_sha256 != request_hash(c)?
        || serde_json::to_value(&w.command).map_err(AppError::internal)?
            != serde_json::to_value(c).map_err(AppError::internal)?
        || identity.task != r.execution.key
        || identity.execution_ref != r.assignment.execution_id.to_string()
        || identity.assignment_ref != r.assignment.assignment_id.to_string()
        || identity.assignment_revision != r.assignment.version
        || identity.agent_ref != r.assignment.agent_id.to_string()
        || identity.task_ref != state.task_id.to_string()
        || identity.root_ref != state.root_task_id.to_string()
        || identity.tracker_project_ref != state.project_id.to_string()
        || identity.tracker_instance_ref != state.tracker_instance_id
        || identity.assignment_operation_key != r.assignment_operation_key
        || native.identity != *identity
        || native.status != PmIdleNativeStatus::Completed
        || native.observation_ref.is_nil()
        || native.session_run_id != c.previous_run_id
        || native.checkpoint_ref.is_some()
        || !(1..=MAX_SAFE_VERSION).contains(&native.fence)
        || native.hermes_run_ref.is_empty()
        || native.hermes_run_ref.len() > 512
        || native.hermes_run_ref.chars().any(char::is_control)
        || native.binding_ref != format!("fleet:pm:{}", c.previous_run_id)
        || native.dispatch_operation_key != format!("fleet-pm-bind:{}", c.previous_run_id)
        || c.previous_run_terminal.sha256
            != crate::sdlc::canonical_hash(
                &serde_json::to_value(native).map_err(AppError::internal)?,
            )?
        || w.feedback_receipt.sha256
            != crate::sdlc::canonical_hash(
                &serde_json::to_value(feedback).map_err(AppError::internal)?,
            )?
        || w.instruction_operation_key != format!("fleet-pm-first-step:{}", c.previous_run_id)
        || !domain::sdlc_native_admission::hash(&w.instruction_request_sha256)
        || !domain::sdlc_native_admission::hash(&w.instruction_result_sha256)
        || w.checkpoint_request.operation_key != typed.checkpoint_operation_key
        || crate::sdlc::canonical_hash(body)? != typed.checkpoint_sha256
        || !body.is_object()
        || body["operation_key"] != typed.checkpoint_operation_key
        || body["session_run_id"] != json!(c.previous_run_id)
        || body["binding_ref"] != native.binding_ref
        || body["hermes_run_ref"] != native.hermes_run_ref
        || body["expected_fence"] != native.fence
        || body["expected_version"]
            .as_i64()
            .is_none_or(|n| !(1..MAX_SAFE_VERSION).contains(&n))
        || body["instruction_profile"] != "single-task-v1"
        || body["instruction_manifest_sha256"]
            .as_str()
            .is_none_or(|v| !domain::sdlc_native_admission::hash(v))
        || [
            "requirements_revision",
            "requirements_sha256",
            "clarification_request_ref",
            "clarification_version",
        ]
        .iter()
        .any(|key| body.get(*key) != Some(&Value::Null))
    {
        return Err(invalid());
    }
    let checkpoint = body["checkpoint_ref"].as_str().ok_or_else(invalid)?;
    let id = Uuid::parse_str(checkpoint).map_err(|_| invalid())?;
    if id.is_nil()
        || checkpoint != id.to_string()
        || typed.checkpoint_operation_key != format!("fleet-pm-checkpoint:{id}")
    {
        return Err(invalid());
    }
    for (key, value) in serde_json::to_value(identity)
        .map_err(AppError::internal)?
        .as_object()
        .ok_or_else(invalid)?
    {
        if body.get(key) != Some(value) {
            return Err(invalid());
        }
    }
    Ok(())
}

pub fn validate_response(
    c: &PermitPmIdleCheckpointRequest,
    response: &PermitPmIdleCheckpointResponse,
) -> Result<(), AppError> {
    validate_command(c)?;
    let p = &response.permit;
    if response.contract_version != 1
        || response.operation != IdleCheckpointOperation::PermitPmIdleCheckpoint
        || response.idempotency_key != c.idempotency_key
        || response.request_sha256 != request_hash(c)?
        || p.scope != IdleCheckpointScope::IdleCheckpoint
        || p.permit_id.is_nil()
        || p.barrier_id != c.terminal_idle_recovery.barrier_id
        || p.session_run_id != c.previous_run_id
        || p.input_id != c.input.input_id()
        || p.checkpoint_operation_key != c.terminal_idle_recovery.checkpoint_operation_key
        || p.checkpoint_sha256 != c.terminal_idle_recovery.checkpoint_sha256
        || p.expires_at <= p.issued_at
        || p.expires_at - p.issued_at > chrono::Duration::seconds(30)
        || response.observed_at < p.issued_at
    {
        return Err(invalid());
    }
    Ok(())
}

impl crate::sdlc::SdlcService {
    pub async fn resume_pm_execution_request(
        &self,
        task: Uuid,
        actor: &Principal,
        command: PmLeaseResumeRequest,
    ) -> Result<(PmLeaseResumeResponse, bool), AppError> {
        match command {
            PmLeaseResumeRequest::Ordinary(c) => self
                .resume_pm_execution_lease(task, actor, c)
                .await
                .map(|(value, replay)| (PmLeaseResumeResponse::Ordinary(value), replay)),
            PmLeaseResumeRequest::TerminalIdle(c) => self
                .permit_pm_idle_checkpoint(task, actor, c)
                .await
                .map(|(value, replay)| (PmLeaseResumeResponse::TerminalIdle(value), replay)),
        }
    }

    pub async fn pm_resume_request_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<PmLeaseResumeOperationResponse, AppError> {
        if key.starts_with("fleet-pm-terminal-idle:") {
            return self
                .repository
                .pm_terminal_idle_checkpoint_operation(task, actor, key)
                .await
                .map(PmLeaseResumeOperationResponse::TerminalIdle);
        }
        self.pm_resume_operation(task, actor, key)
            .await
            .map(PmLeaseResumeOperationResponse::Ordinary)
    }

    pub async fn permit_pm_idle_checkpoint(
        &self,
        task: Uuid,
        actor: &Principal,
        command: PermitPmIdleCheckpointRequest,
    ) -> Result<(PermitPmIdleCheckpointResponse, bool), AppError> {
        validate_command(&command)?;
        if let Some(saved) = self
            .repository
            .preflight_terminal_idle_checkpoint(task, actor, &command)
            .await?
        {
            return Ok((saved, true));
        }
        let reader = self
            .fleet_configuration
            .as_ref()
            .ok_or_else(|| AppError::Unavailable("Fleet PM readback is not configured".into()))?;
        let witness = reader
            .read_terminal_idle_pm_recovery(command.terminal_idle_recovery.barrier_id)
            .await?;
        self.repository
            .permit_terminal_idle_checkpoint(task, actor, command, witness)
            .await
    }
}

#[cfg(test)]
#[path = "sdlc_terminal_idle_recovery_tests.rs"]
mod tests;
