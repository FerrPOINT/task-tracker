//! Recovery regressions; PostgreSQL cases use disposable DBs and synthetic Fleet evidence.
use super::*;
use domain::sdlc_execution_lease::*;
#[path = "sdlc_pm_bootstrap_retry_pg_tests.rs"]
mod bootstrap_retry_tests;

#[tokio::test]
async fn unsubmitted_pm_migration_0098_order_and_down_refusal_without_database() {
    let migrations = migration::Migrator::migrations();
    let names: Vec<_> = migrations.iter().map(|m| m.name()).collect();
    assert_eq!(names.len(), 35);
    assert_eq!(
        &names[31..],
        &[
            "m20261019_0000099_unified_spaces",
            "m20261021_0000100_human_sdlc_spaces",
            "m20261021_0000101_native_sdlc_spaces",
            "m20261022_0000102_request_commit_clock",
        ]
    );
    assert_eq!(names[27], "m20261017_0000097_task_source_binding");
    assert_eq!(names[28], "m20261018_0000098_pm_unsubmitted_recovery");
    assert_eq!(names[29], "m20261018_0000098_project_repository_catalog");
    assert_eq!(names[30], "m20261021_0000099_pm_bootstrap_retry");
    let db = sea_orm::MockDatabase::new(sea_orm::DbBackend::Postgres).into_connection();
    let error = migrations[28]
        .down(&sea_orm_migration::SchemaManager::new(&db))
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("PM recovery history is immutable; destructive rollback is disabled")
    );
    assert!(db.into_transaction_log().is_empty());
}

struct InitialPm {
    task: Uuid,
    pm: Principal,
    reservation: PmDraftReservation,
    claim: ExecutionLeaseReceipt,
    claim_hash: String,
    heartbeat_hash: String,
    lease: ExecutionLeaseReceipt,
    command: RecoverUnsubmittedPmExecutionLease,
    witness: UnsubmittedPmWitness,
}

async fn initial(f: &Fixture, key: &str) -> InitialPm {
    let task = f.draft(key, Some(DraftFlow::SingleTaskV1)).await;
    let (r,_)=f.repo.reserve_pm_draft(task,&f.owner,parse(json!({"expected_owner_version":0,
        "expected_assignment_version":null,"requested_agent_id":f.roles.project_manager.agent_id,"idempotency_key":"reserve"}))).await.unwrap();
    let pm = actor(
        "pm",
        false,
        &[
            "task-tracker:read",
            "task-tracker:write",
            &r.assignment.scope(task),
        ],
    );
    let fence = MachineFence {
        assignment_id: r.assignment.assignment_id,
        execution_id: r.assignment.execution_id,
        agent_id: r.assignment.agent_id,
        assignment_version: r.assignment.version,
    };
    let claim_command = ClaimExecutionLease {
        expected_owner_version: r.owner_cas.version,
        fence: fence.clone(),
        idempotency_key: "original-claim".into(),
    };
    let claim_hash = app::sdlc::canonical_hash(
        &json!({"operation":"claim_pm_execution_lease","payload":claim_command}),
    )
    .unwrap();
    let (claim, replayed) = f
        .repo
        .claim_execution_lease(task, &pm, claim_command)
        .await
        .unwrap();
    assert!(!replayed);
    assert_eq!(claim.lease.version, 1);
    let heartbeat_command = HeartbeatExecutionLease {
        expected_owner_version: r.owner_cas.version,
        fence: fence.clone(),
        lease_id: claim.lease.lease_id,
        expected_lease_version: claim.lease.version,
        idempotency_key: "original-heartbeat".into(),
    };
    let heartbeat_hash = app::sdlc::canonical_hash(
        &json!({"operation":"heartbeat_pm_execution_lease","payload":heartbeat_command}),
    )
    .unwrap();
    let (lease, replayed) = f
        .repo
        .heartbeat_execution_lease(task, &pm, heartbeat_command)
        .await
        .unwrap();
    assert!(!replayed);
    assert_eq!(lease.lease.version, 2);
    assert_eq!(lease.lease.lease_id, claim.lease.lease_id);
    assert_eq!(lease.lease.holder_subject, claim.lease.holder_subject);
    assert_eq!(lease.lease.claimed_at, claim.lease.claimed_at);
    let c = RecoverUnsubmittedPmExecutionLease {
        expected_owner_version: r.owner_cas.version,
        fence,
        lease_id: lease.lease.lease_id,
        expected_lease_version: lease.lease.version,
        session_run_id: Uuid::new_v4(),
        barrier_id: Uuid::new_v4(),
        idempotency_key: "original-recovery".into(),
    };
    let witness = UnsubmittedPmWitness {
        contract_version: 1,
        session_run_id: c.session_run_id,
        barrier_id: c.barrier_id,
        lease_id: c.lease_id,
        lease_version: c.expected_lease_version,
        owner_version: c.expected_owner_version,
        identity: PmExecutionIdentity {
            task: r.execution.key.clone(),
            execution_ref: r.assignment.execution_id.to_string(),
            tracker_instance_ref: r.binding.tracker_instance_id.clone(),
            tracker_project_ref: r.binding.project_id.to_string(),
            task_ref: task.to_string(),
            root_ref: task.to_string(),
            agent_ref: r.assignment.agent_id.to_string(),
            assignment_operation_key: r.assignment_operation_key.clone(),
            assignment_ref: r.assignment.assignment_id.to_string(),
            assignment_revision: r.assignment.version,
        },
    };
    InitialPm {
        task,
        pm,
        reservation: r,
        claim,
        claim_hash,
        heartbeat_hash,
        lease,
        command: c,
        witness,
    }
}

async fn original_operations(f: &Fixture, p: &InitialPm) -> Json {
    let mut operations = Vec::new();
    for (key, hash, expected) in [
        ("original-claim", &p.claim_hash, &p.claim),
        ("original-heartbeat", &p.heartbeat_hash, &p.lease),
    ] {
        let operation = f
            .repo
            .execution_lease(p.task, &p.pm, Some(key))
            .await
            .unwrap()
            .operation
            .unwrap();
        assert_eq!(operation.idempotency_key, key);
        assert_eq!(&operation.request_sha256, hash);
        assert_eq!(
            serde_json::to_value(&operation.result).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        operations.push(serde_json::to_value(operation).unwrap());
    }
    json!(operations)
}

async fn upgrade_snapshot(f: &Fixture, p: &InitialPm) -> Json {
    // Only the additive nullable column is normalized; every original row field is retained.
    f.db.query_one(statement("SELECT jsonb_build_object('task',to_jsonb(t),
        'issue',(SELECT to_jsonb(i) FROM issues i WHERE i.id=t.task_id),
        'reservation',(SELECT to_jsonb(e) FROM sdlc_pm_executions e WHERE execution_id=$2),
        'assignments',(SELECT jsonb_agg(to_jsonb(a) ORDER BY version) FROM sdlc_assignments a WHERE task_id=$1),
        'lease',(SELECT to_jsonb(l) FROM sdlc_pm_execution_leases l WHERE execution_id=$2),
        'operations',(SELECT jsonb_agg(to_jsonb(o)-'recovery' ORDER BY lease_version) FROM sdlc_pm_lease_operations o WHERE execution_id=$2),
        'outbox',(SELECT jsonb_agg(to_jsonb(o) ORDER BY sequence) FROM sdlc_outbox o WHERE task_id=$1)) AS v FROM sdlc_tasks t WHERE task_id=$1",
        vec![p.task.into(),p.command.fence.execution_id.into()])).await.unwrap().unwrap().try_get("","v").unwrap()
}

async fn migration_versions(f: &Fixture) -> Vec<String> {
    f.db.query_all(statement(
        "SELECT version FROM seaql_migrations ORDER BY version",
        vec![],
    ))
    .await
    .unwrap()
    .iter()
    .map(|row| row.try_get("", "version").unwrap())
    .collect()
}

async fn snapshot(f: &Fixture, p: &InitialPm) -> Json {
    f.db.query_one(statement("SELECT jsonb_build_object('state',state,
        'lease',(SELECT result FROM sdlc_pm_execution_leases WHERE execution_id=$2),
        'operations',(SELECT jsonb_agg(to_jsonb(o) ORDER BY lease_version) FROM sdlc_pm_lease_operations o WHERE execution_id=$2),
        'outbox',(SELECT count(*) FROM sdlc_outbox WHERE task_id=$1)) AS v FROM sdlc_tasks WHERE task_id=$1",
        vec![p.task.into(),p.command.fence.execution_id.into()])).await.unwrap().unwrap().try_get("","v").unwrap()
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending in _candidates_test; populated 0097->0098 upgrade and one real lease TTL"]
async fn unsubmitted_pm_recovery_cas_witness_replay_and_history() {
    crate::legacy_schema_fixture::scope(
        unsubmitted_pm_recovery_cas_witness_replay_and_history_seed(),
    )
    .await;
}

async fn unsubmitted_pm_recovery_cas_witness_replay_and_history_seed() {
    let migrations = migration::Migrator::migrations();
    let recovery_index = migrations
        .iter()
        .position(|m| m.name() == "m20261018_0000098_pm_unsubmitted_recovery")
        .unwrap();
    assert_eq!(recovery_index, 28);
    assert_eq!(
        migrations[recovery_index - 1].name(),
        "m20261017_0000097_task_source_binding"
    );
    let f = Box::pin(Fixture::new_with_migration_steps(Some(
        recovery_index.try_into().unwrap(),
    )))
    .await;
    let prior_versions = migration_versions(&f).await;
    assert_eq!(prior_versions.len(), recovery_index);
    assert_eq!(
        prior_versions.last().unwrap(),
        "m20261017_0000097_task_source_binding"
    );
    let column = f.db.query_one(statement("SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_schema='public' AND table_name='sdlc_pm_lease_operations' AND column_name='recovery') AS present",vec![])).await.unwrap().unwrap();
    assert!(!column.try_get::<bool>("", "present").unwrap());
    let p = initial(&f, "unsubmitted-recovery").await;
    let q = initial(&f, "different-key-race").await;
    let mut preserved = Vec::new();
    for pm in [&p, &q] {
        let saved = upgrade_snapshot(&f, pm).await;
        assert_eq!(saved["lease"]["version"], json!(2));
        assert_eq!(saved["operations"].as_array().unwrap().len(), 2);
        assert_eq!(saved["operations"][0]["lease_version"], json!(1));
        assert_eq!(saved["operations"][1]["lease_version"], json!(2));
        preserved.push((saved, original_operations(&f, pm).await));
    }
    migration::Migrator::up(&f.db, Some(1)).await.unwrap();
    let upgraded_versions = migration_versions(&f).await;
    assert_eq!(
        &upgraded_versions[..recovery_index],
        prior_versions.as_slice()
    );
    assert_eq!(upgraded_versions.len(), recovery_index + 1);
    assert_eq!(
        upgraded_versions.last().unwrap(),
        "m20261018_0000098_pm_unsubmitted_recovery"
    );
    for (pm, (saved, operations)) in [&p, &q].into_iter().zip(&preserved) {
        assert_eq!(&upgrade_snapshot(&f, pm).await, saved);
        assert_eq!(&original_operations(&f, pm).await, operations);
    }
    let counts = f.db.query_one(statement("SELECT (SELECT count(*) FROM sdlc_pm_execution_leases) AS leases,
        count(*) AS operations,count(*) FILTER (WHERE recovery IS NOT NULL) AS recovery_operations FROM sdlc_pm_lease_operations",vec![])).await.unwrap().unwrap();
    assert_eq!(counts.try_get::<i64>("", "leases").unwrap(), 2);
    assert_eq!(counts.try_get::<i64>("", "operations").unwrap(), 4);
    assert_eq!(counts.try_get::<i64>("", "recovery_operations").unwrap(), 0);
    let error = migration::Migrator::down(&f.db, Some(1)).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("PM recovery history is immutable; destructive rollback is disabled")
    );
    migration::Migrator::up(&f.db, Some(1)).await.unwrap();
    let catalog_versions = migration_versions(&f).await;
    assert_eq!(
        &catalog_versions[..upgraded_versions.len()],
        upgraded_versions.as_slice()
    );
    assert_eq!(catalog_versions.len(), recovery_index + 2);
    assert_eq!(
        catalog_versions.last().unwrap(),
        "m20261018_0000098_project_repository_catalog"
    );
    migration::Migrator::up(&f.db, None).await.unwrap();
    let mut final_versions = catalog_versions;
    final_versions.extend(
        [
            "m20261021_0000099_pm_bootstrap_retry",
            "m20261019_0000099_unified_spaces",
            "m20261021_0000100_human_sdlc_spaces",
            "m20261021_0000101_native_sdlc_spaces",
            "m20261022_0000102_request_commit_clock",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    // The SQL ledger is ordered by version; registration remains append-only.
    final_versions.sort();
    assert_eq!(migration_versions(&f).await, final_versions);
    for (pm, (saved, operations)) in [&p, &q].into_iter().zip(&preserved) {
        assert_eq!(&upgrade_snapshot(&f, pm).await, saved);
        assert_eq!(&original_operations(&f, pm).await, operations);
    }
    let before = snapshot(&f, &p).await;
    assert!(
        f.repo
            .preflight_unsubmitted_pm_recovery(p.task, &f.writer, &p.command)
            .await
            .is_err()
    );
    assert_eq!(snapshot(&f, &p).await, before); // active lease must use heartbeat
    // No raw timestamp rewrite, disabled trigger or synthetic terminal to expire a lease.
    f.db.execute_unprepared("SELECT pg_sleep(31)")
        .await
        .unwrap();
    let expired = snapshot(&f, &p).await;
    assert!(
        f.repo
            .preflight_unsubmitted_pm_recovery(p.task, &f.writer, &p.command)
            .await
            .unwrap()
            .is_none()
    );
    let foreign = actor("foreign", false, &["task-tracker:write"]);
    for denied in [&p.pm, &f.owner, &foreign, &f.reader] {
        assert!(
            f.repo
                .preflight_unsubmitted_pm_recovery(p.task, denied, &p.command)
                .await
                .is_err()
        );
    }
    let mut stale = p.command.clone();
    stale.fence.assignment_version += 1;
    assert!(
        f.repo
            .preflight_unsubmitted_pm_recovery(p.task, &f.writer, &stale)
            .await
            .is_err()
    );
    for field in [
        "barrier_id",
        "session_run_id",
        "lease_id",
        "owner_version",
        "identity",
    ] {
        let mut value = serde_json::to_value(&p.witness).unwrap();
        match field {
            "identity" => value["identity"]["assignment_operation_key"] = json!("foreign"),
            "owner_version" => value[field] = json!(2),
            _ => value[field] = json!(Uuid::new_v4()),
        };
        assert!(
            f.repo
                .recover_unsubmitted_pm_execution_lease(
                    p.task,
                    &f.writer,
                    p.command.clone(),
                    parse(value)
                )
                .await
                .is_err()
        );
        assert_eq!(snapshot(&f, &p).await, expired);
    }
    exec(&f.db,"DELETE FROM project_members WHERE project_id=$1 AND user_id IN (SELECT id FROM users WHERE central_sub='pm')",vec![f.project.into()]).await.unwrap();
    assert!(
        f.repo
            .preflight_unsubmitted_pm_recovery(p.task, &f.writer, &p.command)
            .await
            .is_err()
    );
    exec(&f.db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub='pm'",vec![f.project.into()]).await.unwrap();
    // Updating expired lease directly remains prohibited without this transaction's evidence.
    let tx = f.db.begin().await.unwrap();
    let now: chrono::DateTime<chrono::Utc> = tx
        .query_one(statement("SELECT clock_timestamp() AS now", vec![]))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "now")
        .unwrap();
    let mut raw = p.lease.clone();
    raw.lease.version += 1;
    raw.lease.heartbeat_at = now;
    raw.lease.expires_at = now + chrono::Duration::seconds(30);
    let error = tx.execute(statement("UPDATE sdlc_pm_execution_leases SET version=$2,heartbeat_at=$3,expires_at=$4,result=$5 WHERE execution_id=$1",
        vec![p.command.fence.execution_id.into(),raw.lease.version.into(),now.into(),raw.lease.expires_at.into(),json_value(&raw).unwrap()])).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("SDLC expired ownership lease requires exact trusted resume transaction")
    );
    tx.rollback().await.unwrap();
    let (a, b) = tokio::join!(
        f.repo.recover_unsubmitted_pm_execution_lease(
            p.task,
            &f.writer,
            p.command.clone(),
            p.witness.clone()
        ),
        f.repo.recover_unsubmitted_pm_execution_lease(
            p.task,
            &f.writer,
            p.command.clone(),
            p.witness.clone()
        )
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
    assert_eq!(
        a.0.lease.expires_at - a.0.lease.heartbeat_at,
        chrono::Duration::seconds(30)
    );
    let after = snapshot(&f, &p).await;
    // Lost ACK/restart reads the original immutable operation without another Fleet witness.
    let reloaded = PostgresSdlcRepository::connect(
        &std::env::var("TT_CANDIDATES_TEST_DATABASE_URL").unwrap(),
        f.repo.config.clone(),
    )
    .await
    .unwrap();
    let replay = reloaded
        .unsubmitted_pm_recovery_operation(p.task, &f.reader, &p.command.idempotency_key)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(replay.result).unwrap(),
        serde_json::to_value(&a.0).unwrap()
    );
    assert_eq!(snapshot(&f, &p).await, after);
    assert_eq!(original_operations(&f, &p).await, preserved[0].1);
    let mut changed = p.command.clone();
    changed.barrier_id = Uuid::new_v4();
    assert!(
        f.repo
            .preflight_unsubmitted_pm_recovery(p.task, &f.writer, &changed)
            .await
            .is_err()
    );
    let mut other = q.command.clone();
    other.idempotency_key = "second-key".into();
    let (a, b) = tokio::join!(
        f.repo.recover_unsubmitted_pm_execution_lease(
            q.task,
            &f.writer,
            q.command.clone(),
            q.witness.clone()
        ),
        f.repo
            .recover_unsubmitted_pm_execution_lease(q.task, &f.writer, other, q.witness.clone())
    );
    assert_ne!(a.is_ok(), b.is_ok()); // exactly one CAS, different keys cannot create another lease
    assert_eq!(original_operations(&f, &q).await, preserved[1].1);
    assert_eq!(
        p.reservation.assignment.execution_id,
        p.command.fence.execution_id
    );
}
