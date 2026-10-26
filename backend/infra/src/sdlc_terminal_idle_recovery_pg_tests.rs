//! Registered under lifecycle::regression_tests to reuse existing producer fixtures.
use super::*;
use domain::sdlc_execution_lease::*;
use domain::sdlc_terminal_idle_recovery::*;

async fn protected_state(db: &DatabaseConnection, task: Uuid) -> Json {
    db.query_one(statement("SELECT jsonb_build_object('task',to_jsonb(t),
        'leases',(SELECT jsonb_agg(to_jsonb(l) ORDER BY l.execution_id) FROM sdlc_pm_execution_leases l
            JOIN sdlc_pm_executions e USING(execution_id) WHERE e.task_id=t.task_id),
        'feedback',(SELECT jsonb_agg(to_jsonb(f) ORDER BY message_id) FROM sdlc_pm_feedback f WHERE task_id=t.task_id),
        'outbox',(SELECT count(*) FROM sdlc_outbox WHERE task_id=t.task_id),
        'resume',(SELECT count(*) FROM sdlc_pm_resume_inputs WHERE task_id=t.task_id),
        'lease_ops',(SELECT count(*) FROM sdlc_pm_lease_operations o JOIN sdlc_pm_executions e USING(execution_id) WHERE e.task_id=t.task_id)) AS value
        FROM sdlc_tasks t WHERE task_id=$1",vec![task.into()])).await.unwrap().unwrap().try_get("","value").unwrap()
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_PM_TERMINAL_IDLE_TEST_DATABASE_URL ending _pm_terminal_idle_test"]
async fn terminal_idle_permit_is_only_metadata_replay_does_not_renew_and_unknown_custody_is_rejected()
 {
    let url =
        std::env::var("TT_PM_TERMINAL_IDLE_TEST_DATABASE_URL").expect("fresh own PG required");
    let db = Database::connect(&url).await.unwrap();
    let row = db.query_one(statement("SELECT current_database() AS name,to_regclass('public.seaql_migrations') IS NULL AS fresh", vec![])).await.unwrap().unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_pm_terminal_idle_test")
    );
    assert!(row.try_get::<bool>("", "fresh").unwrap());
    migration::Migrator::up(&db, None).await.unwrap();
    let project = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    for (subject, id) in [
        ("owner", owner_id),
        ("pm", Uuid::new_v4()),
        ("scheduler", Uuid::new_v4()),
        ("verifier", Uuid::new_v4()),
    ] {
        exec(&db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,$2,$3,$3,'!',$3,true)",
            vec![id.into(),format!("{subject}@example.test").into(),subject.into()]).await.unwrap();
    }
    exec(&db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,'IDLE','Idle fixture',$2,$3)",
        vec![project.into(),owner_id.into(),Uuid::new_v4().into()]).await.unwrap();
    exec(&db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Idle','[]' FROM projects WHERE id=$1",vec![project.into()]).await.unwrap();
    exec(&db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub IN ('pm','scheduler','verifier')",vec![project.into()]).await.unwrap();
    let repo = PostgresSdlcRepository::connect(
        &url,
        SdlcConfig {
            instance_id: "idle-fixture".into(),
            orchestrator_subject: "pm".into(),
            verifier_subject: "verifier".into(),
            reservation_scheduler_subject: "scheduler".into(),
            single_task_project_ids: [project].into_iter().collect(),
        },
    )
    .await
    .unwrap();
    let owner = actor("owner", true, vec![]);
    let scheduler = actor("scheduler", false, vec![]);
    let routes = super::draft_flow_tests::single_task_routes();
    repo.set_routing_policy(
        project,
        &owner,
        domain::sdlc_routing::SetRoutingPolicy {
            expected_version: None,
            routes: routes.clone(),
            idempotency_key: "policy".into(),
        },
    )
    .await
    .unwrap();
    let (draft, _) = repo
        .create_draft(
            project,
            &owner,
            CreateDraftCommand {
                interaction_protocol: None,
                flow: Some(DraftFlow::SingleTaskV1),
                title: "Original goal".into(),
                description: "Synthetic only".into(),
                idempotency_key: "draft".into(),
            },
        )
        .await
        .unwrap();
    let task = draft.task_id;
    let (reservation,_) = repo.reserve_pm_draft(task,&owner,parse(json!({"requested_agent_id":routes.project_manager.agent_id,
        "expected_owner_version":0,"expected_assignment_version":null,"idempotency_key":"reserve"}))).await.unwrap();
    let pm = actor(
        "pm",
        false,
        vec![
            "task-tracker:read".into(),
            reservation.assignment.scope(task),
        ],
    );
    let fence: MachineFence = parse(
        json!({"assignment_id":reservation.assignment.assignment_id,"execution_id":reservation.assignment.execution_id,
        "agent_id":reservation.assignment.agent_id,"assignment_version":1}),
    );
    let (lease, _) = repo
        .claim_execution_lease(
            task,
            &pm,
            ClaimExecutionLease {
                expected_owner_version: 1,
                fence: fence.clone(),
                idempotency_key: "claim".into(),
            },
        )
        .await
        .unwrap();
    let (feedback, _) = repo
        .submit_pm_feedback(
            task,
            &owner,
            SubmitPmFeedback {
                message_id: Uuid::new_v4(),
                fence: fence.clone(),
                text: "Saved owner feedback".into(),
                idempotency_key: "feedback".into(),
            },
        )
        .await
        .unwrap();
    let run = Uuid::new_v4();
    let checkpoint = Uuid::new_v4();
    let identity = PmExecutionIdentity {
        task: reservation.execution.key.clone(),
        execution_ref: fence.execution_id.to_string(),
        tracker_instance_ref: reservation.binding.tracker_instance_id.clone(),
        tracker_project_ref: project.to_string(),
        task_ref: task.to_string(),
        root_ref: task.to_string(),
        agent_ref: fence.agent_id.to_string(),
        assignment_operation_key: reservation.assignment_operation_key.clone(),
        assignment_ref: fence.assignment_id.to_string(),
        assignment_revision: 1,
    };
    let native = PmIdleNativeObservation {
        identity: identity.clone(),
        observation_ref: Uuid::new_v4(),
        binding_ref: format!("fleet:pm:{run}"),
        hermes_run_ref: "run_synthetic_accepted".into(),
        session_run_id: run,
        status: PmIdleNativeStatus::Completed,
        dispatch_operation_key: format!("fleet-pm-bind:{run}"),
        checkpoint_ref: None,
        fence: 1,
    };
    let key = format!("fleet-pm-checkpoint:{checkpoint}");
    let mut body = serde_json::to_value(&identity).unwrap();
    body.as_object_mut().unwrap().extend(json!({"checkpoint_ref":checkpoint,"operation_key":key,"session_run_id":run,
        "binding_ref":native.binding_ref,"hermes_run_ref":native.hermes_run_ref,"expected_fence":1,"expected_version":1,
        "instruction_profile":"single-task-v1","instruction_manifest_sha256":"a".repeat(64),"requirements_revision":null,
        "requirements_sha256":null,"clarification_request_ref":null,"clarification_version":null}).as_object().unwrap().clone());
    let c = PermitPmIdleCheckpointRequest {
        expected_owner_version: 1,
        lease_id: lease.lease.lease_id,
        expected_lease_version: lease.lease.version,
        input: PmResumeInput::Feedback {
            input_id: feedback.message_id,
        },
        previous_run_id: run,
        previous_run_terminal: ArtifactReceipt {
            reference: format!("/internal/runtime/v1/pm/runs/{run}"),
            sha256: app::sdlc::canonical_hash(&serde_json::to_value(&native).unwrap()).unwrap(),
        },
        fence,
        terminal_idle_recovery: PmTerminalIdleRecovery {
            contract_version: 1,
            barrier_id: Uuid::new_v4(),
            checkpoint_operation_key: key.clone(),
            checkpoint_sha256: app::sdlc::canonical_hash(&body).unwrap(),
        },
        idempotency_key: format!("fleet-pm-terminal-idle:{run}:{}", feedback.message_id),
    };
    let w = PmTerminalIdleRecoveryWitness {
        contract_version: 1,
        barrier_id: c.terminal_idle_recovery.barrier_id,
        owner_subject: "owner".into(),
        identity,
        request_sha256: app::sdlc_terminal_idle_recovery::request_hash(&c).unwrap(),
        command: c.clone(),
        native_terminal: native,
        feedback_receipt: ArtifactReceipt {
            reference: "synthetic://feedback".into(),
            sha256: app::sdlc::canonical_hash(&serde_json::to_value(&feedback).unwrap()).unwrap(),
        },
        instruction_operation_key: format!("fleet-pm-first-step:{run}"),
        instruction_request_sha256: "b".repeat(64),
        instruction_result_sha256: "c".repeat(64),
        checkpoint_request: PmIdleCheckpointRequest {
            operation_key: key,
            body,
        },
    };
    let before = protected_state(&db, task).await;
    assert!(
        repo.preflight_terminal_idle(task, &scheduler, &c)
            .await
            .is_err(),
        "live lease cannot use idle permit"
    );
    // Use the real producer's expiry, never mutate timestamps or canonical lease JSON.
    tokio::time::sleep(std::time::Duration::from_secs(31)).await;
    assert!(
        repo.preflight_terminal_idle(task, &scheduler, &c)
            .await
            .unwrap()
            .is_none()
    );
    let mut bad = w.clone();
    bad.native_terminal.status = PmIdleNativeStatus::Running;
    assert!(
        repo.permit_terminal_idle(task, &scheduler, c.clone(), bad)
            .await
            .is_err()
    );
    assert_eq!(protected_state(&db, task).await, before);
    let (first, replayed) = repo
        .permit_terminal_idle(task, &scheduler, c.clone(), w.clone())
        .await
        .unwrap();
    assert!(!replayed);
    assert_eq!(protected_state(&db, task).await, before);
    let core = serde_json::to_value(&first.permit).unwrap();
    let mut scheduler_reader = scheduler.clone();
    scheduler_reader.scopes = ["task-tracker:read".into()].into_iter().collect();
    let readback = repo
        .pm_terminal_idle_checkpoint_operation(task, &scheduler_reader, &c.idempotency_key)
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(&readback.permit).unwrap(), core);
    assert!(readback.observed_at >= first.observed_at);
    assert!(matches!(
        repo.permit_terminal_idle(task, &scheduler_reader, c.clone(), w.clone())
            .await,
        Err(AppError::Forbidden)
    ));
    assert_eq!(protected_state(&db, task).await, before);
    let replay = repo
        .preflight_terminal_idle(task, &scheduler, &c)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(serde_json::to_value(&replay.permit).unwrap(), core);
    assert!(replay.observed_at >= first.observed_at);
    let mut changed = c.clone();
    changed.terminal_idle_recovery.barrier_id = Uuid::new_v4();
    assert!(
        repo.preflight_terminal_idle(task, &scheduler, &changed)
            .await
            .is_err()
    );
    let count: i64 = db
        .query_one(statement(
            "SELECT count(*) AS n FROM sdlc_idempotency WHERE task_id=$1 AND idempotency_key=$2",
            vec![task.into(), c.idempotency_key.clone().into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "n")
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(protected_state(&db, task).await, before);
}
