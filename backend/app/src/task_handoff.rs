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

pub fn prepare_materials(
    state: &TaskState,
    actor: &Principal,
    prepared: &PreparedCurrentStageHandoff,
    command: &TaskHandoffCommand,
    ids: HandoffAllocation,
) -> Result<FrozenHandoffMaterials, AppError> {
    command.validate()?;
    crate::sdlc::validate_document(&command.requirements)?;
    if !actor.trusted_human
        || !actor.human_session
        || !state.owner(actor)
        || actor.subject != prepared.owner_subject
        || crate::sdlc::has_pm_grant(&actor.scopes)
        || crate::sdlc_lifecycle::has_delivery_grant(&actor.scopes)
    {
        return Err(AppError::Forbidden);
    }
    state.require_root()?;
    if !prepared.blockers.is_empty()
        || state.task_id != prepared.task_id
        || state.project_id != prepared.project_id
        || state.stage != prepared.state.stage.canonical()
        || state.status != Some(prepared.state.status)
        || command.expected_revision != prepared.state.revision
        || command.confirmed_issue_sha256 != prepared.issue_sha256
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
    if policy.version != command.expected_routing_policy_version {
        return Err(AppError::conflict("current_stage_handoff_routing_changed"));
    }
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
        stage: state.stage,
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

/// Owner persistence only. A saved consent is not a Fleet claim or a native launch permit.
pub struct PendingHandoffMaterials {
    pub receipt: TaskHandoffReceipt,
    pub routing: TaskRoutingSnapshot,
    pub source: TaskSourceBinding,
}

#[async_trait::async_trait]
pub trait CurrentStageHandoffRepository: Send + Sync {
    /// A verified current consent, not native execution admission. Recheck again at claim.
    async fn pending_handoff_materials(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<PendingHandoffMaterials, AppError>;

    async fn save_handoff_materials(
        &self,
        task: Uuid,
        actor: &Principal,
        command: &TaskHandoffCommand,
    ) -> Result<(TaskHandoffReceipt, bool), AppError>;
}
