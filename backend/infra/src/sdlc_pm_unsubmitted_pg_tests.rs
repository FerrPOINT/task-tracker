//! Disposable PostgreSQL only; Fleet witness below is explicitly synthetic.
use super::*;
use domain::sdlc_execution_lease::*;

struct InitialPm {
    task: Uuid,
    pm: Principal,
    reservation: PmDraftReservation,
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
    let (lease, _) = f
        .repo
        .claim_execution_lease(
            task,
            &pm,
            ClaimExecutionLease {
                expected_owner_version: r.owner_cas.version,
                fence: fence.clone(),
                idempotency_key: "original-claim".into(),
            },
        )
        .await
        .unwrap();
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
        lease,
        command: c,
        witness,
    }
}

async fn snapshot(f: &Fixture, p: &InitialPm) -> Json {
    f.db.query_one(statement("SELECT jsonb_build_object('state',state,
        'lease',(SELECT result FROM sdlc_pm_execution_leases WHERE execution_id=$2),
        'operations',(SELECT jsonb_agg(to_jsonb(o) ORDER BY lease_version) FROM sdlc_pm_lease_operations o WHERE execution_id=$2),
        'outbox',(SELECT count(*) FROM sdlc_outbox WHERE task_id=$1)) AS v FROM sdlc_tasks WHERE task_id=$1",
        vec![p.task.into(),p.command.fence.execution_id.into()])).await.unwrap().unwrap().try_get("","v").unwrap()
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending in _candidates_test; waits one real lease TTL"]
async fn unsubmitted_pm_recovery_cas_witness_replay_and_history() {
    let f = Box::pin(Fixture::new()).await;
    let p = initial(&f, "unsubmitted-recovery").await;
    let q = initial(&f, "different-key-race").await;
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
    let claim = f
        .repo
        .execution_lease(p.task, &p.pm, Some("original-claim"))
        .await
        .unwrap()
        .operation
        .unwrap();
    assert_eq!(
        serde_json::to_value(claim.result).unwrap(),
        serde_json::to_value(&p.lease).unwrap()
    );
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
    assert_eq!(
        p.reservation.assignment.execution_id,
        p.command.fence.execution_id
    );
}
