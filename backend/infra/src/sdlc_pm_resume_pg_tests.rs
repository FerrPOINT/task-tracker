//! Synthetic trusted-scheduler observations only; no live Fleet proof is claimed.
use super::*;
use domain::sdlc_execution_lease::*;
use domain::sdlc_lifecycle::SubmitPmFeedback;

fn resume(
    lease: &ExecutionLeaseReceipt,
    input: PmResumeInput,
    checkpoint: Uuid,
    revision: i64,
    requirements: Option<(i64, String)>,
    key: &str,
) -> ResumePmExecutionLease {
    let (requirement_revision, content_hash) = match requirements {
        Some((n, h)) => (Some(n), Some(h)),
        None => (None, None),
    };
    ResumePmExecutionLease {
        expected_owner_version: lease.owner_version,
        lease_id: lease.lease.lease_id,
        expected_lease_version: lease.lease.version,
        input,
        checkpoint: PmResumeCheckpoint {
            checkpoint_id: checkpoint,
            checkpoint_revision: revision,
            checkpoint_sha256: "c".repeat(64),
            fence: lease.fence.clone(),
            requirement_revision,
            content_hash,
        },
        previous_run_id: Uuid::new_v4(),
        previous_run_terminal: artifact("scheduler-terminal-readback"),
        idempotency_key: key.into(),
    }
}

async fn saved_lease(db: &DatabaseConnection, execution: Uuid) -> Json {
    db.query_one(statement(
        "SELECT result FROM sdlc_pm_execution_leases WHERE execution_id=$1",
        vec![execution.into()],
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "result")
    .unwrap()
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_PM_RESUME_TEST_DATABASE_URL ending in _pm_resume_test"]
async fn pm_resume_typed_inputs_expiry_cas_dedup_atomicity_and_history() {
    let url = std::env::var("TT_PM_RESUME_TEST_DATABASE_URL")
        .expect("isolated PM resume database required");
    let db = Database::connect(&url).await.unwrap();
    let row=db.query_one(statement("SELECT current_database() AS name,to_regclass('public.seaql_migrations') IS NULL AS fresh",vec![])).await.unwrap().unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_pm_resume_test")
    );
    assert!(row.try_get::<bool>("", "fresh").unwrap());
    migration::Migrator::up(&db, None).await.unwrap();
    let row = db
        .query_one(statement(
            "SELECT to_regclass('public.sdlc_delivery_heads') IS NULL AS absent",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap();
    if row.try_get::<bool>("", "absent").unwrap() {
        db.execute_unprepared(include_str!(
            "../../migration/src/single_task_lifecycle.sql"
        ))
        .await
        .unwrap();
    }
    let project = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    for (subject, id) in [
        ("owner", owner_id),
        ("pm", Uuid::new_v4()),
        ("scheduler", Uuid::new_v4()),
        ("verifier", Uuid::new_v4()),
        ("foreign", Uuid::new_v4()),
    ] {
        exec(&db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,$2,$3,$3,'!',$3,true)",
            vec![id.into(),format!("{subject}@example.test").into(),subject.into()]).await.unwrap();
    }
    exec(&db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,'PMR','PM resume',$2,$3)",vec![project.into(),owner_id.into(),Uuid::new_v4().into()]).await.unwrap();
    exec(&db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Resume','[]' FROM projects WHERE id=$1",vec![project.into()]).await.unwrap();
    exec(&db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub IN ('pm','scheduler','verifier')",vec![project.into()]).await.unwrap();
    let repo = PostgresSdlcRepository::connect(
        &url,
        SdlcConfig {
            instance_id: "pm-resume-test".into(),
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
    let mut reader = scheduler.clone();
    reader.scopes = ["task-tracker:read".to_owned()].into_iter().collect();
    let routes = routes();
    repo.set_routing_policy(
        project,
        &owner,
        SetRoutingPolicy {
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
                title: "PM resume fixture".into(),
                description: "Not live runtime evidence".into(),
                idempotency_key: "draft".into(),
            },
        )
        .await
        .unwrap();
    let task = draft.task_id;
    let (reservation,_)=repo.reserve_pm_draft(task,&owner,parse(json!({"requested_agent_id":routes.project_manager.agent_id,"expected_owner_version":0,
        "expected_assignment_version":null,"idempotency_key":"reserve"}))).await.unwrap();
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
    let claim = ClaimExecutionLease {
        expected_owner_version: 1,
        fence: fence.clone(),
        idempotency_key: "claim".into(),
    };
    let (initial, _) = repo
        .claim_execution_lease(task, &pm, claim.clone())
        .await
        .unwrap();
    let checkpoint = Uuid::new_v4();
    let (feedback, _) = repo
        .submit_pm_feedback(
            task,
            &owner,
            SubmitPmFeedback {
                message_id: Uuid::new_v4(),
                fence: fence.clone(),
                text: "Real saved brainstorming input".into(),
                idempotency_key: "feedback".into(),
            },
        )
        .await
        .unwrap();
    let command = resume(
        &initial,
        PmResumeInput::Feedback {
            input_id: feedback.message_id,
        },
        checkpoint,
        1,
        None,
        "resume-feedback",
    );
    let before = saved_lease(&db, fence.execution_id).await;
    let foreign = actor("foreign", false, vec![]);
    for denied in [&pm, &owner, &reader, &foreign] {
        assert!(matches!(
            repo.resume_pm_execution_lease(task, denied, command.clone())
                .await,
            Err(AppError::Forbidden)
        ));
    }
    let mut human_scheduler = scheduler.clone();
    human_scheduler.human_session = true;
    assert!(matches!(
        repo.resume_pm_execution_lease(task, &human_scheduler, command.clone())
            .await,
        Err(AppError::Forbidden)
    ));
    let mut wrong = command.clone();
    wrong.input = PmResumeInput::Feedback {
        input_id: Uuid::new_v4(),
    };
    wrong.idempotency_key = "missing-input".into();
    assert!(matches!(
        repo.resume_pm_execution_lease(task, &scheduler, wrong)
            .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(saved_lease(&db, fence.execution_id).await, before);
    assert_eq!(count(&db, "sdlc_pm_resume_inputs").await, 0);
    // Real database time expiry, without disabling guards or editing protected rows.
    db.execute_unprepared("SELECT pg_sleep(31)").await.unwrap();
    assert!(matches!(
        repo.execution_lease(task, &pm, None).await.unwrap().state,
        ExecutionLeaseState::Expired
    ));
    let heartbeat = HeartbeatExecutionLease {
        expected_owner_version: 1,
        fence: fence.clone(),
        lease_id: initial.lease.lease_id,
        expected_lease_version: 1,
        idempotency_key: "expired-heartbeat".into(),
    };
    assert!(matches!(
        repo.heartbeat_execution_lease(task, &pm, heartbeat).await,
        Err(AppError::Conflict(_))
    ));
    let mut reacquire = claim.clone();
    reacquire.idempotency_key = "reacquire".into();
    assert!(matches!(
        repo.claim_execution_lease(task, &pm, reacquire).await,
        Err(AppError::Conflict(_))
    ));
    db.execute_unprepared("CREATE FUNCTION fixture_resume_fail_update() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture lease update failure'; END $$;
        CREATE TRIGGER fixture_resume_fail_update BEFORE UPDATE ON sdlc_pm_execution_leases FOR EACH ROW EXECUTE FUNCTION fixture_resume_fail_update();").await.unwrap();
    assert!(
        repo.resume_pm_execution_lease(task, &scheduler, command.clone())
            .await
            .is_err()
    );
    assert_eq!(count(&db, "sdlc_pm_resume_inputs").await, 0);
    assert_eq!(saved_lease(&db, fence.execution_id).await, before);
    db.execute_unprepared("DROP TRIGGER fixture_resume_fail_update ON sdlc_pm_execution_leases; DROP FUNCTION fixture_resume_fail_update();").await.unwrap();
    let outbox = count(&db, "sdlc_outbox").await;
    let questions = count(&db, "sdlc_question_versions").await;
    let answers = count(&db, "sdlc_answers").await;
    let (feedback_lease, replayed) = repo
        .resume_pm_execution_lease(task, &scheduler, command.clone())
        .await
        .unwrap();
    assert!(!replayed);
    assert_eq!(feedback_lease.lease.version, 2);
    assert_eq!(feedback_lease.lease.lease_id, initial.lease.lease_id);
    assert_eq!(feedback_lease.lease.claimed_at, initial.lease.claimed_at);
    assert_eq!(
        feedback_lease.lease.holder_subject,
        initial.lease.holder_subject
    );
    assert_eq!(feedback_lease.fence, initial.fence);
    assert_eq!(count(&db, "sdlc_outbox").await, outbox);
    assert_eq!(count(&db, "sdlc_question_versions").await, questions);
    assert_eq!(count(&db, "sdlc_answers").await, answers);
    assert!(matches!(
        repo.execution_lease(task, &pm, None).await.unwrap().state,
        ExecutionLeaseState::Active
    ));
    let old_claim = repo
        .execution_lease(task, &pm, Some("claim"))
        .await
        .unwrap()
        .operation
        .unwrap()
        .result;
    assert_eq!(
        serde_json::to_value(&old_claim).unwrap(),
        serde_json::to_value(&initial).unwrap()
    );
    assert!(
        repo.resume_pm_execution_lease(task, &scheduler, command.clone())
            .await
            .unwrap()
            .1
    );
    let original = repo
        .pm_resume_operation(task, &reader, "resume-feedback")
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&original.result).unwrap(),
        serde_json::to_value(&feedback_lease).unwrap()
    );
    assert!(matches!(
        repo.pm_resume_operation(task, &pm, "resume-feedback").await,
        Err(AppError::Forbidden)
    ));
    assert!(matches!(
        repo.pm_resume_operation(task, &scheduler, "resume-feedback")
            .await,
        Err(AppError::Forbidden)
    ));
    let mut changed = command.clone();
    changed.checkpoint.checkpoint_sha256 = "d".repeat(64);
    assert!(matches!(
        repo.resume_pm_execution_lease(task, &scheduler, changed)
            .await,
        Err(AppError::Conflict(_))
    ));
    let mut duplicate = command.clone();
    duplicate.idempotency_key = "duplicate-input".into();
    duplicate.expected_lease_version = 2;
    duplicate.previous_run_id = Uuid::new_v4();
    duplicate.checkpoint.checkpoint_revision = 2;
    assert!(matches!(
        repo.resume_pm_execution_lease(task, &scheduler, duplicate)
            .await,
        Err(AppError::Conflict(_))
    ));
    let (second_feedback, _) = repo
        .submit_pm_feedback(
            task,
            &owner,
            SubmitPmFeedback {
                message_id: Uuid::new_v4(),
                fence: fence.clone(),
                text: "Second saved input".into(),
                idempotency_key: "feedback-2".into(),
            },
        )
        .await
        .unwrap();
    let mut stale_checkpoint = resume(
        &feedback_lease,
        PmResumeInput::Feedback {
            input_id: second_feedback.message_id,
        },
        checkpoint,
        1,
        None,
        "stale-checkpoint",
    );
    assert!(matches!(
        repo.resume_pm_execution_lease(task, &scheduler, stale_checkpoint.clone())
            .await,
        Err(AppError::Conflict(_))
    ));
    stale_checkpoint.checkpoint.checkpoint_revision = 2;
    stale_checkpoint.previous_run_id = command.previous_run_id;
    assert!(matches!(
        repo.resume_pm_execution_lease(task, &scheduler, stale_checkpoint)
            .await,
        Err(AppError::Conflict(_))
    ));
    exec(&db,"DELETE FROM project_members WHERE project_id=$1 AND user_id=(SELECT id FROM users WHERE central_sub='scheduler')",vec![project.into()]).await.unwrap();
    assert!(matches!(
        repo.resume_pm_execution_lease(task, &scheduler, command.clone())
            .await,
        Err(AppError::Forbidden)
    ));
    assert!(matches!(
        repo.pm_resume_operation(task, &reader, "resume-feedback")
            .await,
        Err(AppError::Forbidden)
    ));
    exec(&db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub='scheduler'",vec![project.into()]).await.unwrap();
    let eligible = resume(
        &feedback_lease,
        PmResumeInput::Feedback {
            input_id: second_feedback.message_id,
        },
        checkpoint,
        2,
        None,
        "revoked-holder",
    );
    exec(
        &db,
        "UPDATE users SET is_active=false WHERE central_sub='pm'",
        vec![],
    )
    .await
    .unwrap();
    assert!(matches!(
        repo.resume_pm_execution_lease(task, &scheduler, eligible)
            .await,
        Err(AppError::Forbidden)
    ));
    exec(
        &db,
        "UPDATE users SET is_active=true WHERE central_sub='pm'",
        vec![],
    )
    .await
    .unwrap();
    let (heartbeat_lease, _) = repo
        .heartbeat_execution_lease(
            task,
            &pm,
            HeartbeatExecutionLease {
                expected_owner_version: 1,
                fence: fence.clone(),
                lease_id: initial.lease.lease_id,
                expected_lease_version: 2,
                idempotency_key: "heartbeat-after-resume".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(heartbeat_lease.lease.version, 3);
    let mut document: RequirementsDocument = parse(
        json!({"goal":"PM resume","scope":["Scope"],"exclusions":[],"scenarios":["Scenario"],"acceptance_criteria":["Accept"],
        "constraints":[],"dependencies":[],"assumptions":[],"checklist":["check"],"prerequisites":["prerequisite"]}),
    );
    let hash = app::sdlc::canonical_hash(&document).unwrap();
    repo.execute_verified(
        task,
        &pm,
        SdlcCommand::PublishRevision(PublishRevision {
            fence: fence.clone(),
            expected_requirement_revision: None,
            document: document.clone(),
            idempotency_key: "publish-1".into(),
        }),
        reservation.clone(),
        None,
    )
    .await
    .unwrap();
    let question = PublishQuestion {
        fence: fence.clone(),
        request_id: Uuid::new_v4(),
        question_id: Uuid::new_v4(),
        expected_question_version: None,
        requirement_revision: 1,
        checkpoint_id: checkpoint,
        requirement_reference: Some("scope".into()),
        text: "Describe scope".into(),
        rationale: "Owner input".into(),
        required: true,
        mode: QuestionMode::Text,
        options: vec![],
        recommended_option_id: None,
        idempotency_key: "question".into(),
    };
    repo.execute_verified(
        task,
        &pm,
        SdlcCommand::PublishQuestion(question.clone()),
        reservation.clone(),
        None,
    )
    .await
    .unwrap();
    let answer: Answer = parse(
        repo.execute_verified(
            task,
            &owner,
            SdlcCommand::Answer {
                question_id: question.question_id,
                command: AnswerCommand {
                    expected_question_version: 1,
                    requirement_revision: 1,
                    selected_option_ids: vec![],
                    text: Some("Exact owner answer".into()),
                    comment: None,
                    idempotency_key: "answer".into(),
                },
            },
            reservation.clone(),
            None,
        )
        .await
        .unwrap(),
    );
    let answer_command = resume(
        &heartbeat_lease,
        PmResumeInput::Answer {
            input_id: answer.id,
        },
        checkpoint,
        2,
        Some((1, hash.clone())),
        "resume-answer",
    );
    // A heartbeat winning CAS creates no consumption; readback supplies the retry version.
    let mut raced = answer_command.clone();
    raced.expected_lease_version = 2;
    assert!(matches!(
        repo.resume_pm_execution_lease(task, &scheduler, raced.clone())
            .await,
        Err(AppError::Conflict(_))
    ));
    let readback = repo.execution_lease(task, &pm, None).await.unwrap();
    raced.expected_lease_version = readback.current.unwrap().version;
    assert_eq!(
        app::sdlc_pm_resume::request_hash(&raced).unwrap(),
        app::sdlc_pm_resume::request_hash(&answer_command).unwrap()
    );
    assert_eq!(count(&db, "sdlc_pm_resume_inputs").await, 1);
    let mut wrong_kind = answer_command.clone();
    wrong_kind.input = PmResumeInput::Feedback {
        input_id: answer.id,
    };
    wrong_kind.idempotency_key = "wrong-kind".into();
    assert!(matches!(
        repo.resume_pm_execution_lease(task, &scheduler, wrong_kind)
            .await,
        Err(AppError::Conflict(_))
    ));
    let mut wrong_checkpoint = answer_command.clone();
    wrong_checkpoint.checkpoint.checkpoint_id = Uuid::new_v4();
    wrong_checkpoint.idempotency_key = "wrong-question-checkpoint".into();
    assert!(matches!(
        repo.resume_pm_execution_lease(task, &scheduler, wrong_checkpoint)
            .await,
        Err(AppError::Conflict(_))
    ));
    let outbox = count(&db, "sdlc_outbox").await;
    let (left, right) = tokio::join!(
        repo.resume_pm_execution_lease(task, &scheduler, answer_command.clone()),
        repo.resume_pm_execution_lease(task, &scheduler, answer_command.clone())
    );
    let left = left.unwrap();
    let right = right.unwrap();
    assert_ne!(left.1, right.1);
    assert_eq!(
        serde_json::to_value(&left.0).unwrap(),
        serde_json::to_value(&right.0).unwrap()
    );
    let answer_lease = left.0;
    assert_eq!(answer_lease.lease.version, 4);
    assert_eq!(count(&db, "sdlc_pm_resume_inputs").await, 2);
    assert_eq!(count(&db, "sdlc_answers").await, 1);
    assert_eq!(count(&db, "sdlc_outbox").await, outbox);
    document.goal = "Updated after actual owner answer".into();
    let hash2 = app::sdlc::canonical_hash(&document).unwrap();
    repo.execute_verified(
        task,
        &pm,
        SdlcCommand::PublishRevision(PublishRevision {
            fence: fence.clone(),
            expected_requirement_revision: Some(1),
            document: document.clone(),
            idempotency_key: "publish-2".into(),
        }),
        reservation.clone(),
        None,
    )
    .await
    .unwrap();
    let mut stale_answer = answer_command;
    stale_answer.idempotency_key = "stale-answer-revision".into();
    stale_answer.expected_lease_version = 4;
    assert!(matches!(
        repo.resume_pm_execution_lease(task, &scheduler, stale_answer)
            .await,
        Err(AppError::Conflict(_))
    ));
    let verifier = actor(
        "verifier",
        false,
        vec![format!(
            "task-tracker:sdlc:evidence:{task}:{}",
            fence.assignment_version
        )],
    );
    for check in ["check", "prerequisite"] {
        let evidence = EvidenceCommand {
            fence: fence.clone(),
            requirement_revision: 2,
            content_hash: hash2.clone(),
            check_id: check.into(),
            evidence_reference: format!("fixture://{check}"),
            idempotency_key: check.into(),
        };
        let wrong_revision_scope = actor(
            "verifier",
            false,
            vec![format!("task-tracker:sdlc:evidence:{task}:2")],
        );
        assert!(matches!(
            repo.execute_verified(
                task,
                &wrong_revision_scope,
                SdlcCommand::Evidence(evidence.clone()),
                reservation.clone(),
                None,
            )
            .await,
            Err(AppError::Forbidden)
        ));
        repo.execute_verified(
            task,
            &verifier,
            SdlcCommand::Evidence(evidence),
            reservation.clone(),
            None,
        )
        .await
        .unwrap();
    }
    let confirmation: Confirmation = parse(
        repo.execute_verified(
            task,
            &owner,
            SdlcCommand::Confirm {
                revision: 2,
                command: ConfirmCommand {
                    content_hash: hash2.clone(),
                    idempotency_key: "confirm".into(),
                    expected_routing_policy_version: Some(1),
                },
            },
            reservation.clone(),
            None,
        )
        .await
        .unwrap(),
    );
    let closing = resume(
        &answer_lease,
        PmResumeInput::Confirmation {
            input_id: confirmation.id,
        },
        checkpoint,
        3,
        Some((2, hash2.clone())),
        "resume-confirmation",
    );
    let outbox = count(&db, "sdlc_outbox").await;
    let (closing_lease, _) = repo
        .resume_pm_execution_lease(task, &scheduler, closing.clone())
        .await
        .unwrap();
    assert_eq!(closing_lease.lease.version, 5);
    assert_eq!(count(&db, "sdlc_outbox").await, outbox);
    let state = repo.read(task, &owner).await.unwrap();
    assert_eq!(state.stage, Stage::Analysis);
    assert_eq!(state.status, Some(LifecycleStatus::Ready));
    assert!(matches!(
        repo.execute_verified(
            task,
            &pm,
            SdlcCommand::PublishRevision(PublishRevision {
                fence: fence.clone(),
                expected_requirement_revision: Some(2),
                document,
                idempotency_key: "frozen-pm".into()
            }),
            reservation,
            None
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    repo.complete_pm_draft(
        task,
        &scheduler,
        CompletePmDraft {
            contract_version: 1,
            fence: fence.clone(),
            lease_id: initial.lease.lease_id,
            lease_version: 5,
            requirement_revision: 2,
            content_hash: hash2,
            terminal_receipt_ref: "fixture://closing-terminal".into(),
            terminal_receipt_sha256: "a".repeat(64),
            workflow_receipt_ref: "fixture://closing-workflow".into(),
            workflow_receipt_sha256: "b".repeat(64),
            idempotency_key: "complete".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        repo.pm_resume_operation(task, &reader, "resume-feedback")
            .await
            .unwrap()
            .result
            .lease
            .version,
        2
    );
    assert!(
        repo.resume_pm_execution_lease(task, &scheduler, command)
            .await
            .unwrap()
            .1
    );
    let mut after_complete = closing;
    after_complete.idempotency_key = "cannot-reopen-completed-pm".into();
    after_complete.expected_lease_version = 5;
    after_complete.previous_run_id = Uuid::new_v4();
    assert!(matches!(
        repo.resume_pm_execution_lease(task, &scheduler, after_complete)
            .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(count(&db, "sdlc_pm_resume_inputs").await, 3);
    assert_eq!(count(&db, "sdlc_pm_lease_operations").await, 2);
    assert_eq!(initial.lease.lease_id, closing_lease.lease.lease_id);
    assert_eq!(initial.lease.claimed_at, closing_lease.lease.claimed_at);
    assert!(
        db.execute_unprepared("DELETE FROM sdlc_pm_resume_inputs")
            .await
            .is_err()
    );
}
