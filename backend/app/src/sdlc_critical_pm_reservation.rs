//! Pure fresh planning only. The caller owns persisted reads, replay and one task transaction.
use domain::sdlc::*;
use domain::sdlc_configured_cli::{CompleteConfiguredPm, ConfiguredPmCompletionReceipt};
use domain::sdlc_critical_blockers::*;
use domain::sdlc_critical_pm_reservation::*;
use domain::sdlc_lifecycle::*;
use domain::sdlc_pm_draft::*;
use domain::sdlc_reservation::{CompletePmDraft, PmDraftCompletionReceipt};
use shared::AppError;
use uuid::Uuid;

/// Old receipts are read as history, never converted into invented native proof.
#[derive(Clone, Copy)]
pub enum CompletedPmReceipt<'a> {
    Historical(&'a PmDraftCompletionReceipt),
    Configured(&'a ConfiguredPmCompletionReceipt),
}

pub struct CriticalPmReservationSource<'a> {
    pub state: &'a TaskState,
    pub lifecycle: &'a LifecycleReceipt,
    pub blocker: &'a CriticalBlockerReceipt,
    pub previous_reservation: &'a PmDraftReservation,
    pub current_completed: CompletedPmReceipt<'a>,
    /// Original persisted PM assignment, not a current project-default selection.
    pub original_pm_assignment: &'a PmAssignment,
    pub original_input: &'a PmDraftInputResponse,
    pub pm_owner_version: i64,
    pub pm_admission_state: &'a str,
}

/// Repository allocation from server UUIDs and the existing execution ordinal sequence.
pub struct CriticalPmReservationAllocation {
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
    pub ordinal: i64,
}

pub struct CriticalPmReservationPlan {
    pub state: TaskState,
    pub lifecycle: LifecycleReceipt,
    pub blocker: CriticalBlockerReceipt,
    pub result: CriticalPmReservationResult,
}

fn conflict() -> AppError {
    AppError::conflict("critical PM reservation does not match persisted returned task")
}

fn next(version: i64) -> Result<i64, AppError> {
    version
        .checked_add(1)
        .filter(|next| version > 0 && *next <= MAX_SAFE_VERSION)
        .ok_or_else(conflict)
}

fn verify_source(
    source: &CriticalPmReservationSource<'_>,
    config: &SdlcConfig,
    command: &ReserveCriticalPmReview,
) -> Result<(), AppError> {
    let state = source.state;
    let current = source.lifecycle;
    let blocker = source.blocker;
    let previous = source.previous_reservation;
    let input = source.original_input;
    let routing = &current.task.routing;
    let revision = state.revisions.last().ok_or_else(conflict)?;
    state.require_root()?;
    blocker.proposal.validate(blocker.role)?;
    crate::sdlc_routing::validate_routes(&routing.policy.routes)?;
    if state.draft_flow()? != Some(DraftFlow::SingleTaskV1)
        || state.project_id.is_nil()
        || state.owner_subject.trim().is_empty()
        || !config.single_task_project_ids.contains(&state.project_id)
        || state.tracker_instance_id != config.instance_id
        || state.stage != Stage::Draft
        || state.status != Some(LifecycleStatus::Ready)
        || state.assignment.as_ref() != Some(&previous.assignment)
        || state.task_id != current.task.task_id
        || state.owner_subject != current.task.owner_subject
        || state.retained_fields.get("lifecycle")
            != Some(&serde_json::to_value(&current.task).map_err(AppError::internal)?)
        || state.retained_fields.get("waiting_reason") != Some(&serde_json::json!(PM_REVIEW_REASON))
        || state.retained_fields.get("runtime_state") != Some(&serde_json::json!("terminal"))
        || current.task.stage != LifecycleStage::Draft
        || current.task.status != LifecycleStatus::Ready
        || current.task.runtime != RuntimeState::Terminal
        || current.task.waiting_reason.as_deref() != Some(PM_REVIEW_REASON)
        || current.task.previous_result_id.is_some()
        || current.task.resume_chat_id.is_some()
        || current.assignment.is_some()
        || current.lease.is_some()
        || current.binding.is_some()
        || current.result.is_some()
        || current.task.version != command.expected_task_version
        || routing.contract_version != 2
        || routing.snapshot_id.is_nil()
        || routing.confirmation_id.is_nil()
        || routing.policy.contract_version != 2
        || routing.tracker_instance_id != state.tracker_instance_id
        || routing.project_id != state.project_id
        || routing.task_id != state.task_id
        || routing.root_task_id != state.task_id
        || routing.policy.tracker_instance_id != state.tracker_instance_id
        || routing.policy.project_id != state.project_id
        || routing.policy.routing_hash != crate::sdlc::canonical_hash(&routing.policy.routes)?
        || routing.requirement_revision != revision.revision
        || !(1..=MAX_SAFE_VERSION).contains(&revision.revision)
        || routing.content_hash != revision.content_hash
        || revision.content_hash != crate::sdlc::canonical_hash(&revision.document)?
        || !state.confirmations.iter().any(|confirmation| {
            confirmation.id == routing.confirmation_id
                && confirmation.task_id == state.task_id
                && confirmation.owner_subject == state.owner_subject
                && confirmation.revision == revision.revision
                && confirmation.content_hash == revision.content_hash
        })
        || blocker.state != CriticalBlockerState::AwaitingPmReview
        || !(1..=MAX_SAFE_VERSION).contains(&blocker.version)
        || blocker.version != command.expected_blocker_version
        || blocker.blocker_id != command.blocker_id
        || blocker.blocker_sha256 != command.blocker_sha256
        || blocker.blocker_sha256 != crate::sdlc::canonical_hash(&blocker.proposal)?
        || blocker.tracker_instance_id != state.tracker_instance_id
        || blocker.project_id != state.project_id
        || blocker.task_id != state.task_id
        || blocker.root_task_id != state.task_id
        || blocker.owner_subject != state.owner_subject
        || blocker.routing_snapshot_id != routing.snapshot_id
        || !(1..=MAX_SAFE_VERSION).contains(&blocker.routing_generation)
        || blocker.proposal.requirement_revision != revision.revision
        || blocker.proposal.content_hash != revision.content_hash
        || blocker.pm_review.is_some()
        || blocker.resolution.is_some()
    {
        return Err(conflict());
    }
    let terminal = blocker.technical_terminal.as_ref().ok_or_else(conflict)?;
    crate::sdlc_lifecycle::validate_artifact(&terminal.receipt)?;
    if blocker.chat_id.is_nil()
        || blocker.run_id.is_nil()
        || blocker.agent_id.is_nil()
        || !sha256(&blocker.assignment_hash)
        || terminal.chat_id != blocker.chat_id
        || terminal.run_id != blocker.run_id
        || terminal.assignment_hash != blocker.assignment_hash
    {
        return Err(conflict());
    }

    let ordinal = previous.execution.ordinal.parse::<i64>().ok();
    if previous.contract_version != 1
        || !matches!(previous.variant, PmDraftVariant::Reserved)
        || !matches!(previous.admission_state, PmDraftAdmissionState::Reserved)
        || previous.dispatch_allowed
        || previous.binding.tracker_instance_id != state.tracker_instance_id
        || previous.binding.project_id != state.project_id
        || previous.binding.task_id != state.task_id
        || previous.binding.root_task_id != state.task_id
        || previous.binding.owner_subject != state.owner_subject
        || previous.assignment.assignment_id.is_nil()
        || previous.assignment.execution_id.is_nil()
        || previous.assignment.agent_id.is_nil()
        || !(1..=MAX_SAFE_VERSION).contains(&previous.assignment.version)
        || previous.assignment.version != command.expected_pm_assignment_version
        || source.original_pm_assignment.assignment_id.is_nil()
        || source.original_pm_assignment.execution_id.is_nil()
        || source.original_pm_assignment.agent_id.is_nil()
        || !(1..=MAX_SAFE_VERSION).contains(&source.original_pm_assignment.version)
        || previous.assignment.agent_id != source.original_pm_assignment.agent_id
        || previous.assignment.version < source.original_pm_assignment.version
        || (previous.assignment.version == source.original_pm_assignment.version
            && previous.assignment != *source.original_pm_assignment)
        || (previous.assignment.version > source.original_pm_assignment.version
            && (previous.assignment.assignment_id == source.original_pm_assignment.assignment_id
                || previous.assignment.execution_id == source.original_pm_assignment.execution_id))
        || previous.interaction_protocol != state.interaction_protocol()?
        || previous.assignment.machine_subject.trim().is_empty()
        || previous.assignment.machine_subject == state.owner_subject
        || previous.assignment.machine_subject == config.verifier_subject
        || previous.assignment.machine_subject == config.reservation_scheduler_subject
        || previous.owner_cas.expected_version < 0
        || !(1..=MAX_SAFE_VERSION).contains(&previous.owner_cas.version)
        || previous.owner_cas.expected_version.checked_add(1) != Some(previous.owner_cas.version)
        || source.pm_owner_version != previous.owner_cas.version
        || source.pm_owner_version != command.expected_pm_owner_version
        || source.pm_admission_state != "completed"
        || ordinal.is_none_or(|n| n <= 0 || n.to_string() != previous.execution.ordinal)
        || previous.execution.key != format!("SDLC-{}", previous.execution.ordinal)
        || previous.assignment_operation_key
            != format!("pm-draft:{}", previous.assignment.assignment_id)
        || input.contract_version != 1
        || input.tracker_instance_id != state.tracker_instance_id
        || input.project_id != state.project_id
        || input.task_id != state.task_id
        || input.root_task_id != state.task_id
        || input.owner_subject != state.owner_subject
        || input.input.snapshot_ref.is_nil()
        || input.input.snapshot_ref != previous.input.snapshot_ref
        || input.input.sha256 != previous.input.sha256
        || input.input.sha256
            != crate::sdlc::pm_draft_input_hash(&input.input.title, &input.input.description)?
    {
        return Err(conflict());
    }
    if state.configured_cli() {
        if previous.assignment.machine_subject == config.orchestrator_subject
            || crate::sdlc::canonical_pm_uuid(&previous.assignment.machine_subject).is_none()
        {
            return Err(conflict());
        }
    } else if previous.assignment.machine_subject != config.orchestrator_subject {
        return Err(conflict());
    }
    verify_completion(source)
}

fn verify_completion(source: &CriticalPmReservationSource<'_>) -> Result<(), AppError> {
    let state = source.state;
    let previous = source.previous_reservation;
    let revision = state.revisions.last().ok_or_else(conflict)?;
    let completion = match source.current_completed {
        CompletedPmReceipt::Configured(saved) => {
            if !state.configured_cli()
                || saved.interaction_protocol != InteractionProtocol::ConfiguredCliV1
                || saved.completion_id.is_nil()
                || saved.stage != Stage::Backlog
                || saved.status != LifecycleStatus::Ready
            {
                return Err(conflict());
            }
            let command = CompleteConfiguredPm {
                interaction_protocol: InteractionProtocol::ConfiguredCliV1,
                fence: saved.fence.clone(),
                requirement_revision: saved.requirement_revision,
                content_hash: saved.content_hash.clone(),
                namespace_id: saved.workflow_receipt.namespace_id,
                workflow_execution_ref: saved.workflow_receipt.execution_ref,
                workflow_operation_key: saved.workflow_receipt.operation_key.clone(),
                workflow_result_version: saved.workflow_receipt.result_version,
                idempotency_key: saved.idempotency_key.clone(),
            };
            crate::sdlc::validate_key(&command.idempotency_key)?;
            crate::sdlc::validate_key(&command.workflow_operation_key)?;
            let snapshot = crate::sdlc_configured_cli::completion_history_state(
                state,
                &previous.assignment,
                &command,
            )?;
            let expected = crate::sdlc_configured_cli::completion_result(
                &snapshot,
                &command,
                saved.workflow_receipt.clone(),
                source.pm_owner_version,
                saved.completion_id,
                saved.created_at,
            )?;
            return if &expected == saved
                && saved.requirement_revision == revision.revision
                && saved.content_hash == revision.content_hash
            {
                Ok(())
            } else {
                Err(conflict())
            };
        }
        CompletedPmReceipt::Historical(saved) if !state.configured_cli() => saved,
        CompletedPmReceipt::Historical(_) => return Err(conflict()),
    };
    if completion.contract_version != 1
        || completion.completion_id.is_nil()
        || completion.lease_id.is_nil()
        || !(1..=MAX_SAFE_VERSION).contains(&completion.lease_version)
        || completion.tracker_instance_id != state.tracker_instance_id
        || completion.project_id != state.project_id
        || completion.task_id != state.task_id
        || completion.root_task_id != state.task_id
        || completion.owner_subject != state.owner_subject
        || completion.fence.assignment_id != previous.assignment.assignment_id
        || completion.fence.execution_id != previous.assignment.execution_id
        || completion.fence.agent_id != previous.assignment.agent_id
        || completion.fence.assignment_version != previous.assignment.version
        || completion.owner_version != source.pm_owner_version
        || completion.requirement_revision != revision.revision
        || completion.content_hash != revision.content_hash
        || completion.previous_pm_admission_state != "reserved"
        || completion.next_pm_admission_state != "completed"
    {
        return Err(conflict());
    }
    crate::sdlc::validate_key(&completion.idempotency_key)?;
    crate::sdlc_reservation::validate_pm_completion(&CompletePmDraft {
        contract_version: completion.contract_version,
        fence: completion.fence.clone(),
        lease_id: completion.lease_id,
        lease_version: completion.lease_version,
        requirement_revision: completion.requirement_revision,
        content_hash: completion.content_hash.clone(),
        terminal_receipt_ref: completion.terminal_receipt_ref.clone(),
        terminal_receipt_sha256: completion.terminal_receipt_sha256.clone(),
        workflow_receipt_ref: completion.workflow_receipt_ref.clone(),
        workflow_receipt_sha256: completion.workflow_receipt_sha256.clone(),
        idempotency_key: completion.idempotency_key.clone(),
    })
}

pub fn prepare_critical_pm_reservation(
    source: CriticalPmReservationSource<'_>,
    actor: &Principal,
    config: &SdlcConfig,
    command: &ReserveCriticalPmReview,
    allocation: CriticalPmReservationAllocation,
) -> Result<CriticalPmReservationPlan, AppError> {
    crate::sdlc_reservation::scheduler(actor, config)?;
    command.validate()?;
    verify_source(&source, config, command)?;
    let previous = source.previous_reservation;
    if allocation.assignment_id.is_nil()
        || allocation.execution_id.is_nil()
        || allocation.assignment_id == allocation.execution_id
        || [
            previous.assignment.assignment_id,
            previous.assignment.execution_id,
            source.original_pm_assignment.assignment_id,
            source.original_pm_assignment.execution_id,
            source.blocker.proposal.fence.assignment_id,
            source.blocker.proposal.fence.execution_id,
        ]
        .iter()
        .any(|id| *id == allocation.assignment_id || *id == allocation.execution_id)
        || allocation.ordinal
            <= previous
                .execution
                .ordinal
                .parse::<i64>()
                .map_err(|_| conflict())?
    {
        return Err(conflict());
    }
    let assignment = PmAssignment {
        assignment_id: allocation.assignment_id,
        execution_id: allocation.execution_id,
        agent_id: previous.assignment.agent_id,
        version: next(previous.assignment.version)?,
        machine_subject: previous.assignment.machine_subject.clone(),
    };
    let mut blocker = source.blocker.clone();
    blocker.version = next(blocker.version)?;
    blocker.state = CriticalBlockerState::InPmReview;
    blocker.pm_review = Some(CriticalPmReviewRef {
        assignment_id: assignment.assignment_id,
        execution_id: assignment.execution_id,
    });
    let mut lifecycle = source.lifecycle.clone();
    lifecycle.task.version = next(lifecycle.task.version)?;
    let mut state = source.state.clone();
    state.assignment = Some(assignment.clone());
    state.draft_submode = Some(DraftSubmode::Brainstorming);
    state.confirmation_revision = None;
    let canonical = crate::sdlc_lifecycle::canonical_projection(
        &serde_json::to_value(&state).map_err(AppError::internal)?,
        &lifecycle.task,
    )?;
    state = serde_json::from_value(canonical).map_err(AppError::internal)?;
    let reservation = PmDraftReservation {
        interaction_protocol: previous.interaction_protocol,
        contract_version: 1,
        variant: PmDraftVariant::Reserved,
        binding: previous.binding.clone(),
        owner_cas: OwnerCas {
            expected_version: source.pm_owner_version,
            version: next(source.pm_owner_version)?,
        },
        assignment_operation_key: format!("pm-draft:{}", assignment.assignment_id),
        assignment,
        execution: PmDraftExecution {
            ordinal: allocation.ordinal.to_string(),
            key: format!("SDLC-{}", allocation.ordinal),
        },
        input: previous.input.clone(),
        admission_state: PmDraftAdmissionState::Reserved,
        dispatch_allowed: false,
    };
    let result = CriticalPmReservationResult {
        purpose: CriticalPmReservationPurpose::CriticalBusinessReview,
        reservation,
        context: CriticalPmReviewContext {
            blocker_id: blocker.blocker_id,
            blocker_sha256: blocker.blocker_sha256.clone(),
            blocker_version: blocker.version,
            requirements: CriticalPmRequirementsRef {
                revision: source.blocker.proposal.requirement_revision,
                content_hash: source.blocker.proposal.content_hash.clone(),
            },
        },
    };
    Ok(CriticalPmReservationPlan {
        state,
        lifecycle,
        blocker,
        result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use domain::sdlc_routing::{
        RoleRoute, RoleRoutes, RoutingPolicy, RoutingVerification, TaskRoutingSnapshot,
    };
    use serde_json::json;

    // Producer-shaped synthetic records, not independently verified native/Workflow evidence.
    struct Fixture {
        state: TaskState,
        lifecycle: LifecycleReceipt,
        blocker: CriticalBlockerReceipt,
        previous: PmDraftReservation,
        completion: PmDraftCompletionReceipt,
        original_pm_assignment: PmAssignment,
        input: PmDraftInputResponse,
        config: SdlcConfig,
        scheduler: Principal,
        command: ReserveCriticalPmReview,
    }

    impl Fixture {
        fn new() -> Self {
            let now = Utc.with_ymd_and_hms(2026, 10, 14, 12, 0, 0).unwrap();
            let task = Uuid::new_v4();
            let project = Uuid::new_v4();
            let route = |role: &str, id: u8| RoleRoute {
                agent_id: Uuid::new_v4(),
                fleet_config_revision: 1,
                package_commit: None,
                package_manifest_sha256: None,
                instruction_profile: Some("single-task-v1".into()),
                instruction_manifest_sha256: Some("b".repeat(64)),
                namespace_id: id.to_string(),
                namespace_name: format!("single-task-{}", role.replace('_', "-")),
                workflow_id: (id + 10).to_string(),
                workflow_key: format!("single-task-v1:{role}"),
                profile: format!("single-task-v1-{}", role.replace('_', "-")),
                workflow_catalog_version: 1,
                workflow_catalog_sha256: "c".repeat(64),
            };
            let routes = RoleRoutes {
                project_manager: route("project_manager", 1),
                analyst: route("analyst", 2),
                architect: route("architect", 3),
                developer: route("developer", 4),
                reviewer: route("reviewer", 5),
                tester: route("tester", 6),
                devops: route("devops", 7),
            };
            let document: RequirementsDocument = serde_json::from_value(json!({
                "goal":"Synthetic retention goal","scope":["retention"],"exclusions":[],
                "scenarios":["Delete requested records"],"acceptance_criteria":["Deletion is verifiable"],
                "constraints":[],"dependencies":[],"assumptions":[],"checklist":[],"prerequisites":[]
            })).unwrap();
            let hash = crate::sdlc::canonical_hash(&document).unwrap();
            let confirmation = Uuid::new_v4();
            let routing = TaskRoutingSnapshot {
                contract_version: 2,
                snapshot_id: Uuid::new_v4(),
                tracker_instance_id: "synthetic-tracker".into(),
                project_id: project,
                task_id: task,
                root_task_id: task,
                confirmation_id: confirmation,
                requirement_revision: 1,
                content_hash: hash.clone(),
                policy: RoutingPolicy {
                    contract_version: 2,
                    tracker_instance_id: "synthetic-tracker".into(),
                    project_id: project,
                    version: 1,
                    routing_hash: crate::sdlc::canonical_hash(&routes).unwrap(),
                    routes,
                    verification: RoutingVerification::Declared,
                    native_ready: false,
                    dispatch_allowed: false,
                    author_subject: "owner".into(),
                    created_at: now,
                },
                created_at: now,
            };
            let input = PmDraftInputResponse {
                contract_version: 1,
                tracker_instance_id: "synthetic-tracker".into(),
                project_id: project,
                task_id: task,
                root_task_id: task,
                owner_subject: "owner".into(),
                input: PmDraftInput {
                    snapshot_ref: Uuid::new_v4(),
                    title: "Original owner input".into(),
                    description: "Retain exact original bytes.\nNo replacement with the blocker."
                        .into(),
                    sha256: crate::sdlc::pm_draft_input_hash(
                        "Original owner input",
                        "Retain exact original bytes.\nNo replacement with the blocker.",
                    )
                    .unwrap(),
                },
            };
            let previous = PmDraftReservation {
                interaction_protocol: None,
                contract_version: 1,
                variant: PmDraftVariant::Reserved,
                binding: PmDraftBinding {
                    tracker_instance_id: input.tracker_instance_id.clone(),
                    project_id: project,
                    task_id: task,
                    root_task_id: task,
                    owner_subject: "owner".into(),
                },
                owner_cas: OwnerCas {
                    expected_version: 0,
                    version: 1,
                },
                assignment: PmAssignment {
                    assignment_id: Uuid::new_v4(),
                    execution_id: Uuid::new_v4(),
                    agent_id: routing.policy.routes.project_manager.agent_id,
                    version: 1,
                    machine_subject: "pm".into(),
                },
                execution: PmDraftExecution {
                    ordinal: "1".into(),
                    key: "SDLC-1".into(),
                },
                input: domain::sdlc_metadata::MetadataInputRef {
                    snapshot_ref: input.input.snapshot_ref,
                    sha256: input.input.sha256.clone(),
                },
                assignment_operation_key: String::new(),
                admission_state: PmDraftAdmissionState::Reserved,
                dispatch_allowed: false,
            };
            let mut previous = previous;
            previous.assignment_operation_key =
                format!("pm-draft:{}", previous.assignment.assignment_id);
            let completion = PmDraftCompletionReceipt {
                contract_version: 1,
                completion_id: Uuid::new_v4(),
                tracker_instance_id: input.tracker_instance_id.clone(),
                project_id: project,
                task_id: task,
                root_task_id: task,
                owner_subject: "owner".into(),
                fence: MachineFence {
                    assignment_id: previous.assignment.assignment_id,
                    execution_id: previous.assignment.execution_id,
                    agent_id: previous.assignment.agent_id,
                    assignment_version: 1,
                },
                owner_version: 1,
                lease_id: Uuid::new_v4(),
                lease_version: 4,
                requirement_revision: 1,
                content_hash: hash.clone(),
                terminal_receipt_ref: "fixture://actual-row-not-live-native-proof".into(),
                terminal_receipt_sha256: "e".repeat(64),
                workflow_receipt_ref: "fixture://actual-row-not-live-workflow-proof".into(),
                workflow_receipt_sha256: "f".repeat(64),
                previous_pm_admission_state: "reserved".into(),
                next_pm_admission_state: "completed".into(),
                idempotency_key: "original-pm-complete".into(),
                created_at: now,
            };
            let lifecycle = LifecycleReceipt {
                task: LifecycleTask {
                    task_id: task,
                    owner_subject: "owner".into(),
                    version: 8,
                    stage: LifecycleStage::Draft,
                    status: LifecycleStatus::Ready,
                    cycle: 2,
                    transient_retries: 0,
                    waiting_reason: Some(PM_REVIEW_REASON.into()),
                    runtime: RuntimeState::Terminal,
                    routing,
                    previous_result_id: None,
                    resume_chat_id: None,
                },
                assignment: None,
                lease: None,
                binding: None,
                result: None,
            };
            let proposal: SubmitCriticalBlocker = serde_json::from_value(json!({
                "fence":{"assignment_id":Uuid::new_v4(),"execution_id":Uuid::new_v4(),
                    "fencing_token":9,"lease_id":Uuid::new_v4(),"expected_lease_version":3},
                "requirement_revision":1,"content_hash":hash,
                "kind":"requirements_contradiction","question":"Delete or retain?",
                "rationale":"Retention and deletion conflict.","options":[],"recommendation":null,
                "evidence":[],"idempotency_key":"original-blocker"
            }))
            .unwrap();
            let mut proposal = proposal;
            proposal
                .evidence
                .push(domain::attachment_artifact::AttachmentArtifact {
                    receipt: ArtifactReceipt {
                        reference: format!("tracker-attachment:{}", Uuid::new_v4()),
                        sha256: "a".repeat(64),
                    },
                    binding: domain::attachment_artifact::ArtifactBinding {
                        assignment_id: proposal.fence.assignment_id,
                        execution_id: proposal.fence.execution_id,
                        role: DeliveryRole::Developer,
                    },
                });
            let chat = Uuid::new_v4();
            let run = Uuid::new_v4();
            let blocker = CriticalBlockerReceipt {
                blocker_id: Uuid::new_v4(),
                version: 2,
                tracker_instance_id: input.tracker_instance_id.clone(),
                project_id: project,
                task_id: task,
                root_task_id: task,
                owner_subject: "owner".into(),
                producer_subject: "technical-child".into(),
                agent_id: lifecycle.task.routing.policy.routes.developer.agent_id,
                role: DeliveryRole::Developer,
                assignment_hash: "d".repeat(64),
                chat_id: chat,
                run_id: run,
                routing_snapshot_id: lifecycle.task.routing.snapshot_id,
                routing_generation: 1,
                blocker_sha256: crate::sdlc::canonical_hash(&proposal).unwrap(),
                proposal,
                state: CriticalBlockerState::AwaitingPmReview,
                pm_review: None,
                resolution: None,
                technical_terminal: Some(RuntimeTerminalObservation {
                    assignment_hash: "d".repeat(64),
                    chat_id: chat,
                    run_id: run,
                    state: RuntimeTerminalState::Cancelled,
                    receipt: ArtifactReceipt {
                        reference: "fixture://retained-return-terminal-not-live-proof".into(),
                        sha256: "a".repeat(64),
                    },
                }),
                created_at: now,
            };
            let raw = json!({"tracker_instance_id":input.tracker_instance_id,"project_id":project,
                "task_id":task,"root_task_id":task,"owner_subject":"owner","stage":"Draft",
                "status":"Ready","draft_submode":"awaiting_confirmation","confirmation_revision":1,
                "assignment":previous.assignment,"questions":[],"revisions":[{
                    "revision":1,"content_hash":completion.content_hash,"author_subject":"pm",
                    "created_at":now,"goal":document.goal,"scope":document.scope,
                    "exclusions":document.exclusions,"scenarios":document.scenarios,
                    "acceptance_criteria":document.acceptance_criteria,"constraints":document.constraints,
                    "dependencies":document.dependencies,"assumptions":document.assumptions,
                    "checklist":document.checklist,"prerequisites":document.prerequisites}],
                "confirmations":[{"id":confirmation,"task_id":task,"revision":1,
                    "content_hash":completion.content_hash,"owner_subject":"owner","created_at":now,"stage":"Analysis"}],
                "evidence":[],"flow":"single-task-v1","waiting_reason":PM_REVIEW_REASON,
                "runtime_state":"terminal","lifecycle":lifecycle.task});
            let state = serde_json::from_value(raw).unwrap();
            let config = SdlcConfig {
                single_task_project_ids: [project].into(),
                reservation_scheduler_subject: "scheduler".into(),
                instance_id: input.tracker_instance_id.clone(),
                orchestrator_subject: "pm".into(),
                verifier_subject: "verifier".into(),
            };
            let command = ReserveCriticalPmReview {
                blocker_id: blocker.blocker_id,
                blocker_sha256: blocker.blocker_sha256.clone(),
                expected_blocker_version: 2,
                expected_task_version: lifecycle.task.version,
                expected_pm_owner_version: 1,
                expected_pm_assignment_version: 1,
                idempotency_key: "original-critical-pm-reservation".into(),
            };
            let original_pm_assignment = previous.assignment.clone();
            Self {
                state,
                lifecycle,
                blocker,
                previous,
                completion,
                original_pm_assignment,
                input,
                config,
                command,
                scheduler: Principal {
                    subject: "scheduler".into(),
                    human_session: false,
                    trusted_human: false,
                    scopes: ["task-tracker:write".into()].into(),
                },
            }
        }

        fn source(&self) -> CriticalPmReservationSource<'_> {
            CriticalPmReservationSource {
                state: &self.state,
                lifecycle: &self.lifecycle,
                blocker: &self.blocker,
                previous_reservation: &self.previous,
                current_completed: CompletedPmReceipt::Historical(&self.completion),
                original_pm_assignment: &self.original_pm_assignment,
                original_input: &self.input,
                pm_owner_version: self.previous.owner_cas.version,
                pm_admission_state: "completed",
            }
        }

        fn configured_completion(&mut self) -> ConfiguredPmCompletionReceipt {
            use domain::sdlc_configured_cli::ConfiguredWorkflowReceipt;
            let subject = Uuid::new_v4().to_string();
            self.previous.interaction_protocol = Some(InteractionProtocol::ConfiguredCliV1);
            self.previous.assignment.machine_subject = subject.clone();
            self.original_pm_assignment.machine_subject = subject.clone();
            self.state.assignment = Some(self.previous.assignment.clone());
            self.state
                .retained_fields
                .insert("interaction_protocol".into(), json!("configured-cli-v1"));
            self.state.retained_fields.insert(
                "workflow_task_key".into(),
                json!(self.previous.execution.key),
            );
            for confirmation in &mut self.state.confirmations {
                confirmation.stage = Stage::Draft;
            }
            let confirmation = self.state.current_confirmation().unwrap();
            let workflow = ConfiguredWorkflowReceipt {
                interaction_protocol: InteractionProtocol::ConfiguredCliV1,
                execution_ref: Uuid::new_v4(),
                task: self.previous.execution.key.clone(),
                tracker_task_id: self.state.task_id,
                namespace_id: 1,
                concrete_agent_ref: self.previous.assignment.agent_id,
                assignment_ref: self.previous.assignment.assignment_id,
                tracker_execution_id: self.previous.assignment.execution_id,
                assignment_version: self.previous.assignment.version,
                machine_subject: subject,
                operation_key: "approved-final-phase".into(),
                request_sha256: "a".repeat(64),
                result_version: 4,
                requirements_revision: Some(confirmation.revision),
                requirements_sha256: Some(confirmation.content_hash.clone()),
                owner_subject: self.state.owner_subject.clone(),
                confirmation_id: Some(confirmation.id),
                history_id: 7,
                status: "done".into(),
            };
            let command = CompleteConfiguredPm {
                interaction_protocol: InteractionProtocol::ConfiguredCliV1,
                fence: self.completion.fence.clone(),
                requirement_revision: confirmation.revision,
                content_hash: confirmation.content_hash.clone(),
                namespace_id: workflow.namespace_id,
                workflow_execution_ref: workflow.execution_ref,
                workflow_operation_key: workflow.operation_key.clone(),
                workflow_result_version: workflow.result_version,
                idempotency_key: "approved-pm-completion".into(),
            };
            crate::sdlc_configured_cli::completion_result(
                &self.state,
                &command,
                workflow,
                self.previous.owner_cas.version,
                Uuid::new_v4(),
                self.completion.created_at,
            )
            .unwrap()
        }

        fn completed_generation(&mut self, version: i64) {
            assert!(version > 1);
            self.previous.assignment.assignment_id = Uuid::new_v4();
            self.previous.assignment.execution_id = Uuid::new_v4();
            self.previous.assignment.version = version;
            self.previous.assignment_operation_key =
                format!("pm-draft:{}", self.previous.assignment.assignment_id);
            self.previous.owner_cas = OwnerCas {
                expected_version: version - 1,
                version,
            };
            self.previous.execution = PmDraftExecution {
                ordinal: "7".into(),
                key: "SDLC-7".into(),
            };
            self.completion.completion_id = Uuid::new_v4();
            self.completion.fence = MachineFence {
                assignment_id: self.previous.assignment.assignment_id,
                execution_id: self.previous.assignment.execution_id,
                agent_id: self.previous.assignment.agent_id,
                assignment_version: version,
            };
            self.completion.owner_version = version;
            self.completion.idempotency_key = format!("completed-pm-generation-{version}");
            self.state.assignment = Some(self.previous.assignment.clone());
            self.command.expected_pm_owner_version = version;
            self.command.expected_pm_assignment_version = version;
        }

        fn allocation() -> CriticalPmReservationAllocation {
            CriticalPmReservationAllocation {
                assignment_id: Uuid::new_v4(),
                execution_id: Uuid::new_v4(),
                ordinal: 11,
            }
        }

        fn plan(&self) -> Result<CriticalPmReservationPlan, AppError> {
            prepare_critical_pm_reservation(
                self.source(),
                &self.scheduler,
                &self.config,
                &self.command,
                Self::allocation(),
            )
        }
    }

    #[test]
    fn configured_review_uses_the_saved_business_receipt_and_permanent_identity() {
        let mut f = Fixture::new();
        let completed = f.configured_completion();
        let mut source = f.source();
        source.current_completed = CompletedPmReceipt::Configured(&completed);
        let plan = prepare_critical_pm_reservation(
            source,
            &f.scheduler,
            &f.config,
            &f.command,
            Fixture::allocation(),
        )
        .unwrap();
        assert_eq!(
            plan.result.reservation.interaction_protocol,
            Some(InteractionProtocol::ConfiguredCliV1)
        );
        assert_eq!(
            plan.result.reservation.assignment.machine_subject,
            completed.workflow_receipt.machine_subject
        );
        assert_eq!(
            plan.state.retained_fields["workflow_task_key"],
            json!(completed.workflow_receipt.task)
        );
        assert_ne!(
            plan.result.reservation.execution.key,
            completed.workflow_receipt.task
        );
        assert!(plan.lifecycle.lease.is_none() && plan.lifecycle.binding.is_none());
        // A legacy native receipt is history, not authority for configured work.
        assert!(f.plan().is_err());
    }

    #[test]
    fn configured_review_rejects_a_changed_completion_or_machine_identity() {
        for changed in 0..6 {
            let mut f = Fixture::new();
            let mut completed = f.configured_completion();
            match changed {
                0 => completed.workflow_receipt.assignment_ref = Uuid::new_v4(),
                1 => completed.workflow_receipt.confirmation_id = Some(Uuid::new_v4()),
                2 => completed.workflow_receipt_sha256 = "b".repeat(64),
                3 => completed.content_hash = "c".repeat(64),
                4 => completed.owner_subject = "foreign".into(),
                _ => {
                    f.previous.assignment.machine_subject = f.config.orchestrator_subject.clone();
                    f.state.assignment = Some(f.previous.assignment.clone());
                }
            }
            let mut source = f.source();
            source.current_completed = CompletedPmReceipt::Configured(&completed);
            assert!(
                prepare_critical_pm_reservation(
                    source,
                    &f.scheduler,
                    &f.config,
                    &f.command,
                    Fixture::allocation()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn review_preserves_original_pm_task_input_and_history_without_dispatch_or_consent() {
        let f = Fixture::new();
        let before = json!({"state":f.state,"previous":f.previous,"completion":f.completion,
            "blocker":f.blocker,"input":f.input,"lifecycle":f.lifecycle});
        let allocation = Fixture::allocation();
        let new_assignment = allocation.assignment_id;
        let new_execution = allocation.execution_id;
        let plan = prepare_critical_pm_reservation(
            f.source(),
            &f.scheduler,
            &f.config,
            &f.command,
            allocation,
        )
        .unwrap();
        let result = &plan.result;
        assert_eq!(
            result.purpose,
            CriticalPmReservationPurpose::CriticalBusinessReview
        );
        assert_eq!(result.reservation.binding, f.previous.binding);
        assert_eq!(
            result.reservation.assignment.agent_id,
            f.previous.assignment.agent_id
        );
        assert_eq!(result.reservation.assignment.assignment_id, new_assignment);
        assert_eq!(result.reservation.assignment.execution_id, new_execution);
        assert_eq!(result.reservation.assignment.version, 2);
        assert_eq!(result.reservation.owner_cas.expected_version, 1);
        assert_eq!(result.reservation.owner_cas.version, 2);
        assert_eq!(result.reservation.execution.key, "SDLC-11");
        assert_eq!(
            serde_json::to_value(&result.reservation.input).unwrap(),
            serde_json::to_value(&f.previous.input).unwrap()
        );
        assert!(!result.reservation.dispatch_allowed);
        assert_eq!(
            plan.state.assignment.as_ref(),
            Some(&result.reservation.assignment)
        );
        assert_eq!(plan.state.draft_submode, Some(DraftSubmode::Brainstorming));
        assert_eq!(plan.state.confirmation_revision, None);
        assert!(!plan.state.ready());
        assert_eq!(
            json!(plan.state.confirmations),
            json!(f.state.confirmations)
        );
        assert_eq!(json!(plan.state.revisions), json!(f.state.revisions));
        assert_eq!(plan.lifecycle.task.cycle, 2);
        assert_eq!(plan.lifecycle.task.version, 9);
        assert_eq!(plan.lifecycle.task.runtime, RuntimeState::Terminal);
        assert!(
            plan.lifecycle.assignment.is_none()
                && plan.lifecycle.lease.is_none()
                && plan.lifecycle.binding.is_none()
                && plan.lifecycle.result.is_none()
        );
        assert_eq!(
            plan.state.retained_fields["lifecycle"],
            json!(plan.lifecycle.task)
        );
        assert_eq!(
            plan.state.retained_fields["runtime_state"],
            json!("terminal")
        );
        assert_eq!(plan.blocker.state, CriticalBlockerState::InPmReview);
        assert_eq!(plan.blocker.version, 3);
        let review = plan.blocker.pm_review.as_ref().unwrap();
        assert_eq!(
            (review.assignment_id, review.execution_id),
            (new_assignment, new_execution)
        );
        assert_eq!(
            plan.blocker.technical_terminal,
            f.blocker.technical_terminal
        );
        assert_eq!(result.context.blocker_sha256, f.blocker.blocker_sha256);
        assert_eq!(
            result.context.requirements.content_hash,
            f.completion.content_hash
        );
        assert_eq!(result.context.requirements.revision, 1);
        assert_eq!(result.context.blocker_version, 3);
        assert!(serde_json::from_value::<PmDraftReservation>(json!(result)).is_err());
        assert_eq!(
            before,
            json!({"state":f.state,"previous":f.previous,"completion":f.completion,
            "blocker":f.blocker,"input":f.input,"lifecycle":f.lifecycle})
        );
    }

    #[test]
    fn only_existing_scheduler_write_child_can_allocate_not_owner_or_old_pm_grant() {
        let f = Fixture::new();
        for (subject, human, scopes) in [
            ("owner", true, vec![]),
            (
                "pm",
                false,
                vec![
                    "task-tracker:write".into(),
                    f.previous.assignment.scope(f.state.task_id),
                ],
            ),
            ("scheduler", false, vec!["task-tracker:read".into()]),
            ("foreign", false, vec!["task-tracker:write".into()]),
            (
                "scheduler",
                false,
                vec![
                    "task-tracker:write".into(),
                    f.previous.assignment.scope(f.state.task_id),
                ],
            ),
        ] {
            let actor = Principal {
                subject: subject.into(),
                human_session: human,
                trusted_human: human,
                scopes: scopes.into_iter().collect(),
            };
            assert!(matches!(
                prepare_critical_pm_reservation(
                    f.source(),
                    &actor,
                    &f.config,
                    &f.command,
                    Fixture::allocation()
                ),
                Err(AppError::Forbidden)
            ));
        }
    }

    #[test]
    fn exact_blocker_task_and_pm_cas_are_required() {
        for changed in 0..6 {
            let mut f = Fixture::new();
            match changed {
                0 => f.command.expected_blocker_version += 1,
                1 => f.command.expected_task_version += 1,
                2 => f.command.expected_pm_owner_version += 1,
                3 => f.command.expected_pm_assignment_version += 1,
                4 => f.command.blocker_id = Uuid::new_v4(),
                _ => f.command.blocker_sha256 = "f".repeat(64),
            }
            assert!(f.plan().is_err());
        }
    }

    #[test]
    fn forged_completed_or_foreign_previous_pm_context_is_rejected() {
        for changed in 0..9 {
            let mut f = Fixture::new();
            match changed {
                0 => f.completion.next_pm_admission_state = "reserved".into(),
                1 => f.completion.fence.agent_id = Uuid::new_v4(),
                2 => f.completion.fence.execution_id = Uuid::new_v4(),
                3 => f.completion.owner_subject = "foreign".into(),
                4 => f.completion.lease_id = Uuid::nil(),
                5 => f.previous.assignment.agent_id = Uuid::new_v4(),
                6 => f.previous.input.snapshot_ref = Uuid::new_v4(),
                7 => f.input.input.description.push_str("changed"),
                _ => f.previous.owner_cas.version = 2,
            }
            assert!(f.plan().is_err());
        }
        let f = Fixture::new();
        let mut source = f.source();
        source.pm_admission_state = "reserved";
        assert!(
            prepare_critical_pm_reservation(
                source,
                &f.scheduler,
                &f.config,
                &f.command,
                Fixture::allocation()
            )
            .is_err()
        );
    }

    #[test]
    fn subsequent_reviews_increment_actual_completed_generation_and_keep_frozen_pm_and_input() {
        for version in [2, 17] {
            let mut f = Fixture::new();
            let original = json!({"assignment":f.original_pm_assignment,"input":f.input});
            f.completed_generation(version);
            f.blocker.version = 6;
            f.blocker.routing_generation = 2;
            f.command.expected_blocker_version = 6;
            let previous = json!({"reservation":f.previous,"completion":f.completion});
            let plan = f.plan().unwrap();
            let reservation = &plan.result.reservation;
            assert_eq!(reservation.assignment.version, version + 1);
            assert_eq!(reservation.owner_cas.expected_version, version);
            assert_eq!(reservation.owner_cas.version, version + 1);
            assert_eq!(
                reservation.assignment.agent_id,
                f.original_pm_assignment.agent_id
            );
            assert_ne!(
                reservation.assignment.assignment_id,
                f.previous.assignment.assignment_id
            );
            assert_ne!(
                reservation.assignment.execution_id,
                f.previous.assignment.execution_id
            );
            assert_eq!(reservation.input.snapshot_ref, f.input.input.snapshot_ref);
            assert_eq!(reservation.input.sha256, f.input.input.sha256);
            assert_eq!(plan.blocker.version, 7);
            assert_eq!(plan.blocker.routing_generation, 2);
            assert!(!reservation.dispatch_allowed);
            assert_eq!(plan.lifecycle.task.runtime, RuntimeState::Terminal);
            assert_eq!(
                original,
                json!({"assignment":f.original_pm_assignment,"input":f.input})
            );
            assert_eq!(
                previous,
                json!({"reservation":f.previous,"completion":f.completion})
            );
        }
    }

    #[test]
    fn older_completion_or_generation_cannot_stand_in_for_current_completed_pm() {
        let mut f = Fixture::new();
        let older_completion = f.completion.clone();
        let older_reservation = f.previous.clone();
        f.completed_generation(2);
        let mut source = f.source();
        source.current_completed = CompletedPmReceipt::Historical(&older_completion);
        assert!(
            prepare_critical_pm_reservation(
                source,
                &f.scheduler,
                &f.config,
                &f.command,
                Fixture::allocation()
            )
            .is_err()
        );
        let mut source = f.source();
        source.previous_reservation = &older_reservation;
        assert!(
            prepare_critical_pm_reservation(
                source,
                &f.scheduler,
                &f.config,
                &f.command,
                Fixture::allocation()
            )
            .is_err()
        );
        assert!(f.plan().is_ok());

        let replacement = Uuid::new_v4();
        f.previous.assignment.agent_id = replacement;
        f.completion.fence.agent_id = replacement;
        f.state.assignment = Some(f.previous.assignment.clone());
        assert!(
            f.plan().is_err(),
            "even matching current records cannot replace the frozen PM"
        );
    }

    #[test]
    fn bounded_current_assignment_owner_and_blocker_versions_cannot_wrap() {
        for overflow in 0..3 {
            let mut f = Fixture::new();
            f.completed_generation(2);
            match overflow {
                0 => {
                    f.previous.assignment.version = MAX_SAFE_VERSION;
                    f.completion.fence.assignment_version = MAX_SAFE_VERSION;
                    f.state.assignment = Some(f.previous.assignment.clone());
                    f.command.expected_pm_assignment_version = MAX_SAFE_VERSION;
                }
                1 => {
                    f.previous.owner_cas = OwnerCas {
                        expected_version: MAX_SAFE_VERSION - 1,
                        version: MAX_SAFE_VERSION,
                    };
                    f.completion.owner_version = MAX_SAFE_VERSION;
                    f.command.expected_pm_owner_version = MAX_SAFE_VERSION;
                }
                _ => {
                    f.blocker.version = MAX_SAFE_VERSION;
                    f.command.expected_blocker_version = MAX_SAFE_VERSION;
                }
            }
            assert!(f.plan().is_err());
        }
    }

    #[test]
    fn contradictory_canonical_or_missing_return_context_never_plans() {
        for changed in 0..10 {
            let mut f = Fixture::new();
            match changed {
                0 => f.state.status = Some(LifecycleStatus::InProgress),
                1 => f.state.retained_fields.get_mut("lifecycle").unwrap()["version"] = json!(1),
                2 => {
                    f.state
                        .retained_fields
                        .insert("runtime_state".into(), json!("running"));
                }
                3 => f.blocker.state = CriticalBlockerState::Held,
                4 => f.blocker.technical_terminal = None,
                5 => f.blocker.technical_terminal.as_mut().unwrap().run_id = Uuid::new_v4(),
                6 => f.blocker.proposal.question.push('?'),
                7 => f
                    .state
                    .revisions
                    .last_mut()
                    .unwrap()
                    .document
                    .goal
                    .push('?'),
                8 => f.lifecycle.task.routing.task_id = Uuid::new_v4(),
                _ => f.state.confirmations.clear(),
            }
            assert!(f.plan().is_err());
        }
    }

    #[test]
    fn server_allocation_must_be_new_and_version_overflow_does_not_reset_cas() {
        let f = Fixture::new();
        for changed in 0..5 {
            let mut allocation = Fixture::allocation();
            match changed {
                0 => allocation.assignment_id = Uuid::nil(),
                1 => allocation.execution_id = f.previous.assignment.execution_id,
                2 => allocation.assignment_id = f.blocker.proposal.fence.assignment_id,
                3 => allocation.ordinal = 1,
                _ => allocation.ordinal = -1,
            }
            assert!(
                prepare_critical_pm_reservation(
                    f.source(),
                    &f.scheduler,
                    &f.config,
                    &f.command,
                    allocation
                )
                .is_err()
            );
        }
        let mut f = Fixture::new();
        f.lifecycle.task.version = MAX_SAFE_VERSION;
        f.command.expected_task_version = MAX_SAFE_VERSION;
        f.state
            .retained_fields
            .insert("lifecycle".into(), json!(f.lifecycle.task));
        assert!(f.plan().is_err());
    }
}
