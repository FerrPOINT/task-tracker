//! Genuine human input materialization; this is not native execution admission.
use chrono::{DateTime, Utc};
use domain::{
    sdlc::{Confirmation, MAX_SAFE_VERSION, Principal, RequirementsRevision, TaskState},
    sdlc_routing::TaskRoutingSnapshot,
    sdlc_source::TaskSourceBinding,
    task_handoff::{
        CurrentStageEntry, PreparedCurrentStageHandoff, TaskHandoffCommand, TaskHandoffReceipt,
    },
};
use shared::AppError;
use uuid::Uuid;

pub struct HandoffAllocation {
    pub handoff_id: Uuid,
    pub confirmation_id: Uuid,
    pub routing_snapshot_id: Uuid,
    pub source_binding_id: Uuid,
    pub observed_at: DateTime<Utc>,
}

/// Persist all these members in one locked owner transaction or none of them.
/// No run, prior role result, PM completion or deployment receipt is allocated.
pub struct FrozenHandoffMaterials {
    pub receipt: TaskHandoffReceipt,
    pub next_state: TaskState,
    pub requirement: RequirementsRevision,
    pub confirmation: Confirmation,
    pub routing: TaskRoutingSnapshot,
    pub source: TaskSourceBinding,
}

/// Validate human inputs before peer HTTP and again inside the locked owner write.
pub fn validate_prepared_command(
    prepared: &PreparedCurrentStageHandoff,
    command: &TaskHandoffCommand,
) -> Result<(), AppError> {
    command.validate()?;
    if !prepared.blockers.is_empty()
        || prepared.state.revision != command.expected_revision
        || prepared.issue_sha256 != command.confirmed_issue_sha256
        || prepared.routing.as_ref().map(|p| p.version)
            != Some(command.expected_routing_policy_version)
    {
        return Err(AppError::conflict(
            "current_stage_handoff_inputs_changed_or_blocked",
        ));
    }
    match prepared.state.stage {
        domain::sdlc_lifecycle::LifecycleStage::Draft
        | domain::sdlc_lifecycle::LifecycleStage::Backlog => {
            crate::sdlc::validate_initial_document(&command.requirements)
        }
        _ => crate::sdlc::validate_document(&command.requirements),
    }
}

pub fn prepare_materials(
    state: &TaskState,
    actor: &Principal,
    prepared: &PreparedCurrentStageHandoff,
    command: &TaskHandoffCommand,
    ids: HandoffAllocation,
) -> Result<FrozenHandoffMaterials, AppError> {
    validate_prepared_command(prepared, command)?;
    if !actor.trusted_human
        || actor.subject != state.owner_subject
        || actor.subject != prepared.owner_subject
        || crate::sdlc::has_pm_grant(&actor.scopes)
        || crate::sdlc_lifecycle::has_delivery_grant(&actor.scopes)
    {
        return Err(AppError::Forbidden);
    }
    state.require_root()?;
    let business_state = domain::task_lifecycle::effective_state(
        &serde_json::to_value(state).map_err(AppError::internal)?,
        prepared.state.revision,
    )?;
    if state.task_id != prepared.task_id
        || state.project_id != prepared.project_id
        || business_state != prepared.state
    {
        return Err(AppError::conflict(
            "current_stage_handoff_inputs_changed_or_blocked",
        ));
    }
    let entry = CurrentStageEntry::derive(prepared.state, false)?;
    if prepared.entry != Some(entry) {
        return Err(AppError::conflict("current_stage_handoff_entry_changed"));
    }
    let policy = prepared
        .routing
        .as_ref()
        .ok_or_else(|| AppError::Unavailable("routing_not_configured".into()))?;
    crate::sdlc_draft_flow::validate_policy(policy, state.project_id, &state.tracker_instance_id)?;
    let source_policy = prepared
        .source
        .as_ref()
        .ok_or_else(|| AppError::Unavailable("source_not_configured".into()))?;
    let allocated = [
        ids.handoff_id,
        ids.confirmation_id,
        ids.routing_snapshot_id,
        ids.source_binding_id,
    ];
    if allocated.iter().any(Uuid::is_nil)
        || allocated
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != allocated.len()
    {
        return Err(AppError::internal("invalid handoff allocation"));
    }
    let resulting_revision = prepared
        .state
        .revision
        .checked_add(1)
        .filter(|r| *r <= MAX_SAFE_VERSION)
        .ok_or_else(|| AppError::conflict("task_lifecycle_revision_exhausted"))?;
    let requirement_revision = state
        .current_revision()
        .unwrap_or(0)
        .checked_add(1)
        .filter(|r| *r <= MAX_SAFE_VERSION)
        .ok_or_else(|| AppError::conflict("requirements_revision_exhausted"))?;
    let requirement = RequirementsRevision {
        revision: requirement_revision,
        content_hash: crate::sdlc::canonical_hash(&command.requirements)?,
        document: command.requirements.clone(),
        author_subject: actor.subject.clone(),
        created_at: ids.observed_at,
    };
    let confirmation = Confirmation {
        id: ids.confirmation_id,
        task_id: state.task_id,
        revision: requirement.revision,
        content_hash: requirement.content_hash.clone(),
        owner_subject: actor.subject.clone(),
        created_at: ids.observed_at,
        stage: prepared.state.stage.canonical(),
    };
    let mut next_state = state.clone();
    next_state.revisions.push(requirement.clone());
    next_state.confirmations.push(confirmation.clone());
    next_state.confirmation_revision = Some(requirement.revision);
    next_state
        .retained_fields
        .insert("human_handoff_id".into(), serde_json::json!(ids.handoff_id));
    let routing = TaskRoutingSnapshot {
        contract_version: policy.contract_version,
        snapshot_id: ids.routing_snapshot_id,
        tracker_instance_id: state.tracker_instance_id.clone(),
        project_id: state.project_id,
        task_id: state.task_id,
        root_task_id: state.root_task_id,
        confirmation_id: confirmation.id,
        requirement_revision: requirement.revision,
        content_hash: requirement.content_hash.clone(),
        policy: policy.clone(),
        created_at: ids.observed_at,
    };
    let source = crate::sdlc_source::freeze_handoff_binding(
        &next_state,
        entry.state,
        &confirmation,
        &routing,
        source_policy,
    )?;
    let receipt = TaskHandoffReceipt {
        contract_version: 1,
        handoff_id: ids.handoff_id,
        task_id: state.task_id,
        project_id: state.project_id,
        actor_subject: actor.subject.clone(),
        entry,
        confirmed_issue_sha256: prepared.issue_sha256.clone(),
        issue_materials_sha256: prepared.issue_materials_sha256.clone(),
        resulting_revision,
        requirement_revision: requirement.revision,
        requirements_sha256: requirement.content_hash.clone(),
        confirmation_id: confirmation.id,
        routing_snapshot_id: routing.snapshot_id,
        source_binding_id: ids.source_binding_id,
        idempotency_key: command.idempotency_key.clone(),
        created_at: ids.observed_at,
    };
    Ok(FrozenHandoffMaterials {
        receipt,
        next_state,
        requirement,
        confirmation,
        routing,
        source,
    })
}

/// A current-stage metadata reader is the existing exact scheduler principal.
/// It cannot borrow a different native request or a write-capable request frame.
pub fn scheduler_reader(
    actor: &Principal,
    config: &domain::sdlc::SdlcConfig,
) -> Result<(), AppError> {
    crate::sdlc_lifecycle::scheduler_read(actor, config)?;
    if actor.trusted_human {
        return Err(AppError::Forbidden);
    }
    if let Some(native) = crate::native_space_access::current()? {
        if native.identity().subject != actor.subject || native.may_write() {
            return Err(AppError::Forbidden);
        }
    }
    Ok(())
}

pub fn scheduler_writer(
    actor: &Principal,
    config: &domain::sdlc::SdlcConfig,
) -> Result<(), AppError> {
    crate::sdlc_reservation::scheduler(actor, config)?;
    if actor.trusted_human {
        return Err(AppError::Forbidden);
    }
    if let Some(native) = crate::native_space_access::current()? {
        if native.identity().subject != actor.subject || !native.may_write() {
            return Err(AppError::Forbidden);
        }
    }
    Ok(())
}

/// Owner persistence only. A saved consent is not a Fleet claim or a native launch permit.
pub struct PendingHandoffMaterials {
    pub receipt: TaskHandoffReceipt,
    pub routing: TaskRoutingSnapshot,
    pub source: TaskSourceBinding,
}

/// Immutable original reservation outcome; runtime state is read separately.
pub struct SavedHandoffReservation {
    pub command_hash: String,
    pub result: domain::current_stage_execution::CurrentStageExecution,
}

/// The command hash remains internal; callers cannot use it as owner evidence.
pub struct SavedHandoffOperation {
    pub operation: domain::task_handoff::TaskHandoffOperation,
    pub command_hash: String,
}

#[async_trait::async_trait]
pub trait CurrentStageHandoffRepository: Send + Sync {
    async fn prepare_requested_handoff(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<PreparedCurrentStageHandoff, AppError>;

    async fn reserve_current_stage_handoff(
        &self,
        task: Uuid,
        actor: &Principal,
        command: &domain::task_handoff::ReserveTaskHandoffCommand,
        observed: &crate::task_handoff_observations::MatchedHandoffInputs,
    ) -> Result<(domain::current_stage_execution::CurrentStageExecution, bool), AppError>;

    async fn handoff_reservation_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
        mutation: bool,
    ) -> Result<Option<SavedHandoffReservation>, AppError>;

    async fn current_owner_handoff(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<Option<domain::task_handoff::CurrentHandoffStatus>, AppError>;

    /// Reuse the existing bounded project/UUID cursor query. Discovery does not
    /// reserve capacity, acquire a lease or assert that external owners are ready.
    async fn requested_handoffs(
        &self,
        actor: &Principal,
        query: domain::sdlc_candidates::DeliveryCandidatesQuery,
    ) -> Result<domain::task_handoff::RequestedHandoffs, AppError>;

    async fn requested_handoff_execution(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<domain::current_stage_execution::CurrentStageExecution, AppError>;

    /// Read the original operation after lost responses, even if inputs changed or
    /// it was cancelled. Current owner, account and project access still apply.
    async fn handoff_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<Option<SavedHandoffOperation>, AppError>;

    /// A verified current consent, not native execution admission. Recheck again at claim.
    async fn pending_handoff_materials(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<PendingHandoffMaterials, AppError>;

    /// Cancel only an unclaimed queued request; native stop uses the real runner protocol.
    async fn cancel_current_stage_handoff(
        &self,
        task: Uuid,
        handoff: Uuid,
        actor: &Principal,
        command: &domain::task_handoff::CancelTaskHandoffCommand,
    ) -> Result<(domain::task_handoff::CancelTaskHandoffReceipt, bool), AppError>;

    /// A durable request, not a process-start receipt. Peer observations are
    /// obtained outside the transaction and rechecked against the locked inputs.
    async fn request_current_stage_handoff(
        &self,
        task: Uuid,
        actor: &Principal,
        command: &TaskHandoffCommand,
        observed: &crate::task_handoff_observations::MatchedHandoffInputs,
    ) -> Result<(TaskHandoffReceipt, bool), AppError>;

    async fn save_handoff_materials(
        &self,
        task: Uuid,
        actor: &Principal,
        command: &TaskHandoffCommand,
    ) -> Result<(TaskHandoffReceipt, bool), AppError>;
}

/// Build the pending business execution from a real saved human consent. This
/// allocates no assignment, native run, prior result or deployment receipt.
pub fn seed_current_stage(
    pending: &PendingHandoffMaterials,
) -> Result<domain::current_stage_execution::CurrentStageExecution, AppError> {
    use domain::sdlc_lifecycle::{LifecycleTask, RuntimeState};
    let consent = &pending.receipt;
    let routing = &pending.routing;
    let source = &pending.source;
    let entry = CurrentStageEntry::derive(consent.entry.state, false)?;
    if consent.contract_version != 1
        || !(1..MAX_SAFE_VERSION).contains(&consent.entry.state.revision)
        || !(1..=MAX_SAFE_VERSION).contains(&consent.requirement_revision)
        || consent.actor_subject.is_empty()
        || [
            consent.confirmation_id,
            consent.routing_snapshot_id,
            consent.source_binding_id,
        ]
        .iter()
        .any(Uuid::is_nil)
        || [
            &consent.confirmed_issue_sha256,
            &consent.issue_materials_sha256,
            &consent.requirements_sha256,
        ]
        .iter()
        .any(|s| !crate::sdlc_source::lower_hex(s, 64))
        || consent.handoff_id.is_nil()
        || consent.task_id.is_nil()
        || consent.project_id.is_nil()
        || entry != consent.entry
        || consent.resulting_revision
            != consent
                .entry
                .state
                .revision
                .checked_add(1)
                .ok_or_else(|| AppError::conflict("task_lifecycle_revision_exhausted"))?
        || consent.resulting_revision > MAX_SAFE_VERSION
        || routing.task_id != consent.task_id
        || routing.root_task_id != consent.task_id
        || routing.project_id != consent.project_id
        || routing.snapshot_id != consent.routing_snapshot_id
        || routing.confirmation_id != consent.confirmation_id
        || routing.requirement_revision != consent.requirement_revision
        || routing.content_hash != consent.requirements_sha256
        || source.task_id != consent.task_id
        || source.root_task_id != consent.task_id
        || source.project_id != consent.project_id
        || source.owner_subject != consent.actor_subject
        || source.routing_snapshot_id != routing.snapshot_id
        || source.confirmation_id != consent.confirmation_id
        || source.requirement_revision != consent.requirement_revision
        || source.content_hash != consent.requirements_sha256
        || source.tracker_instance_id != routing.tracker_instance_id
        || source.source_hash != crate::sdlc_source::sealed_hash(source)?
    {
        return Err(AppError::conflict(
            "current_stage_handoff_basis_inconsistent",
        ));
    }
    crate::sdlc_draft_flow::validate_policy(
        &routing.policy,
        consent.project_id,
        &routing.tracker_instance_id,
    )?;
    Ok(domain::current_stage_execution::CurrentStageExecution {
        contract_version: 1,
        interaction_protocol: domain::sdlc::InteractionProtocol::ConfiguredCliV1,
        consent: consent.clone(),
        task: LifecycleTask {
            task_id: consent.task_id,
            owner_subject: consent.actor_subject.clone(),
            version: 1,
            stage: entry.state.stage,
            status: entry.state.status,
            cycle: 0,
            transient_retries: 0,
            waiting_reason: Some("queued_for_assignment".into()),
            runtime: RuntimeState::Queued,
            routing: routing.clone(),
            previous_result_id: None,
            resume_chat_id: None,
        },
        source_binding: source.clone(),
        assignment: None,
        lease: None,
        binding: None,
        result: None,
    })
}
