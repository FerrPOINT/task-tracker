//! A filled, genuinely migrated predecessor. No live task or native execution.
use super::*;

async fn retained_rows(f: &ReworkFixture) -> Json {
    f.db.query_one(statement(
        "SELECT jsonb_build_object('task',(SELECT to_jsonb(t) FROM sdlc_tasks t WHERE task_id=$1),
         'head',(SELECT to_jsonb(h) FROM sdlc_delivery_heads h WHERE task_id=$1),
         'assignments',(SELECT jsonb_agg(to_jsonb(a) ORDER BY assignment_id) FROM sdlc_delivery_assignments a WHERE task_id=$1),
         'operations',(SELECT jsonb_agg(to_jsonb(o) ORDER BY idempotency_key) FROM sdlc_delivery_operations o WHERE task_id=$1),
         'holds',(SELECT jsonb_agg(to_jsonb(h) ORDER BY assignment_id) FROM sdlc_delivery_holds h WHERE task_id=$1),
         'pm_leases',(SELECT jsonb_agg(to_jsonb(l) ORDER BY execution_id) FROM sdlc_pm_execution_leases l)) AS value",
        vec![f.task.into()],
    )).await.unwrap().unwrap().try_get("", "value").unwrap()
}

const REVIEWER_116: &str = "m20261108_0000116_reviewer_terminal";
const UNTIMED_116: &str = "m20261110_0000116_untimed_delivery_binding";

struct Released116<const REVIEWER: bool>;

#[async_trait::async_trait]
impl<const REVIEWER: bool> MigratorTrait for Released116<REVIEWER> {
    fn migrations() -> Vec<Box<dyn sea_orm_migration::MigrationTrait>> {
        let absent = if REVIEWER { UNTIMED_116 } else { REVIEWER_116 };
        migration::Migrator::migrations()
            .into_iter()
            .take_while(|item| item.name() != "m20261111_0000117_reviewer_handoff")
            .filter(|item| item.name() != absent)
            .collect()
    }
}

async fn assert_binding_upgrade(reviewer_history: Option<bool>) {
    let (f, queued) = ReworkFixture::with_schema_tail(
        None,
        None,
        "owner",
        "scheduler",
        Some("m20261107_0000115_agent_stage_status"),
    )
    .await;
    let producer = &f.producers["analyst"];
    let command = LifecycleCommand::Reserve {
        expected_task_version: queued.task.version,
        machine_subject: Some(producer.subject.clone()),
        idempotency_key: "predecessor-reserve".into(),
    };
    // Reuse the real reservation writer and 115 guards; only its old timestamp
    // representation is supplied by this fixture before the original INSERT.
    let tx = f.repo.db.begin().await.unwrap();
    f.repo.load(&tx, f.task, &f.scheduler).await.unwrap();
    let mut saved = current(&tx, f.task).await.unwrap().unwrap();
    f.repo
        .delivery_reserve(
            &tx,
            &f.scheduler,
            &mut saved,
            queued.task.version,
            Some(&producer.subject),
        )
        .await
        .unwrap();
    let binding = saved.lease.as_mut().unwrap();
    binding.claimed_at -= chrono::Duration::minutes(2);
    binding.heartbeat_at = Some(binding.claimed_at);
    binding.expires_at = Some(binding.claimed_at + chrono::Duration::seconds(30));
    PostgresSdlcRepository::save_delivery_projection(&tx, &f.scheduler, &saved, false)
        .await
        .unwrap();
    exec(&tx, "INSERT INTO sdlc_delivery_operations(task_id,actor_subject,idempotency_key,payload_hash,result,event_id)
        VALUES($1,$2,$3,$4,$5,NULL)", vec![f.task.into(),f.scheduler.subject.clone().into(),command.key().into(),
        app::sdlc::canonical_hash(&command).unwrap().into(),serde_json::to_value(&saved).unwrap().into()]).await.unwrap();
    tx.commit().await.unwrap();
    match reviewer_history {
        Some(true) => Released116::<true>::up(&f.db, None).await.unwrap(),
        Some(false) => Released116::<false>::up(&f.db, None).await.unwrap(),
        None => {}
    }
    let before = retained_rows(&f).await;
    let ledger: Vec<(String, i64)> =
        f.db.query_all(statement(
            "SELECT version,applied_at FROM seaql_migrations ORDER BY version",
            vec![],
        ))
        .await
        .unwrap()
        .into_iter()
        .map(|r| {
            (
                r.try_get("", "version").unwrap(),
                r.try_get("", "applied_at").unwrap(),
            )
        })
        .collect();

    migration::Migrator::up(&f.db, None).await.unwrap();
    assert_eq!(retained_rows(&f).await, before);
    let added =
        f.db.query_all(statement(
            "SELECT version FROM seaql_migrations WHERE version IN ($1,$2) ORDER BY version",
            vec![REVIEWER_116.into(), UNTIMED_116.into()],
        ))
        .await
        .unwrap();
    assert_eq!(
        added
            .into_iter()
            .map(|row| row.try_get::<String>("", "version").unwrap())
            .collect::<Vec<_>>(),
        vec![REVIEWER_116, UNTIMED_116],
    );
    for (name, at) in &ledger {
        let row =
            f.db.query_one(statement(
                "SELECT applied_at FROM seaql_migrations WHERE version=$1",
                vec![name.clone().into()],
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.try_get::<i64>("", "applied_at").unwrap(), *at);
    }
    migration::Migrator::up(&f.db, None).await.unwrap();
    assert_eq!(retained_rows(&f).await, before);
    let fresh = f.repo.lifecycle_read(f.task, &f.reader).await.unwrap();
    assert_eq!(fresh.runtime_state, RuntimeState::Reserved);
    assert_eq!(fresh.receipt.assignment, saved.assignment);
    let binding = fresh.receipt.lease.as_ref().unwrap();
    assert!(binding.heartbeat_at.is_none() && binding.expires_at.is_none());
    assert_eq!(binding.lease_id, saved.lease.as_ref().unwrap().lease_id);
    assert_eq!(binding.version, saved.lease.as_ref().unwrap().version);
    assert_eq!(
        binding.holder_subject,
        saved.lease.as_ref().unwrap().holder_subject
    );
    assert_eq!(binding.claimed_at, saved.lease.as_ref().unwrap().claimed_at);
    let replay = f
        .repo
        .lifecycle_command(f.task, &f.scheduler, command.clone())
        .await
        .unwrap();
    assert!(replay.1);
    assert_eq!(
        serde_json::to_value(&replay.0).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
    assert_eq!(
        f.repo
            .lifecycle_operation(f.task, &f.reader, command.key())
            .await
            .unwrap(),
        saved
    );
    assert_eq!(retained_rows(&f).await, before);

    let native = RunBinding {
        chat_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        machine_subject: producer.subject.clone(),
        dispatch_receipt: artifact("untimed-native-binding"),
    };
    let mut invalid = delivery_fence(&fresh.receipt);
    invalid.expected_lease_version += 1;
    assert!(matches!(
        f.repo
            .lifecycle_command(
                f.task,
                &f.scheduler,
                LifecycleCommand::BindRun {
                    fence: invalid,
                    binding: native.clone(),
                    idempotency_key: "wrong-version".into(),
                }
            )
            .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(retained_rows(&f).await, before);
    let mut foreign = f.scheduler.clone();
    foreign.subject = f.owner.subject.clone();
    assert!(matches!(
        f.repo
            .lifecycle_command(
                f.task,
                &foreign,
                LifecycleCommand::BindRun {
                    fence: delivery_fence(&fresh.receipt),
                    binding: native.clone(),
                    idempotency_key: "wrong-holder".into(),
                }
            )
            .await,
        Err(AppError::Forbidden)
    ));
    assert_eq!(retained_rows(&f).await, before);
    let (bound, _) = f
        .repo
        .lifecycle_command(
            f.task,
            &f.scheduler,
            LifecycleCommand::BindRun {
                fence: delivery_fence(&fresh.receipt),
                binding: native,
                idempotency_key: "bind-after-expiry".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(bound.task.runtime, RuntimeState::Running);
    let start = StartStage {
        fence: StageStartFence::Delivery(delivery_fence(&bound)),
        idempotency_key: "start-after-expiry".into(),
    };
    f.repo.start_stage(f.task, producer, start).await.unwrap();
    let running = f.repo.lifecycle_read(f.task, &f.reader).await.unwrap();
    assert_eq!(running.receipt.task.status, LifecycleStatus::InProgress);
    assert_eq!(running.receipt.lease, fresh.receipt.lease);
    assert!(migration::Migrator::down(&f.db, Some(1)).await.is_err());
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending in _lifecycle_test"]
async fn forward_116_preserves_expired_binding_history_and_binds_without_renewal() {
    assert_binding_upgrade(None).await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending in _lifecycle_test"]
async fn reviewer_116_history_accepts_untimed_binding_without_rewriting_rows_or_ledger() {
    assert_binding_upgrade(Some(true)).await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_LIFECYCLE_TEST_DATABASE_URL ending in _lifecycle_test"]
async fn untimed_116_history_accepts_reviewer_without_rewriting_rows_or_ledger() {
    assert_binding_upgrade(Some(false)).await;
}
