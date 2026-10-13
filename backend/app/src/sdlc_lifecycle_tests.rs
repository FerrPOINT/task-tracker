use super::*;
use chrono::Utc;
use domain::sdlc::Stage;
use domain::sdlc_routing::{
    RoleRoute, RoleRoutes, RoutingPolicy, RoutingVerification, TaskRoutingSnapshot,
};

fn artifact(name: &str) -> ArtifactReceipt {
    ArtifactReceipt {
        reference: format!("evidence/{name}.json"),
        sha256: "a".repeat(64),
    }
}

fn task() -> LifecycleTask {
    let route = RoleRoute {
        agent_id: Uuid::new_v4(),
        fleet_config_revision: 1,
        package_commit: "b".repeat(40),
        package_manifest_sha256: "c".repeat(64),
        namespace_id: "1".into(),
        namespace_name: "test".into(),
        workflow_id: "1".into(),
        workflow_key: "test".into(),
        profile: "test".into(),
        workflow_catalog_version: 3,
        workflow_catalog_sha256: "d".repeat(64),
    };
    let routes = RoleRoutes {
        project_manager: route.clone(),
        analyst: route.clone(),
        architect: route.clone(),
        developer: route.clone(),
        reviewer: route.clone(),
        tester: route.clone(),
        devops: route,
    };
    let id = Uuid::new_v4();
    let project = Uuid::new_v4();
    LifecycleTask {
        task_id: id,
        owner_subject: "owner".into(),
        version: 1,
        stage: LifecycleStage::Analysis,
        status: LifecycleStatus::InProgress,
        cycle: 0,
        transient_retries: 0,
        waiting_reason: None,
        runtime: RuntimeState::Running,
        previous_result_id: None,
        routing: TaskRoutingSnapshot {
            contract_version: 1,
            snapshot_id: Uuid::new_v4(),
            tracker_instance_id: "test".into(),
            project_id: project,
            task_id: id,
            root_task_id: id,
            confirmation_id: Uuid::new_v4(),
            requirement_revision: 1,
            content_hash: "e".repeat(64),
            policy: RoutingPolicy {
                contract_version: 1,
                tracker_instance_id: "test".into(),
                project_id: project,
                version: 1,
                routing_hash: "f".repeat(64),
                routes,
                verification: RoutingVerification::Declared,
                native_ready: false,
                dispatch_allowed: false,
                author_subject: "owner".into(),
                created_at: Utc::now(),
            },
            created_at: Utc::now(),
        },
    }
}

fn developer() -> DeliveryResult {
    DeliveryResult::Developer {
        commit_sha: "b".repeat(40),
        changes: artifact("changes"),
        checks: vec![artifact("tests")],
    }
}

fn running(task: &mut LifecycleTask) {
    task.status = LifecycleStatus::InProgress;
    task.runtime = RuntimeState::Running;
}

#[test]
fn one_task_traverses_all_six_delivery_roles_and_completes_only_after_devops() {
    let mut state = task();
    let original_id = state.task_id;
    let snapshot = state.routing.clone();
    let outcomes = [
        (
            DeliveryResult::Analyst {
                analysis: artifact("analysis"),
            },
            LifecycleStage::Architecture,
        ),
        (
            DeliveryResult::Architect {
                design: artifact("design"),
            },
            LifecycleStage::Development,
        ),
        (developer(), LifecycleStage::Review),
        (
            DeliveryResult::Reviewer {
                verdict: ReviewVerdict::Approved,
                report: artifact("review"),
            },
            LifecycleStage::Testing,
        ),
        (
            DeliveryResult::Tester {
                verdict: TestVerdict::Passed,
                report: artifact("test"),
            },
            LifecycleStage::Deployment,
        ),
        (
            DeliveryResult::DevOps {
                deployment: artifact("deploy"),
                verification: artifact("health"),
            },
            LifecycleStage::Deployment,
        ),
    ];
    for (i, (output, next)) in outcomes.iter().enumerate() {
        running(&mut state);
        accept_result(&mut state, output).unwrap();
        assert_eq!(state.task_id, original_id);
        assert_eq!(state.routing, snapshot);
        assert_eq!(state.stage, *next);
        assert_eq!(
            state.status,
            if i == 5 {
                LifecycleStatus::Completed
            } else {
                LifecycleStatus::Ready
            }
        );
    }
    assert_eq!(state.version, 7);
    assert_eq!(state.runtime, RuntimeState::Terminal);
}

#[test]
fn review_and_testing_rework_both_return_to_developer_then_repeat_review_and_testing() {
    for source in [LifecycleStage::Review, LifecycleStage::Testing] {
        let mut state = task();
        state.stage = source;
        let rejected = if source == LifecycleStage::Review {
            DeliveryResult::Reviewer {
                verdict: ReviewVerdict::Rework,
                report: artifact("review"),
            }
        } else {
            DeliveryResult::Tester {
                verdict: TestVerdict::Rework,
                report: artifact("test"),
            }
        };
        for cycle in 1..=MAX_REWORK_CYCLES {
            state.stage = source;
            running(&mut state);
            accept_result(&mut state, &rejected).unwrap();
            assert_eq!(state.stage, LifecycleStage::Rework);
            assert_eq!(state.stage.role(), Some(DeliveryRole::Developer));
            assert_eq!(state.cycle, cycle);
            running(&mut state);
            accept_result(&mut state, &developer()).unwrap();
            assert_eq!(state.stage, LifecycleStage::Review);
            running(&mut state);
            accept_result(
                &mut state,
                &DeliveryResult::Reviewer {
                    verdict: ReviewVerdict::Approved,
                    report: artifact("review-again"),
                },
            )
            .unwrap();
            assert_eq!(state.stage, LifecycleStage::Testing);
        }
        state.stage = source;
        running(&mut state);
        accept_result(&mut state, &rejected).unwrap();
        assert_eq!(state.stage, source);
        assert_eq!(state.cycle, 3);
        assert_eq!(state.status, LifecycleStatus::Ready);
        assert_eq!(state.runtime, RuntimeState::Terminal);
        assert_eq!(
            state.waiting_reason.as_deref(),
            Some("rework_limit_reached")
        );
    }
}

#[test]
fn wrong_role_reserved_run_and_exhausted_version_cannot_mutate_state() {
    for variant in 0..3 {
        let mut state = task();
        let output = if variant == 0 {
            developer()
        } else {
            DeliveryResult::Analyst {
                analysis: artifact("analysis"),
            }
        };
        if variant == 1 {
            state.runtime = RuntimeState::Reserved;
        }
        if variant == 2 {
            state.version = MAX_SAFE_VERSION;
        }
        let before = state.clone();
        assert!(accept_result(&mut state, &output).is_err());
        assert_eq!(state, before);
    }
}

fn complete() -> LifecycleCommand {
    let result = developer();
    LifecycleCommand::Complete {
        fence: DeliveryFence {
            assignment_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            fencing_token: 1,
            lease_id: Uuid::new_v4(),
            expected_lease_version: 1,
        },
        terminal: TerminalReceipt {
            chat_id: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
            fleet_receipt: artifact("fleet-terminal"),
            workflow_receipt: artifact("workflow-terminal"),
            output_sha256: crate::sdlc::canonical_hash(&result).unwrap(),
        },
        result,
        idempotency_key: "result-1".into(),
    }
}

#[test]
fn terminal_output_requires_actual_identifiers_hashes_commit_and_executed_checks() {
    validate_command(&complete()).unwrap();
    for variant in 0..5 {
        let mut command = complete();
        if let LifecycleCommand::Complete {
            fence,
            terminal,
            result,
            ..
        } = &mut command
        {
            match variant {
                0 => terminal.run_id = Uuid::nil(),
                1 => terminal.output_sha256 = "f".repeat(64),
                2 => fence.expected_lease_version = MAX_SAFE_VERSION + 1,
                3 => terminal.fleet_receipt.reference.clear(),
                _ => {
                    if let DeliveryResult::Developer { checks, .. } = result {
                        checks.clear();
                    }
                    terminal.output_sha256 = crate::sdlc::canonical_hash(result).unwrap();
                }
            }
        }
        assert!(validate_command(&command).is_err());
    }
}

#[test]
fn success_flags_children_and_integration_modes_are_not_result_contracts() {
    let valid = serde_json::to_value(complete()).unwrap();
    for field in [
        "success",
        "native_ready",
        "children",
        "integration_mode",
        "dispatch_allowed",
    ] {
        let mut value = valid.clone();
        value[field] = serde_json::json!(true);
        assert!(serde_json::from_value::<LifecycleCommand>(value).is_err());
    }
    let mut value = valid;
    value["result"] = serde_json::json!({"role":"developer","success":true});
    assert!(serde_json::from_value::<LifecycleCommand>(value).is_err());
}

#[test]
fn legacy_clarification_maps_to_draft_without_mutating_history() {
    assert_eq!(
        LifecycleStage::from_legacy(Stage::Clarification),
        LifecycleStage::Draft
    );
    assert_eq!(
        serde_json::to_string(&Stage::Clarification).unwrap(),
        "\"Clarification\""
    );
}

#[test]
fn transient_retries_require_safe_replay_and_are_bounded_at_three() {
    for retries in 0..=4 {
        assert_eq!(transient_retry_allowed(retries, true), retries < 3);
        assert!(!transient_retry_allowed(retries, false));
    }
}

#[test]
fn configured_project_pm_default_is_authoritative_without_native_admission() {
    let mut policy = task().routing.policy;
    policy.routing_hash = crate::sdlc::canonical_hash(&policy.routes).unwrap();
    let project = policy.project_id;
    let agent = policy.routes.project_manager.agent_id;
    crate::sdlc_routing::validate_project_pm_default(&policy, project, "test", agent).unwrap();
    assert!(
        crate::sdlc_routing::validate_project_pm_default(&policy, project, "test", Uuid::new_v4())
            .is_err()
    );
    assert!(
        crate::sdlc_routing::validate_project_pm_default(&policy, Uuid::new_v4(), "test", agent)
            .is_err()
    );
    assert!(
        crate::sdlc_routing::validate_project_pm_default(&policy, project, "foreign", agent)
            .is_err()
    );
    policy.routing_hash = "a".repeat(64);
    assert!(
        crate::sdlc_routing::validate_project_pm_default(&policy, project, "test", agent).is_err()
    );
}

#[test]
fn producer_has_no_scheduler_or_owner_powers_and_cannot_submit_fleet_terminal_proof() {
    let mut actor = Principal {
        subject: "bound-agent".into(),
        human_session: false,
        trusted_human: false,
        scopes: ["task-tracker:read".into(), "task-tracker:write".into()].into(),
    };
    producer(&actor, true).unwrap();
    producer(&actor, false).unwrap();
    actor.human_session = true;
    assert!(producer(&actor, true).is_err());
    actor.human_session = false;
    actor.scopes.insert("task-tracker:sdlc:assign".into());
    assert!(producer(&actor, true).is_err());
    let command = complete();
    let LifecycleCommand::Complete { fence, result, .. } = command else {
        unreachable!()
    };
    let proposal = SubmitDeliveryProposal {
        fence,
        workflow_receipt: artifact("workflow"),
        result,
        idempotency_key: "proposal-1".into(),
    };
    let mut wire = serde_json::to_value(proposal).unwrap();
    assert!(serde_json::from_value::<SubmitDeliveryProposal>(wire.clone()).is_ok());
    wire["fleet_receipt"] = serde_json::json!({"reference":"invented","sha256":"a".repeat(64)});
    assert!(serde_json::from_value::<SubmitDeliveryProposal>(wire).is_err());
}

#[test]
fn fresh_server_clock_is_separate_from_retained_receipt_and_nullable_fields_are_explicit() {
    let receipt = LifecycleReceipt {
        task: task(),
        assignment: None,
        lease: None,
        binding: None,
        result: None,
    };
    let original = serde_json::to_value(&receipt).unwrap();
    let readback = LifecycleReadback {
        observed_at: Utc::now(),
        receipt: receipt.clone(),
    };
    let wire = serde_json::to_value(readback).unwrap();
    assert_eq!(wire["receipt"], original);
    assert!(wire["observed_at"].as_str().unwrap().ends_with('Z'));
    for field in ["assignment", "lease", "binding", "result"] {
        assert!(wire["receipt"].as_object().unwrap().contains_key(field));
        assert!(wire["receipt"][field].is_null());
    }
    assert_eq!(serde_json::to_value(receipt).unwrap(), original);
}
