use super::*;
use chrono::{Duration, TimeZone, Utc};
use domain::attachment_artifact::{ArtifactBinding, AttachmentArtifact};
use domain::sdlc_execution_lease::ExecutionLease;
use domain::sdlc_routing::{
    RoleRoute, RoleRoutes, RoutingPolicy, RoutingVerification, TaskRoutingSnapshot,
};
use serde_json::json;

// All identities, evidence and terminal observations are synthetic. No live runtime is exercised.
struct Fixture {
    current: LifecycleReceipt,
    config: SdlcConfig,
    scheduler: Principal,
    producer: Principal,
    proposal: SubmitCriticalBlocker,
    now: chrono::DateTime<Utc>,
}

fn artifact(reference: &str) -> ArtifactReceipt {
    ArtifactReceipt {
        reference: reference.into(),
        sha256: "a".repeat(64),
    }
}

impl Fixture {
    fn new(stage: LifecycleStage) -> Self {
        let now = Utc.with_ymd_and_hms(2026, 10, 14, 12, 0, 0).unwrap();
        let task_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
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
        let role = stage.role().unwrap();
        let agent_id = routes
            .entries()
            .into_iter()
            .find(|(key, _)| *key == role.key())
            .unwrap()
            .1
            .agent_id;
        let routing_hash = crate::sdlc::canonical_hash(&routes).unwrap();
        let routing = TaskRoutingSnapshot {
            contract_version: 2,
            snapshot_id: Uuid::new_v4(),
            tracker_instance_id: "synthetic-tracker".into(),
            project_id,
            task_id,
            root_task_id: task_id,
            confirmation_id: Uuid::new_v4(),
            requirement_revision: 1,
            content_hash: "d".repeat(64),
            policy: RoutingPolicy {
                contract_version: 2,
                tracker_instance_id: "synthetic-tracker".into(),
                project_id,
                version: 1,
                routing_hash,
                routes,
                verification: RoutingVerification::Declared,
                native_ready: false,
                dispatch_allowed: false,
                author_subject: "owner".into(),
                created_at: now,
            },
            created_at: now,
        };
        let lease = ExecutionLease {
            lease_id: Uuid::new_v4(),
            version: 1,
            holder_subject: "scheduler".into(),
            claimed_at: now,
            heartbeat_at: now,
            expires_at: now + Duration::seconds(30),
        };
        let mut assignment = DeliveryAssignment {
            machine_subject: None,
            assignment_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            execution: Some(DeliveryExecution::from_ordinal(7).unwrap()),
            task_id,
            owner_subject: "owner".into(),
            role,
            stage,
            cycle: 0,
            attempt: 1,
            agent_id,
            routing: routing.clone(),
            previous_result_id: None,
            fencing_token: 1,
            lease_id: lease.lease_id,
            assignment_hash: String::new(),
        };
        assignment.assignment_hash = crate::sdlc_lifecycle::assignment_hash(&assignment).unwrap();
        let fence = DeliveryFence {
            assignment_id: assignment.assignment_id,
            execution_id: assignment.execution_id,
            fencing_token: 1,
            lease_id: lease.lease_id,
            expected_lease_version: 1,
        };
        let producer = Principal {
            subject: "child".into(),
            human_session: false,
            trusted_human: false,
            scopes: [
                "task-tracker:read".into(),
                "task-tracker:write".into(),
                assignment.scope(),
            ]
            .into(),
        };
        let proposal = SubmitCriticalBlocker {
            fence,
            requirement_revision: 1,
            content_hash: routing.content_hash.clone(),
            kind: CriticalBlockerKind::MissingBusinessDecision,
            question: "Which retention rule is authoritative?".into(),
            rationale: "Approved requirements contain mutually exclusive rules.".into(),
            options: vec![
                CriticalBlockerOption {
                    option_id: Uuid::new_v4(),
                    label: "Delete".into(),
                    consequences: "No later recovery.".into(),
                },
                CriticalBlockerOption {
                    option_id: Uuid::new_v4(),
                    label: "Retain".into(),
                    consequences: "Retention exception required.".into(),
                },
            ],
            recommendation: None,
            evidence: vec![AttachmentArtifact {
                receipt: artifact(&format!("tracker-attachment:{}", Uuid::new_v4())),
                binding: ArtifactBinding {
                    assignment_id: assignment.assignment_id,
                    execution_id: assignment.execution_id,
                    role,
                },
            }],
            idempotency_key: "synthetic-blocker".into(),
        };
        Self {
            current: LifecycleReceipt {
                task: LifecycleTask {
                    task_id,
                    owner_subject: "owner".into(),
                    version: 5,
                    stage,
                    status: LifecycleStatus::InProgress,
                    cycle: 0,
                    transient_retries: 0,
                    waiting_reason: None,
                    runtime: RuntimeState::Running,
                    routing,
                    previous_result_id: None,
                    resume_chat_id: None,
                },
                assignment: Some(assignment),
                lease: Some(lease),
                binding: Some(RunBinding {
                    chat_id: Uuid::new_v4(),
                    run_id: Uuid::new_v4(),
                    machine_subject: "child".into(),
                    dispatch_receipt: artifact("synthetic-dispatch"),
                }),
                result: None,
            },
            config: SdlcConfig {
                single_task_project_ids: [project_id].into(),
                reservation_scheduler_subject: "scheduler".into(),
                instance_id: "synthetic-tracker".into(),
                orchestrator_subject: "pm".into(),
                verifier_subject: "verifier".into(),
            },
            scheduler: Principal {
                subject: "scheduler".into(),
                human_session: false,
                trusted_human: false,
                scopes: ["task-tracker:write".into()].into(),
            },
            producer,
            proposal,
            now,
        }
    }

    fn plan(&self) -> Result<CriticalPolicyChange, AppError> {
        prepare_and_hold(
            &self.current,
            &self.producer,
            &self.config,
            &self.proposal,
            BlockerAllocation {
                blocker_id: Uuid::new_v4(),
                routing_generation: 1,
                observed_at: self.now,
            },
        )
    }

    fn guard(&self, blocker: &CriticalBlockerReceipt) -> CriticalPmReviewGuard {
        let mut fence = self.proposal.fence.clone();
        fence.expected_lease_version = self.current.lease.as_ref().unwrap().version;
        CriticalPmReviewGuard {
            blocker_id: blocker.blocker_id,
            blocker_sha256: blocker.blocker_sha256.clone(),
            expected_blocker_version: blocker.version,
            expected_task_version: self.current.task.version,
            fence,
            idempotency_key: "synthetic-return".into(),
        }
    }

    fn hold(&mut self) -> CriticalBlockerReceipt {
        let change = self.plan().unwrap();
        self.current = change.lifecycle;
        change.blocker
    }

    fn handoff(&self, blocker: &CriticalBlockerReceipt) -> ReturnCriticalToPm {
        ReturnCriticalToPm {
            review: self.guard(blocker),
            terminal: RuntimeTerminalObservation {
                assignment_hash: blocker.assignment_hash.clone(),
                chat_id: blocker.chat_id,
                run_id: blocker.run_id,
                state: RuntimeTerminalState::Cancelled,
                receipt: artifact("synthetic-actual-native-terminal"),
            },
        }
    }
}

#[test]
fn every_technical_stage_can_hold_without_releasing_custody_or_completing_workflow() {
    for stage in [
        LifecycleStage::Analysis,
        LifecycleStage::Architecture,
        LifecycleStage::Development,
        LifecycleStage::Review,
        LifecycleStage::Testing,
        LifecycleStage::Rework,
        LifecycleStage::Deployment,
    ] {
        let mut f = Fixture::new(stage);
        let before = f.current.clone();
        let blocker = f.hold();
        let mut expected = before.clone();
        expected.task.version += 1;
        expected.task.waiting_reason = Some(HOLD_REASON.into());
        assert_eq!(f.current, expected);
        assert_eq!(blocker.state, CriticalBlockerState::Held);
        assert_eq!(blocker.version, 1);
        assert_eq!(blocker.producer_subject, f.producer.subject);
        assert_eq!(f.current.assignment, before.assignment);
        assert_eq!(f.current.lease, before.lease);
        assert_eq!(f.current.binding, before.binding);
        assert_eq!(f.current.task.routing, before.task.routing);
        assert_eq!(f.current.task.stage, stage);
        assert_eq!(f.current.task.status, LifecycleStatus::InProgress);
        assert_eq!(f.current.task.runtime, RuntimeState::Running);
        assert_eq!(f.current.task.version, before.task.version + 1);
        assert_eq!(f.current.result, None);
        assert_eq!(blocker.technical_terminal, None);
        assert_eq!(blocker.pm_review, None);
        assert!(ensure_technical_progress_allowed(&f.current, Some(&blocker)).is_err());
        assert!(ensure_technical_progress_allowed(&f.current, None).is_err());
    }
}

#[test]
fn return_requires_terminal_and_preserves_generation_one_verbatim() {
    let mut f = Fixture::new(LifecycleStage::Review);
    let original = serde_json::to_value(&f.current.task.routing).unwrap();
    let blocker = f.hold();
    let command = f.handoff(&blocker);
    let change = return_to_pm(&f.current, &blocker, &f.scheduler, &f.config, &command).unwrap();
    assert_eq!(
        serde_json::to_value(&change.lifecycle.task.routing).unwrap(),
        original
    );
    assert_eq!(change.lifecycle.task.task_id, blocker.task_id);
    assert_eq!(change.lifecycle.task.owner_subject, blocker.owner_subject);
    assert_eq!(change.lifecycle.task.stage, LifecycleStage::Draft);
    assert_eq!(change.lifecycle.task.status, LifecycleStatus::Ready);
    assert_eq!(change.lifecycle.task.runtime, RuntimeState::Terminal);
    assert_eq!(
        change.lifecycle.task.waiting_reason.as_deref(),
        Some(PM_REVIEW_REASON)
    );
    assert_eq!(change.lifecycle.assignment, None);
    assert_eq!(change.lifecycle.lease, None);
    assert_eq!(change.lifecycle.binding, None);
    assert_eq!(change.lifecycle.task.previous_result_id, None);
    assert_eq!(change.lifecycle.task.resume_chat_id, None);
    assert_eq!(change.blocker.state, CriticalBlockerState::AwaitingPmReview);
    assert_eq!(change.blocker.version, 2);
    assert_eq!(change.blocker.technical_terminal, Some(command.terminal));
    assert_eq!(change.blocker.pm_review, None); // No fabricated/reused initial PM assignment.
    assert_eq!(change.blocker.resolution, None); // No fabricated owner confirmation/new revision.
    assert!(ensure_technical_progress_allowed(&change.lifecycle, Some(&change.blocker)).is_err());
    assert!(
        return_to_pm(
            &change.lifecycle,
            &change.blocker,
            &f.scheduler,
            &f.config,
            &f.handoff(&blocker)
        )
        .is_err()
    );
}

#[test]
fn return_transport_preserves_existing_typed_plan_and_body_hashes() {
    let mut f = Fixture::new(LifecycleStage::Review);
    let held = f.hold();
    let command = f.handoff(&held);
    let command_hash = crate::sdlc::canonical_hash(&command).unwrap();
    let held_hash = crate::sdlc::canonical_hash(&held).unwrap();
    validate_return_request(f.current.task.task_id, &f.scheduler, &command).unwrap();
    let plan = return_to_pm(&f.current, &held, &f.scheduler, &f.config, &command).unwrap();
    let wire = serde_json::to_value(&plan).unwrap();
    assert_eq!(wire.as_object().unwrap().len(), 2);
    assert!(wire.get("lifecycle").is_some());
    assert!(wire.get("blocker").is_some());
    let typed: domain::sdlc_critical_blockers::CriticalPolicyChange =
        serde_json::from_value(wire.clone()).unwrap();
    let compatibility: crate::sdlc_critical_blockers::CriticalPolicyChange = typed.clone();
    assert_eq!(compatibility, plan);
    assert_eq!(crate::sdlc::canonical_hash(&command).unwrap(), command_hash);
    assert_eq!(crate::sdlc::canonical_hash(&held).unwrap(), held_hash);
    let mut extra = wire.clone();
    extra["dispatch_allowed"] = json!(true);
    assert!(serde_json::from_value::<CriticalPolicyChange>(extra).is_err());
    for missing in ["lifecycle", "blocker"] {
        let mut invalid = wire.clone();
        invalid.as_object_mut().unwrap().remove(missing);
        assert!(serde_json::from_value::<CriticalPolicyChange>(invalid).is_err());
    }
}

#[test]
fn return_transport_actor_shapes_and_keys_fail_closed_without_rewriting_input() {
    let mut f = Fixture::new(LifecycleStage::Testing);
    let held = f.hold();
    let command = f.handoff(&held);
    let original = command.clone();
    for invalid in [
        ".",
        "..",
        "",
        "space key",
        "line\nkey",
        "return/original",
        "return\\original",
    ] {
        let mut changed = command.clone();
        changed.review.idempotency_key = invalid.into();
        assert!(validate_return_request(f.current.task.task_id, &f.scheduler, &changed).is_err());
        assert!(validate_return_key(f.current.task.task_id, invalid).is_err());
    }
    let key = "return:original:1?x=+%#";
    validate_return_key(f.current.task.task_id, key).unwrap();
    assert!(validate_return_key(Uuid::nil(), key).is_err());
    let mut reader = f.scheduler.clone();
    reader.scopes = ["task-tracker:read".into()].into();
    return_actor_shape(&reader, false).unwrap();
    assert!(return_actor_shape(&reader, true).is_err());
    assert!(return_actor_shape(&f.scheduler, false).is_err());
    for actor in [
        f.producer.clone(),
        Principal {
            human_session: true,
            ..f.scheduler.clone()
        },
        Principal {
            trusted_human: true,
            ..f.scheduler.clone()
        },
        Principal {
            scopes: ["task-tracker:write".into(), "admin".into()].into(),
            ..f.scheduler.clone()
        },
        Principal {
            scopes: [
                "task-tracker:write".into(),
                "task-tracker:sdlc:pm:invalid".into(),
            ]
            .into(),
            ..f.scheduler.clone()
        },
    ] {
        assert!(matches!(
            validate_return_request(f.current.task.task_id, &actor, &command),
            Err(AppError::Forbidden)
        ));
    }
    // Shape-only preflight is not scheduler authorization; the repository's configured policy is required.
    let foreign = Principal {
        subject: "foreign-machine".into(),
        ..f.scheduler.clone()
    };
    validate_return_request(f.current.task.task_id, &foreign, &command).unwrap();
    assert!(matches!(
        return_to_pm(&f.current, &held, &foreign, &f.config, &command),
        Err(AppError::Forbidden)
    ));
    assert_eq!(command, original);
}

#[test]
fn producer_cannot_impersonate_owner_scheduler_or_another_assignment() {
    for change in 0..7 {
        let mut f = Fixture::new(LifecycleStage::Analysis);
        match change {
            0 => f.producer.human_session = true,
            1 => f.producer.subject = "owner".into(),
            2 => f.producer.subject = "scheduler".into(),
            3 => {
                f.producer.scopes.insert("admin".into());
            }
            4 => {
                f.producer.scopes.remove("task-tracker:write");
            }
            5 => {
                f.producer
                    .scopes
                    .retain(|s| !s.starts_with(DELIVERY_GRANT_PREFIX));
            }
            _ => f.proposal.fence.execution_id = Uuid::new_v4(),
        }
        assert!(f.plan().is_err(), "case {change}");
    }
}

#[test]
fn new_proposals_deny_stale_revision_foreign_root_project_owner_and_unknown_runtime() {
    for change in 0..11 {
        let mut f = Fixture::new(LifecycleStage::Analysis);
        match change {
            0 => f.proposal.requirement_revision += 1,
            1 => f.proposal.content_hash = "f".repeat(64),
            2 => f.current.task.routing.root_task_id = Uuid::new_v4(),
            3 => f.config.single_task_project_ids.clear(),
            4 => f.config.instance_id = "foreign".into(),
            5 => f.current.task.owner_subject = "another-owner".into(),
            6 => f.current.task.runtime = RuntimeState::Unknown,
            7 => f.current.task.runtime = RuntimeState::Terminal,
            8 => f.current.lease.as_mut().unwrap().expires_at = f.now,
            9 => f.current.lease.as_mut().unwrap().holder_subject = "agent".into(),
            _ => f.current.task.routing.contract_version = 1,
        }
        assert!(f.plan().is_err(), "case {change}");
    }
}

#[test]
fn immutable_journal_replay_does_not_replace_proposal_with_current_detail() {
    let mut f = Fixture::new(LifecycleStage::Analysis);
    let saved = f.hold();
    let original = serde_json::to_value(&saved).unwrap();
    let change = return_to_pm(
        &f.current,
        &saved,
        &f.scheduler,
        &f.config,
        &f.handoff(&saved),
    )
    .unwrap();
    f.current = change.lifecycle;
    assert_eq!(replay_blocker(&saved, &f.proposal).unwrap(), saved);
    assert_ne!(change.blocker, saved);
    assert!(replay_blocker(&change.blocker, &f.proposal).is_err());
    let mut proposed_only = saved.clone();
    proposed_only.state = CriticalBlockerState::Proposed;
    assert!(replay_blocker(&proposed_only, &f.proposal).is_err());
    assert_eq!(serde_json::to_value(&saved).unwrap(), original);
    for change in 0..4 {
        let mut changed = f.proposal.clone();
        match change {
            0 => changed.question.push('?'),
            1 => changed.recommendation = Some(changed.options[0].option_id),
            2 => changed.evidence[0].receipt.sha256 = "f".repeat(64),
            _ => changed.fence.expected_lease_version += 1,
        }
        assert!(replay_blocker(&saved, &changed).is_err());
    }
}

#[test]
fn prepare_and_hold_validation_failure_has_no_partial_change_or_accepted_proposal() {
    for change in 0..8 {
        let mut f = Fixture::new(LifecycleStage::Analysis);
        match change {
            0 => f.proposal.requirement_revision += 1,
            1 => f.proposal.question.clear(),
            2 => f.proposal.evidence[0].receipt.sha256 = "invalid".into(),
            3 => f.producer.subject = "other-agent".into(),
            4 => f.producer.human_session = true,
            5 => {
                f.producer.scopes.insert("admin".into());
            }
            6 => f.proposal.recommendation = Some(Uuid::new_v4()),
            _ => f.proposal.fence.expected_lease_version += 1,
        }
        let before = f.current.clone();
        let proposal = f.proposal.clone();
        assert!(f.plan().is_err(), "case {change}");
        assert_eq!(f.current, before);
        assert_eq!(f.proposal, proposal);
    }
}

#[test]
fn heartbeat_can_advance_while_held_but_commands_require_fresh_lease_version() {
    let mut f = Fixture::new(LifecycleStage::Analysis);
    let blocker = f.hold();
    let stale = f.handoff(&blocker);
    let lease = f.current.lease.as_mut().unwrap();
    lease.version += 1;
    lease.heartbeat_at += Duration::seconds(10);
    lease.expires_at += Duration::seconds(10);
    assert!(return_to_pm(&f.current, &blocker, &f.scheduler, &f.config, &stale).is_err());
    assert!(
        return_to_pm(
            &f.current,
            &blocker,
            &f.scheduler,
            &f.config,
            &f.handoff(&blocker)
        )
        .is_ok()
    );
}

#[test]
fn current_producer_fence_survives_heartbeat_without_renewing_or_changing_lease_identity() {
    let mut f = Fixture::new(LifecycleStage::Analysis);
    let lease = f.current.lease.as_mut().unwrap();
    lease.version += 2;
    lease.heartbeat_at += Duration::seconds(10);
    lease.expires_at += Duration::seconds(10);
    f.now += Duration::seconds(11);
    let retained = f.proposal.clone();
    let change = f.plan().unwrap();
    assert_eq!(change.blocker.proposal, retained);
    assert_eq!(change.lifecycle.lease, f.current.lease);
    assert_eq!(change.blocker.proposal.fence.expected_lease_version, 1);
    assert_eq!(change.lifecycle.lease.as_ref().unwrap().version, 3);
    f.current = change.lifecycle;
    assert!(
        return_to_pm(
            &f.current,
            &change.blocker,
            &f.scheduler,
            &f.config,
            &f.handoff(&change.blocker)
        )
        .is_ok()
    );

    let mut future = Fixture::new(LifecycleStage::Analysis);
    future.proposal.fence.expected_lease_version += 1;
    assert!(future.plan().is_err());
    let mut foreign = Fixture::new(LifecycleStage::Analysis);
    foreign.proposal.fence.lease_id = Uuid::new_v4();
    assert!(foreign.plan().is_err());
}

#[test]
fn empty_and_single_options_hold_v1_with_own_grant_without_scheduler_impersonation() {
    for count in [0, 1] {
        let mut f = Fixture::new(LifecycleStage::Architecture);
        f.proposal.options.truncate(count);
        let change = f.plan().unwrap();
        assert_eq!(change.blocker.state, CriticalBlockerState::Held);
        assert_eq!(change.blocker.version, 1);
        assert_eq!(change.blocker.proposal.options.len(), count);
        assert_eq!(change.blocker.producer_subject, f.producer.subject);
        assert_ne!(change.blocker.producer_subject, f.scheduler.subject);
        assert!(crate::sdlc_reservation::scheduler(&f.producer, &f.config).is_err());
        assert_eq!(change.lifecycle.lease, f.current.lease);
        assert_eq!(change.lifecycle.binding, f.current.binding);
        assert_eq!(change.blocker.technical_terminal, None);
        if count == 1 {
            f.proposal.recommendation = Some(f.proposal.options[0].option_id);
            assert!(f.plan().is_ok());
        }
        f.proposal.recommendation = Some(Uuid::new_v4());
        assert!(f.plan().is_err());
    }
}

#[test]
fn matching_terminal_may_release_expired_custody_but_expiry_never_proves_terminal() {
    let mut f = Fixture::new(LifecycleStage::Analysis);
    let blocker = f.hold();
    f.now += Duration::seconds(60);
    f.current.task.runtime = RuntimeState::Unknown;
    assert!(ensure_technical_progress_allowed(&f.current, Some(&blocker)).is_err());
    let command = f.handoff(&blocker);
    assert!(return_to_pm(&f.current, &blocker, &f.scheduler, &f.config, &command).is_ok());
    let mut missing = command;
    missing.terminal.receipt.reference.clear();
    assert!(return_to_pm(&f.current, &blocker, &f.scheduler, &f.config, &missing).is_err());
    assert!(f.current.assignment.is_some());
    assert!(f.current.lease.is_some());
}

#[test]
fn held_receipt_and_actual_terminal_do_not_allow_agent_or_owner_to_return_to_pm() {
    let mut f = Fixture::new(LifecycleStage::Analysis);
    assert!(
        prepare_and_hold(
            &f.current,
            &f.scheduler,
            &f.config,
            &f.proposal,
            BlockerAllocation {
                blocker_id: Uuid::new_v4(),
                routing_generation: 1,
                observed_at: f.now
            },
        )
        .is_err()
    );
    let blocker = f.hold();
    let command = f.handoff(&blocker);
    let before = f.current.clone();
    let mut owner = f.scheduler.clone();
    owner.subject = "owner".into();
    owner.human_session = true;
    for actor in [&f.producer, &owner] {
        assert!(return_to_pm(&f.current, &blocker, actor, &f.config, &command).is_err());
        assert_eq!(f.current, before);
    }
    assert!(return_to_pm(&f.current, &blocker, &f.scheduler, &f.config, &command).is_ok());
}

#[test]
fn return_denies_wrong_terminal_run_assignment_chat_hash_and_unheld_state() {
    let mut f = Fixture::new(LifecycleStage::Testing);
    let blocker = f.hold();
    let before = f.current.clone();
    for change in 0..7 {
        let mut command = f.handoff(&blocker);
        let mut saved = blocker.clone();
        match change {
            0 => command.terminal.run_id = Uuid::new_v4(),
            1 => command.terminal.chat_id = Uuid::new_v4(),
            2 => command.terminal.assignment_hash = "f".repeat(64),
            3 => command.terminal.receipt.sha256 = "invalid".into(),
            4 => saved.state = CriticalBlockerState::Proposed,
            5 => command.review.fence.fencing_token += 1,
            _ => command.review.expected_task_version -= 1,
        }
        assert!(return_to_pm(&f.current, &saved, &f.scheduler, &f.config, &command).is_err());
        assert_eq!(f.current, before);
    }
}

#[test]
fn atomic_post_losing_to_complete_fails_and_committed_hold_blocks_complete_and_reserve() {
    let mut f = Fixture::new(LifecycleStage::Analysis);
    let complete = DeliveryResult::Analyst {
        analysis: artifact("synthetic-analysis"),
    };
    crate::sdlc_lifecycle::accept_result(&mut f.current.task, &complete).unwrap();
    let completed = f.current.clone();
    assert!(f.plan().is_err());
    assert_eq!(f.current, completed);
    let mut f = Fixture::new(LifecycleStage::Analysis);
    let held = f.hold();
    let before = f.current.clone();
    let mut after = before.clone();
    let complete_attempt = ensure_technical_progress_allowed(&after, Some(&held))
        .and_then(|()| crate::sdlc_lifecycle::accept_result(&mut after.task, &complete));
    assert!(complete_attempt.is_err());
    assert_eq!(after, before);
    let reserve_gate = ensure_technical_progress_allowed(&f.current, Some(&held));
    assert!(reserve_gate.is_err());
    assert_eq!(f.current, before);
    // Existing accept_result does not read blocker state. Integration MUST call the guard
    // under the task lock before accept_result/Reserve/EnsureQueued, not just at HTTP admission.
}

#[test]
fn version_exhaustion_is_atomic_and_repeated_hold_requires_original_journal_readback() {
    let mut f = Fixture::new(LifecycleStage::Analysis);
    f.current.task.version = MAX_SAFE_VERSION;
    let before = f.current.clone();
    assert!(f.plan().is_err());
    assert_eq!(f.current, before);
    let mut f = Fixture::new(LifecycleStage::Analysis);
    let held = f.hold();
    let held_before = f.current.clone();
    assert!(f.plan().is_err());
    assert_eq!(f.current, held_before);
    assert_eq!(replay_blocker(&held, &f.proposal).unwrap(), held);
    let mut exhausted = held.clone();
    exhausted.version = MAX_SAFE_VERSION;
    assert!(
        return_to_pm(
            &f.current,
            &exhausted,
            &f.scheduler,
            &f.config,
            &f.handoff(&exhausted)
        )
        .is_err()
    );
}

#[test]
fn legacy_lifecycle_wire_is_unchanged() {
    let command = json!({"operation":"ensure_queued","idempotency_key":"legacy"});
    let old: LifecycleCommand = serde_json::from_value(command.clone()).unwrap();
    assert_eq!(serde_json::to_value(old).unwrap(), command);
    let f = Fixture::new(LifecycleStage::Analysis);
    let heartbeat = json!({"operation":"heartbeat", "fence":f.proposal.fence, "idempotency_key":"legacy-heartbeat"});
    let old: LifecycleCommand = serde_json::from_value(heartbeat.clone()).unwrap();
    assert_eq!(serde_json::to_value(old).unwrap(), heartbeat);
}
