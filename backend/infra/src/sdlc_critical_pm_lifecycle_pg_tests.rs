//! Child of the existing critical candidates fixture (hook supplied separately).
//! Native terminal, Workflow receipts and business evidence are SYNTHETIC, not live proofs.
use super::*;
use domain::sdlc_critical_pm_reservation::{CriticalPmReservationResult, ReserveCriticalPmReview};

struct ReviewFixture {
    f: Fixture,
    task: Uuid,
    review: CriticalPmReservationResult,
    pm: Principal,
    fence: MachineFence,
    original: Json,
}

async fn frozen_history(f: &Fixture, task: Uuid) -> Json {
    f.db.query_one(statement("SELECT jsonb_build_object(
        'initial',(SELECT payload FROM sdlc_task_routing_snapshots WHERE task_id=$1),
        'initial_assignment',(SELECT payload FROM sdlc_assignments WHERE task_id=$1 ORDER BY version LIMIT 1),
        'initial_pm',(SELECT result FROM sdlc_pm_executions WHERE task_id=$1 ORDER BY assignment_version LIMIT 1),
        'initial_completion',(SELECT result FROM sdlc_configured_pm_completions WHERE task_id=$1 ORDER BY created_at LIMIT 1),
        'held',(SELECT jsonb_agg(payload ORDER BY blocker_id) FROM sdlc_critical_blockers WHERE task_id=$1),
        'return',(SELECT jsonb_agg(after_blocker ORDER BY return_id) FROM sdlc_critical_pm_returns WHERE task_id=$1),
        'review',(SELECT jsonb_agg(result ORDER BY review_id) FROM sdlc_critical_pm_reviews WHERE task_id=$1)) AS value",
        vec![task.into()])).await.unwrap().unwrap().try_get("","value").unwrap()
}

async fn setup() -> ReviewFixture {
    let f = Box::pin(Fixture::new()).await;
    assert!(
        f.db.query_one(statement(
            "SELECT to_regclass('sdlc_critical_pm_lifecycle_operations') IS NOT NULL AS installed",
            vec![]
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<bool>("", "installed")
        .unwrap(),
        "main must include the companion in its unreleased096"
    );
    exec(&f.db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active)
        VALUES($1,'critical-agent@example.test','77777777-7777-4777-8777-777777777777','77777777-7777-4777-8777-777777777777','!','77777777-7777-4777-8777-777777777777',true)",vec![Uuid::new_v4().into()]).await.unwrap();
    exec(
        &f.db,
        "INSERT INTO project_members(project_id,user_id,role)
        SELECT $1,id,'developer' FROM users WHERE central_sub='77777777-7777-4777-8777-777777777777'",
        vec![f.project.into()],
    )
    .await
    .unwrap();
    let storage = domain::InMemoryStorage::default();
    let r = Box::pin(start(&f, "critical-review-lifecycle", false, &storage)).await;
    let (held, _) = Box::pin(f.repo.submit_critical_blocker_with_storage(
        r.task,
        &r.producer,
        blocker(&r),
        &storage,
    ))
    .await
    .unwrap();
    let head = f
        .repo
        .lifecycle_read(r.task, &f.reader)
        .await
        .unwrap()
        .receipt;
    let (returned, _) = Box::pin(f.repo.return_critical_to_pm_with_terminal(
        r.task,
        &f.writer,
        ReturnCriticalToPm {
            review: CriticalPmReviewGuard {
                blocker_id: held.blocker_id,
                blocker_sha256: held.blocker_sha256.clone(),
                expected_blocker_version: held.version,
                expected_task_version: head.task.version,
                fence: fence(&head),
                idempotency_key: "lifecycle-return".into(),
            },
            terminal: RuntimeTerminalObservation {
                assignment_hash: held.assignment_hash,
                chat_id: held.chat_id,
                run_id: held.run_id,
                state: RuntimeTerminalState::Cancelled,
                receipt: ArtifactReceipt {
                    reference: "fixture://synthetic-terminal".into(),
                    sha256: "a".repeat(64),
                },
            },
        },
    ))
    .await
    .unwrap();
    let previous = f
        .repo
        .pm_draft_assignment(r.task, &f.owner, None)
        .await
        .unwrap()
        .current
        .unwrap();
    let (review, _) = Box::pin(f.repo.reserve_critical_pm_review(
        r.task,
        &f.writer,
        ReserveCriticalPmReview {
            blocker_id: held.blocker_id,
            blocker_sha256: held.blocker_sha256,
            expected_blocker_version: returned.blocker.version,
            expected_task_version: returned.lifecycle.task.version,
            expected_pm_owner_version: previous.owner_cas.version,
            expected_pm_assignment_version: previous.assignment.version,
            idempotency_key: "lifecycle-review".into(),
        },
    ))
    .await
    .unwrap();
    let a = &review.reservation.assignment;
    let pm = actor(
        &a.machine_subject,
        false,
        &["task-tracker:read", "task-tracker:write"],
    );
    let fence = MachineFence {
        assignment_id: a.assignment_id,
        execution_id: a.execution_id,
        agent_id: a.agent_id,
        assignment_version: a.version,
    };
    let original = frozen_history(&f, r.task).await;
    ReviewFixture {
        f,
        task: r.task,
        review,
        pm,
        fence,
        original,
    }
}

async fn command(r: &ReviewFixture, actor: &Principal, command: SdlcCommand) -> Json {
    Box::pin(r.f.repo.execute(r.task, actor, command))
        .await
        .unwrap()
}

async fn questions_and_revision(r: &mut ReviewFixture) -> (String, Confirmation) {
    let q: Question = parse(
        command(
            r,
            &r.pm,
            SdlcCommand::PublishQuestion(PublishQuestion {
                fence: r.fence.clone(),
                request_id: Uuid::new_v4(),
                question_id: Uuid::new_v4(),
                expected_question_version: None,
                requirement_revision: 1,

                requirement_reference: None,
                text: "Synthetic business decision?".into(),
                rationale: "Synthetic critical review".into(),
                required: true,
                mode: QuestionMode::Text,
                options: vec![],
                recommended_option_id: None,
                idempotency_key: "review-question".into(),
            }),
        )
        .await,
    );
    let answer: Answer = parse(
        command(
            r,
            &r.f.owner,
            SdlcCommand::Answer {
                question_id: q.id,
                command: AnswerCommand {
                    expected_question_version: q.version,
                    requirement_revision: 1,
                    selected_option_ids: vec![],
                    text: Some("Synthetic owner decision".into()),
                    comment: None,
                    idempotency_key: "review-answer".into(),
                },
            },
        )
        .await,
    );
    assert!(q.checkpoint_id.is_none());
    assert!(
        serde_json::to_value(&answer)
            .unwrap()
            .get("checkpoint_id")
            .is_none()
    );
    let tx = r.f.db.begin().await.unwrap();
    let state = r.f.repo.load(&tx, r.task, &r.f.owner).await.unwrap();
    let mut document = state.revisions.last().unwrap().document.clone();
    document.goal = "Synthetic revised owner decision".into();
    tx.rollback().await.unwrap();
    let revision: RequirementsRevision = parse(
        command(
            r,
            &r.pm,
            SdlcCommand::PublishRevision(PublishRevision {
                fence: r.fence.clone(),
                expected_requirement_revision: Some(1),
                document: document.clone(),
                idempotency_key: "review-revision".into(),
            }),
        )
        .await,
    );
    assert_eq!(revision.revision, 2);
    // Legacy exact/latest still rejects the old baseline; ONLY saved current review admits the read.
    let tx = r.f.db.begin().await.unwrap();
    let state = r.f.repo.load(&tx, r.task, &r.f.owner).await.unwrap();
    let receipt = crate::sdlc::lifecycle::current(&tx, r.task)
        .await
        .unwrap()
        .unwrap();
    assert!(crate::sdlc::lifecycle::exact_revision(&state, &receipt).is_err());
    assert_eq!(
        json!(
            r.f.repo
                .review_snapshot(&tx, &state, &receipt)
                .await
                .unwrap()
                .unwrap()
        ),
        json!(receipt.task.routing)
    );
    let mut stale = state.clone();
    stale.assignment.as_mut().unwrap().execution_id = Uuid::new_v4();
    assert!(matches!(
        r.f.repo.review_snapshot(&tx, &stale, &receipt).await,
        Err(AppError::Conflict(_))
    ));
    tx.rollback().await.unwrap();
    let confirm = SdlcCommand::Confirm {
        revision: 2,
        command: ConfirmCommand {
            content_hash: revision.content_hash.clone(),
            idempotency_key: "review-confirm".into(),
            expected_routing_policy_version: Some(1),
        },
    };
    let confirmation: Confirmation = parse(command(r, &r.f.owner, confirm.clone()).await);
    assert_eq!(command(r, &r.f.owner, confirm).await, json!(confirmation));
    let confirmed = r.f.db.query_one(statement(
        "SELECT c.payload,
            (SELECT count(*) FROM sdlc_confirmations WHERE task_id=$1 AND revision=2) AS confirmations,
            (SELECT count(*) FROM sdlc_critical_pm_lifecycle_operations
                WHERE task_id=$1 AND execution_id=$2 AND kind='command' AND idempotency_key='review-confirm') AS journals
         FROM sdlc_confirmations c WHERE c.task_id=$1 AND c.revision=2",
        vec![r.task.into(), r.fence.execution_id.into()],
    )).await.unwrap().unwrap();
    assert_eq!(
        confirmed.try_get::<Json>("", "payload").unwrap(),
        json!(confirmation)
    );
    assert_eq!(confirmed.try_get::<i64>("", "confirmations").unwrap(), 1);
    assert_eq!(confirmed.try_get::<i64>("", "journals").unwrap(), 1);
    let row=r.f.db.query_one(statement("SELECT state->>'stage' AS stage,state->>'waiting_reason' AS reason,
        NOT EXISTS(SELECT 1 FROM sdlc_analysis_intents WHERE task_id=$1 AND requirement_revision=2) AS no_intent,
        NOT EXISTS(SELECT 1 FROM sdlc_configured_pm_completions WHERE execution_id=$2) AS no_completion FROM sdlc_tasks WHERE task_id=$1",
        vec![r.task.into(),r.fence.execution_id.into()])).await.unwrap().unwrap();
    assert_eq!(row.try_get::<String>("", "stage").unwrap(), "Draft");
    assert_eq!(
        row.try_get::<String>("", "reason").unwrap(),
        "critical_pm_awaiting_finalization"
    );
    assert!(row.try_get::<bool>("", "no_intent").unwrap());
    assert!(row.try_get::<bool>("", "no_completion").unwrap());
    (revision.content_hash, confirmation)
}

async fn completion(r: &ReviewFixture, hash: String, confirmation: Confirmation) {
    let saved = crate::sdlc::configured_test_support::complete_pm(
        &r.f.repo,
        r.task,
        &r.f.owner,
        &r.pm,
        &r.review.reservation,
    )
    .await;
    assert_eq!(saved.requirement_revision, 2);
    assert_eq!(saved.content_hash, hash);
    assert_eq!(saved.confirmation_id, confirmation.id);
    let state = r.f.repo.read(r.task, &r.f.owner).await.unwrap();
    assert_eq!(state.stage, Stage::Backlog);
    assert_eq!(state.status, Some(LifecycleStatus::Ready));
    let row = r.f.db.query_one(statement(
        "SELECT g.generation,g.previous_snapshot_id,g.confirmation_id,g.configured_completion_id,
            s.snapshot_id AS initial,s.payload->'policy'=g.payload->'policy' AS frozen,
            (SELECT count(*) FROM sdlc_routing_snapshot_inventory WHERE task_id=$1) AS inventory_count,
            sdlc_critical_pm_detail(g.blocker_id)->>'state' AS resolved,
            NOT EXISTS(SELECT 1 FROM sdlc_analysis_intents WHERE task_id=$1 AND requirement_revision=2) AS no_automatic_intent
         FROM sdlc_task_routing_generations g JOIN sdlc_task_routing_snapshots s USING(task_id)
         WHERE g.task_id=$1", vec![r.task.into()]
    )).await.unwrap().unwrap();
    assert_eq!(row.try_get::<i64>("", "generation").unwrap(), 2);
    assert_eq!(
        row.try_get::<Uuid>("", "previous_snapshot_id").unwrap(),
        row.try_get::<Uuid>("", "initial").unwrap()
    );
    assert_eq!(
        row.try_get::<Uuid>("", "confirmation_id").unwrap(),
        confirmation.id
    );
    assert_eq!(
        row.try_get::<Uuid>("", "configured_completion_id").unwrap(),
        saved.completion_id
    );
    assert!(row.try_get::<bool>("", "frozen").unwrap());
    assert!(row.try_get::<bool>("", "no_automatic_intent").unwrap());
    assert_eq!(row.try_get::<i64>("", "inventory_count").unwrap(), 2);
    assert_eq!(row.try_get::<String>("", "resolved").unwrap(), "resolved");
    assert_eq!(r.original, frozen_history(&r.f, r.task).await);
    for sql in [
        "UPDATE sdlc_task_routing_generations SET payload=payload WHERE task_id=$1",
        "UPDATE sdlc_configured_pm_completions SET result=result WHERE task_id=$1",
        "DELETE FROM sdlc_configured_pm_completions WHERE task_id=$1",
    ] {
        let tx = r.f.db.begin().await.unwrap();
        assert!(
            tx.execute(statement(sql, vec![r.task.into()]))
                .await
                .is_err()
        );
        tx.rollback().await.unwrap();
    }
}

#[tokio::test]
#[ignore = "fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test; main-applied hooks/096; synthetic runtime only"]
async fn actual_critical_pm_lifecycle_is_atomic_and_preserves_initial_history() {
    let mut r = Box::pin(setup()).await;
    let (hash, confirmation) = Box::pin(questions_and_revision(&mut r)).await;
    Box::pin(completion(&r, hash, confirmation)).await;
}
