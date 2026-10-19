use domain::sdlc::*;
use domain::sdlc_execution_lease::*;
use domain::sdlc_pm_draft::PmDraftReservation;
use shared::AppError;
use uuid::Uuid;

impl crate::sdlc::SdlcService {
    pub async fn recover_unsubmitted_pm_execution_lease(
        &self,
        task: Uuid,
        actor: &Principal,
        command: RecoverUnsubmittedPmExecutionLease,
    ) -> Result<(ExecutionLeaseReceipt, bool), AppError> {
        validate_command(&command)?;
        if let Some(receipt) = self
            .repository
            .preflight_unsubmitted_pm_recovery(task, actor, &command)
            .await?
        {
            return Ok((receipt, true));
        }
        let reader = self
            .fleet_configuration
            .as_ref()
            .ok_or_else(|| AppError::Unavailable("Fleet PM readback is not configured".into()))?;
        let witness = reader
            .read_unsubmitted_pm(command.session_run_id, command.barrier_id)
            .await?;
        // No database transaction crosses HTTP. The repository rechecks all authority/CAS.
        self.repository
            .recover_unsubmitted_pm_execution_lease(task, actor, command, witness)
            .await
    }

    pub async fn unsubmitted_pm_recovery_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<ExecutionLeaseOperation, AppError> {
        self.repository
            .unsubmitted_pm_recovery_operation(task, actor, key)
            .await
    }
}

pub fn validate_command(c: &RecoverUnsubmittedPmExecutionLease) -> Result<(), AppError> {
    crate::sdlc::validate_key(&c.idempotency_key)?;
    if [
        c.lease_id,
        c.session_run_id,
        c.barrier_id,
        c.fence.assignment_id,
        c.fence.execution_id,
        c.fence.agent_id,
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
    {
        return Err(AppError::validation(
            "invalid unsubmitted PM recovery identity",
        ));
    }
    Ok(())
}

pub fn request_hash(c: &RecoverUnsubmittedPmExecutionLease) -> Result<String, AppError> {
    crate::sdlc::canonical_hash(&serde_json::json!({
        "operation":"recover_unsubmitted_pm_execution_lease", "payload":c
    }))
}

pub fn validate_context(
    state: &TaskState,
    r: &PmDraftReservation,
    c: &RecoverUnsubmittedPmExecutionLease,
) -> Result<(), AppError> {
    validate_command(c)?;
    state.require_root()?;
    let a = state
        .assignment
        .as_ref()
        .ok_or_else(|| AppError::conflict("PM assignment absent"))?;
    if r.assignment != *a
        || r.binding.task_id != state.task_id
        || r.binding.root_task_id != state.root_task_id
        || r.binding.project_id != state.project_id
        || r.binding.tracker_instance_id != state.tracker_instance_id
        || r.binding.owner_subject != state.owner_subject
        || r.owner_cas.version != c.expected_owner_version
        || a.assignment_id != c.fence.assignment_id
        || a.execution_id != c.fence.execution_id
        || a.agent_id != c.fence.agent_id
        || a.version != c.fence.assignment_version
        || r.owner_cas.expected_version != 0
        || a.version != 1
        || state.draft_flow()? != Some(DraftFlow::SingleTaskV1)
        || !matches!(state.stage, Stage::Draft)
        || state.status == Some(domain::sdlc_lifecycle::LifecycleStatus::Completed)
        || !state.revisions.is_empty()
        || !state.questions.is_empty()
        || !state.confirmations.is_empty()
    {
        return Err(AppError::conflict(
            "initial unsubmitted PM reservation changed",
        ));
    }
    Ok(())
}

pub fn validate_witness(
    w: &UnsubmittedPmWitness,
    r: &PmDraftReservation,
    c: &RecoverUnsubmittedPmExecutionLease,
) -> Result<(), AppError> {
    let i = &w.identity;
    let b = &r.binding;
    let a = &r.assignment;
    if w.contract_version != 1
        || w.session_run_id != c.session_run_id
        || w.barrier_id != c.barrier_id
        || w.lease_id != c.lease_id
        || w.lease_version != c.expected_lease_version
        || w.owner_version != c.expected_owner_version
        || i.task != r.execution.key
        || i.execution_ref != a.execution_id.to_string()
        || i.tracker_instance_ref != b.tracker_instance_id
        || i.tracker_project_ref != b.project_id.to_string()
        || i.task_ref != b.task_id.to_string()
        || i.root_ref != b.root_task_id.to_string()
        || i.agent_ref != a.agent_id.to_string()
        || i.assignment_ref != a.assignment_id.to_string()
        || i.assignment_revision != a.version
        || i.assignment_operation_key != r.assignment_operation_key
    {
        return Err(AppError::conflict(
            "Fleet unsubmitted PM witness does not match reservation",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "sdlc_pm_unsubmitted_tests.rs"]
mod tests;
