//! Initial bootstrap-attempt lease recovery; canonical assignment stays immutable.
//! Observe outside a DB transaction, then plan against fresh task-locked rows.
use chrono::{DateTime, Duration, Utc};
use domain::sdlc::*;
use domain::sdlc_execution_lease::*;
use domain::sdlc_pm_bootstrap_retry::*;
use domain::sdlc_pm_draft::*;
use shared::AppError;
use uuid::Uuid;

/// Private fields prevent the public handler from supplying caller booleans.
pub struct BootstrapRetryObservation {
    command_sha256: String,
    readback: BootstrapFailureReadback,
}

pub struct BootstrapRetrySource<'a> {
    pub state: &'a TaskState,
    pub reservation: &'a PmDraftReservation,
    pub original_input: &'a PmDraftInputResponse,
    pub lease: &'a ExecutionLeaseReceipt,
    pub owner_version: i64,
    pub admission_state: &'a str,
    pub predecessor_consumed: bool,
    pub delivery_owned: bool,
    /// Database clock obtained with the fresh task/lease rows.
    pub now: DateTime<Utc>,
}

pub struct BootstrapRetryPlan {
    pub operation: ExecutionLeaseOperation,
    /// Recheck exact rows before committing this plan; it is not a dispatch permit.
    pub expected_state_sha256: String,
    pub expected_reservation_sha256: String,
    pub expected_lease_sha256: String,
    pub previous_session_run_id: Uuid,
    pub next_session_run_id: Uuid,
    pub chat_id: Uuid,
}

pub struct BootstrapRetryService;

fn conflict() -> AppError {
    AppError::conflict("PM bootstrap retry requires exact terminal unstarted custody")
}

pub fn request_hash(c: &BootstrapRetryCommand) -> Result<String, AppError> {
    crate::sdlc::validate_key(&c.idempotency_key)?;
    validate_predecessor(&c.predecessor, c)?;
    if [
        c.lease_id,
        c.creation_operation_id,
        c.previous_session_run_id,
        c.next_session_run_id,
        c.chat_id,
        c.barrier_id,
        c.fence.assignment_id,
        c.fence.execution_id,
        c.fence.agent_id,
    ]
    .iter()
    .any(Uuid::is_nil)
        || c.creation_operation_id != c.previous_session_run_id
        || c.next_session_run_id == c.previous_session_run_id
        || c.expected_owner_version != 1
        || c.fence.assignment_version != 1
        || !(1..MAX_SAFE_VERSION).contains(&c.expected_lease_version)
        || c.previous_native_run_ref.trim().is_empty()
        || c.previous_native_run_ref.len() > 256
        || c.previous_native_run_ref.chars().any(char::is_control)
    {
        return Err(AppError::validation(
            "invalid initial PM bootstrap retry command",
        ));
    }
    crate::sdlc::canonical_hash(&serde_json::json!({
        "operation":"retry_pm_bootstrap_before_first_instruction", "payload":c,
    }))
}

pub fn identity(r: &PmDraftReservation) -> PmExecutionIdentity {
    PmExecutionIdentity {
        task: r.execution.key.clone(),
        execution_ref: r.assignment.execution_id.to_string(),
        tracker_instance_ref: r.binding.tracker_instance_id.clone(),
        tracker_project_ref: r.binding.project_id.to_string(),
        task_ref: r.binding.task_id.to_string(),
        root_ref: r.binding.root_task_id.to_string(),
        agent_ref: r.assignment.agent_id.to_string(),
        assignment_operation_key: r.assignment_operation_key.clone(),
        assignment_ref: r.assignment.assignment_id.to_string(),
        assignment_revision: r.assignment.version,
    }
}

fn validate_predecessor(
    p: &WorkflowBootstrapPredecessor,
    c: &BootstrapRetryCommand,
) -> Result<(), AppError> {
    crate::sdlc::validate_key(&p.assignment_operation_key)?;
    if !(1..MAX_SAFE_VERSION).contains(&p.task_id) || p.assignment_revision != 1 || p.attempt != 1 {
        return Err(conflict());
    }
    if let Some(b) = &p.finalized_binding {
        crate::sdlc::validate_key(&b.bind_operation_key)?;
        crate::sdlc_lifecycle::validate_artifact(&domain::sdlc_lifecycle::ArtifactReceipt {
            reference: b.binding_ref.clone(),
            sha256: b.bind_request_sha256.clone(),
        })?;
        if b.session_run_id != c.previous_session_run_id
            || b.native_run_ref != c.previous_native_run_ref
        {
            return Err(conflict());
        }
    }
    Ok(())
}

fn original_lease(
    r: &PmDraftReservation,
    l: &ExecutionLeaseReceipt,
    c: &BootstrapRetryCommand,
) -> bool {
    let ordinal = r.execution.ordinal.parse::<i64>().ok();
    r.contract_version == 1
        && !r.dispatch_allowed
        && ordinal.is_some_and(|n| {
            n > 0 && n.to_string() == r.execution.ordinal && r.execution.key == format!("SDLC-{n}")
        })
        && r.assignment_operation_key == format!("pm-draft:{}", r.assignment.assignment_id)
        && r.owner_cas.expected_version == 0
        && r.owner_cas.version == 1
        && r.assignment.assignment_id == c.fence.assignment_id
        && r.assignment.execution_id == c.fence.execution_id
        && r.assignment.agent_id == c.fence.agent_id
        && r.assignment.version == 1
        && l.contract_version == 1
        && l.binding == r.binding
        && l.owner_version == 1
        && l.fence == c.fence
        && !l.dispatch_allowed
        && l.ttl_seconds == LEASE_TTL_SECONDS
        && l.heartbeat_seconds == LEASE_HEARTBEAT_SECONDS
        && l.lease.lease_id == c.lease_id
        && l.lease.version == c.expected_lease_version
        && l.lease.holder_subject == r.assignment.machine_subject
        && l.lease.heartbeat_at >= l.lease.claimed_at
        && (l.lease.version != 1 || l.lease.heartbeat_at == l.lease.claimed_at)
        && l.lease.expires_at == l.lease.heartbeat_at + Duration::seconds(LEASE_TTL_SECONDS)
}

pub fn validate_witness(
    r: &PmDraftReservation,
    c: &BootstrapRetryCommand,
    w: &BootstrapFailureReadback,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    request_hash(c)?;
    if w.contract_version != 1
        || w.observed_at > now
        || now - w.observed_at > Duration::seconds(LEASE_TTL_SECONDS)
        || w.identity != identity(r)
        || w.owner_subject != r.binding.owner_subject
        || w.input.snapshot_ref != r.input.snapshot_ref
        || w.input.sha256 != r.input.sha256
        || w.owner_version != c.expected_owner_version
        || w.lease_id != c.lease_id
        || w.lease_version != c.expected_lease_version
        || w.creation_operation_id != c.creation_operation_id
        || w.session_run_id != c.previous_session_run_id
        || w.native_run_ref != c.previous_native_run_ref
        || w.chat_id != c.chat_id
        || w.barrier_id != c.barrier_id
        || w.next_session_run_id != c.next_session_run_id
        || w.terminal.is_none()
        || w.predecessor != c.predecessor
        || c.predecessor.assignment_operation_key != r.assignment_operation_key
        || !(1..=MAX_SAFE_VERSION).contains(&w.initial_phase_id)
        || w.current_phase_id != w.initial_phase_id
        || w.workflow_status != "active"
        || w.first_instruction != BootstrapReadbackState::Absent
        || w.pm_enrollment != BootstrapReadbackState::Absent
        || w.business_result != BootstrapReadbackState::Absent
        || w.inflight_work != BootstrapReadbackState::Absent
        || w.generic_binding
            != if c.predecessor.finalized_binding.is_some() {
                BootstrapReadbackState::Present
            } else {
                BootstrapReadbackState::Absent
            }
    {
        return Err(conflict());
    }
    crate::sdlc_lifecycle::validate_artifact(&w.terminal_receipt)?;
    crate::sdlc_lifecycle::validate_artifact(&w.bootstrap_receipt)?;
    Ok(())
}

impl crate::sdlc::SdlcService {
    pub async fn retry_pm_bootstrap(
        &self,
        task: Uuid,
        actor: &Principal,
        c: BootstrapRetryCommand,
    ) -> Result<(ExecutionLeaseReceipt, bool), AppError> {
        request_hash(&c)?;
        if let Some(saved) = self
            .repository
            .preflight_pm_bootstrap_retry(task, actor, &c)
            .await?
        {
            return Ok((saved, true));
        }
        let reader = self
            .fleet_configuration
            .as_ref()
            .ok_or_else(|| AppError::Unavailable("Fleet PM readback is not configured".into()))?;
        let witness = reader.read_pm_bootstrap_retry(&c).await?;
        self.repository
            .retry_pm_bootstrap(task, actor, c, witness)
            .await
    }

    pub async fn pm_bootstrap_retry_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<ExecutionLeaseOperation, AppError> {
        self.repository
            .pm_bootstrap_retry_operation(task, actor, key)
            .await
    }
}

impl BootstrapRetryService {
    /// Internal repository boundary, not a public POST DTO. The application obtains
    /// this value exclusively through the configured authenticated Fleet reader.
    pub fn trusted_observation(
        c: &BootstrapRetryCommand,
        readback: BootstrapFailureReadback,
    ) -> Result<BootstrapRetryObservation, AppError> {
        Ok(BootstrapRetryObservation {
            command_sha256: request_hash(c)?,
            readback,
        })
    }

    pub fn prepare(
        source: BootstrapRetrySource<'_>,
        actor: &Principal,
        config: &SdlcConfig,
        c: &BootstrapRetryCommand,
        observation: &BootstrapRetryObservation,
    ) -> Result<BootstrapRetryPlan, AppError> {
        crate::sdlc_reservation::scheduler(actor, config)?;
        let hash = request_hash(c)?;
        let s = source.state;
        let r = source.reservation;
        let l = source.lease;
        let input = source.original_input;
        s.require_root()?;
        validate_witness(r, c, &observation.readback, source.now)?;
        if observation.command_sha256 != hash
            || !original_lease(r, l, c)
            || s.tracker_instance_id != config.instance_id
            || !config.single_task_project_ids.contains(&s.project_id)
            || s.draft_flow()? != Some(DraftFlow::SingleTaskV1)
            || s.stage != Stage::Draft
            || s.status != Some(domain::sdlc_lifecycle::LifecycleStatus::InProgress)
            || s.draft_submode != Some(DraftSubmode::Brainstorming)
            || s.assignment.as_ref() != Some(&r.assignment)
            || !s.questions.is_empty()
            || !s.revisions.is_empty()
            || !s.confirmations.is_empty()
            || !s.evidence.is_empty()
            || s.confirmation_revision.is_some()
            || r.binding.tracker_instance_id != s.tracker_instance_id
            || r.binding.project_id != s.project_id
            || r.binding.task_id != s.task_id
            || r.binding.root_task_id != s.root_task_id
            || r.binding.owner_subject != s.owner_subject
            || s.owner_subject.trim().is_empty()
            || r.assignment.machine_subject != config.orchestrator_subject
            || r.assignment.machine_subject == s.owner_subject
            || r.assignment.machine_subject == config.verifier_subject
            || r.assignment.machine_subject == actor.subject
            || source.owner_version != 1
            || source.admission_state != "reserved"
            || source.predecessor_consumed
            || source.delivery_owned
            || input.contract_version != 1
            || input.tracker_instance_id != s.tracker_instance_id
            || input.project_id != s.project_id
            || input.task_id != s.task_id
            || input.root_task_id != s.root_task_id
            || input.owner_subject != s.owner_subject
            || input.input.snapshot_ref.is_nil()
            || input.input.snapshot_ref != r.input.snapshot_ref
            || input.input.sha256 != r.input.sha256
            || input.input.sha256
                != crate::sdlc::pm_draft_input_hash(&input.input.title, &input.input.description)?
            || l.lease.expires_at > source.now
        {
            return Err(conflict());
        }
        let mut result = l.clone();
        result.lease.version += 1;
        result.lease.heartbeat_at = source.now;
        result.lease.expires_at = source.now + Duration::seconds(LEASE_TTL_SECONDS);
        Ok(BootstrapRetryPlan {
            operation: ExecutionLeaseOperation {
                idempotency_key: c.idempotency_key.clone(),
                request_sha256: hash,
                result,
            },
            expected_state_sha256: crate::sdlc::canonical_hash(s)?,
            expected_reservation_sha256: crate::sdlc::canonical_hash(r)?,
            expected_lease_sha256: crate::sdlc::canonical_hash(l)?,
            previous_session_run_id: c.previous_session_run_id,
            next_session_run_id: c.next_session_run_id,
            chat_id: c.chat_id,
        })
    }

    /// Exact actor/key replay precedes fresh observation and current-state gates.
    /// The repository supplies immutable original pre-retry lease/reservation rows.
    pub fn replay<'b>(
        actor: &Principal,
        config: &SdlcConfig,
        c: &BootstrapRetryCommand,
        original: (&PmDraftReservation, &ExecutionLeaseReceipt),
        saved: (&str, &'b ExecutionLeaseOperation),
    ) -> Result<&'b ExecutionLeaseReceipt, AppError> {
        if actor.scopes.contains("task-tracker:write") {
            crate::sdlc_reservation::scheduler(actor, config)?;
        } else {
            crate::sdlc_lifecycle::scheduler_read(actor, config)?;
        }
        let (r, before) = original;
        let (saved_actor, operation) = saved;
        let hash = request_hash(c)?;
        let after = &operation.result;
        let mut expected = before.clone();
        expected.lease.version = c.expected_lease_version + 1;
        expected.lease.heartbeat_at = after.lease.heartbeat_at;
        expected.lease.expires_at =
            expected.lease.heartbeat_at + Duration::seconds(LEASE_TTL_SECONDS);
        if saved_actor != actor.subject
            || operation.idempotency_key != c.idempotency_key
            || operation.request_sha256 != hash
            || !original_lease(r, before, c)
            || r.binding.tracker_instance_id != config.instance_id
            || after.lease.heartbeat_at < before.lease.expires_at
            || serde_json::to_value(after).map_err(AppError::internal)?
                != serde_json::to_value(expected).map_err(AppError::internal)?
        {
            return Err(conflict());
        }
        Ok(after)
    }
}

#[cfg(test)]
#[path = "sdlc_pm_bootstrap_retry_tests.rs"]
mod tests;
