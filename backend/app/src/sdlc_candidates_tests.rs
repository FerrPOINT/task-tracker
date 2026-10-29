//! Synthetic policy fixtures only; no native execution or artifact acceptance claim.
use super::*;
use domain::sdlc_lifecycle::{LifecycleTask, RuntimeState};
use domain::sdlc_routing::{RoleRoutes, RoutingPolicy, RoutingVerification, TaskRoutingSnapshot};
use serde_json::json;
use uuid::Uuid;

fn config() -> SdlcConfig {
    SdlcConfig {
        instance_id: "candidates-test".into(),
        orchestrator_subject: "pm".into(),
        verifier_subject: "verifier".into(),
        reservation_scheduler_subject: "scheduler".into(),
        single_task_project_ids: Default::default(),
    }
}

#[test]
fn only_configured_scheduler_with_read_only_scope_can_discover() {
    let actor = Principal {
        subject: "scheduler".into(),
        human_session: false,
        trusted_human: false,
        scopes: ["task-tracker:read".into()].into_iter().collect(),
    };
    authorize(&actor, &config()).unwrap();
    for scopes in [
        vec![],
        vec!["task-tracker:write"],
        vec!["task-tracker:read", "task-tracker:write"],
        vec!["task-tracker:read", "task-tracker:sdlc:delivery:foreign"],
    ] {
        let mut wrong = actor.clone();
        wrong.scopes = scopes.into_iter().map(str::to_owned).collect();
        assert!(matches!(
            authorize(&wrong, &config()),
            Err(AppError::Forbidden)
        ));
    }
    for subject in ["owner", "pm", "verifier", "another-scheduler"] {
        let mut wrong = actor.clone();
        wrong.subject = subject.into();
        assert!(matches!(
            authorize(&wrong, &config()),
            Err(AppError::Forbidden)
        ));
    }
    let mut human = actor.clone();
    human.human_session = true;
    assert!(matches!(
        authorize(&human, &config()),
        Err(AppError::Forbidden)
    ));
    let mut missing = config();
    missing.reservation_scheduler_subject.clear();
    assert!(matches!(
        authorize(&actor, &missing),
        Err(AppError::Unavailable(_))
    ));
}

fn fixture() -> (TaskState, LifecycleReceipt) {
    let task = Uuid::new_v4();
    let project = Uuid::new_v4();
    let mut routes = json!({});
    for (i, role) in [
        "project_manager",
        "analyst",
        "architect",
        "developer",
        "reviewer",
        "tester",
        "devops",
    ]
    .iter()
    .enumerate()
    {
        let suffix = role.replace('_', "-");
        routes[*role] = json!({"agent_id":Uuid::new_v4(),"fleet_config_revision":1,
            "instruction_profile":"single-task-v1","instruction_manifest_sha256":"a".repeat(64),
            "namespace_id":(i+1).to_string(),"namespace_name":format!("single-task-{suffix}"),
            "workflow_id":(i+11).to_string(),"workflow_key":format!("single-task-v1:{role}"),
            "profile":format!("single-task-v1-{suffix}"),"workflow_catalog_version":1,"workflow_catalog_sha256":"b".repeat(64)});
    }
    let routes: RoleRoutes = serde_json::from_value(routes).unwrap();
    let policy = RoutingPolicy {
        contract_version: 2,
        tracker_instance_id: "candidates-test".into(),
        project_id: project,
        version: 1,
        routing_hash: crate::sdlc::canonical_hash(&routes).unwrap(),
        routes,
        verification: RoutingVerification::Declared,
        native_ready: false,
        dispatch_allowed: false,
        author_subject: "owner".into(),
        created_at: chrono::Utc::now(),
    };
    let snapshot = TaskRoutingSnapshot {
        contract_version: 2,
        snapshot_id: Uuid::new_v4(),
        tracker_instance_id: "candidates-test".into(),
        project_id: project,
        task_id: task,
        root_task_id: task,
        confirmation_id: Uuid::new_v4(),
        requirement_revision: 1,
        content_hash: "c".repeat(64),
        policy,
        created_at: chrono::Utc::now(),
    };
    let state: TaskState = serde_json::from_value(json!({"tracker_instance_id":"candidates-test",
        "project_id":project,"task_id":task,"root_task_id":task,"owner_subject":"owner","stage":"Analysis",
        "flow":"single-task-v1","confirmation_revision":1,"assignment":null,"questions":[],"confirmations":[],"evidence":[],
        "revisions":[{"revision":1,"content_hash":"c".repeat(64),"goal":"fixture","scope":[],"exclusions":[],
        "scenarios":[],"acceptance_criteria":[],"constraints":[],"dependencies":[],"assumptions":[],"checklist":[],
        "prerequisites":[],"author_subject":"pm","created_at":chrono::Utc::now()}]})).unwrap();
    let receipt = LifecycleReceipt {
        task: LifecycleTask {
            task_id: task,
            owner_subject: "owner".into(),
            version: 7,
            stage: LifecycleStage::Analysis,
            status: LifecycleStatus::Ready,
            cycle: 0,
            transient_retries: 0,
            waiting_reason: Some("queued_for_assignment".into()),
            runtime: RuntimeState::Queued,
            routing: snapshot,
            previous_result_id: None,
            resume_chat_id: None,
        },
        assignment: None,
        lease: None,
        binding: None,
        result: None,
    };
    (state, receipt)
}

#[test]
fn consent_alone_never_discovers_analyst_before_actual_pm_completion() {
    let (state, _) = fixture();
    assert!(candidate(&state, None, false, false).unwrap().is_none());
    let ready = candidate(&state, None, true, false).unwrap().unwrap();
    assert_eq!(ready.next_operation, CandidateOperation::EnsureQueued);
    assert_eq!(ready.task_version, None);
    let mut legacy = state.clone();
    legacy.retained_fields.remove("flow");
    assert!(candidate(&legacy, None, true, false).unwrap().is_none());
    for stage in [
        Stage::Draft,
        Stage::Backlog,
        Stage::Development,
        Stage::Deployment,
    ] {
        let mut early = state.clone();
        early.stage = stage;
        assert!(candidate(&early, None, true, false).unwrap().is_none());
    }
}

#[test]
fn only_ready_queued_unassigned_heads_produce_reserve_with_actual_cas() {
    let (state, head) = fixture();
    let ready = candidate(&state, Some(&head), false, false)
        .unwrap()
        .unwrap();
    assert_eq!(ready.next_operation, CandidateOperation::Reserve);
    assert_eq!(ready.task_version, Some(7));
    for runtime in [
        RuntimeState::Reserved,
        RuntimeState::Running,
        RuntimeState::Unknown,
        RuntimeState::Terminal,
    ] {
        let mut wrong = head.clone();
        wrong.task.runtime = runtime;
        assert!(
            candidate(&state, Some(&wrong), true, false)
                .unwrap()
                .is_none()
        );
    }
    for status in [LifecycleStatus::InProgress, LifecycleStatus::Completed] {
        let mut wrong = head.clone();
        wrong.task.status = status;
        assert!(
            candidate(&state, Some(&wrong), true, false)
                .unwrap()
                .is_none()
        );
    }
    let mut wrong = head.clone();
    wrong.task.version = 0;
    assert!(candidate(&state, Some(&wrong), true, false).is_err());
    let mut assigned = head.clone();
    assigned.assignment = Some(serde_json::from_value(json!({"assignment_id":Uuid::new_v4(),
        "execution_id":Uuid::new_v4(),"task_id":state.task_id,"owner_subject":"owner","role":"analyst",
        "stage":"Analysis","cycle":0,"attempt":1,"agent_id":head.task.routing.policy.routes.analyst.agent_id,
        "routing":head.task.routing,"previous_result_id":null,"fencing_token":1,"lease_id":Uuid::new_v4(),
        "assignment_hash":"a".repeat(64)})).unwrap());
    assert!(
        candidate(&state, Some(&assigned), true, false)
            .unwrap()
            .is_none()
    );
    let mut wrong = state.clone();
    wrong.root_task_id = Uuid::new_v4();
    assert!(candidate(&wrong, Some(&head), true, false).is_err());
    for stage in [
        Stage::Analysis,
        Stage::Architecture,
        Stage::Development,
        Stage::Review,
        Stage::Testing,
        Stage::Rework,
        Stage::Deployment,
    ] {
        let mut state = state.clone();
        let mut head = head.clone();
        state.stage = stage;
        head.task.stage = LifecycleStage::from_legacy(stage);
        assert_eq!(
            candidate(&state, Some(&head), true, false)
                .unwrap()
                .unwrap()
                .next_operation,
            CandidateOperation::Reserve
        );
    }
}

#[test]
fn configured_cli_pm_never_opts_in_to_downstream_discovery() {
    let (mut state, mut head) = fixture();
    state
        .retained_fields
        .insert("interaction_protocol".into(), json!("configured-cli-v1"));
    for stage in [
        Stage::Draft,
        Stage::Backlog,
        Stage::Analysis,
        Stage::Architecture,
        Stage::Development,
        Stage::Review,
        Stage::Testing,
        Stage::Rework,
        Stage::Deployment,
    ] {
        state.stage = stage;
        head.task.stage = LifecycleStage::from_legacy(stage);
        assert!(
            candidate(&state, None, true, false).unwrap().is_none(),
            "{stage:?}"
        );
        assert!(
            candidate(&state, Some(&head), true, false)
                .unwrap()
                .is_none(),
            "{stage:?}"
        );
    }
    state.retained_fields.remove("interaction_protocol");
    state.stage = Stage::Analysis;
    head.task.stage = LifecycleStage::Analysis;
    assert_eq!(
        candidate(&state, None, true, false)
            .unwrap()
            .unwrap()
            .next_operation,
        CandidateOperation::EnsureQueued
    );
    assert_eq!(
        candidate(&state, Some(&head), true, false)
            .unwrap()
            .unwrap()
            .next_operation,
        CandidateOperation::Reserve
    );
}

#[test]
fn configured_discovery_requires_explicit_start_current_completion_and_head() {
    let (mut state, head) = fixture();
    state
        .retained_fields
        .insert("interaction_protocol".into(), json!("configured-cli-v1"));
    assert!(
        candidate(&state, Some(&head), true, false)
            .unwrap()
            .is_none()
    );
    assert!(
        candidate(&state, Some(&head), false, true)
            .unwrap()
            .is_none()
    );
    assert!(candidate(&state, None, true, true).unwrap().is_none());
    assert_eq!(
        candidate(&state, Some(&head), true, true)
            .unwrap()
            .unwrap()
            .next_operation,
        CandidateOperation::Reserve
    );
}

#[test]
fn uuid_keyset_uses_last_returned_not_lookahead_and_empty_pages_have_null_cursor() {
    let query = DeliveryCandidatesQuery {
        project_id: Uuid::new_v4(),
        after: None,
        limit: 2,
    };
    let item = |n| DeliveryCandidate {
        task_id: Uuid::from_u128(n),
        root_task_id: Uuid::from_u128(n),
        requirement_revision: 1,
        next_operation: CandidateOperation::Reserve,
        task_version: Some(7),
    };
    let (first, cursor) = page(&query, vec![item(2), item(7), item(99)]).unwrap();
    assert_eq!(first.len(), 2);
    assert_eq!(cursor, Some(Uuid::from_u128(7)));
    let second = DeliveryCandidatesQuery {
        after: cursor,
        ..query.clone()
    };
    let (last, cursor) = page(&second, vec![item(99)]).unwrap();
    assert_eq!(last.len(), 1);
    assert_eq!(cursor, None);
    assert_eq!(page(&query, vec![]).unwrap(), (vec![], None));
    assert!(page(&second, vec![item(7)]).is_err());
    assert!(page(&query, vec![item(7), item(2)]).is_err());
    assert!(page(&query, vec![item(2), item(2)]).is_err());
    assert!(page(&query, vec![item(1), item(2), item(3), item(4)]).is_err());
}
