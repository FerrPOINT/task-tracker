//! A persisted 116 completion is read-only history after the 117 upgrade.
use super::*;

// Write the released terminal Review shape through the predecessor's real guards.
// This fixture does not call the candidate's changed transition function.
pub(super) async fn complete_released_review(
    f: &ReworkFixture,
    proposed: &LifecycleReceipt,
    command: &LifecycleCommand,
) -> LifecycleReceipt {
    let LifecycleCommand::Complete {
        fence: expected,
        terminal,
        result,
        ..
    } = command
    else {
        panic!("Complete required");
    };
    assert!(matches!(result, DeliveryResult::Reviewer { .. }));
    let tx = f.repo.db.begin().await.unwrap();
    let state = f.repo.load(&tx, f.task, &f.scheduler).await.unwrap();
    let mut receipt = current(&tx, f.task).await.unwrap().unwrap();
    assert_eq!(&receipt, proposed);
    fence(&receipt, expected, &f.scheduler).unwrap();
    let accepted = AcceptedDeliveryResult {
        result_id: Uuid::now_v7(),
        assignment_id: expected.assignment_id,
        task_id: f.task,
        previous_result_id: None,
        terminal: terminal.clone(),
        output: result.clone(),
        previous_stage: LifecycleStage::Review,
        next_stage: LifecycleStage::Review,
        next_status: LifecycleStatus::Completed,
        cycle: receipt.task.cycle,
        created_at: now(&tx).await.unwrap(),
    };
    exec(
        &tx,
        "INSERT INTO sdlc_delivery_results(result_id,assignment_id,task_id,payload)
        VALUES($1,$2,$3,$4)",
        vec![
            accepted.result_id.into(),
            expected.assignment_id.into(),
            f.task.into(),
            json_value(&accepted).unwrap(),
        ],
    )
    .await
    .unwrap();
    exec(
        &tx,
        "DELETE FROM sdlc_delivery_holds WHERE assignment_id=$1",
        vec![expected.assignment_id.into()],
    )
    .await
    .unwrap();
    receipt.task.version += 1;
    receipt.task.runtime = RuntimeState::Terminal;
    receipt.task.waiting_reason = Some("review_completed".into());
    receipt.task.previous_result_id = Some(accepted.result_id);
    receipt.result = Some(accepted);
    receipt.assignment = None;
    receipt.lease = None;
    receipt.binding = None;
    PostgresSdlcRepository::save_delivery_projection(&tx, &f.scheduler, &receipt, false)
        .await
        .unwrap();
    let event_id = Uuid::now_v7();
    exec(&tx, "INSERT INTO sdlc_delivery_operations(task_id,actor_subject,idempotency_key,payload_hash,result,event_id)
        VALUES($1,$2,$3,$4,$5,$6)", vec![f.task.into(), f.scheduler.subject.clone().into(),
            command.key().into(), app::sdlc::canonical_hash(command).unwrap().into(),
            json_value(&receipt).unwrap(), event_id.into()]).await.unwrap();
    let payload = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
        "project_id":state.project_id,"task_id":f.task,"root_task_id":f.task,
        "owner_subject":state.owner_subject,"stage":receipt.task.stage,
        "requirement_revision":receipt.task.routing.requirement_revision,"result":&receipt});
    exec(
        &tx,
        "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,$3,$4)",
        vec![
            event_id.into(),
            f.task.into(),
            "delivery.result_accepted".into(),
            payload.into(),
        ],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    receipt
}

async fn assert_archived_review_upgrade(verdict: ReviewVerdict) {
    let (f, queued) = ReworkFixture::with_schema_tail(
        None,
        None,
        "owner",
        "scheduler",
        Some("m20261110_0000116_untimed_delivery_binding"),
    )
    .await;
    let (cursor, mut originals) = through_review(&f, queued).await;
    let original = f
        .step_with_released_review(&cursor, reviewer(verdict), 3, true)
        .await;
    let archived = original.completed.clone();
    originals.push(original);
    let before = f.journal_snapshot().await;
    let ledger =
        f.db.query_all(statement(
            "SELECT version,applied_at FROM seaql_migrations ORDER BY version",
            vec![],
        ))
        .await
        .unwrap()
        .into_iter()
        .map(|row| {
            (
                row.try_get::<String>("", "version").unwrap(),
                row.try_get::<i64>("", "applied_at").unwrap(),
            )
        })
        .collect::<Vec<_>>();
    // pg_get_functiondef may retain CRLF from a released Windows checkout.
    let definition =
        f.db.query_one(statement(
            "SELECT pg_get_functiondef('sdlc_delivery_result_gate()'::regprocedure) AS definition",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<String>("", "definition")
        .unwrap();
    f.db.execute_unprepared(&definition.replace("\r\n", "\n").replace('\n', "\r\n"))
        .await
        .unwrap();
    migration::Migrator::up(&f.db, None).await.unwrap();
    assert_eq!(f.journal_snapshot().await, before);
    for (name, applied_at) in ledger {
        let row =
            f.db.query_one(statement(
                "SELECT applied_at FROM seaql_migrations WHERE version=$1",
                vec![name.into()],
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.try_get::<i64>("", "applied_at").unwrap(), applied_at);
    }
    migration::Migrator::up(&f.db, None).await.unwrap();
    assert_eq!(f.journal_snapshot().await, before);
    assert_settled_replays(&f, &archived, &originals).await;
    assert_eq!(f.journal_snapshot().await, before);
    assert_eq!(archived.task.stage, LifecycleStage::Review);
    assert_eq!(archived.task.status, LifecycleStatus::Completed);
    assert_eq!(
        archived.task.waiting_reason.as_deref(),
        Some("review_completed")
    );
    assert!(
        f.repo
            .lifecycle_command(
                f.task,
                &f.scheduler,
                LifecycleCommand::EnsureQueued {
                    idempotency_key: "archive-must-not-requeue".into(),
                }
            )
            .await
            .is_err()
    );
    assert_eq!(f.journal_snapshot().await, before);
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending in _lifecycle_test"]
async fn approved_review_archive_survives_forward_upgrade_without_starting_tester() {
    assert_archived_review_upgrade(ReviewVerdict::Approved).await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending in _lifecycle_test"]
async fn rework_review_archive_survives_forward_upgrade_without_starting_developer() {
    assert_archived_review_upgrade(ReviewVerdict::Rework).await;
}
