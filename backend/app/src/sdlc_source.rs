use chrono::{DateTime, Utc};
use domain::{
    sdlc::{Confirmation, MAX_SAFE_VERSION, Principal, TaskState},
    sdlc_lifecycle::{LifecycleReadback, LifecycleStatus, RuntimeState},
    sdlc_routing::TaskRoutingSnapshot,
    sdlc_source::*,
};
use shared::{AppError, resource_context::NamespaceRef};

impl crate::sdlc::SdlcService {
    pub fn source_repository(&self) -> Result<&dyn TaskSourceRepository, AppError> {
        self.repository.source_repository().ok_or_else(|| {
            AppError::Unavailable("Tracker task source repository not configured".into())
        })
    }
}

pub fn human_owner(actor: &Principal) -> Result<(), AppError> {
    if std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_some() {
        crate::authz::require_central_administrator()?;
    }
    if !actor.human_session
        || crate::sdlc::has_pm_grant(&actor.scopes)
        || crate::sdlc_lifecycle::has_delivery_grant(&actor.scopes)
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub fn branch_ref(reference: &str) -> bool {
    reference.starts_with("refs/heads/")
        && reference.len() <= 255
        && !reference.ends_with(['/', '.'])
        && !["..", "@{", "//"]
            .iter()
            .any(|part| reference.contains(*part))
        && reference
            .bytes()
            .all(|c| c.is_ascii_graphic() && !b"~^:?*[\\".contains(&c))
        && reference
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.') && !part.ends_with(".lock"))
}

pub fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

pub fn validate_spec(spec: &SourcePolicySpec) -> Result<(), AppError> {
    if spec.forge_instance_id.is_nil()
        || spec.repository_id.is_nil()
        || !branch_ref(&spec.base_ref)
        || !lower_hex(&spec.base_commit, 40)
        || spec.base_commit.bytes().all(|c| c == b'0')
    {
        return Err(AppError::validation("invalid exact source identity"));
    }
    Ok(())
}

pub fn validate_verification(
    spec: &SourcePolicySpec,
    namespace: &NamespaceRef,
    observed: &VerifiedSource,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    validate_spec(spec)?;
    let age = now.signed_duration_since(observed.observed_at);
    if observed.namespace != *namespace
        || namespace.registry_instance_id.is_nil()
        || namespace.namespace_id.is_nil()
        || observed.forge_instance_id != spec.forge_instance_id
        || observed.repository_id != spec.repository_id
        || observed.public_name.is_empty()
        || observed.public_name.len() > 512
        || observed.public_name.chars().any(char::is_control)
        || observed.base_ref != spec.base_ref
        || observed.base_commit != spec.base_commit
        || !branch_ref(&observed.default_ref)
        || observed.protected_refs.len() > 256
        || observed.protected_refs.iter().any(|r| !branch_ref(r))
        || age < chrono::Duration::zero()
        || age > chrono::Duration::seconds(30)
    {
        return Err(AppError::Unavailable(
            "Forge source verification absent, stale or mismatched".into(),
        ));
    }
    Ok(())
}

pub fn sealed_hash<T: serde::Serialize>(value: &T) -> Result<String, AppError> {
    let mut value = serde_json::to_value(value).map_err(AppError::internal)?;
    value
        .as_object_mut()
        .ok_or_else(|| AppError::conflict("invalid source receipt"))?
        .remove("source_hash");
    crate::sdlc::canonical_hash(&value)
}

pub fn validate_policy(policy: &ProjectSourcePolicy) -> Result<(), AppError> {
    validate_spec(&policy.source)?;
    // Validate the historical observation at its own timestamp, not as a live permit.
    validate_verification(
        &policy.source,
        &policy.verification.namespace,
        &policy.verification,
        policy.verification.observed_at,
    )?;
    if policy.contract_version != 1
        || policy.project_id.is_nil()
        || !(1..=MAX_SAFE_VERSION).contains(&policy.source_revision)
        || policy.tracker_instance_id.is_empty()
        || policy.author_subject.is_empty()
        || policy.source_hash != sealed_hash(policy)?
    {
        return Err(AppError::conflict("source policy history inconsistent"));
    }
    Ok(())
}

pub fn freeze_binding(
    state: &TaskState,
    confirmation: &Confirmation,
    routing: &TaskRoutingSnapshot,
    policy: &ProjectSourcePolicy,
) -> Result<TaskSourceBinding, AppError> {
    state.require_root()?;
    validate_policy(policy)?;
    let candidate = candidate_ref(state.task_id);
    if state.draft_flow()? != Some(domain::sdlc::DraftFlow::SingleTaskV1)
        || candidate == policy.source.base_ref
        || candidate == policy.verification.default_ref
        || policy.verification.protected_refs.contains(&candidate)
        || state.tracker_instance_id != policy.tracker_instance_id
        || state.project_id != policy.project_id
        || routing.task_id != state.task_id
        || routing.root_task_id != state.root_task_id
        || routing.project_id != state.project_id
        || routing.tracker_instance_id != state.tracker_instance_id
        || routing.confirmation_id != confirmation.id
        || routing.requirement_revision != confirmation.revision
        || routing.content_hash != confirmation.content_hash
        || confirmation.task_id != state.task_id
        || confirmation.owner_subject != state.owner_subject
        || !state.confirmations.contains(confirmation)
    {
        return Err(AppError::conflict(
            "exact owner source confirmation differs",
        ));
    }
    let mut binding = TaskSourceBinding {
        contract_version: 1,
        tracker_instance_id: state.tracker_instance_id.clone(),
        project_id: state.project_id,
        task_id: state.task_id,
        root_task_id: state.root_task_id,
        confirmation_id: confirmation.id,
        requirement_revision: confirmation.revision,
        content_hash: confirmation.content_hash.clone(),
        routing_snapshot_id: routing.snapshot_id,
        namespace: policy.verification.namespace.clone(),
        forge_instance_id: policy.source.forge_instance_id,
        repository_id: policy.source.repository_id,
        base_ref: policy.source.base_ref.clone(),
        base_commit: policy.source.base_commit.clone(),
        candidate_ref: candidate,
        source_revision: policy.source_revision,
        policy_hash: policy.source_hash.clone(),
        source_hash: String::new(),
        owner_subject: confirmation.owner_subject.clone(),
        created_at: confirmation.created_at,
    };
    binding.source_hash = sealed_hash(&binding)?;
    Ok(binding)
}

pub fn candidate_ref(task: uuid::Uuid) -> String {
    format!("refs/heads/sdlc/{task}")
}

pub fn readback(
    binding: Option<TaskSourceBinding>,
    current: Option<LifecycleReadback>,
) -> Result<TaskSourceReadback, AppError> {
    let reason = match (&binding, &current) {
        (None, _) => Some(SourceBlockReason::MissingBinding),
        (Some(_), None) => Some(SourceBlockReason::MissingCurrentExecution),
        (Some(binding), Some(current)) => {
            if binding.source_hash != sealed_hash(binding)? {
                return Err(AppError::conflict("task source history inconsistent"));
            }
            let receipt = &current.receipt;
            let routing = &receipt.task.routing;
            if binding.task_id != receipt.task.task_id
                || binding.root_task_id != binding.task_id
                || binding.task_id != routing.task_id
                || binding.root_task_id != routing.root_task_id
                || binding.owner_subject != receipt.task.owner_subject
                || binding.project_id != routing.project_id
                || binding.tracker_instance_id != routing.tracker_instance_id
                || binding.routing_snapshot_id != routing.snapshot_id
                || binding.confirmation_id != routing.confirmation_id
                || binding.requirement_revision != routing.requirement_revision
                || binding.content_hash != routing.content_hash
            {
                Some(SourceBlockReason::SourceContextChanged)
            } else if receipt
                .lease
                .as_ref()
                .is_some_and(|lease| lease.expires_at <= current.observed_at)
            {
                Some(SourceBlockReason::LeaseExpired)
            } else if current.runtime_state != RuntimeState::Running
                || receipt.task.runtime != RuntimeState::Running
                || receipt.task.status != LifecycleStatus::InProgress
                || receipt.task.waiting_reason.is_some()
            {
                Some(SourceBlockReason::ExecutionNotRunning)
            } else {
                match (&receipt.assignment, &receipt.lease, &receipt.binding) {
                    (Some(assignment), Some(lease), Some(native))
                        if assignment.task_id == binding.task_id
                            && assignment.routing == *routing
                            && assignment.lease_id == lease.lease_id
                            && assignment.owner_subject == binding.owner_subject
                            && assignment.stage == receipt.task.stage
                            && Some(assignment.role) == receipt.task.stage.role()
                            && assignment.cycle == receipt.task.cycle
                            && assignment.attempt > 0
                            && assignment.fencing_token > 0
                            && !assignment.agent_id.is_nil()
                            && lease.version > 0
                            && !lease.holder_subject.is_empty()
                            && lease.holder_subject != binding.owner_subject
                            && !native.chat_id.is_nil()
                            && !native.run_id.is_nil()
                            && !native.machine_subject.is_empty()
                            && native.machine_subject != lease.holder_subject
                            && native.machine_subject != binding.owner_subject =>
                    {
                        if lease.claimed_at <= lease.heartbeat_at
                            && lease.heartbeat_at <= current.observed_at
                            && current.observed_at < lease.expires_at
                            && lease.expires_at - lease.heartbeat_at
                                <= chrono::Duration::seconds(30)
                        {
                            None
                        } else {
                            Some(SourceBlockReason::LeaseExpired)
                        }
                    }
                    _ => Some(SourceBlockReason::MissingCurrentExecution),
                }
            }
        }
    };
    match (reason, binding, current) {
        (None, Some(binding), Some(current)) => Ok(TaskSourceReadback::Bound { binding, current }),
        (Some(reason), binding, current) => Ok(TaskSourceReadback::Blocked {
            reason,
            binding,
            current,
        }),
        _ => Err(AppError::conflict("source readback inconsistent")),
    }
}

#[cfg(test)]
#[path = "sdlc_source_tests.rs"]
mod tests;
