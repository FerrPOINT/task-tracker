//! Child of the existing PM lease fixture. All Fleet/Workflow proof is synthetic.
use super::*;
use domain::sdlc_pm_bootstrap_retry::*;

fn retry(p: &InitialPm) -> (BootstrapRetryCommand, BootstrapFailureReadback) {
    let run = Uuid::new_v4();
    let predecessor = WorkflowBootstrapPredecessor {
        task_id: 7,
        assignment_operation_key: p.reservation.assignment_operation_key.clone(),
        assignment_revision: 1,
        attempt: 1,
        finalized_binding: Some(FinalizedBootstrapBinding {
            bind_operation_key: "original-finalized-bind".into(),
            bind_request_sha256: "a".repeat(64),
            binding_ref: "fixture://binding".into(),
            session_run_id: run,
            native_run_ref: "run_failed_before_first_step".into(),
        }),
    };
    let c = BootstrapRetryCommand {
        expected_owner_version: 1,
        fence: p.command.fence.clone(),
        lease_id: p.command.lease_id,
        expected_lease_version: p.lease.lease.version,
        creation_operation_id: run,
        previous_session_run_id: run,
        next_session_run_id: Uuid::new_v4(),
        previous_native_run_ref: "run_failed_before_first_step".into(),
        chat_id: Uuid::new_v4(),
        barrier_id: Uuid::new_v4(),
        predecessor,
        idempotency_key: "technical-retry".into(),
    };
    let w = BootstrapFailureReadback {
        contract_version: 1,
        observed_at: chrono::Utc::now(),
        identity: p.witness.identity.clone(),
        owner_subject: p.reservation.binding.owner_subject.clone(),
        input: p.reservation.input.clone(),
        owner_version: 1,
        lease_id: c.lease_id,
        lease_version: c.expected_lease_version,
        creation_operation_id: run,
        session_run_id: run,
        native_run_ref: c.previous_native_run_ref.clone(),
        chat_id: c.chat_id,
        barrier_id: c.barrier_id,
        next_session_run_id: c.next_session_run_id,
        terminal: Some(BootstrapNativeTerminal::Failed),
        predecessor: c.predecessor.clone(),
        initial_phase_id: 3,
        current_phase_id: 3,
        workflow_status: "active".into(),
        first_instruction: BootstrapReadbackState::Absent,
        pm_enrollment: BootstrapReadbackState::Absent,
        generic_binding: BootstrapReadbackState::Present,
        business_result: BootstrapReadbackState::Absent,
        inflight_work: BootstrapReadbackState::Absent,
        terminal_receipt: ArtifactReceipt {
            reference: "fixture://native-terminal".into(),
            sha256: "b".repeat(64),
        },
        bootstrap_receipt: ArtifactReceipt {
            reference: "fixture://workflow-readback".into(),
            sha256: "c".repeat(64),
        },
    };
    (c, w)
}

#[tokio::test]
async fn bootstrap_retry_0099_appends_after_0098_and_refuses_destructive_down() {
    let migrations = migration::Migrator::migrations();
    let index = migrations
        .iter()
        .position(|migration| migration.name() == "m20261021_0000099_pm_bootstrap_retry")
        .unwrap();
    assert_eq!(index, 30);
    let last = &migrations[index];
    assert_eq!(last.name(), "m20261021_0000099_pm_bootstrap_retry");
    assert_eq!(
        migrations[index - 1].name(),
        "m20261018_0000098_project_repository_catalog"
    );
    let db = sea_orm::MockDatabase::new(sea_orm::DbBackend::Postgres).into_connection();
    let error = last
        .down(&sea_orm_migration::SchemaManager::new(&db))
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("PM bootstrap retry history is immutable")
    );
    assert!(db.into_transaction_log().is_empty());
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test and one real 30s lease TTL"]
async fn bootstrap_retry_populated_history_concurrency_bad_witness_and_lost_ack() {
    let index = migration::Migrator::migrations()
        .iter()
        .position(|m| m.name() == "m20261021_0000099_pm_bootstrap_retry")
        .unwrap();
    let f = Fixture::new_with_migration_steps(Some(index as u32)).await;
    let p = initial(&f, "bootstrap-original").await;
    let q = initial(&f, "bootstrap-concurrent").await;
    let u = initial(&f, "bootstrap-unbound").await;
    let before_p = upgrade_snapshot(&f, &p).await;
    let before_q = upgrade_snapshot(&f, &q).await;
    let original_p = original_operations(&f, &p).await;
    let original_q = original_operations(&f, &q).await;
    let original_u = original_operations(&f, &u).await;
    let old_ledger = migration_versions(&f).await;
    migration::Migrator::up(&f.db, None).await.unwrap();
    assert_eq!(upgrade_snapshot(&f, &p).await, before_p);
    assert_eq!(upgrade_snapshot(&f, &q).await, before_q);
    let mut expected_ledger = old_ledger;
    expected_ledger.extend(
        [
            "m20261021_0000099_pm_bootstrap_retry",
            "m20261019_0000099_unified_spaces",
            "m20261021_0000100_human_sdlc_spaces",
            "m20261021_0000101_native_sdlc_spaces",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    // The SQL ledger is ordered by version; registration remains append-only.
    expected_ledger.sort();
    assert_eq!(migration_versions(&f).await, expected_ledger);
    let (c, mut w) = retry(&p);
    let (mut unbound, mut unbound_witness) = retry(&u);
    let late_binding = unbound.predecessor.finalized_binding.clone();
    unbound.predecessor.finalized_binding = None;
    unbound_witness.predecessor = unbound.predecessor.clone();
    unbound_witness.generic_binding = BootstrapReadbackState::Absent;
    assert!(
        f.repo
            .preflight_pm_bootstrap_retry(p.task, &f.writer, &c)
            .await
            .is_err(),
        "active lease uses heartbeat"
    );
    tokio::time::sleep(std::time::Duration::from_secs(31)).await;
    unbound_witness.observed_at = chrono::Utc::now();
    let unbound_before = snapshot(&f, &u).await;
    let mut late = unbound_witness.clone();
    late.generic_binding = BootstrapReadbackState::Present;
    late.predecessor.finalized_binding = late_binding;
    assert!(
        f.repo
            .retry_pm_bootstrap(u.task, &f.writer, unbound.clone(), late)
            .await
            .is_err()
    );
    assert_eq!(
        snapshot(&f, &u).await,
        unbound_before,
        "late-bind witness must not consume unbound retry"
    );
    let (unbound_result, replayed) = f
        .repo
        .retry_pm_bootstrap(u.task, &f.writer, unbound, unbound_witness)
        .await
        .unwrap();
    assert!(!replayed);
    assert_eq!(unbound_result.lease.version, u.lease.lease.version + 1);
    assert_eq!(unbound_result.lease.lease_id, u.lease.lease.lease_id);
    assert_eq!(unbound_result.lease.claimed_at, u.lease.lease.claimed_at);
    assert_eq!(original_operations(&f, &u).await, original_u);
    w.observed_at = chrono::Utc::now();
    let stable = snapshot(&f, &p).await;
    for case in 0..12 {
        let mut bad = w.clone();
        match case {
            0 => bad.pm_enrollment = BootstrapReadbackState::Present,
            1 => bad.first_instruction = BootstrapReadbackState::Unknown,
            2 => bad.inflight_work = BootstrapReadbackState::Pending,
            3 => bad.business_result = BootstrapReadbackState::Present,
            4 => bad.generic_binding = BootstrapReadbackState::Unknown,
            5 => bad.terminal = None,
            6 => bad.identity.agent_ref = Uuid::new_v4().to_string(),
            7 => bad.lease_version += 1,
            8 => bad.barrier_id = Uuid::new_v4(),
            9 => {
                bad.predecessor
                    .finalized_binding
                    .as_mut()
                    .unwrap()
                    .bind_request_sha256 = "d".repeat(64)
            }
            10 => bad.current_phase_id += 1,
            _ => bad.observed_at -= chrono::Duration::seconds(31),
        }
        assert!(
            f.repo
                .retry_pm_bootstrap(p.task, &f.writer, c.clone(), bad)
                .await
                .is_err(),
            "case {case}"
        );
        assert_eq!(snapshot(&f, &p).await, stable);
    }
    for denied in [
        &f.owner,
        &p.pm,
        &actor("foreign", false, &["task-tracker:write"]),
    ] {
        assert!(
            f.repo
                .preflight_pm_bootstrap_retry(p.task, denied, &c)
                .await
                .is_err()
        );
    }
    let mut stale = c.clone();
    stale.fence.assignment_version += 1;
    assert!(
        f.repo
            .preflight_pm_bootstrap_retry(p.task, &f.writer, &stale)
            .await
            .is_err()
    );
    // The old expired-renewal gate still refuses a raw update without ledger evidence.
    let tx = f.db.begin().await.unwrap();
    let now = crate::sdlc::execution_lease::clock(&tx).await.unwrap();
    let mut raw = p.lease.clone();
    raw.lease.version += 1;
    raw.lease.heartbeat_at = now;
    raw.lease.expires_at = now + chrono::Duration::seconds(30);
    assert!(tx.execute(statement("UPDATE sdlc_pm_execution_leases SET version=$2,heartbeat_at=$3,expires_at=$4,result=$5 WHERE execution_id=$1",
        vec![c.fence.execution_id.into(),raw.lease.version.into(),now.into(),raw.lease.expires_at.into(),json_value(&raw).unwrap()])).await.is_err());
    tx.rollback().await.unwrap();
    w.observed_at = chrono::Utc::now();
    let (a, b) = tokio::join!(
        f.repo
            .retry_pm_bootstrap(p.task, &f.writer, c.clone(), w.clone()),
        f.repo
            .retry_pm_bootstrap(p.task, &f.writer, c.clone(), w.clone())
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a.1, b.1);
    assert_eq!(
        serde_json::to_value(&a.0).unwrap(),
        serde_json::to_value(&b.0).unwrap()
    );
    assert_eq!(a.0.lease.version, p.lease.lease.version + 1);
    assert_eq!(a.0.lease.lease_id, p.lease.lease.lease_id);
    assert_eq!(a.0.lease.claimed_at, p.lease.lease.claimed_at);
    assert_eq!(a.0.lease.holder_subject, p.lease.lease.holder_subject);
    assert!(!a.0.dispatch_allowed);
    let after = snapshot(&f, &p).await;
    let reloaded = PostgresSdlcRepository::connect(
        &std::env::var("TT_CANDIDATES_TEST_DATABASE_URL").unwrap(),
        f.repo.config.clone(),
    )
    .await
    .unwrap();
    let saved = reloaded
        .pm_bootstrap_retry_operation(p.task, &f.reader, &c.idempotency_key)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(saved.result).unwrap(),
        serde_json::to_value(&a.0).unwrap()
    );
    let historical = reloaded
        .preflight_pm_bootstrap_retry(p.task, &f.writer, &c)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(historical).unwrap(),
        serde_json::to_value(&a.0).unwrap()
    );
    assert_eq!(snapshot(&f, &p).await, after);
    let mut changed = c.clone();
    changed.next_session_run_id = Uuid::new_v4();
    assert!(
        reloaded
            .preflight_pm_bootstrap_retry(p.task, &f.writer, &changed)
            .await
            .is_err()
    );
    let (q1, mut qw1) = retry(&q);
    let mut q2 = q1.clone();
    q2.next_session_run_id = Uuid::new_v4();
    q2.idempotency_key = "competing-key".into();
    qw1.observed_at = chrono::Utc::now();
    let mut qw2 = qw1.clone();
    qw2.next_session_run_id = q2.next_session_run_id;
    let (a, b) = tokio::join!(
        f.repo.retry_pm_bootstrap(q.task, &f.writer, q1, qw1),
        f.repo.retry_pm_bootstrap(q.task, &f.writer, q2, qw2)
    );
    assert_ne!(
        a.is_ok(),
        b.is_ok(),
        "only one successor can consume original execution"
    );
    assert_eq!(original_operations(&f, &p).await, original_p);
    assert_eq!(original_operations(&f, &q).await, original_q);
    // No task/assignment/issue/outbox rewrite: only lease and one appended operation.
    let final_p = upgrade_snapshot(&f, &p).await;
    for field in ["task", "issue", "reservation", "assignments", "outbox"] {
        assert_eq!(final_p[field], before_p[field]);
    }
}
