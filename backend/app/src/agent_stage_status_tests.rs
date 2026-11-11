use super::*;
use crate::sdlc_lifecycle::accept_result;
use domain::sdlc::{MachineFence, PmAssignment};
use domain::sdlc_lifecycle::*;
use serde_json::json;

fn now() -> DateTime<Utc> {
    "2026-11-07T12:00:00Z".parse().unwrap()
}

fn pm() -> (TaskState, Principal, StartStage) {
    let task_id = Uuid::new_v4();
    let actor = Principal {
        subject: Uuid::new_v4().to_string(),
        human_session: false,
        trusted_human: false,
        scopes: ["task-tracker:read".into(), "task-tracker:write".into()].into(),
    };
    let assignment = PmAssignment {
        assignment_id: Uuid::new_v4(),
        execution_id: Uuid::new_v4(),
        agent_id: Uuid::new_v4(),
        version: 3,
        machine_subject: actor.subject.clone(),
    };
    let command = StartStage {
        fence: StageStartFence::Pm(MachineFence {
            assignment_id: assignment.assignment_id,
            execution_id: assignment.execution_id,
            agent_id: assignment.agent_id,
            assignment_version: assignment.version,
        }),
        idempotency_key: "agent-start".into(),
    };
    let state = TaskState {
        tracker_instance_id: "test".into(),
        project_id: Uuid::new_v4(),
        task_id,
        root_task_id: task_id,
        owner_subject: "owner".into(),
        stage: Stage::Draft,
        status: Some(LifecycleStatus::Ready),
        draft_submode: None,
        confirmation_revision: None,
        assignment: Some(assignment),
        questions: vec![],
        revisions: vec![],
        confirmations: vec![],
        evidence: vec![],
        retained_fields:
            json!({"interaction_protocol":"configured-cli-v1","flow":"single-task-v1"})
                .as_object()
                .unwrap()
                .clone(),
    };
    (state, actor, command)
}

fn delivery(stage: LifecycleStage) -> (TaskState, Principal, StartStage, LifecycleReceipt) {
    let (mut state, actor, _) = pm();
    state.stage = stage.canonical();
    state.assignment = None;
    let route = json!({"agent_id":Uuid::new_v4(),"fleet_config_revision":1,
        "package_commit":"b".repeat(40),"package_manifest_sha256":"c".repeat(64),
        "namespace_id":"1","namespace_name":"test","workflow_id":"1","workflow_key":"test",
        "profile":"test","workflow_catalog_version":3,"workflow_catalog_sha256":"d".repeat(64)});
    let routing = json!({"contract_version":1,"snapshot_id":Uuid::new_v4(),"tracker_instance_id":"test",
        "project_id":state.project_id,"task_id":state.task_id,"root_task_id":state.root_task_id,
        "confirmation_id":Uuid::new_v4(),"requirement_revision":1,"content_hash":"e".repeat(64),
        "created_at":now(),"policy":{"contract_version":1,"tracker_instance_id":"test",
        "project_id":state.project_id,"version":1,"routing_hash":"f".repeat(64),
        "verification":"declared","native_ready":false,"dispatch_allowed":false,
        "author_subject":"owner","created_at":now(),"routes":{
            "project_manager":route,"analyst":route,"architect":route,"developer":route,
            "reviewer":route,"tester":route,"devops":route}}});
    let fence = DeliveryFence {
        assignment_id: Uuid::new_v4(),
        execution_id: Uuid::new_v4(),
        fencing_token: 5,
        lease_id: Uuid::new_v4(),
        expected_lease_version: 7,
    };
    let mut assignment = DeliveryAssignment {
        assignment_id: fence.assignment_id,
        execution_id: fence.execution_id,
        execution: None,
        task_id: state.task_id,
        owner_subject: state.owner_subject.clone(),
        role: stage.role().unwrap(),
        stage,
        cycle: 0,
        attempt: 1,
        agent_id: serde_json::from_value(route["agent_id"].clone()).unwrap(),
        machine_subject: Some(actor.subject.clone()),
        routing: serde_json::from_value(routing.clone()).unwrap(),
        previous_result_id: None,
        fencing_token: fence.fencing_token,
        lease_id: fence.lease_id,
        assignment_hash: String::new(),
    };
    assignment.assignment_hash = crate::sdlc_lifecycle::assignment_hash(&assignment).unwrap();
    let receipt = LifecycleReceipt {
        task: LifecycleTask {
            task_id: state.task_id,
            owner_subject: state.owner_subject.clone(),
            version: 2,
            stage,
            status: LifecycleStatus::Ready,
            cycle: 0,
            transient_retries: 0,
            waiting_reason: None,
            runtime: RuntimeState::Running,
            routing: serde_json::from_value(routing).unwrap(),
            previous_result_id: None,
            resume_chat_id: None,
        },
        assignment: Some(assignment),
        lease: Some(DeliveryBinding {
            lease_id: fence.lease_id,
            version: fence.expected_lease_version,
            holder_subject: "scheduler".into(),
            claimed_at: now(),
            heartbeat_at: None,
            expires_at: None,
        }),
        binding: Some(RunBinding {
            chat_id: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
            machine_subject: actor.subject.clone(),
            dispatch_receipt: artifact("native-start"),
        }),
        result: None,
    };
    (
        state,
        actor,
        StartStage {
            fence: StageStartFence::Delivery(fence),
            idempotency_key: "agent-start".into(),
        },
        receipt,
    )
}

fn artifact(name: &str) -> ArtifactReceipt {
    ArtifactReceipt {
        reference: format!("evidence/{name}.json"),
        sha256: "a".repeat(64),
    }
}

fn results() -> Vec<(LifecycleStage, DeliveryResult, LifecycleStage)> {
    vec![
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
            DeliveryResult::Developer {
                commit_sha: "b".repeat(40),
                changes: artifact("changes"),
                checks: vec![artifact("tests")],
            },
            LifecycleStage::Review,
        ),
        (
            LifecycleStage::Review,
            DeliveryResult::Reviewer {
                verdict: ReviewVerdict::Approved,
                report: artifact("review"),
            },
            LifecycleStage::Testing,
        ),
        (
            LifecycleStage::Testing,
            DeliveryResult::Tester {
                verdict: TestVerdict::Passed,
                report: artifact("tests"),
            },
            LifecycleStage::Deployment,
        ),
        (
            LifecycleStage::Deployment,
            DeliveryResult::DevOps {
                deployment: artifact("deployment"),
                verification: artifact("verification"),
            },
            LifecycleStage::Deployment,
        ),
    ]
}

#[test]
fn pm_can_start_before_requirements_and_consent_without_changing_the_assignment() {
    let (state, actor, command) = pm();
    let before = json!(state);
    let receipt = prepare_stage_start(&state, &actor, &command, None, now()).unwrap();
    assert_eq!(receipt.task_id, state.task_id);
    assert_eq!(receipt.stage, Stage::Draft);
    assert_eq!(receipt.status, LifecycleStatus::InProgress);
    assert_eq!(
        receipt.execution_id,
        state.assignment.as_ref().unwrap().execution_id
    );
    assert_eq!(receipt.idempotency_key, command.idempotency_key);
    assert_eq!(receipt.started_at, now());
    assert_eq!(json!(state), before);
    assert!(state.confirmations.is_empty());
}

#[test]
fn every_delivery_role_and_developer_rework_start_only_their_current_stage() {
    for stage in results()
        .into_iter()
        .map(|(stage, _, _)| stage)
        .chain([LifecycleStage::Rework])
    {
        let (state, actor, command, custody) = delivery(stage);
        let before = json!((&state, &custody));
        let receipt = prepare_stage_start(&state, &actor, &command, Some(&custody), now()).unwrap();
        assert_eq!(receipt.stage, stage.canonical());
        assert_eq!(receipt.status, LifecycleStatus::InProgress);
        assert_eq!(
            receipt.execution_id,
            custody.assignment.as_ref().unwrap().execution_id
        );
        assert_eq!(json!((&state, &custody)), before);
    }
}

#[test]
fn pm_start_refuses_foreign_and_stale_fences() {
    for case in 0..10 {
        let (mut state, mut actor, mut command) = pm();
        let StageStartFence::Pm(fence) = &mut command.fence else {
            unreachable!()
        };
        match case {
            0 => fence.assignment_id = Uuid::new_v4(),
            1 => fence.execution_id = Uuid::new_v4(),
            2 => fence.agent_id = Uuid::new_v4(),
            3 => fence.assignment_version += 1,
            4 => actor.subject = Uuid::new_v4().to_string(),
            5 => {
                fence.assignment_id = Uuid::nil();
                state.assignment.as_mut().unwrap().assignment_id = Uuid::nil();
            }
            6 => {
                fence.execution_id = Uuid::nil();
                state.assignment.as_mut().unwrap().execution_id = Uuid::nil();
            }
            7 => {
                fence.agent_id = Uuid::nil();
                state.assignment.as_mut().unwrap().agent_id = Uuid::nil();
            }
            8 => {
                fence.assignment_version = 0;
                state.assignment.as_mut().unwrap().version = 0;
            }
            _ => {
                fence.assignment_version = domain::sdlc::MAX_SAFE_VERSION + 1;
                state.assignment.as_mut().unwrap().version = fence.assignment_version;
            }
        }
        let before = json!(state);
        assert!(
            prepare_stage_start(&state, &actor, &command, None, now()).is_err(),
            "case {case}"
        );
        assert_eq!(json!(state), before);
    }
}

#[test]
fn start_refuses_humans_admins_missing_scopes_and_unassigned_actors() {
    for delivery_mode in [false, true] {
        for case in 0..6 {
            let (state, mut actor, command, custody) = if delivery_mode {
                let (s, a, c, r) = delivery(LifecycleStage::Analysis);
                (s, a, c, Some(r))
            } else {
                let (s, a, c) = pm();
                (s, a, c, None)
            };
            match case {
                0 => actor.human_session = true,
                1 => actor.trusted_human = true,
                2 => {
                    actor.scopes.insert("admin".into());
                }
                3 => {
                    actor.scopes.remove("task-tracker:read");
                }
                4 => {
                    actor.scopes.remove("task-tracker:write");
                }
                _ => actor.subject = Uuid::new_v4().to_string(),
            }
            assert!(
                prepare_stage_start(&state, &actor, &command, custody.as_ref(), now()).is_err(),
                "delivery={delivery_mode}, case={case}"
            );
        }
    }
}

#[test]
fn delivery_start_refuses_every_stale_fence_component() {
    for case in 0..5 {
        let (state, actor, mut command, custody) = delivery(LifecycleStage::Analysis);
        let StageStartFence::Delivery(fence) = &mut command.fence else {
            unreachable!()
        };
        match case {
            0 => fence.assignment_id = Uuid::new_v4(),
            1 => fence.execution_id = Uuid::new_v4(),
            2 => fence.fencing_token += 1,
            3 => fence.lease_id = Uuid::new_v4(),
            _ => fence.expected_lease_version += 1,
        }
        assert!(
            prepare_stage_start(&state, &actor, &command, Some(&custody), now()).is_err(),
            "case {case}"
        );
    }
}

#[test]
fn delivery_start_uses_current_fence_not_archival_dates_or_elapsed_time() {
    let (state, actor, command, mut custody) = delivery(LifecycleStage::Analysis);
    assert!(prepare_stage_start(&state, &actor, &command, Some(&custody), now()).is_ok());
    let binding = custody.lease.as_mut().unwrap();
    binding.heartbeat_at = Some(binding.claimed_at);
    binding.expires_at = Some(binding.claimed_at + chrono::Duration::seconds(30));
    assert!(
        prepare_stage_start(
            &state,
            &actor,
            &command,
            Some(&custody),
            now() + chrono::Duration::days(1)
        )
        .is_ok()
    );
    custody.task.runtime = RuntimeState::Unknown;
    assert!(prepare_stage_start(&state, &actor, &command, Some(&custody), now()).is_err());
}

#[test]
fn delivery_start_requires_exact_custody_and_native_binding() {
    for case in 0..23 {
        let (state, actor, command, mut custody) = delivery(LifecycleStage::Analysis);
        match case {
            0 => custody.task.task_id = Uuid::new_v4(),
            1 => custody.task.owner_subject = "foreign".into(),
            2 => custody.task.stage = LifecycleStage::Architecture,
            3 => custody.task.runtime = RuntimeState::Unknown,
            4 => custody.assignment = None,
            5 => custody.lease = None,
            6 => custody.binding = None,
            7 => custody.lease.as_mut().unwrap().version += 1,
            8 => custody.lease.as_mut().unwrap().version = 0,
            9 => custody.binding.as_mut().unwrap().machine_subject = Uuid::new_v4().to_string(),
            10 => custody.assignment.as_mut().unwrap().machine_subject = None,
            11 => custody.binding.as_mut().unwrap().run_id = Uuid::nil(),
            12 => custody.binding.as_mut().unwrap().chat_id = Uuid::nil(),
            13 => custody.assignment.as_mut().unwrap().role = DeliveryRole::Architect,
            14 => custody.assignment.as_mut().unwrap().owner_subject = "foreign".into(),
            15 => custody.assignment.as_mut().unwrap().task_id = Uuid::new_v4(),
            16 => custody.assignment.as_mut().unwrap().cycle += 1,
            17 => custody.assignment.as_mut().unwrap().routing.snapshot_id = Uuid::new_v4(),
            18 => custody.assignment.as_mut().unwrap().assignment_hash = "0".repeat(64),
            19 => custody.assignment.as_mut().unwrap().stage = LifecycleStage::Architecture,
            20 => custody.lease.as_mut().unwrap().lease_id = Uuid::new_v4(),
            21 => custody.task.status = LifecycleStatus::InProgress,
            _ => custody.task.status = LifecycleStatus::Completed,
        }
        if matches!(case, 10 | 13..=17 | 19) {
            let assignment = custody.assignment.as_mut().unwrap();
            assignment.assignment_hash =
                crate::sdlc_lifecycle::assignment_hash(assignment).unwrap();
        }
        let before = json!((&state, &custody));
        assert!(
            prepare_stage_start(&state, &actor, &command, Some(&custody), now()).is_err(),
            "case {case}"
        );
        assert_eq!(json!((&state, &custody)), before);
    }
}

#[test]
fn start_requires_ready_root_configured_stage_and_correct_fence_kind() {
    for case in 0..7 {
        let (mut state, actor, mut command) = pm();
        match case {
            0 => state.status = None,
            1 => state.status = Some(LifecycleStatus::InProgress),
            2 => state.status = Some(LifecycleStatus::Completed),
            3 => state.root_task_id = Uuid::new_v4(),
            4 => {
                state.retained_fields.remove("interaction_protocol");
            }
            5 => state.stage = Stage::Analysis,
            _ => command.idempotency_key.clear(),
        }
        assert!(
            prepare_stage_start(&state, &actor, &command, None, now()).is_err(),
            "case {case}"
        );
    }
    let (pm_state, actor, pm_command) = pm();
    let (delivery_state, _, delivery_command, custody) = delivery(LifecycleStage::Analysis);
    assert!(prepare_stage_start(&pm_state, &actor, &pm_command, Some(&custody), now()).is_err());
    assert!(prepare_stage_start(&pm_state, &actor, &delivery_command, None, now()).is_err());
    assert!(prepare_stage_start(&delivery_state, &actor, &pm_command, None, now()).is_err());
}

#[test]
fn start_wire_requires_explicit_fence_kind_and_rejects_status_or_stage_overrides() {
    let (_, _, command) = pm();
    let raw = json!(command);
    assert_eq!(raw["fence"]["kind"], "pm");
    assert_eq!(
        json!(serde_json::from_value::<StartStage>(raw.clone()).unwrap()),
        raw
    );
    let (_, _, command, _) = delivery(LifecycleStage::Analysis);
    assert_eq!(json!(command)["fence"]["kind"], "delivery");
    for key in [
        "kind",
        "assignment_id",
        "execution_id",
        "agent_id",
        "assignment_version",
    ] {
        let mut bad = raw.clone();
        bad["fence"].as_object_mut().unwrap().remove(key);
        assert!(
            serde_json::from_value::<StartStage>(bad).is_err(),
            "missing {key}"
        );
    }
    for key in ["stage", "status", "next_stage"] {
        let mut bad = raw.clone();
        bad[key] = json!("Completed");
        assert!(
            serde_json::from_value::<StartStage>(bad).is_err(),
            "extra {key}"
        );
    }
}

#[test]
fn delivery_results_require_agent_completed_not_merely_native_running() {
    for (stage, output, _) in results() {
        for status in [LifecycleStatus::Ready, LifecycleStatus::InProgress] {
            let (_, _, _, mut custody) = delivery(stage);
            custody.task.status = status;
            let before = json!(custody.task);
            assert!(
                accept_result(&mut custody.task, &output).is_err(),
                "{stage:?}/{status:?}"
            );
            assert_eq!(json!(custody.task), before);
        }
    }
}

#[test]
fn completed_delivery_handoff_resets_ready_but_final_devops_remains_completed() {
    for (stage, output, next_stage) in results() {
        let (_, _, _, mut custody) = delivery(stage);
        custody.task.status = LifecycleStatus::Completed;
        let before = custody.task.clone();
        accept_result(&mut custody.task, &output).unwrap();
        assert_eq!(custody.task.stage, next_stage);
        assert_eq!(custody.task.version, before.version + 1);
        assert_eq!(custody.task.task_id, before.task_id);
        assert_eq!(custody.task.owner_subject, before.owner_subject);
        assert_eq!(custody.task.routing, before.routing);
        if stage == LifecycleStage::Deployment {
            assert_eq!(custody.task.status, LifecycleStatus::Completed);
            assert_eq!(custody.task.runtime, RuntimeState::Terminal);
            assert_eq!(custody.task.waiting_reason, None);
        } else {
            assert_eq!(custody.task.status, LifecycleStatus::Ready);
            assert_eq!(custody.task.runtime, RuntimeState::Queued);
            assert_eq!(
                custody.task.waiting_reason.as_deref(),
                Some("queued_for_assignment")
            );
        }
        let after = json!(custody.task);
        assert!(accept_result(&mut custody.task, &output).is_err());
        assert_eq!(json!(custody.task), after);
    }
}

#[test]
fn test_rework_handoff_resets_ready_for_the_developer() {
    let stage = LifecycleStage::Testing;
    let output = DeliveryResult::Tester {
        verdict: TestVerdict::Rework,
        report: artifact("tests"),
    };
    let (_, _, _, mut custody) = delivery(stage);
    custody.task.status = LifecycleStatus::Completed;
    accept_result(&mut custody.task, &output).unwrap();
    assert_eq!(custody.task.stage, LifecycleStage::Rework);
    assert_eq!(custody.task.status, LifecycleStatus::Ready);
    assert_eq!(custody.task.cycle, 1);
    assert_eq!(custody.task.runtime, RuntimeState::Queued);
    let (_, _, _, mut limited) = delivery(stage);
    limited.task.status = LifecycleStatus::Completed;
    limited.task.cycle = MAX_REWORK_CYCLES;
    accept_result(&mut limited.task, &output).unwrap();
    assert_eq!(limited.task.stage, stage);
    assert_eq!(limited.task.status, LifecycleStatus::Completed);
    assert_eq!(limited.task.runtime, RuntimeState::Terminal);
    assert_eq!(
        limited.task.waiting_reason.as_deref(),
        Some("rework_limit_reached")
    );
}

#[test]
fn completed_wrong_role_unknown_runtime_and_exhausted_version_cannot_handoff() {
    for case in 0..3 {
        let (_, _, _, mut custody) = delivery(LifecycleStage::Analysis);
        custody.task.status = LifecycleStatus::Completed;
        match case {
            0 => custody.task.stage = LifecycleStage::Architecture,
            1 => custody.task.runtime = RuntimeState::Unknown,
            _ => custody.task.version = domain::sdlc::MAX_SAFE_VERSION,
        }
        let before = json!(custody.task);
        let (_, output, _) = results().remove(0);
        assert!(
            accept_result(&mut custody.task, &output).is_err(),
            "case {case}"
        );
        assert_eq!(json!(custody.task), before);
    }
}

#[test]
fn archived_completed_review_cannot_be_applied_or_queued_again() {
    for verdict in [ReviewVerdict::Approved, ReviewVerdict::Rework] {
        let (_, _, _, mut custody) = delivery(LifecycleStage::Review);
        custody.task.status = LifecycleStatus::Completed;
        custody.task.runtime = RuntimeState::Terminal;
        custody.task.waiting_reason = Some("review_completed".into());
        let before = json!(custody.task);
        assert!(
            accept_result(
                &mut custody.task,
                &DeliveryResult::Reviewer {
                    verdict,
                    report: artifact("archived-review")
                },
            )
            .is_err()
        );
        assert!(crate::sdlc_lifecycle::ensure_queued(&mut custody.task).is_err());
        assert_eq!(json!(custody.task), before);
    }
}
