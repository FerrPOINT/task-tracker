//! Synthetic Fleet barriers on a disposable PostgreSQL database, not live authority.
use super::*;

struct Fixture {
    db: DatabaseConnection,
    repo: PostgresSdlcRepository,
    task: Uuid,
    scheduler: Principal,
    pm: Principal,
    reservation: PmDraftReservation,
    original: ResumePmExecutionLease,
    original_receipt: ExecutionLeaseReceipt,
}

async fn fixture() -> Fixture {
    let url =
        std::env::var("TT_SETTLED_INPUT_TEST_DATABASE_URL").expect("own disposable PG required");
    let db = Database::connect(&url).await.unwrap();
    let row=db.query_one(statement("SELECT current_database() AS name,to_regclass('public.seaql_migrations') IS NULL AS fresh",vec![])).await.unwrap().unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_settled_input_test")
    );
    assert!(row.try_get::<bool>("", "fresh").unwrap());
    let versions = migration::Migrator::migrations();
    // Registration is integrator-owned. Stop before it, then apply this artifact once.
    let predecessor = versions
        .iter()
        .position(|m| m.name().ends_with("_pm_settled_input_recovery"))
        .unwrap_or(versions.len());
    migration::Migrator::up(&db, Some(predecessor as u32))
        .await
        .unwrap();
    let project = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    for (subject, id) in [
        ("owner", owner_id),
        ("pm", Uuid::new_v4()),
        ("scheduler", Uuid::new_v4()),
        ("verifier", Uuid::new_v4()),
    ] {
        exec(
            &db,
            "INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active)
            VALUES($1,$2,$3,$3,'!',$3,true)",
            vec![
                id.into(),
                format!("{subject}@example.test").into(),
                subject.into(),
            ],
        )
        .await
        .unwrap();
    }
    exec(&db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,'SET','Settled input fixture',$2,$3)",
        vec![project.into(),owner_id.into(),Uuid::new_v4().into()]).await.unwrap();
    exec(&db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Settled','[]' FROM projects WHERE id=$1",vec![project.into()]).await.unwrap();
    exec(&db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub IN ('pm','scheduler','verifier')",vec![project.into()]).await.unwrap();
    let repo = PostgresSdlcRepository::connect(
        &url,
        SdlcConfig {
            instance_id: "settled-input-fixture".into(),
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
    let routes = super::super::draft_flow_tests::single_task_routes();
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
                flow: Some(DraftFlow::SingleTaskV1),
                title: "Settled input".into(),
                description: "Synthetic saved feedback".into(),
                idempotency_key: "draft".into(),
            },
        )
        .await
        .unwrap();
    let task = draft.task_id;
    let (reservation,_)=repo.reserve_pm_draft(task,&owner,parse(json!({"requested_agent_id":routes.project_manager.agent_id,
        "expected_owner_version":0,"expected_assignment_version":null,"idempotency_key":"reserve"}))).await.unwrap();
    let pm = actor(
        "pm",
        false,
        vec![
            "task-tracker:read".into(),
            reservation.assignment.scope(task),
        ],
    );
    let fence: MachineFence = parse(json!({"assignment_id":reservation.assignment.assignment_id,
        "execution_id":reservation.assignment.execution_id,"agent_id":reservation.assignment.agent_id,"assignment_version":1}));
    let (initial, _) = repo
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
                fence,
                text: "Saved owner brainstorming feedback".into(),
                idempotency_key: "feedback".into(),
            },
        )
        .await
        .unwrap();
    let original = resume(
        &initial,
        PmResumeInput::Feedback {
            input_id: feedback.message_id,
        },
        Uuid::new_v4(),
        1,
        None,
        "original-resume",
    );
    let (original_receipt, _) = repo
        .resume_pm_execution_lease(task, &scheduler, original.clone())
        .await
        .unwrap();
    let guard_sql="SELECT jsonb_object_agg(proname,pg_get_functiondef(oid)) AS value FROM pg_proc WHERE oid IN (
        'sdlc_pm_lease_gate()'::regprocedure,'sdlc_pm_unsubmitted_recovery_gate()'::regprocedure,
        'sdlc_pm_bootstrap_retry_gate()'::regprocedure,'sdlc_pm_unsubmitted_recovery_commit_gate()'::regprocedure)";
    let guard_before: Json = db
        .query_one(statement(guard_sql, vec![]))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "value")
        .unwrap();
    db.execute_unprepared(include_str!(
        "../../migration/src/pm_settled_input_recovery.sql"
    ))
    .await
    .unwrap();
    let guard_after: Json = db
        .query_one(statement(guard_sql, vec![]))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "value")
        .unwrap();
    assert_eq!(guard_after, guard_before);
    Fixture {
        db,
        repo,
        task,
        scheduler,
        pm,
        reservation,
        original,
        original_receipt,
    }
}

fn recovery(
    f: &Fixture,
    current: &ExecutionLeaseReceipt,
    key: &str,
    previous: &ExecutionLeaseOperation,
) -> ResumePmExecutionLease {
    let mut command = f.original.clone();
    command.idempotency_key = key.into();
    command.expected_lease_version = current.lease.version;
    command.recovery = Some(PmSettledInputRecovery {
        kind: PmSettledInputRecoveryKind::SettledInputExpiry,
        original_key: f.original.idempotency_key.clone(),
        original_request_sha256: app::sdlc_pm_resume::request_hash(&f.original).unwrap(),
        predecessor_key: previous.idempotency_key.clone(),
        predecessor_request_sha256: previous.request_sha256.clone(),
        barrier_id: Uuid::new_v4(),
    });
    command
}

fn witness(f: &Fixture, c: &ResumePmExecutionLease) -> PmSettledInputRecoveryWitness {
    PmSettledInputRecoveryWitness {
        contract_version: 1,
        session_run_id: c.previous_run_id,
        barrier_id: c.recovery.as_ref().unwrap().barrier_id,
        identity: app::sdlc_pm_bootstrap_retry::identity(&f.reservation),
        request_sha256: app::sdlc_pm_resume::request_hash(c).unwrap(),
        command: c.clone(),
        manual_budget: None,
    }
}

fn manual_witness(f: &Fixture, c: &ResumePmExecutionLease) -> PmSettledInputRecoveryWitness {
    let mut w = witness(f, c);
    w.manual_budget = Some(PmSettledInputManualBudget {
        contract_version: 1,
        root_operation_id: Uuid::from_u128(100),
        control_operation_id: Uuid::from_u128(101),
        retry_operation_key: format!("fleet-pm-resume-retry:{}", "a".repeat(64)),
        retry_request_sha256: "b".repeat(64),
        original_resume_request_sha256: c
            .recovery
            .as_ref()
            .unwrap()
            .original_request_sha256
            .clone(),
        prefix_count: 3,
        prefix_sha256: "c".repeat(64),
        allowance: 3,
    });
    w
}

async fn original_history(f: &Fixture) -> Json {
    f.db.query_one(statement("SELECT jsonb_build_object('inputs',(SELECT jsonb_agg(to_jsonb(i) ORDER BY lease_version) FROM sdlc_pm_resume_inputs i WHERE task_id=$1),
        'feedback',(SELECT jsonb_agg(to_jsonb(b) ORDER BY message_id) FROM sdlc_pm_feedback b WHERE task_id=$1),
        'messages',(SELECT count(*) FROM sdlc_outbox WHERE task_id=$1)) AS value",vec![f.task.into()])).await.unwrap().unwrap().try_get("","value").unwrap()
}

async fn expire(f: &Fixture) {
    f.db.execute_unprepared("SELECT pg_sleep(31)")
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_SETTLED_INPUT_TEST_DATABASE_URL ending _settled_input_test; real TTL waits"]
async fn settled_input_recovery_expiry_lost_ack_chain_cas_and_immutable_history() {
    let f = fixture().await;
    let mut reader = f.scheduler.clone();
    reader.scopes = ["task-tracker:read".into()].into_iter().collect();
    let original = f
        .repo
        .pm_resume_operation(f.task, &reader, &f.original.idempotency_key)
        .await
        .unwrap();
    let history = original_history(&f).await;
    let initial = recovery(&f, &f.original_receipt, "recovery-1", &original);
    assert!(
        f.repo
            .preflight_settled_input_recovery(f.task, &f.scheduler, &initial)
            .await
            .is_err()
    ); // Active never renews via recovery.
    expire(&f).await;
    assert!(
        f.repo
            .heartbeat_execution_lease(
                f.task,
                &f.pm,
                HeartbeatExecutionLease {
                    expected_owner_version: 1,
                    fence: f.original_receipt.fence.clone(),
                    lease_id: f.original_receipt.lease.lease_id,
                    expected_lease_version: f.original_receipt.lease.version,
                    idempotency_key: "expired-heartbeat".into(),
                }
            )
            .await
            .is_err()
    );
    let before = saved_lease(&f.db, f.reservation.assignment.execution_id).await;
    let tx = f.db.begin().await.unwrap();
    let collision_error=tx.execute(statement("INSERT INTO sdlc_pm_lease_operations(execution_id,actor_subject,idempotency_key,payload_hash,lease_version,result)
        SELECT execution_id,holder_subject,$2,repeat('a',64),version,result FROM sdlc_pm_execution_leases WHERE execution_id=$1",
        vec![f.reservation.assignment.execution_id.into(),f.original.idempotency_key.clone().into()])).await.unwrap_err();
    assert!(
        collision_error
            .to_string()
            .contains("PM resume key already exists"),
        "{collision_error}"
    );
    tx.rollback().await.unwrap();
    let tx = f.db.begin().await.unwrap();
    let reverse_collision=tx.execute(statement("INSERT INTO sdlc_pm_resume_inputs(task_id,execution_id,actor_subject,idempotency_key,payload_hash,
        input_kind,input_id,checkpoint_id,checkpoint_revision,previous_run_id,lease_version,transaction_id,command,result)
        SELECT task_id,execution_id,actor_subject,'claim',payload_hash,input_kind,input_id,checkpoint_id,checkpoint_revision,
        previous_run_id,lease_version,pg_current_xact_id(),jsonb_set(command,'{idempotency_key}','\"claim\"'),result
        FROM sdlc_pm_resume_inputs WHERE task_id=$1 AND idempotency_key=$2",
        vec![f.task.into(),f.original.idempotency_key.clone().into()])).await.unwrap_err();
    assert!(
        reverse_collision
            .to_string()
            .contains("PM resume key already exists"),
        "{reverse_collision}"
    );
    tx.rollback().await.unwrap();
    let tx = f.db.begin().await.unwrap();
    let expiry_error=tx.execute(statement("WITH instant AS (SELECT clock_timestamp() AS v) UPDATE sdlc_pm_execution_leases l
        SET version=l.version+1,heartbeat_at=instant.v,expires_at=instant.v+interval '30 seconds',
        result=jsonb_set(l.result,'{lease}',l.result->'lease'||jsonb_build_object('version',l.version+1,
            'heartbeat_at',instant.v,'expires_at',instant.v+interval '30 seconds'))
        FROM instant WHERE l.execution_id=$1",vec![f.reservation.assignment.execution_id.into()])).await.unwrap_err();
    assert!(
        expiry_error
            .to_string()
            .contains("expired ownership lease requires exact trusted resume transaction"),
        "{expiry_error}"
    );
    tx.rollback().await.unwrap();
    assert!(
        f.repo
            .preflight_settled_input_recovery(f.task, &f.pm, &initial)
            .await
            .is_err()
    );
    let mut stale = initial.clone();
    stale.expected_lease_version -= 1;
    assert!(
        f.repo
            .preflight_settled_input_recovery(f.task, &f.scheduler, &stale)
            .await
            .is_err()
    );
    let mut collision = initial.clone();
    collision.idempotency_key = "claim".into();
    assert!(
        f.repo
            .preflight_settled_input_recovery(f.task, &f.scheduler, &collision)
            .await
            .is_err()
    );
    exec(&f.db,"DELETE FROM project_members WHERE project_id=$1 AND user_id=(SELECT id FROM users WHERE central_sub='pm')",
        vec![f.reservation.binding.project_id.into()]).await.unwrap();
    assert!(
        f.repo
            .preflight_settled_input_recovery(f.task, &f.scheduler, &initial)
            .await
            .is_err()
    );
    exec(&f.db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub='pm'",
        vec![f.reservation.binding.project_id.into()]).await.unwrap();
    for pointer in [
        "/barrier_id",
        "/session_run_id",
        "/identity/agent_ref",
        "/request_sha256",
    ] {
        let mut raw = serde_json::to_value(witness(&f, &initial)).unwrap();
        *raw.pointer_mut(pointer).unwrap() = if pointer == "/request_sha256" {
            json!("f".repeat(64))
        } else {
            json!(Uuid::new_v4().to_string())
        };
        assert!(
            f.repo
                .recover_settled_input_lease(f.task, &f.scheduler, initial.clone(), parse(raw))
                .await
                .is_err()
        );
        assert_eq!(
            saved_lease(&f.db, f.reservation.assignment.execution_id).await,
            before
        );
    }
    let mut duplicate = f.original.clone();
    duplicate.idempotency_key = "second-input-consumption".into();
    duplicate.expected_lease_version = f.original_receipt.lease.version;
    assert!(
        f.repo
            .resume_pm_execution_lease(f.task, &f.scheduler, duplicate)
            .await
            .is_err()
    );
    for pointer in [
        "/input/input_id",
        "/checkpoint/checkpoint_sha256",
        "/recovery/original_request_sha256",
        "/recovery/predecessor_request_sha256",
    ] {
        let mut raw = serde_json::to_value(&initial).unwrap();
        *raw.pointer_mut(pointer).unwrap() = if pointer == "/input/input_id" {
            json!(Uuid::new_v4())
        } else {
            json!("e".repeat(64))
        };
        let changed: ResumePmExecutionLease = parse(raw);
        assert!(
            f.repo
                .recover_settled_input_lease(
                    f.task,
                    &f.scheduler,
                    changed.clone(),
                    witness(&f, &changed)
                )
                .await
                .is_err()
        );
    }
    f.db.execute_unprepared("CREATE FUNCTION fixture_settled_fail_update() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture rollback'; END $$;
        CREATE TRIGGER fixture_settled_fail_update BEFORE UPDATE ON sdlc_pm_execution_leases FOR EACH ROW EXECUTE FUNCTION fixture_settled_fail_update();").await.unwrap();
    assert!(
        f.repo
            .recover_settled_input_lease(
                f.task,
                &f.scheduler,
                initial.clone(),
                witness(&f, &initial)
            )
            .await
            .is_err()
    );
    assert_eq!(count(&f.db, "sdlc_pm_lease_operations").await, 1); // Only original claim; recovery insert rolled back.
    f.db.execute_unprepared("DROP TRIGGER fixture_settled_fail_update ON sdlc_pm_execution_leases; DROP FUNCTION fixture_settled_fail_update();").await.unwrap();
    let (first, replay) = f
        .repo
        .recover_settled_input_lease(f.task, &f.scheduler, initial.clone(), witness(&f, &initial))
        .await
        .unwrap();
    assert!(!replay);
    assert_eq!(first.lease.version, f.original_receipt.lease.version + 1);
    assert_eq!(first.lease.lease_id, f.original_receipt.lease.lease_id);
    assert_eq!(first.lease.claimed_at, f.original_receipt.lease.claimed_at);
    assert!(!first.dispatch_allowed);
    let retained = f
        .repo
        .pm_resume_operation(f.task, &reader, &initial.idempotency_key)
        .await
        .unwrap();
    assert_eq!(
        json_value(&retained.result).unwrap(),
        json_value(&first).unwrap()
    );
    let mut changed = initial.clone();
    changed.checkpoint.checkpoint_sha256 = "d".repeat(64);
    assert!(
        f.repo
            .preflight_settled_input_recovery(f.task, &f.scheduler, &changed)
            .await
            .is_err()
    );
    expire(&f).await; // Lost ACK can be recovered after the accepted receipt is already historical.
    assert_eq!(
        json_value(
            &f.repo
                .preflight_settled_input_recovery(f.task, &f.scheduler, &initial)
                .await
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        json_value(&first).unwrap()
    );
    let second = recovery(&f, &first, "recovery-2", &retained);
    let fork = recovery(&f, &first, "stale-predecessor", &original);
    assert!(
        f.repo
            .preflight_settled_input_recovery(f.task, &f.scheduler, &fork)
            .await
            .is_err()
    );
    let (a, b) = tokio::join!(
        f.repo.recover_settled_input_lease(
            f.task,
            &f.scheduler,
            second.clone(),
            witness(&f, &second)
        ),
        f.repo.recover_settled_input_lease(
            f.task,
            &f.scheduler,
            second.clone(),
            witness(&f, &second)
        )
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_ne!(a.1, b.1);
    assert_eq!(json_value(&a.0).unwrap(), json_value(&b.0).unwrap());
    let second_result = f
        .repo
        .pm_resume_operation(f.task, &reader, &second.idempotency_key)
        .await
        .unwrap();
    expire(&f).await;
    let third = recovery(&f, &a.0, "recovery-3", &second_result);
    let (third_receipt, _) = f
        .repo
        .recover_settled_input_lease(f.task, &f.scheduler, third.clone(), witness(&f, &third))
        .await
        .unwrap();
    let third_result = f
        .repo
        .pm_resume_operation(f.task, &reader, &third.idempotency_key)
        .await
        .unwrap();
    expire(&f).await;
    let fourth = recovery(&f, &third_receipt, "recovery-4", &third_result);
    // Preflight beyond three only permits reading Fleet, never lease issuance.
    assert!(
        f.repo
            .preflight_settled_input_recovery(f.task, &f.scheduler, &fourth)
            .await
            .unwrap()
            .is_none()
    );
    let before_fourth = saved_lease(&f.db, f.reservation.assignment.execution_id).await;
    assert!(
        f.repo
            .recover_settled_input_lease(f.task, &f.scheduler, fourth.clone(), witness(&f, &fourth))
            .await
            .is_err()
    );
    assert_eq!(original_history(&f).await, history);
    assert_eq!(
        json_value(
            &f.repo
                .pm_resume_operation(f.task, &reader, &f.original.idempotency_key)
                .await
                .unwrap()
        )
        .unwrap(),
        json_value(&original).unwrap()
    );
    assert_eq!(count(&f.db, "sdlc_pm_resume_inputs").await, 1);
    assert_eq!(count(&f.db, "sdlc_pm_lease_operations").await, 4);

    let fourth_witness = manual_witness(&f, &fourth);
    // Same genuine history/request: legacy SQL103 rejects the new permission.
    assert!(
        f.repo
            .recover_settled_input_lease(
                f.task,
                &f.scheduler,
                fourth.clone(),
                fourth_witness.clone()
            )
            .await
            .is_err()
    );
    assert_eq!(
        saved_lease(&f.db, f.reservation.assignment.execution_id).await,
        before_fourth
    );
    assert_eq!(count(&f.db, "sdlc_pm_lease_operations").await, 4);
    let gate_sql = "SELECT oid::bigint AS id,pg_get_functiondef(oid) AS body FROM pg_proc
        WHERE oid='sdlc_pm_settled_input_recovery_gate()'::regprocedure";
    let old_gate =
        f.db.query_one(statement(gate_sql, vec![]))
            .await
            .unwrap()
            .unwrap();
    f.db.execute_unprepared(include_str!(
        "../../migration/src/pm_manual_recovery_budget.sql"
    ))
    .await
    .unwrap();
    let new_gate =
        f.db.query_one(statement(gate_sql, vec![]))
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        old_gate.try_get::<i64>("", "id").unwrap(),
        new_gate.try_get::<i64>("", "id").unwrap()
    );
    assert_eq!(new_gate.try_get::<String>("", "body").unwrap(),
        old_gate.try_get::<String>("", "body").unwrap()
            .replace("AND NEW.idempotency_key<>predecessor_key AND attempts<3",
                     "AND NEW.idempotency_key<>predecessor_key AND sdlc_pm_settled_manual_budget_allows(w,c,attempts)")
            .replace("AND w-ARRAY['contract_version','session_run_id','barrier_id','identity','request_sha256','command']='{}'::jsonb",
                     "AND w-ARRAY['contract_version','session_run_id','barrier_id','identity','request_sha256','command','manual_budget']='{}'::jsonb"));
    let raw_witness = json_value(&fourth_witness).unwrap();
    let raw_command = json_value(&fourth).unwrap();
    for position in 0i64..7 {
        let row =
            f.db.query_one(statement(
                "SELECT sdlc_pm_settled_manual_budget_allows($1,$3,$4) AS manual,
                    sdlc_pm_settled_manual_budget_allows($2,$3,$4) AS legacy",
                vec![
                    raw_witness.clone().into(),
                    json_value(&witness(&f, &fourth)).unwrap().into(),
                    raw_command.clone().into(),
                    position.into(),
                ],
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.try_get::<bool>("", "manual").unwrap(),
            (3..6).contains(&position)
        );
        assert_eq!(row.try_get::<bool>("", "legacy").unwrap(), position < 3);
    }
    for (field, bad) in [
        ("original_resume_request_sha256", json!("f".repeat(64))),
        ("allowance", json!(4)),
        ("prefix_count", json!(4)),
        ("prefix_count", json!(3.5)),
        ("prefix_count", json!("3")),
        ("contract_version", json!(1.0)),
        ("root_operation_id", json!(Uuid::nil())),
        (
            "control_operation_id",
            json!("00000000-0000-0000-0000-00000000000A"),
        ),
        ("prefix_sha256", json!("bad")),
        ("unexpected", json!(true)),
    ] {
        let mut changed = raw_witness.clone();
        changed["manual_budget"][field] = bad;
        let row =
            f.db.query_one(statement(
                "SELECT sdlc_pm_settled_manual_budget_allows($1,$2,3) AS allowed",
                vec![changed.clone().into(), raw_command.clone().into()],
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(!row.try_get::<bool>("", "allowed").unwrap(), "SQL {field}");
        if let Ok(changed) = serde_json::from_value(changed) {
            assert!(
                f.repo
                    .recover_settled_input_lease(f.task, &f.scheduler, fourth.clone(), changed)
                    .await
                    .is_err(),
                "Rust {field}"
            );
        }
    }
    let mut null_proof = raw_witness.clone();
    null_proof["manual_budget"] = Json::Null;
    let row =
        f.db.query_one(statement(
            "SELECT sdlc_pm_settled_manual_budget_allows($1,$2,3) AS allowed",
            vec![null_proof.into(), raw_command.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(!row.try_get::<bool>("", "allowed").unwrap());
    assert_eq!(
        saved_lease(&f.db, f.reservation.assignment.execution_id).await,
        before_fourth
    );
    let (a, b) = tokio::join!(
        f.repo.recover_settled_input_lease(
            f.task,
            &f.scheduler,
            fourth.clone(),
            fourth_witness.clone()
        ),
        f.repo.recover_settled_input_lease(
            f.task,
            &f.scheduler,
            fourth.clone(),
            fourth_witness.clone()
        )
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_ne!(a.1, b.1);
    assert_eq!(json_value(&a.0).unwrap(), json_value(&b.0).unwrap());
    assert_eq!(a.0.lease.version, third_receipt.lease.version + 1);
    assert_eq!(a.0.lease.claimed_at, third_receipt.lease.claimed_at);
    assert!(!a.0.dispatch_allowed);
    assert_eq!(count(&f.db, "sdlc_pm_lease_operations").await, 5);
    let mut current = a.0;
    for position in 4..=6 {
        expire(&f).await;
        let predecessor_key = format!("recovery-{position}");
        let predecessor = f
            .repo
            .pm_resume_operation(f.task, &reader, &predecessor_key)
            .await
            .unwrap();
        let command = recovery(
            &f,
            &current,
            &format!("recovery-{}", position + 1),
            &predecessor,
        );
        let before = saved_lease(&f.db, f.reservation.assignment.execution_id).await;
        if position == 6 {
            assert!(
                f.repo
                    .recover_settled_input_lease(
                        f.task,
                        &f.scheduler,
                        command.clone(),
                        manual_witness(&f, &command)
                    )
                    .await
                    .is_err()
            );
            assert_eq!(
                saved_lease(&f.db, f.reservation.assignment.execution_id).await,
                before
            );
            break;
        }
        let mut stale = command.clone();
        stale.expected_lease_version -= 1;
        stale.idempotency_key = format!("stale-{position}");
        assert!(
            f.repo
                .recover_settled_input_lease(
                    f.task,
                    &f.scheduler,
                    stale.clone(),
                    manual_witness(&f, &stale)
                )
                .await
                .is_err()
        );
        assert!(
            f.repo
                .recover_settled_input_lease(
                    f.task,
                    &f.scheduler,
                    command.clone(),
                    witness(&f, &command)
                )
                .await
                .is_err()
        );
        assert_eq!(
            saved_lease(&f.db, f.reservation.assignment.execution_id).await,
            before
        );
        current = f
            .repo
            .recover_settled_input_lease(
                f.task,
                &f.scheduler,
                command.clone(),
                manual_witness(&f, &command),
            )
            .await
            .unwrap()
            .0;
    }
    // Already accepted results remain historical after expiry and cap exhaustion.
    // No mutable Fleet parent status or new callback is consulted by readback.
    let replay = f
        .repo
        .preflight_settled_input_recovery(f.task, &f.scheduler, &fourth)
        .await
        .unwrap()
        .unwrap();
    let accepted = f
        .repo
        .pm_resume_operation(f.task, &reader, "recovery-4")
        .await
        .unwrap();
    assert_eq!(
        json_value(&replay).unwrap(),
        json_value(&accepted.result).unwrap()
    );
    let replay = f
        .repo
        .recover_settled_input_lease(f.task, &f.scheduler, fourth.clone(), witness(&f, &fourth))
        .await
        .unwrap();
    assert!(replay.1);
    assert_eq!(original_history(&f).await, history);
    assert_eq!(count(&f.db, "sdlc_pm_resume_inputs").await, 1);
    assert_eq!(count(&f.db, "sdlc_pm_lease_operations").await, 7);
    f.repo.db.close().await.unwrap();
    f.db.close().await.unwrap();
}
