//! Child of existing critical candidates fixture. All runtime observations remain synthetic.
use super::*;
use app::sdlc_critical_pm_reservation::{
    CriticalPmReservationAllocation, CriticalPmReservationSource, prepare_critical_pm_reservation,
};
use domain::sdlc_critical_pm_reservation::*;

async fn review_snapshot(f: &Fixture, task: Uuid) -> Json {
    f.db.query_one(statement("SELECT jsonb_build_object(
        'state',(SELECT state FROM sdlc_tasks WHERE task_id=$1),
        'cursor',(SELECT jsonb_build_object('owner',pm_owner_version,'execution',pm_execution_id,'admission',pm_admission_state)
            FROM sdlc_tasks WHERE task_id=$1),
        'execution',(SELECT execution FROM sdlc_delivery_heads WHERE task_id=$1),
        'issue',(SELECT jsonb_build_object('status',status_id,'updated',updated_at) FROM issues WHERE id=$1),
        'reviews',(SELECT count(*) FROM sdlc_critical_pm_reviews WHERE task_id=$1),
        'assignments',(SELECT count(*) FROM sdlc_assignments WHERE task_id=$1),
        'pm_executions',(SELECT count(*) FROM sdlc_pm_executions WHERE task_id=$1),
        'outbox',(SELECT count(*) FROM sdlc_outbox WHERE task_id=$1),
        'holds',(SELECT count(*) FROM sdlc_delivery_holds WHERE task_id=$1),
        'permits',(SELECT count(*) FROM sdlc_delivery_projection_permits WHERE task_id=$1),
        'pm_permits',(SELECT count(*) FROM sdlc_pm_state_write_permits WHERE task_id=$1)) AS value",
        vec![task.into()])).await.unwrap().unwrap().try_get("", "value").unwrap()
}

async fn ordinal(f: &Fixture) -> i64 {
    f.db.query_one(statement(
        "SELECT pg_sequence_last_value(
        pg_get_serial_sequence('sdlc_pm_executions','ordinal')::regclass) AS value",
        vec![],
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "value")
    .unwrap()
}

// A raw SQL proposal-only transaction must not COMMIT without its projection/rows/outbox.
async fn reject_journal_only(f: &Fixture, task: Uuid, command: &ReserveCriticalPmReview) {
    let tx = f.db.begin().await.unwrap();
    let state = f.repo.load(&tx, task, &f.writer).await.unwrap();
    let current = crate::sdlc::lifecycle::current(&tx, task)
        .await
        .unwrap()
        .unwrap();
    let cursor = tx
        .query_one(statement(
            "SELECT state,pm_owner_version,pm_execution_id FROM sdlc_tasks
        WHERE task_id=$1",
            vec![task.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    let previous_id: Uuid = cursor.try_get("", "pm_execution_id").unwrap();
    let owner: i64 = cursor.try_get("", "pm_owner_version").unwrap();
    let previous = PostgresSdlcRepository::reservation(&tx, &state, previous_id)
        .await
        .unwrap();
    let completed: domain::sdlc_configured_cli::ConfiguredPmCompletionReceipt = parse(
        tx.query_one(statement(
            "SELECT result FROM sdlc_configured_pm_completions WHERE task_id=$1 AND execution_id=$2",
            vec![task.into(), previous_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "result")
        .unwrap(),
    );
    let returned: CriticalBlockerReceipt = parse(
        tx.query_one(statement(
            "SELECT after_blocker FROM sdlc_critical_pm_returns WHERE task_id=$1 AND blocker_id=$2",
            vec![task.into(), command.blocker_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "after_blocker")
        .unwrap(),
    );
    let input = PmDraftInputResponse {
        contract_version: 1,
        tracker_instance_id: state.tracker_instance_id.clone(),
        project_id: state.project_id,
        task_id: task,
        root_task_id: task,
        owner_subject: state.owner_subject.clone(),
        input: PostgresSdlcRepository::original_input(&tx, &state)
            .await
            .unwrap(),
    };
    let number: i64 = tx
        .query_one(statement(
            "SELECT nextval(
        pg_get_serial_sequence('sdlc_pm_executions','ordinal')) AS n",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "n")
        .unwrap();
    let plan = prepare_critical_pm_reservation(
        CriticalPmReservationSource {
            state: &state,
            lifecycle: &current,
            blocker: &returned,
            previous_reservation: &previous,
            current_completed: app::sdlc_critical_pm_reservation::CompletedPmReceipt::Configured(
                &completed,
            ),
            original_pm_assignment: &previous.assignment,
            original_input: &input,
            pm_owner_version: owner,
            pm_admission_state: "completed",
        },
        &f.writer,
        &f.repo.config,
        command,
        CriticalPmReservationAllocation {
            assignment_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            ordinal: number,
        },
    )
    .unwrap();
    let before_state: Json = cursor.try_get("", "state").unwrap();
    let execution: Json = tx
        .query_one(statement(
            "SELECT execution FROM sdlc_delivery_heads WHERE task_id=$1",
            vec![task.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "execution")
        .unwrap();
    let reservation = &plan.result.reservation;
    exec(
        &tx,
        "INSERT INTO sdlc_critical_pm_reviews(review_id,task_id,blocker_id,actor_subject,
        idempotency_key,request_sha256,command,previous_execution_id,previous_owner_version,
        execution_id,assignment_id,assignment_version,owner_version,
        before_state,after_state,before_execution,after_execution,after_blocker,result)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$16,$17,$18)",
        vec![
            Uuid::new_v4().into(),
            task.into(),
            command.blocker_id.into(),
            f.writer.subject.clone().into(),
            command.idempotency_key.clone().into(),
            app::sdlc::canonical_hash(command).unwrap().into(),
            json_value(command).unwrap(),
            previous_id.into(),
            owner.into(),
            reservation.assignment.execution_id.into(),
            reservation.assignment.assignment_id.into(),
            reservation.assignment.version.into(),
            reservation.owner_cas.version.into(),
            before_state.into(),
            json_value(&plan.state).unwrap(),
            execution.into(),
            json_value(&plan.blocker).unwrap(),
            json_value(&plan.result).unwrap(),
        ],
    )
    .await
    .unwrap();
    assert!(
        tx.commit().await.is_err(),
        "journal alone must fail deferred commit"
    );
}

#[tokio::test]
#[ignore = "fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test; synthetic native/Workflow fixtures"]
async fn actual_critical_pm_review_reservation_is_atomic_scoped_and_replayable() {
    let f = Box::pin(Fixture::new()).await;
    if f.db
        .query_one(statement(
            "SELECT to_regclass('sdlc_critical_pm_reviews') IS NULL AS missing",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<bool>("", "missing")
        .unwrap()
    {
        f.db.execute_unprepared(include_str!("../../migration/src/critical_pm_review.sql"))
            .await
            .unwrap();
    }
    let user = Uuid::new_v4();
    exec(&f.db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active)
        VALUES($1,'critical-agent@example.test','critical-agent','critical-agent','!','77777777-7777-4777-8777-777777777777',true)",
        vec![user.into()]).await.unwrap();
    exec(
        &f.db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'developer')",
        vec![f.project.into(), user.into()],
    )
    .await
    .unwrap();
    let storage = domain::InMemoryStorage::default();
    let r = Box::pin(start(&f, "pm-review-root", false, &storage)).await;
    let body = blocker(&r);
    let (held, _) =
        Box::pin(
            f.repo
                .submit_critical_blocker_with_storage(r.task, &r.producer, body, &storage),
        )
        .await
        .unwrap();
    let head = f
        .repo
        .lifecycle_read(r.task, &f.reader)
        .await
        .unwrap()
        .receipt;
    let return_command = ReturnCriticalToPm {
        review: CriticalPmReviewGuard {
            blocker_id: held.blocker_id,
            blocker_sha256: held.blocker_sha256.clone(),
            expected_blocker_version: held.version,
            expected_task_version: head.task.version,
            fence: fence(&head),
            idempotency_key: "return-before-review".into(),
        },
        terminal: RuntimeTerminalObservation {
            assignment_hash: held.assignment_hash.clone(),
            chat_id: held.chat_id,
            run_id: held.run_id,
            state: RuntimeTerminalState::Cancelled,
            receipt: ArtifactReceipt {
                reference: "fixture://synthetic-terminal-not-live-proof".into(),
                sha256: "d".repeat(64),
            },
        },
    };
    let (returned, _) = Box::pin(f.repo.return_critical_to_pm_with_terminal(
        r.task,
        &f.writer,
        return_command,
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
    let completion = f
        .repo
        .configured_pm_completion(r.task, &f.reader, None)
        .await
        .unwrap()
        .unwrap()
        .result;
    let original_input = f.repo.pm_draft_input(r.task, &f.owner).await.unwrap();
    let command = ReserveCriticalPmReview {
        blocker_id: held.blocker_id,
        blocker_sha256: held.blocker_sha256.clone(),
        expected_blocker_version: returned.blocker.version,
        expected_task_version: returned.lifecycle.task.version,
        expected_pm_owner_version: previous.owner_cas.version,
        expected_pm_assignment_version: previous.assignment.version,
        idempotency_key: "original-pm-review-reservation".into(),
    };
    let before = review_snapshot(&f, r.task).await;
    let old_history = json!({"reservation":previous,"completion":completion,"input":original_input,
        "held":held,"returned":returned.blocker});
    let old_pm = actor(
        "pm",
        false,
        &["task-tracker:write", &previous.assignment.scope(r.task)],
    );
    let paired = actor(
        "scheduler",
        false,
        &["task-tracker:read", "task-tracker:write"],
    );
    for denied in [&f.owner, &f.reader, &old_pm, &paired] {
        assert!(matches!(
            Box::pin(
                f.repo
                    .reserve_critical_pm_review(r.task, denied, command.clone())
            )
            .await,
            Err(AppError::Forbidden)
        ));
        assert_eq!(review_snapshot(&f, r.task).await, before);
    }
    for changed in 0..5 {
        let mut stale = command.clone();
        match changed {
            0 => stale.expected_task_version += 1,
            1 => stale.expected_blocker_version += 1,
            2 => stale.expected_pm_owner_version += 1,
            3 => stale.expected_pm_assignment_version += 1,
            _ => stale.blocker_sha256 = "f".repeat(64),
        }
        assert!(
            Box::pin(f.repo.reserve_critical_pm_review(r.task, &f.writer, stale))
                .await
                .is_err()
        );
        assert_eq!(review_snapshot(&f, r.task).await, before);
    }
    // A forged unbacked cursor cannot bypass old canonical/PM guards.
    let tx = f.db.begin().await.unwrap();
    assert!(
        exec(
            &tx,
            "UPDATE sdlc_tasks SET pm_owner_version=pm_owner_version+1 WHERE task_id=$1",
            vec![r.task.into()]
        )
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    assert_eq!(review_snapshot(&f, r.task).await, before);
    Box::pin(reject_journal_only(&f, r.task, &command)).await;
    assert_eq!(review_snapshot(&f, r.task).await, before);
    f.db.execute_unprepared("CREATE FUNCTION pm_review_test_reject_event() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN IF NEW.event_type='critical.pm_review_reserved' THEN RAISE EXCEPTION 'synthetic late failure'; END IF;
        RETURN NEW; END $$; CREATE TRIGGER pm_review_test_reject_event BEFORE INSERT ON sdlc_outbox
        FOR EACH ROW EXECUTE FUNCTION pm_review_test_reject_event();").await.unwrap();
    assert!(
        Box::pin(
            f.repo
                .reserve_critical_pm_review(r.task, &f.writer, command.clone())
        )
        .await
        .is_err()
    );
    assert_eq!(review_snapshot(&f, r.task).await, before);
    f.db.execute_unprepared(
        "DROP TRIGGER pm_review_test_reject_event ON sdlc_outbox;
        DROP FUNCTION pm_review_test_reject_event();",
    )
    .await
    .unwrap();
    let (one, two) = tokio::join!(
        Box::pin(
            f.repo
                .reserve_critical_pm_review(r.task, &f.writer, command.clone())
        ),
        Box::pin(
            f.repo
                .reserve_critical_pm_review(r.task, &f.writer, command.clone())
        ),
    );
    let (one, replay_one) = one.unwrap();
    let (two, replay_two) = two.unwrap();
    assert_eq!(json!(one), json!(two));
    assert_ne!(replay_one, replay_two);
    assert_eq!(
        one.reservation.assignment.agent_id,
        previous.assignment.agent_id
    );
    assert_ne!(
        one.reservation.assignment.assignment_id,
        previous.assignment.assignment_id
    );
    assert_ne!(
        one.reservation.assignment.execution_id,
        previous.assignment.execution_id
    );
    assert_eq!(
        one.reservation.assignment.version,
        previous.assignment.version + 1
    );
    assert_eq!(
        one.reservation.owner_cas.version,
        previous.owner_cas.version + 1
    );
    assert_eq!(json!(one.reservation.input), json!(previous.input));
    assert!(!one.reservation.dispatch_allowed);
    assert_eq!(one.context.blocker_version, 3);
    let after = review_snapshot(&f, r.task).await;
    assert_eq!(after["reviews"], json!(1));
    assert_eq!(after["assignments"], json!(2));
    assert_eq!(after["pm_executions"], json!(2));
    assert_eq!(after["holds"], json!(0));
    assert_eq!(after["permits"], json!(0));
    assert_eq!(after["pm_permits"], json!(0));
    assert_eq!(after["cursor"]["admission"], json!("reserved"));
    assert_eq!(after["state"]["runtime_state"], json!("terminal"));
    assert_eq!(after["state"]["draft_submode"], json!("brainstorming"));
    assert_eq!(after["state"]["confirmation_revision"], json!(null));
    assert_eq!(after["execution"], before["execution"]);
    let sequence = ordinal(&f).await;
    assert!(
        Box::pin(
            f.repo
                .reserve_critical_pm_review(r.task, &f.writer, command.clone())
        )
        .await
        .unwrap()
        .1
    );
    assert_eq!(
        ordinal(&f).await,
        sequence,
        "original replay must not allocate another ordinal"
    );
    assert_eq!(
        json!(
            f.repo
                .critical_pm_review_operation(r.task, &f.reader, &command.idempotency_key)
                .await
                .unwrap()
        ),
        json!(one)
    );
    for denied in [&f.writer, &f.owner, &old_pm, &paired] {
        assert!(matches!(
            f.repo
                .critical_pm_review_operation(r.task, denied, &command.idempotency_key)
                .await,
            Err(AppError::Forbidden)
        ));
    }
    exec(
        &f.db,
        "UPDATE users SET is_active=false WHERE central_sub='owner'",
        vec![],
    )
    .await
    .unwrap();
    assert!(matches!(
        f.repo
            .critical_pm_review_operation(r.task, &f.reader, &command.idempotency_key)
            .await,
        Err(AppError::Forbidden)
    ));
    exec(
        &f.db,
        "UPDATE users SET is_active=true WHERE central_sub='owner'",
        vec![],
    )
    .await
    .unwrap();
    exec(
        &f.db,
        "DELETE FROM project_members WHERE project_id=$1 AND user_id IN
        (SELECT id FROM users WHERE central_sub='scheduler')",
        vec![f.project.into()],
    )
    .await
    .unwrap();
    assert!(matches!(
        f.repo
            .critical_pm_review_operation(r.task, &f.reader, &command.idempotency_key)
            .await,
        Err(AppError::Forbidden)
    ));
    exec(
        &f.db,
        "INSERT INTO project_members(project_id,user_id,role)
        SELECT $1,id,'developer' FROM users WHERE central_sub='scheduler'",
        vec![f.project.into()],
    )
    .await
    .unwrap();
    // Reconstruct the repository and recover a committed operation using GET-only readback.
    let url = std::env::var("TT_CANDIDATES_TEST_DATABASE_URL").unwrap();
    let recovered = PostgresSdlcRepository::connect(&url, f.repo.config.clone())
        .await
        .unwrap();
    assert_eq!(
        json!(
            recovered
                .critical_pm_review_operation(r.task, &f.reader, &command.idempotency_key)
                .await
                .unwrap()
        ),
        json!(one)
    );
    assert_eq!(ordinal(&f).await, sequence);
    let tx = f.db.begin().await.unwrap();
    assert!(
        exec(
            &tx,
            "UPDATE sdlc_critical_pm_reviews SET result='{}' WHERE task_id=$1",
            vec![r.task.into()]
        )
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    let other = f.draft("review-foreign-lookup", None).await;
    assert!(matches!(
        f.repo
            .critical_pm_review_operation(other, &f.reader, &command.idempotency_key)
            .await,
        Err(AppError::NotFound { .. })
    ));
    let mut changed = command.clone();
    changed.expected_task_version += 1;
    assert!(matches!(
        Box::pin(
            f.repo
                .reserve_critical_pm_review(r.task, &f.writer, changed)
        )
        .await,
        Err(AppError::Conflict(_))
    ));
    let current_pm = f
        .repo
        .pm_draft_assignment(r.task, &f.owner, None)
        .await
        .unwrap()
        .current
        .unwrap();
    assert_eq!(json!(current_pm), json!(one.reservation));
    assert!(
        f.repo
            .configured_pm_completion(r.task, &f.reader, None)
            .await
            .unwrap()
            .is_none()
    );
    // The main-applied companion query must expose review detail without rewriting old receipts.
    let detail = f
        .repo
        .critical_blocker(r.task, &f.reader, held.blocker_id)
        .await
        .unwrap();
    assert_eq!(detail.state, CriticalBlockerState::InPmReview);
    assert_eq!(detail.version, 3);
    assert_eq!(
        detail.pm_review.as_ref().unwrap().execution_id,
        one.reservation.assignment.execution_id
    );
    let history =
        f.db.query_one(statement(
            "SELECT e.result AS reservation,c.result AS completion,
        b.payload AS held,r.after_blocker AS returned FROM sdlc_pm_executions e
        JOIN sdlc_configured_pm_completions c USING(task_id,execution_id)
        JOIN sdlc_critical_blockers b ON b.task_id=e.task_id
        JOIN sdlc_critical_pm_returns r ON r.blocker_id=b.blocker_id
        WHERE e.task_id=$1 AND e.execution_id=$2",
            vec![r.task.into(), previous.assignment.execution_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        old_history,
        json!({"reservation":history.try_get::<Json>("","reservation").unwrap(),
        "completion":history.try_get::<Json>("","completion").unwrap(),
        "input":f.repo.pm_draft_input(r.task,&f.owner).await.unwrap(),
        "held":history.try_get::<Json>("","held").unwrap(),"returned":history.try_get::<Json>("","returned").unwrap()})
    );
    let no_dispatch =
        f.db.query_one(statement(
            "SELECT NOT EXISTS(SELECT 1 FROM sdlc_pm_execution_leases WHERE execution_id=$1)
        AND NOT EXISTS(SELECT 1 FROM sdlc_pm_native_admissions WHERE execution_id=$1)
        AND NOT EXISTS(SELECT 1 FROM sdlc_configured_pm_completions WHERE execution_id=$1) AS absent",
            vec![one.reservation.assignment.execution_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(no_dispatch.try_get::<bool>("", "absent").unwrap());
    assert_eq!(review_snapshot(&f, r.task).await, after);
}
