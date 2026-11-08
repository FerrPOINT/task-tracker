//! Real owner-ledger regression. Workflow observations are synthetic, never live acceptance.
use super::*;
use domain::sdlc_stage_status::{StageStartFence, StartStage};

#[tokio::test]
#[ignore = "fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn pm_explicit_start_survives_dialogue_and_hands_completed_to_analysis_ready() {
    let migrations = migration::Migrator::migrations();
    let pre115 = migrations
        .iter()
        .position(|m| m.name() == "m20261107_0000115_agent_stage_status")
        .expect("agent stage status migration registered");
    let f = Fixture::new_with_migration_steps(Some(u32::try_from(pre115).unwrap())).await;
    let row = f.db.query_one(statement(
        "SELECT pg_get_functiondef('sdlc_critical_pm_lifecycle_insert_gate()'::regprocedure) AS definition,
            current_setting('standard_conforming_strings') AS standard_conforming_strings,
            (SELECT jsonb_agg(version ORDER BY version) FROM seaql_migrations) AS migrations",
        vec![],
    )).await.unwrap().unwrap();
    println!(
        "\nMIGRATION115_PRE_FUNCTION {}",
        json!({
            "function":"sdlc_critical_pm_lifecycle_insert_gate()",
            "definition":row.try_get::<String>("", "definition").unwrap(),
            "standard_conforming_strings":row.try_get::<String>("", "standard_conforming_strings").unwrap(),
            "migrations":row.try_get::<Json>("", "migrations").unwrap(),
        })
    );
    // Exercise the captured CRLF function body even from an LF-only checkout.
    let definition = row.try_get::<String>("", "definition").unwrap();
    f.db.execute_unprepared(&definition.replace("\r\n", "\n").replace('\n', "\r\n"))
        .await
        .unwrap();
    let before = f.db.query_one(statement(
        "SELECT pg_get_functiondef('sdlc_critical_pm_lifecycle_insert_gate()'::regprocedure) AS definition",
        vec![],
    )).await.unwrap().unwrap().try_get::<String>("", "definition").unwrap();
    assert!(before.contains("\r\n"));
    migration::Migrator::up(&f.db, None).await.unwrap();
    let after = f.db.query_one(statement(
        "SELECT pg_get_functiondef('sdlc_critical_pm_lifecycle_insert_gate()'::regprocedure) AS definition",
        vec![],
    )).await.unwrap().unwrap().try_get::<String>("", "definition").unwrap();
    assert!(!after.contains("\r\n"));
    let task = f
        .draft("agent-owned-status", Some(DraftFlow::SingleTaskV1))
        .await;
    let (reservation, pm) = super::super::configured_test_support::assigned_pm(
        &f.repo,
        task,
        &f.owner,
        f.roles.project_manager.agent_id,
    )
    .await;
    let assignment = &reservation.assignment;
    let fence = MachineFence {
        assignment_id: assignment.assignment_id,
        execution_id: assignment.execution_id,
        agent_id: assignment.agent_id,
        assignment_version: assignment.version,
    };
    let document: RequirementsDocument = parse(json!({
        "goal":"Exact owner business brief","scope":["scope"],"exclusions":[],
        "scenarios":["scenario"],"acceptance_criteria":["acceptance"],"constraints":[],
        "dependencies":[],"assumptions":[],"checklist":["check"],"prerequisites":["prerequisite"]
    }));
    let revision = PublishRevision {
        fence: fence.clone(),
        expected_requirement_revision: None,
        document: document.clone(),
        idempotency_key: "publish-one".into(),
    };
    assert!(
        f.repo
            .execute(task, &pm, SdlcCommand::PublishRevision(revision.clone()))
            .await
            .is_err()
    );
    let start = StartStage {
        fence: StageStartFence::Pm(fence.clone()),
        idempotency_key: "start-pm".into(),
    };
    let before = f.snapshot().await;
    assert!(matches!(
        f.repo.start_stage(task, &f.owner, start.clone()).await,
        Err(AppError::Forbidden)
    ));
    let mut stale = start.clone();
    if let StageStartFence::Pm(f) = &mut stale.fence {
        f.assignment_version += 1;
    }
    assert!(f.repo.start_stage(task, &pm, stale).await.is_err());
    assert_eq!(f.snapshot().await, before);
    let (saved, replay) = f.repo.start_stage(task, &pm, start.clone()).await.unwrap();
    assert!(!replay);
    assert_eq!(saved.stage, Stage::Draft);
    assert_eq!(saved.status, LifecycleStatus::InProgress);
    let snapshot = f.snapshot().await;
    let (same, replay) = f.repo.start_stage(task, &pm, start.clone()).await.unwrap();
    assert!(replay);
    assert_eq!(same, saved);
    assert_eq!(f.snapshot().await, snapshot);
    let mut changed = start.clone();
    if let StageStartFence::Pm(f) = &mut changed.fence {
        f.agent_id = Uuid::new_v4();
    }
    assert!(f.repo.start_stage(task, &pm, changed).await.is_err());
    f.repo
        .execute(task, &pm, SdlcCommand::PublishRevision(revision))
        .await
        .unwrap();
    let question: PublishQuestion = parse(json!({
        "fence":fence,"request_id":Uuid::new_v4(),"question_id":Uuid::new_v4(),
        "expected_question_version":null,"requirement_revision":1,"requirement_reference":null,
        "text":"Business choice?","rationale":"Required owner decision","required":true,"mode":"text",
        "options":[],"recommended_option_id":null,"idempotency_key":"ask-owner"
    }));
    f.repo
        .execute(task, &pm, SdlcCommand::PublishQuestion(question.clone()))
        .await
        .unwrap();
    assert_eq!(
        f.repo.read(task, &pm).await.unwrap().status,
        Some(LifecycleStatus::InProgress)
    );
    f.repo
        .execute(
            task,
            &f.owner,
            SdlcCommand::Answer {
                question_id: question.question_id,
                command: AnswerCommand {
                    expected_question_version: 1,
                    requirement_revision: 1,
                    selected_option_ids: vec![],
                    text: Some("Owner decision".into()),
                    comment: None,
                    idempotency_key: "answer".into(),
                },
            },
        )
        .await
        .unwrap();
    let mut revised = document;
    revised.constraints.push("Owner decision".into());
    f.repo
        .execute(
            task,
            &pm,
            SdlcCommand::PublishRevision(PublishRevision {
                fence: fence.clone(),
                expected_requirement_revision: Some(1),
                document: revised.clone(),
                idempotency_key: "publish-two".into(),
            }),
        )
        .await
        .unwrap();
    let consent = f
        .repo
        .execute(
            task,
            &f.owner,
            SdlcCommand::Confirm {
                revision: 2,
                command: ConfirmCommand {
                    content_hash: app::sdlc::canonical_hash(&revised).unwrap(),
                    idempotency_key: "owner-confirm".into(),
                    expected_routing_policy_version: None,
                },
            },
        )
        .await
        .unwrap();
    let confirmed = f.repo.read(task, &pm).await.unwrap();
    assert_eq!(confirmed.status, Some(LifecycleStatus::InProgress));
    assert_eq!(
        confirmed.draft_submode,
        Some(DraftSubmode::AwaitingConfirmation)
    );
    assert_eq!(consent["stage"], "Draft");
    let completed = super::super::configured_test_support::complete_pm(
        &f.repo,
        task,
        &f.owner,
        &pm,
        &reservation,
    )
    .await;
    assert_eq!(completed.stage, Stage::Draft);
    assert_eq!(completed.status, LifecycleStatus::Completed);
    let after = f.repo.read(task, &f.owner).await.unwrap();
    assert_eq!(after.stage, Stage::Analysis);
    assert_eq!(after.status, Some(LifecycleStatus::Ready));
    assert!(after.draft_submode.is_none());
    let cleared =
        f.db.query_one(statement(
            "SELECT NOT (state ? 'draft_submode') AS cleared FROM sdlc_tasks WHERE task_id=$1",
            vec![task.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(cleared.try_get::<bool>("", "cleared").unwrap());
    assert_eq!(json!(after.revisions), json!(confirmed.revisions));
    assert_eq!(after.confirmations, confirmed.confirmations);
    assert_eq!(json!(after.questions), json!(confirmed.questions));
    // The aggregate retains PM history; current execution authority must not.
    assert_eq!(after.assignment, confirmed.assignment);
    let current = f.repo.context(task, &f.owner).await.unwrap();
    assert_eq!(current.stage, Stage::Analysis);
    assert_eq!(current.status, Some(LifecycleStatus::Ready));
    assert!(current.assignment.is_none());
    assert!(current.expected_assignment_version.is_none());
    let intent = f.repo.analysis_intent(task, &f.owner).await.unwrap();
    assert_eq!(intent.confirmation_id, completed.confirmation_id);
    assert_eq!(intent.requirement_revision, completed.requirement_revision);
    assert_eq!(intent.content_hash, completed.content_hash);
    let start_operation = f.db.query_one(statement(
        "SELECT o.actor_subject,o.idempotency_key,o.transaction_id=c.transaction_id AS same_transaction
         FROM sdlc_delivery_operations o JOIN sdlc_configured_pm_completions c
            ON c.task_id=o.task_id AND c.completion_id=(o.command->>'pm_completion_id')::uuid
         WHERE o.task_id=$1 AND c.completion_id=$2 AND o.command->>'operation'='start_analysis'",
        vec![task.into(),completed.completion_id.into()],
    )).await.unwrap().unwrap();
    assert_eq!(
        start_operation
            .try_get::<String>("", "actor_subject")
            .unwrap(),
        pm.subject
    );
    assert_eq!(
        start_operation
            .try_get::<String>("", "idempotency_key")
            .unwrap(),
        format!("pm-analysis:{}", completed.completion_id)
    );
    assert!(
        start_operation
            .try_get::<bool>("", "same_transaction")
            .unwrap()
    );
    assert_eq!(
        f.repo
            .stage_start_operation(task, &pm, &start.idempotency_key)
            .await
            .unwrap(),
        saved
    );
    assert_eq!(
        f.repo.start_stage(task, &pm, start).await.unwrap(),
        (saved, true)
    );
    let history: Json =
        f.db.query_one(statement(
            "SELECT jsonb_agg(jsonb_build_object('id',h.id,'from_status_id',h.from_status_id,
                'to_status_id',h.to_status_id,'created_at',h.created_at,
                'aliases',(SELECT jsonb_agg(jsonb_build_object('stage',m.stage,'status',m.status)
                    ORDER BY m.stage,m.status) FROM sdlc_issue_status_mapping m WHERE m.status_id=h.to_status_id))
                ORDER BY h.created_at,h.id) AS history FROM issue_status_history h WHERE h.issue_id=$1",
            vec![task.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "history")
        .unwrap();
    println!(
        "\nSTAGE_STATUS_RAW_HISTORY {}",
        json!({"task_id":task,"events":history})
    );
    let mapping = f.db.query_one(statement(
        "SELECT
            (SELECT status_id FROM sdlc_issue_status_mapping WHERE stage='Draft' AND status='InProgress') AS started,
            (SELECT status_id FROM sdlc_issue_status_mapping WHERE stage='Draft' AND status='Completed') AS completed,
            (SELECT status_id FROM sdlc_issue_status_mapping WHERE stage='Analysis' AND status='Ready') AS ready",
        vec![],
    )).await.unwrap().unwrap();
    let started: Uuid = mapping.try_get("", "started").unwrap();
    let completed_status: Uuid = mapping.try_get("", "completed").unwrap();
    let ready: Uuid = mapping.try_get("", "ready").unwrap();
    let history = history.as_array().unwrap();
    let completed_at = history
        .iter()
        .position(|r| r["to_status_id"] == json!(completed_status))
        .unwrap();
    assert_eq!(
        history
            .iter()
            .filter(|r| r["to_status_id"] == json!(completed_status))
            .count(),
        1
    );
    assert_eq!(history.len(), completed_at + 2);
    assert_eq!(history[completed_at]["from_status_id"], json!(started));
    assert_ne!(history[completed_at]["id"], history[completed_at + 1]["id"]);
    assert_eq!(
        history[completed_at + 1]["from_status_id"],
        json!(completed_status)
    );
    assert_eq!(history[completed_at + 1]["to_status_id"], json!(ready));
    let tx = f.db.begin().await.unwrap();
    let queued = super::super::lifecycle::current(&tx, task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(queued.task.runtime, RuntimeState::Queued);
    assert!(queued.assignment.is_none() && queued.lease.is_none() && queued.binding.is_none());
    tx.rollback().await.unwrap();
}
