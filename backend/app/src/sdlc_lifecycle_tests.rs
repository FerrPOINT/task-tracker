use super::*;
use chrono::Utc;
use domain::sdlc::Stage;
use domain::sdlc_routing::{
    RoleRoute, RoleRoutes, RoutingPolicy, RoutingVerification, TaskRoutingSnapshot,
};

#[test]
fn historical_unsubmitted_observation_decodes_but_cannot_release_current_ownership() {
    let raw = serde_json::json!({"operation":"reconcile","fence":{
        "assignment_id":Uuid::new_v4(),"execution_id":Uuid::new_v4(),"fencing_token":1,
        "lease_id":Uuid::new_v4(),"expected_lease_version":1},
        "never_submitted":{"assignment_hash":"a".repeat(64),"reservation_key":"reserve:original",
            "reservation_payload_sha256":"b".repeat(64)},
        "recovery":{"disposition":"retry_unsubmitted"},"idempotency_key":"recover:original"});
    let command: LifecycleCommand = serde_json::from_value(raw.clone()).unwrap();
    validate_command(&command).unwrap();
    assert!(validate_active_command(&command).is_err());
    assert!(serde_json::to_value(&command).unwrap().get("terminal").is_none());
    let mut stop = raw.clone();
    stop["recovery"] = serde_json::json!({"disposition":"stop"});
    let command: LifecycleCommand = serde_json::from_value(stop.clone()).unwrap();
    validate_command(&command).unwrap();
    validate_active_command(&command).unwrap();
    stop["never_submitted"] = serde_json::Value::Null;
    assert!(validate_command(&serde_json::from_value(stop).unwrap()).is_err());
}

#[test]
fn never_submitted_stop_keeps_stage_inputs_without_retry_or_native_identity() {
    let mut state = task();
    state.runtime = RuntimeState::Reserved;
    state.status = LifecycleStatus::Ready;
    let before = state.clone();
    stop_never_submitted(&mut state).unwrap();
    let mut expected = before.clone();
    expected.version += 1;
    expected.runtime = RuntimeState::Terminal;
    expected.waiting_reason = Some("cancelled_before_submission".into());
    assert_eq!(state, expected);
    for runtime in [RuntimeState::Queued, RuntimeState::Running, RuntimeState::Unknown, RuntimeState::Terminal] {
        let mut wrong = before.clone();
        wrong.runtime = runtime;
        let original = wrong.clone();
        assert!(stop_never_submitted(&mut wrong).is_err());
        assert_eq!(wrong, original);
    }
    for status in [LifecycleStatus::InProgress, LifecycleStatus::Completed] {
        let mut wrong = before.clone();
        wrong.status = status;
        let original = wrong.clone();
        assert!(stop_never_submitted(&mut wrong).is_err());
        assert_eq!(wrong, original);
    }
}

#[test]
fn explicit_start_command_is_closed_and_has_no_owner_or_dispatch_override() {
    use serde_json::json;
    let raw = json!({"operation":"start_analysis","pm_completion_id":Uuid::new_v4(),"idempotency_key":"start"});
    let command: LifecycleCommand = serde_json::from_value(raw.clone()).unwrap();
    validate_command(&command).unwrap();
    assert_eq!(command.key(), "start");
    for field in [
        "owner_subject",
        "project_id",
        "requirement_revision",
        "content_hash",
        "confirmation_id",
        "agent_id",
        "assignment_id",
        "transaction_id",
    ] {
        let mut wrong = raw.clone();
        wrong[field] = json!("override");
        assert!(
            serde_json::from_value::<LifecycleCommand>(wrong).is_err(),
            "{field}"
        );
    }
    assert!(
        validate_command(&LifecycleCommand::StartAnalysis {
            pm_completion_id: Uuid::nil(),
            idempotency_key: "start".into(),
        })
        .is_err()
    );
    assert!(
        validate_command(&LifecycleCommand::StartAnalysis {
            pm_completion_id: Uuid::new_v4(),
            idempotency_key: String::new(),
        })
        .is_err()
    );
}

fn artifact(name: &str) -> ArtifactReceipt {
    ArtifactReceipt {
        reference: format!("evidence/{name}.json"),
        sha256: "a".repeat(64),
    }
}

#[test]
fn delivery_execution_reference_is_canonical_and_closed() {
    for ordinal in [1, 9_007_199_254_740_992, i64::MAX] {
        let execution = DeliveryExecution::from_ordinal(ordinal).unwrap();
        let raw = serde_json::to_value(&execution).unwrap();
        assert_eq!(
            raw,
            serde_json::json!({"ordinal":ordinal.to_string(),"key":format!("SDLC-{ordinal}")})
        );
        assert_eq!(
            serde_json::from_value::<DeliveryExecution>(raw).unwrap(),
            execution
        );
    }
    for ordinal in [
        "0",
        "-1",
        "01",
        "+1",
        " 1",
        "1.0",
        "9223372036854775808",
        "",
    ] {
        assert!(
            serde_json::from_value::<DeliveryExecution>(
                serde_json::json!({"ordinal":ordinal,"key":format!("SDLC-{ordinal}")})
            )
            .is_err()
        );
    }
    for raw in [
        serde_json::json!({"ordinal":"1","key":"TASK-1"}),
        serde_json::json!({"ordinal":"1","key":"SDLC-2"}),
        serde_json::json!({"ordinal":1,"key":"SDLC-1"}),
        serde_json::json!({"ordinal":"1"}),
        serde_json::json!({"ordinal":"1","key":"SDLC-1","ready":true}),
    ] {
        assert!(serde_json::from_value::<DeliveryExecution>(raw).is_err());
    }
    assert!(DeliveryExecution::from_ordinal(0).is_err());
    assert!(DeliveryExecution::from_ordinal(-1).is_err());
}

#[test]
fn optional_delivery_execution_preserves_legacy_wire_and_binds_new_hash() {
    let task = task();
    let raw = serde_json::json!({"assignment_id":Uuid::new_v4(),"execution_id":Uuid::new_v4(),
        "task_id":task.task_id,"owner_subject":task.owner_subject,"role":"analyst","stage":"Analysis","cycle":0,"attempt":1,
        "agent_id":task.routing.policy.routes.analyst.agent_id,"routing":task.routing,"previous_result_id":null,
        "fencing_token":1,"lease_id":Uuid::new_v4(),"assignment_hash":""});
    let mut assignment: DeliveryAssignment = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(assignment.execution, None);
    assert_eq!(serde_json::to_value(&assignment).unwrap(), raw);
    let legacy_hash = assignment_hash(&assignment).unwrap();
    let mut original = raw;
    original.as_object_mut().unwrap().remove("assignment_hash");
    assert_eq!(legacy_hash, crate::sdlc::canonical_hash(&original).unwrap());
    assignment.execution = Some(DeliveryExecution::from_ordinal(123).unwrap());
    let hash = assignment_hash(&assignment).unwrap();
    assert_ne!(hash, legacy_hash);
    assignment.assignment_hash = hash.clone();
    let wire = serde_json::to_value(&assignment).unwrap();
    assert_eq!(
        wire["execution"],
        serde_json::json!({"ordinal":"123","key":"SDLC-123"})
    );
    let decoded: DeliveryAssignment = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(assignment_hash(&decoded).unwrap(), hash);
    assert_eq!(decoded, assignment);
    let mut bad = wire;
    bad["execution"]["key"] = serde_json::json!("SDLC-999");
    assert!(serde_json::from_value::<DeliveryAssignment>(bad).is_err());
    assignment.execution.as_mut().unwrap().key = "SDLC-999".into();
    assert!(assignment_hash(&assignment).is_err());
}

fn task() -> LifecycleTask {
    let route = RoleRoute {
        agent_id: Uuid::new_v4(),
        fleet_config_revision: 1,
        package_commit: Some("b".repeat(40)),
        package_manifest_sha256: Some("c".repeat(64)),
        instruction_profile: None,
        instruction_manifest_sha256: None,
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
        resume_chat_id: None,
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

// Synthetic state after the agent's accepted proposal, before native settlement.
fn proposed(task: &mut LifecycleTask) {
    running(task);
    task.status = LifecycleStatus::Completed;
}

#[test]
fn review_preserves_both_verdicts_and_queues_testing_without_rework() {
    for verdict in [ReviewVerdict::Approved, ReviewVerdict::Rework] {
        for cycle in 0..=MAX_REWORK_CYCLES {
            let mut state = task();
            state.stage = LifecycleStage::Review;
            state.cycle = cycle;
            let routing = state.routing.clone();
            proposed(&mut state);
            accept_result(
                &mut state,
                &DeliveryResult::Reviewer {
                    verdict,
                    report: artifact("review"),
                },
            )
            .unwrap();
            assert_eq!(state.stage, LifecycleStage::Testing);
            assert_eq!(state.status, LifecycleStatus::Ready);
            assert_eq!(state.runtime, RuntimeState::Queued);
            assert_eq!(
                state.waiting_reason.as_deref(),
                Some("queued_for_assignment")
            );
            assert_eq!(state.cycle, cycle);
            assert_eq!(state.routing, routing);
            let settled = serde_json::to_value(&state).unwrap();
            assert!(
                accept_result(
                    &mut state,
                    &DeliveryResult::Reviewer {
                        verdict,
                        report: artifact("review")
                    },
                )
                .is_err()
            );
            assert_eq!(serde_json::to_value(&state).unwrap(), settled);
        }
    }
}

#[test]
fn other_roles_preserve_their_existing_handoff() {
    let cases = [
        (
            LifecycleStage::Analysis,
            DeliveryResult::Analyst {
                analysis: artifact("analysis"),
            },
            LifecycleStage::Architecture,
        ),
        (
            LifecycleStage::Architecture,
            DeliveryResult::Architect {
                design: artifact("design"),
            },
            LifecycleStage::Development,
        ),
        (
            LifecycleStage::Development,
            developer(),
            LifecycleStage::Review,
        ),
        (LifecycleStage::Rework, developer(), LifecycleStage::Review),
        (
            LifecycleStage::Testing,
            DeliveryResult::Tester {
                verdict: TestVerdict::Passed,
                report: artifact("test"),
            },
            LifecycleStage::Deployment,
        ),
    ];
    for (from, output, to) in cases {
        let mut state = task();
        state.stage = from;
        proposed(&mut state);
        accept_result(&mut state, &output).unwrap();
        assert_eq!(state.stage, to);
        assert_eq!(state.status, LifecycleStatus::Ready);
        assert_eq!(state.runtime, RuntimeState::Queued);
    }
}

#[test]
fn testing_preserves_both_verdicts_and_queues_devops_without_rework() {
    for verdict in [TestVerdict::Passed, TestVerdict::Rework] {
        for cycle in [0, 1, MAX_REWORK_CYCLES] {
            let mut state = task();
            state.stage = LifecycleStage::Testing;
            state.cycle = cycle;
            proposed(&mut state);
            accept_result(
                &mut state,
                &DeliveryResult::Tester {
                    verdict,
                    report: artifact("test"),
                },
            )
            .unwrap();
            assert_eq!(state.stage, LifecycleStage::Deployment);
            assert_eq!(state.status, LifecycleStatus::Ready);
            assert_eq!(state.runtime, RuntimeState::Queued);
            assert_eq!(
                state.waiting_reason.as_deref(),
                Some("queued_for_assignment")
            );
            assert_eq!(state.cycle, cycle);
        }
    }
}

#[test]
fn in_progress_stage_cannot_accept_terminal_result_before_proposal() {
    let mut state = task();
    running(&mut state);
    let before = state.clone();
    let output = DeliveryResult::Analyst {
        analysis: artifact("analysis"),
    };
    assert!(matches!(
        accept_result(&mut state, &output),
        Err(AppError::Conflict(_))
    ));
    assert_eq!(state, before);
}

#[test]
fn wrong_role_reserved_run_and_exhausted_version_cannot_mutate_state() {
    for variant in 0..3 {
        let mut state = task();
        proposed(&mut state);
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
fn retired_recovery_roundtrips_but_cannot_mutate_current_work() {
    let terminal = RuntimeTerminalObservation {
        assignment_hash: "a".repeat(64), chat_id: Uuid::new_v4(), run_id: Uuid::new_v4(),
        state: RuntimeTerminalState::Failed, receipt: artifact("historical-terminal"),
    };
    let fence = DeliveryFence { assignment_id: Uuid::new_v4(), execution_id: Uuid::new_v4(),
        fencing_token: 1, lease_id: Uuid::new_v4(), expected_lease_version: 1 };
    for recovery in [RecoveryDisposition::RetryUnsubmitted, RecoveryDisposition::RetryTransient {
        replay_safety_receipt: artifact("historical-replay-safety"),
    }] {
        let unsent = matches!(recovery, RecoveryDisposition::RetryUnsubmitted);
        let command = LifecycleCommand::Reconcile {
            fence: fence.clone(), terminal: if unsent { None } else { Some(terminal.clone()) },
            never_submitted: if unsent { Some(NeverSubmittedObservation {
                assignment_hash: "a".repeat(64), reservation_key: "original:reserve".into(),
                reservation_payload_sha256: "b".repeat(64),
            }) } else { None }, recovery: recovery.clone(), idempotency_key: "historical:recovery".into(),
        };
        let saved = serde_json::to_value(&command).unwrap();
        let decoded: LifecycleCommand = serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(serde_json::to_value(&decoded).unwrap(), saved);
        validate_command(&decoded).unwrap();
        assert!(validate_active_command(&decoded).is_err());
        let mut current = task();
        let before = current.clone();
        assert!(reconcile_terminal(&mut current, &terminal, &recovery).is_err());
        assert_eq!(current, before);
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
    assert!(producer(&actor, true).is_err());
    actor.scopes.insert(format!(
        "{DELIVERY_GRANT_PREFIX}{}:{}:{}:{}:1",
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4()
    ));
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
fn delivery_grants_are_canonical_single_task_capabilities_not_generic_permissions() {
    let task_id = Uuid::new_v4();
    let assignment = Uuid::new_v4();
    let execution = Uuid::new_v4();
    let agent = Uuid::new_v4();
    let valid = format!("{DELIVERY_GRANT_PREFIX}{task_id}:{assignment}:{execution}:{agent}:7");
    let principal = |grant: String| Principal {
        subject: "machine".into(),
        human_session: false,
        trusted_human: false,
        scopes: [
            "task-tracker:read".into(),
            "task-tracker:write".into(),
            grant,
        ]
        .into(),
    };
    let actor = principal(valid.clone());
    assert_eq!(delivery_grant(&actor).unwrap().unwrap().task_id, task_id);
    let prefix = format!("/api/v1/issues/{task_id}/sdlc/");
    for (method, resource) in [
        ("GET", "lifecycle"),
        ("GET", "requirements"),
        ("GET", "events"),
        ("POST", "delivery-proposals"),
        ("GET", "delivery-proposals/operations/key"),
        ("POST", "critical-blockers"),
        ("GET", "critical-blockers/operations/key"),
    ] {
        delivery_request_policy(&actor, method, &format!("{prefix}{resource}")).unwrap();
    }
    let result_id = Uuid::parse_str("abcdef12-1234-5678-9abc-abcdef123456").unwrap();
    let result_path = format!("{prefix}lifecycle/results/{result_id}");
    delivery_request_policy(&actor, "GET", &result_path).unwrap();
    let blocker_path = format!("{prefix}critical-blockers/{result_id}");
    delivery_request_policy(&actor, "GET", &blocker_path).unwrap();
    for id in [
        result_id.to_string().to_uppercase(),
        result_id.simple().to_string(),
        Uuid::nil().to_string(),
        "not-a-uuid".into(),
        format!("{result_id}/extra"),
        String::new(),
    ] {
        assert!(
            delivery_request_policy(&actor, "GET", &format!("{prefix}lifecycle/results/{id}"))
                .is_err()
        );
        assert!(
            delivery_request_policy(&actor, "GET", &format!("{prefix}critical-blockers/{id}"))
                .is_err()
        );
    }
    for method in ["POST", "PUT", "PATCH", "DELETE"] {
        assert!(delivery_request_policy(&actor, method, &result_path).is_err());
        assert!(delivery_request_policy(&actor, method, &blocker_path).is_err());
    }
    assert!(
        delivery_request_policy(
            &actor,
            "GET",
            &format!(
                "/api/v1/issues/{}/sdlc/lifecycle/results/{result_id}",
                Uuid::new_v4()
            )
        )
        .is_err()
    );
    for (method, path) in [
        ("GET", "/api/v1/issues/foreign".to_owned()),
        ("GET", "/api/v1/sdlc/project-directory".into()),
        ("POST", format!("{prefix}lifecycle")),
        ("POST", format!("{prefix}requirements/1/confirm")),
        ("POST", format!("{prefix}pm-feedback")),
        ("POST", format!("{prefix}assignment")),
        ("GET", format!("{prefix}critical-blockers")),
        (
            "GET",
            format!("{prefix}critical-blockers/operations/bad key"),
        ),
        (
            "GET",
            format!(
                "/api/v1/issues/{}/sdlc/critical-blockers/{result_id}",
                Uuid::new_v4()
            ),
        ),
        (
            "GET",
            format!("/api/v1/issues/{}/sdlc/lifecycle", Uuid::new_v4()),
        ),
    ] {
        assert!(delivery_request_policy(&actor, method, &path).is_err());
    }
    for malformed in [
        valid.to_uppercase(),
        valid.replace(":7", ":07"),
        valid.replace(":7", ":9007199254740992"),
        format!("{DELIVERY_GRANT_PREFIX}garbage"),
        "task-tracker:sdlc:delivery".into(),
    ] {
        assert!(delivery_grant(&principal(malformed)).is_err());
    }
    let mut ambiguous = actor.clone();
    ambiguous.scopes.insert(valid.replace(":7", ":8"));
    assert!(delivery_grant(&ambiguous).is_err());
    let mut mixed = actor;
    mixed.scopes.insert(format!(
        "{}{}:{}:{}:{}:1",
        crate::sdlc::PM_GRANT_PREFIX,
        task_id,
        assignment,
        execution,
        agent
    ));
    assert!(delivery_grant(&mixed).is_err());
}

#[test]
fn verified_terminal_stop_preserves_business_context_and_requires_explicit_intervention() {
    let mut state = task();
    let terminal = RuntimeTerminalObservation {
        assignment_hash: "a".repeat(64), chat_id: Uuid::new_v4(), run_id: Uuid::new_v4(),
        state: RuntimeTerminalState::Failed, receipt: artifact("actual-runtime-terminal"),
    };
    for _ in 0..6 {
        state.runtime = RuntimeState::Unknown;
        let before = state.clone();
        reconcile_terminal(&mut state, &terminal, &RecoveryDisposition::Stop).unwrap();
        assert_eq!(state.stage, before.stage);
        assert_eq!(state.task_id, before.task_id);
        assert_eq!(state.previous_result_id, before.previous_result_id);
        assert_eq!(state.transient_retries, before.transient_retries);
        assert_eq!(state.runtime, RuntimeState::Terminal);
        assert_eq!(state.status, LifecycleStatus::Ready);
        assert_eq!(state.resume_chat_id, Some(terminal.chat_id));
        assert_eq!(state.waiting_reason.as_deref(), Some("runtime_terminal_requires_intervention"));
        ensure_queued(&mut state).unwrap();
        assert_eq!(state.runtime, RuntimeState::Queued);
        assert_eq!(state.transient_retries, 0);
    }
}

#[test]
fn explicit_queue_intervention_preserves_inputs_and_clears_historical_retry_metadata() {
    let mut state = task();
    state.stage = LifecycleStage::Development;
    state.status = LifecycleStatus::Ready;
    state.runtime = RuntimeState::Terminal;
    state.resume_chat_id = Some(Uuid::new_v4());
    let original = state.clone();
    ensure_queued(&mut state).unwrap();
    assert_eq!(state.runtime, RuntimeState::Queued);
    assert_eq!(state.version, original.version + 1);
    assert_eq!(state.transient_retries, 0);
    assert_eq!(state.task_id, original.task_id);
    assert_eq!(state.stage, original.stage);
    assert_eq!(state.routing, original.routing);
    assert_eq!(state.previous_result_id, original.previous_result_id);
    assert_eq!(state.resume_chat_id, original.resume_chat_id);
    let queued = state.clone();
    ensure_queued(&mut state).unwrap();
    assert_eq!(state, queued);
    for runtime in [
        RuntimeState::Reserved,
        RuntimeState::Running,
        RuntimeState::Unknown,
    ] {
        state.runtime = runtime;
        let before = state.clone();
        ensure_queued(&mut state).unwrap();
        assert_eq!(state, before);
    }
    for status in [LifecycleStatus::InProgress, LifecycleStatus::Completed] {
        state = original.clone();
        state.status = status;
        let before = state.clone();
        assert!(ensure_queued(&mut state).is_err());
        assert_eq!(state, before);
    }
    state = original.clone();
    for _ in 0..6 {
        state.runtime = RuntimeState::Terminal;
        state.transient_retries = 3; // Archived pre121 counter, never a current recovery limit.
        ensure_queued(&mut state).unwrap();
        assert_eq!(state.runtime, RuntimeState::Queued);
        assert_eq!(state.transient_retries, 0);
    }
}

#[test]
fn canonical_projection_preserves_original_history_and_exact_status_keys() {
    let original = serde_json::json!({"stage":"Analysis","questions":[{"historical":"Clarification"}],
        "confirmations":[{"id":"original-consent"}],"assignment":{"original":"pm"},"extra":"preserve"});
    let state = task();
    let projected = canonical_projection(&original, &state).unwrap();
    for field in ["questions", "confirmations", "assignment", "extra"] {
        assert_eq!(projected[field], original[field]);
    }
    assert_eq!(projected["stage"], "Analysis");
    assert_eq!(projected["status"], "InProgress");
    assert_eq!(
        issue_mapping_keys(&state).unwrap(),
        ("Analysis".into(), "InProgress".into())
    );
    assert_eq!(
        projected["lifecycle"]["task_id"],
        serde_json::json!(state.task_id)
    );
}

#[test]
fn stage_changes_independently_of_ready_status() {
    let mut state = task();
    state.status = LifecycleStatus::Ready;
    state.stage = LifecycleStage::Development;
    let development = canonical_projection(&serde_json::json!({}), &state).unwrap();
    state.stage = LifecycleStage::Review;
    let review = canonical_projection(&development, &state).unwrap();
    assert_ne!(development["stage"], review["stage"]);
    assert_eq!(development["status"], review["status"]);
    assert_eq!(review["status"], "Ready");
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
        runtime_state: receipt.task.runtime,
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

#[test]
fn configured_original_proposal_receipt_has_its_own_frozen_identity() {
    let task = task();
    let subject = Uuid::new_v4().to_string();
    let actor = Principal {
        subject: subject.clone(),
        human_session: false,
        trusted_human: false,
        scopes: ["task-tracker:read".into(), "task-tracker:write".into()].into(),
    };
    let assignment: DeliveryAssignment = serde_json::from_value(serde_json::json!({
        "assignment_id":Uuid::new_v4(),"execution_id":Uuid::new_v4(),"task_id":task.task_id,
        "owner_subject":task.owner_subject,"role":"analyst","stage":"Analysis","cycle":0,"attempt":1,
        "agent_id":task.routing.policy.routes.analyst.agent_id,"routing":task.routing,"previous_result_id":null,
        "machine_subject":subject,"fencing_token":1,"lease_id":Uuid::new_v4(),"assignment_hash":"a".repeat(64)
    })).unwrap();
    let receipt = DeliveryProposalReceipt {
        proposal_id: Uuid::new_v4(),
        task_id: assignment.task_id,
        assignment_id: assignment.assignment_id,
        execution_id: assignment.execution_id,
        agent_id: assignment.agent_id,
        producer_subject: actor.subject.clone(),
        workflow_receipt: artifact("workflow"),
        result: DeliveryResult::Analyst {
            analysis: artifact("analysis"),
        },
        output_sha256: "b".repeat(64),
        created_at: Utc::now(),
    };
    authorize_configured_proposal_receipt(&actor, &receipt, &assignment, &task.owner_subject)
        .unwrap();
    for field in ["task", "assignment", "execution", "agent", "producer"] {
        let mut changed = receipt.clone();
        match field {
            "task" => changed.task_id = Uuid::new_v4(),
            "assignment" => changed.assignment_id = Uuid::new_v4(),
            "execution" => changed.execution_id = Uuid::new_v4(),
            "agent" => changed.agent_id = Uuid::new_v4(),
            "producer" => changed.producer_subject = Uuid::new_v4().to_string(),
            _ => unreachable!(),
        }
        assert!(
            authorize_configured_proposal_receipt(
                &actor,
                &changed,
                &assignment,
                &task.owner_subject
            )
            .is_err(),
            "{field}"
        );
    }
    let mut foreign = actor.clone();
    foreign.subject = Uuid::new_v4().to_string();
    assert!(
        authorize_configured_proposal_receipt(&foreign, &receipt, &assignment, &task.owner_subject)
            .is_err()
    );
    let mut no_write = actor.clone();
    no_write.scopes.remove("task-tracker:write");
    assert!(
        authorize_configured_proposal_receipt(
            &no_write,
            &receipt,
            &assignment,
            &task.owner_subject
        )
        .is_err()
    );
    assert!(
        authorize_configured_proposal_receipt(&actor, &receipt, &assignment, "changed-owner")
            .is_err()
    );
}

#[test]
fn persistent_ownership_preserves_historical_wire_and_closed_current_commands() {
    use serde_json::json;
    let old = json!({"lease_id":Uuid::new_v4(),"version":3,"holder_subject":"scheduler",
        "claimed_at":"2026-01-01T00:00:00.000000000Z","heartbeat_at":"2026-01-01T00:00:10.000000000Z",
        "expires_at":"2026-01-01T00:00:40.000000000Z"});
    let history: DeliveryBinding = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(&history).unwrap(), old);
    let mut current = history.clone();
    current.expires_at = None;
    let encoded = serde_json::to_value(&current).unwrap();
    assert!(encoded["expires_at"].is_null());
    assert_eq!(encoded["lease_id"], old["lease_id"]);
    assert_eq!(encoded["version"], old["version"]);
    assert_eq!(encoded["holder_subject"], old["holder_subject"]);
    let reserve = json!({"operation":"reserve","expected_task_version":1,"idempotency_key":"reserve"});
    let command: LifecycleCommand = serde_json::from_value(reserve.clone()).unwrap();
    validate_active_command(&command).unwrap();
    let mut timed = reserve;
    timed["lease_seconds"] = json!(30);
    assert!(serde_json::from_value::<LifecycleCommand>(timed).is_err());
    let fence = DeliveryFence { assignment_id:Uuid::new_v4(),execution_id:Uuid::new_v4(),
        fencing_token:1,lease_id:current.lease_id,expected_lease_version:current.version };
    let renewal = LifecycleCommand::Heartbeat { fence:fence.clone(),idempotency_key:"retired-heartbeat".into() };
    validate_command(&renewal).unwrap();
    assert!(validate_active_command(&renewal).is_err());

}


fn routing_resume_fixture() -> (LifecycleReceipt, RoutingPolicy) {
    let mut task = task();
    let mut routes = serde_json::json!({});
    for (i, role) in ["project_manager", "analyst", "architect", "developer", "reviewer", "tester", "devops"].iter().enumerate() {
        let suffix = role.replace('_', "-");
        routes[*role] = serde_json::json!({"agent_id":Uuid::new_v4(),"fleet_config_revision":2,
            "instruction_profile":"single-task-v1","instruction_manifest_sha256":"a".repeat(64),
            "namespace_id":(i+1).to_string(),"namespace_name":format!("single-task-{suffix}"),
            "workflow_id":(i+11).to_string(),"workflow_key":format!("single-task-v1:{role}"),
            "profile":format!("single-task-v1-{suffix}"),"workflow_catalog_version":1,"workflow_catalog_sha256":"b".repeat(64)});
    }
    task.routing.contract_version = 2;
    task.routing.policy.contract_version = 2;
    task.routing.policy.routes = serde_json::from_value(routes).unwrap();
    task.routing.policy.routing_hash = crate::sdlc::canonical_hash(&task.routing.policy.routes).unwrap();
    task.runtime = RuntimeState::Terminal;
    task.status = LifecycleStatus::Ready;
    task.resume_chat_id = Some(Uuid::new_v4());
    task.previous_result_id = Some(Uuid::new_v4());
    task.cycle = 2;
    task.transient_retries = 3; // Retained historical metadata.
    let mut policy = task.routing.policy.clone();
    policy.version += 1;
    policy.routes.analyst.fleet_config_revision += 1;
    policy.routing_hash = crate::sdlc::canonical_hash(&policy.routes).unwrap();
    (LifecycleReceipt { task, assignment: None, lease: None, binding: None, result: None }, policy)
}

#[test]
fn routing_resume_preserves_confirmed_business_context_for_every_downstream_stage() {
    for stage in [LifecycleStage::Analysis, LifecycleStage::Architecture, LifecycleStage::Development,
        LifecycleStage::Review, LifecycleStage::Testing, LifecycleStage::Rework, LifecycleStage::Deployment] {
        let (mut receipt, policy) = routing_resume_fixture();
        receipt.task.stage = stage;
        let original = receipt.clone();
        resume_with_current_routing(&mut receipt, &policy, original.task.version, policy.version,
            Uuid::new_v4(), Utc::now()).unwrap();
        assert_eq!(receipt.task.stage, original.task.stage);
        assert_eq!(receipt.task.cycle, original.task.cycle);
        assert_eq!(receipt.task.previous_result_id, original.task.previous_result_id);
        assert_eq!(receipt.task.resume_chat_id, original.task.resume_chat_id);
        assert_eq!(receipt.task.routing.confirmation_id, original.task.routing.confirmation_id);
        assert_eq!(receipt.task.routing.requirement_revision, original.task.routing.requirement_revision);
        assert_eq!(receipt.task.routing.content_hash, original.task.routing.content_hash);
        assert_eq!(receipt.task.routing.policy, policy);
        assert_ne!(receipt.task.routing.snapshot_id, original.task.routing.snapshot_id);
        assert_eq!(receipt.task.version, original.task.version + 1);
        assert_eq!(receipt.task.runtime, RuntimeState::Queued);
        assert_eq!(receipt.task.transient_retries, 0);
    }
}

#[test]
fn routing_resume_rejects_live_custody_foreign_policy_and_stale_cas_without_effects() {
    let (original, policy) = routing_resume_fixture();
    for runtime in [RuntimeState::Queued, RuntimeState::Reserved, RuntimeState::Running, RuntimeState::Unknown] {
        let mut changed = original.clone(); changed.task.runtime = runtime;
        let before = changed.clone();
        assert!(resume_with_current_routing(&mut changed, &policy, original.task.version,
            policy.version, Uuid::new_v4(), Utc::now()).is_err());
        assert_eq!(changed, before);
    }
    for (task_version, policy_version, foreign) in [(original.task.version+1,policy.version,false),
        (original.task.version,policy.version+1,false),(original.task.version,policy.version,true)] {
        let mut changed = original.clone(); let mut selected = policy.clone();
        if foreign { selected.project_id = Uuid::new_v4(); }
        assert!(resume_with_current_routing(&mut changed, &selected, task_version, policy_version,
            Uuid::new_v4(), Utc::now()).is_err());
        assert_eq!(changed, original);
    }
    let mut changed = original.clone();
    changed.binding = Some(RunBinding { chat_id:Uuid::new_v4(), run_id:Uuid::new_v4(),
        machine_subject:"original-producer".into(), dispatch_receipt:artifact("original-dispatch") });
    let before = changed.clone();
    assert!(resume_with_current_routing(&mut changed, &policy, original.task.version,policy.version,
        Uuid::new_v4(),Utc::now()).is_err());
    assert_eq!(changed,before);
    let mut legacy = original.clone(); legacy.task.routing.contract_version = 1;
    assert!(resume_with_current_routing(&mut legacy,&policy,original.task.version,policy.version,
        Uuid::new_v4(),Utc::now()).is_err());
}

#[test]
fn routing_resume_drops_only_the_current_chat_hint_when_the_assigned_agent_changes() {
    let (mut receipt, mut policy) = routing_resume_fixture();
    policy.routes.analyst.agent_id = Uuid::new_v4();
    policy.routing_hash = crate::sdlc::canonical_hash(&policy.routes).unwrap();
    let version = receipt.task.version;
    resume_with_current_routing(&mut receipt,&policy,version,policy.version,Uuid::new_v4(),Utc::now()).unwrap();
    assert_eq!(receipt.task.resume_chat_id,None);
}
