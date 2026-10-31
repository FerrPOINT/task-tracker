//! Exercise the real agent-result owner without compiling unrelated monolithic test modules.
use app::sdlc_lifecycle::accept_result;
use chrono::Utc;
use domain::sdlc_lifecycle::*;
use domain::sdlc_routing::{
    RoleRoute, RoleRoutes, RoutingPolicy, RoutingVerification, TaskRoutingSnapshot,
};
use domain::task_lifecycle::{self, Action, State};
use uuid::Uuid;
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

fn artifact() -> ArtifactReceipt {
    ArtifactReceipt {
        reference: "fixture/evidence".into(),
        sha256: "a".repeat(64),
    }
}
#[test]
fn agent_results_and_human_completion_follow_the_same_stages() {
    let cases = [
        (
            LifecycleStage::Analysis,
            DeliveryResult::Analyst {
                analysis: artifact(),
            },
        ),
        (
            LifecycleStage::Architecture,
            DeliveryResult::Architect { design: artifact() },
        ),
        (
            LifecycleStage::Development,
            DeliveryResult::Developer {
                commit_sha: "a".repeat(40),
                changes: artifact(),
                checks: vec![artifact()],
            },
        ),
        (
            LifecycleStage::Review,
            DeliveryResult::Reviewer {
                verdict: ReviewVerdict::Approved,
                report: artifact(),
            },
        ),
        (
            LifecycleStage::Testing,
            DeliveryResult::Tester {
                verdict: TestVerdict::Passed,
                report: artifact(),
            },
        ),
        (
            LifecycleStage::Deployment,
            DeliveryResult::DevOps {
                deployment: artifact(),
                verification: artifact(),
            },
        ),
    ];
    for (stage, result) in cases {
        let mut task = task();
        task.stage = stage;
        let human = task_lifecycle::transition(
            State {
                stage,
                status: LifecycleStatus::InProgress,
                revision: task.version,
            },
            Action::Complete,
        )
        .unwrap();
        accept_result(&mut task, &result).unwrap();
        assert_eq!(
            (task.stage, task.status, task.version),
            (human.stage, human.status, human.revision)
        );
    }
}
#[test]
fn rework_is_a_shared_transition_and_wrong_role_or_unknown_runtime_is_rejected() {
    let result = DeliveryResult::Reviewer {
        verdict: ReviewVerdict::Rework,
        report: artifact(),
    };
    let mut state = task();
    state.stage = LifecycleStage::Review;
    let human = task_lifecycle::transition(
        State {
            stage: state.stage,
            status: state.status,
            revision: state.version,
        },
        Action::RequestRework,
    )
    .unwrap();
    accept_result(&mut state, &result).unwrap();
    assert_eq!((state.stage, state.status), (human.stage, human.status));
    assert_eq!(state.cycle, 1);
    let mut wrong = task();
    assert!(accept_result(&mut wrong, &result).is_err());
    let mut unknown = task();
    unknown.stage = LifecycleStage::Review;
    unknown.runtime = RuntimeState::Unknown;
    assert!(accept_result(&mut unknown, &result).is_err());
}
