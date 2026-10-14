//! Pure fresh planning only. The caller owns persisted reads, replay and one task transaction.
use domain::sdlc::*;
use domain::sdlc_critical_blockers::*;
use domain::sdlc_critical_pm_reservation::*;
use domain::sdlc_lifecycle::*;
use domain::sdlc_pm_draft::*;
use domain::sdlc_reservation::{CompletePmDraft, PmDraftCompletionReceipt};
use shared::AppError;
use uuid::Uuid;

pub struct CriticalPmReservationSource<'a> {
    pub state: &'a TaskState,
    pub lifecycle: &'a LifecycleReceipt,
    pub blocker: &'a CriticalBlockerReceipt,
    pub initial_reservation: &'a PmDraftReservation,
    pub initial_completion: &'a PmDraftCompletionReceipt,
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
    let initial = source.initial_reservation;
    let completion = source.initial_completion;
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
        || state.assignment.as_ref() != Some(&initial.assignment)
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
        || blocker.version != 2
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
        || blocker.routing_generation != 1
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

    let ordinal = initial.execution.ordinal.parse::<i64>().ok();
    if initial.contract_version != 1
        || !matches!(initial.variant, PmDraftVariant::Reserved)
        || !matches!(initial.admission_state, PmDraftAdmissionState::Reserved)
        || initial.dispatch_allowed
        || initial.binding.tracker_instance_id != state.tracker_instance_id
        || initial.binding.project_id != state.project_id
        || initial.binding.task_id != state.task_id
        || initial.binding.root_task_id != state.task_id
        || initial.binding.owner_subject != state.owner_subject
        || initial.assignment.assignment_id.is_nil()
        || initial.assignment.execution_id.is_nil()
        || initial.assignment.agent_id.is_nil()
        || initial.assignment.version != 1
        || initial.assignment.version != command.expected_pm_assignment_version
        || initial.assignment.machine_subject != config.orchestrator_subject
        || initial.assignment.machine_subject.trim().is_empty()
        || initial.assignment.machine_subject == state.owner_subject
        || initial.assignment.machine_subject == config.verifier_subject
        || initial.assignment.machine_subject == config.reservation_scheduler_subject
        || initial.owner_cas.expected_version != 0
        || initial.owner_cas.version != 1
        || source.pm_owner_version != initial.owner_cas.version
        || source.pm_owner_version != command.expected_pm_owner_version
        || source.pm_admission_state != "completed"
        || ordinal.is_none_or(|n| n <= 0 || n.to_string() != initial.execution.ordinal)
        || initial.execution.key != format!("SDLC-{}", initial.execution.ordinal)
        || initial.assignment_operation_key
            != format!("pm-draft:{}", initial.assignment.assignment_id)
        || input.contract_version != 1
        || input.tracker_instance_id != state.tracker_instance_id
        || input.project_id != state.project_id
        || input.task_id != state.task_id
        || input.root_task_id != state.task_id
        || input.owner_subject != state.owner_subject
        || input.input.snapshot_ref.is_nil()
        || input.input.snapshot_ref != initial.input.snapshot_ref
        || input.input.sha256 != initial.input.sha256
        || input.input.sha256
            != crate::sdlc::pm_draft_input_hash(&input.input.title, &input.input.description)?
        || completion.contract_version != 1
        || completion.completion_id.is_nil()
        || completion.lease_id.is_nil()
        || !(1..=MAX_SAFE_VERSION).contains(&completion.lease_version)
        || completion.tracker_instance_id != state.tracker_instance_id
        || completion.project_id != state.project_id
        || completion.task_id != state.task_id
        || completion.root_task_id != state.task_id
        || completion.owner_subject != state.owner_subject
        || completion.fence.assignment_id != initial.assignment.assignment_id
        || completion.fence.execution_id != initial.assignment.execution_id
        || completion.fence.agent_id != initial.assignment.agent_id
        || completion.fence.assignment_version != initial.assignment.version
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
    let initial = source.initial_reservation;
    if allocation.assignment_id.is_nil()
        || allocation.execution_id.is_nil()
        || allocation.assignment_id == allocation.execution_id
        || [
            initial.assignment.assignment_id,
            initial.assignment.execution_id,
            source.blocker.proposal.fence.assignment_id,
            source.blocker.proposal.fence.execution_id,
        ]
        .iter()
        .any(|id| *id == allocation.assignment_id || *id == allocation.execution_id)
        || allocation.ordinal
            <= initial
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
        agent_id: initial.assignment.agent_id,
        version: next(initial.assignment.version)?,
        machine_subject: initial.assignment.machine_subject.clone(),
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
        contract_version: 1,
        variant: PmDraftVariant::Reserved,
        binding: initial.binding.clone(),
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
        input: initial.input.clone(),
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
        initial: PmDraftReservation,
        completion: PmDraftCompletionReceipt,
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
            let initial = PmDraftReservation {
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
            let mut initial = initial;
            initial.assignment_operation_key =
                format!("pm-draft:{}", initial.assignment.assignment_id);
            let completion = PmDraftCompletionReceipt {
                contract_version: 1,
                completion_id: Uuid::new_v4(),
                tracker_instance_id: input.tracker_instance_id.clone(),
                project_id: project,
                task_id: task,
                root_task_id: task,
                owner_subject: "owner".into(),
                fence: MachineFence {
                    assignment_id: initial.assignment.assignment_id,
                    execution_id: initial.assignment.execution_id,
                    agent_id: initial.assignment.agent_id,
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
                "assignment":initial.assignment,"questions":[],"revisions":[{
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
            Self {
                state,
                lifecycle,
                blocker,
                initial,
                completion,
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
                initial_reservation: &self.initial,
                initial_completion: &self.completion,
                original_input: &self.input,
                pm_owner_version: 1,
                pm_admission_state: "completed",
            }
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
    fn review_preserves_original_pm_task_input_and_history_without_dispatch_or_consent() {
        let f = Fixture::new();
        let before = json!({"state":f.state,"initial":f.initial,"completion":f.completion,
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
        assert_eq!(result.reservation.binding, f.initial.binding);
        assert_eq!(
            result.reservation.assignment.agent_id,
            f.initial.assignment.agent_id
        );
        assert_eq!(result.reservation.assignment.assignment_id, new_assignment);
        assert_eq!(result.reservation.assignment.execution_id, new_execution);
        assert_eq!(result.reservation.assignment.version, 2);
        assert_eq!(result.reservation.owner_cas.expected_version, 1);
        assert_eq!(result.reservation.owner_cas.version, 2);
        assert_eq!(result.reservation.execution.key, "SDLC-11");
        assert_eq!(
            serde_json::to_value(&result.reservation.input).unwrap(),
            serde_json::to_value(&f.initial.input).unwrap()
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
            json!({"state":f.state,"initial":f.initial,"completion":f.completion,
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
                    f.initial.assignment.scope(f.state.task_id),
                ],
            ),
            ("scheduler", false, vec!["task-tracker:read".into()]),
            ("foreign", false, vec!["task-tracker:write".into()]),
            (
                "scheduler",
                false,
                vec![
                    "task-tracker:write".into(),
                    f.initial.assignment.scope(f.state.task_id),
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
    fn forged_completed_or_foreign_initial_pm_context_is_rejected() {
        for changed in 0..9 {
            let mut f = Fixture::new();
            match changed {
                0 => f.completion.next_pm_admission_state = "reserved".into(),
                1 => f.completion.fence.agent_id = Uuid::new_v4(),
                2 => f.completion.fence.execution_id = Uuid::new_v4(),
                3 => f.completion.owner_subject = "foreign".into(),
                4 => f.completion.lease_id = Uuid::nil(),
                5 => f.initial.assignment.agent_id = Uuid::new_v4(),
                6 => f.initial.input.snapshot_ref = Uuid::new_v4(),
                7 => f.input.input.description.push_str("changed"),
                _ => f.initial.owner_cas.version = 2,
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
                1 => allocation.execution_id = f.initial.assignment.execution_id,
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
