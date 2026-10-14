//! Synthetic policy/owner ledger tests, not live Fleet runtime readiness evidence.
use super::*;

pub(super) fn single_task_routes() -> RoleRoutes {
    let legacy = routes();
    let mut value = serde_json::to_value(&legacy).unwrap();
    for (role, _) in legacy.entries() {
        let suffix = role.replace('_', "-");
        let route = value[role].as_object_mut().unwrap();
        route.remove("package_commit");
        route.remove("package_manifest_sha256");
        route.insert("instruction_profile".into(), json!("single-task-v1"));
        route.insert("instruction_manifest_sha256".into(), json!("c".repeat(64)));
        route.insert(
            "namespace_name".into(),
            json!(format!("single-task-{suffix}")),
        );
        route.insert(
            "workflow_key".into(),
            json!(format!("single-task-v1:{role}")),
        );
        route.insert("profile".into(), json!(format!("single-task-v1-{suffix}")));
        route.insert("workflow_catalog_version".into(), json!(1));
    }
    parse(value)
}

fn config(project: Uuid, enabled: bool) -> SdlcConfig {
    SdlcConfig {
        instance_id: "draft-flow-test".into(),
        orchestrator_subject: "pm".into(),
        verifier_subject: "verifier".into(),
        reservation_scheduler_subject: "scheduler".into(),
        single_task_project_ids: if enabled {
            [project].into_iter().collect()
        } else {
            Default::default()
        },
    }
}

fn create(key: &str, flow: Option<DraftFlow>) -> CreateDraftCommand {
    CreateDraftCommand {
        title: "Draft flow fixture".into(),
        description: "Real saved synthetic input".into(),
        idempotency_key: key.into(),
        flow,
    }
}

async fn checkpoint_counts(db: &DatabaseConnection) -> Vec<i64> {
    let mut counts = vec![];
    for table in [
        "issues",
        "sdlc_tasks",
        "sdlc_draft_creations",
        "sdlc_confirmations",
        "sdlc_task_routing_snapshots",
        "sdlc_analysis_intents",
        "sdlc_idempotency",
        "sdlc_outbox",
        "issue_status_history",
    ] {
        counts.push(count(db, table).await);
    }
    counts
}

async fn legacy_history_snapshot(db: &DatabaseConnection, task: Uuid) -> Json {
    db.query_one(statement(
        "SELECT jsonb_build_object('binding',to_jsonb(t),
         'issue',(SELECT to_jsonb(i) FROM issues i WHERE i.id=t.task_id),
         'status',(SELECT to_jsonb(s) FROM statuses s JOIN issues i ON i.status_id=s.id WHERE i.id=t.task_id),
         'creation',(SELECT to_jsonb(c) FROM sdlc_draft_creations c WHERE c.task_id=t.task_id),
         'history',(SELECT jsonb_agg(to_jsonb(h) ORDER BY h.id) FROM issue_status_history h WHERE h.issue_id=t.task_id),
         'outbox',(SELECT jsonb_agg(to_jsonb(o) ORDER BY o.sequence) FROM sdlc_outbox o WHERE o.task_id=t.task_id)) AS snapshot
         FROM sdlc_tasks t WHERE t.task_id=$1",
        vec![task.into()],
    )).await.unwrap().unwrap().try_get("", "snapshot").unwrap()
}

async fn assert_policy_constraint_rejects(
    db: &DatabaseConnection,
    policy: &domain::sdlc_routing::RoutingPolicy,
    payload: Json,
) {
    let tx = db.begin().await.unwrap();
    let inserted = tx.execute(statement(
        "INSERT INTO sdlc_project_routing_revisions(project_id,version,tracker_instance_id,routing_hash,payload)
         VALUES($1,$2,$3,$4,$5)",
        vec![policy.project_id.into(), (policy.version+1).into(), policy.tracker_instance_id.clone().into(),
            policy.routing_hash.clone().into(), payload.into()],
    )).await;
    tx.rollback().await.unwrap();
    assert!(
        inserted
            .unwrap_err()
            .to_string()
            .contains("sdlc_project_routing_revisions_check")
    );
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_DRAFT_FLOW_TEST_DATABASE_URL ending in _draft_flow_test"]
async fn draft_flow_creation_and_atomic_owner_snapshot_preserve_legacy_and_replay() {
    let url = std::env::var("TT_DRAFT_FLOW_TEST_DATABASE_URL")
        .expect("isolated Draft flow database required");
    let db = Database::connect(&url).await.unwrap();
    let row=db.query_one(statement("SELECT current_database() AS name,to_regclass('public.seaql_migrations') IS NULL AS fresh",vec![])).await.unwrap().unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_draft_flow_test")
    );
    assert!(row.try_get::<bool>("", "fresh").unwrap());
    let prior = migration::Migrator::migrations()
        .iter()
        .position(|m| m.name() == "m20261013_0000096_single_task_delivery")
        .expect("registered single-task migration required");
    migration::Migrator::up(&db, Some(prior.try_into().unwrap()))
        .await
        .unwrap();
    let project = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    for (subject, id) in [
        ("owner", owner_id),
        ("pm", Uuid::new_v4()),
        ("verifier", Uuid::new_v4()),
        ("member", Uuid::new_v4()),
    ] {
        exec(&db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,$2,$3,$3,'!',$3,true)",
            vec![id.into(),format!("{subject}@example.test").into(),subject.into()]).await.unwrap();
    }
    exec(&db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,'DFT','Draft flow',$2,$3)",vec![project.into(),owner_id.into(),Uuid::new_v4().into()]).await.unwrap();
    exec(&db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Draft','[]' FROM projects WHERE id=$1",vec![project.into()]).await.unwrap();
    exec(&db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub IN ('pm','verifier','member')",vec![project.into()]).await.unwrap();
    let repo = PostgresSdlcRepository::connect(&url, config(project, true))
        .await
        .unwrap();
    let disabled = PostgresSdlcRepository::connect(&url, config(project, false))
        .await
        .unwrap();
    let owner = actor("owner", true, vec![]);
    let member = actor("member", true, vec![]);
    // Exercise the actual additive migration with populated immutable legacy history.
    let legacy_command = create("legacy", None);
    let (legacy, _) = repo
        .create_draft(project, &owner, legacy_command.clone())
        .await
        .unwrap();
    let old_history = legacy_history_snapshot(&db, legacy.task_id).await;
    assert!(!old_history["creation"].is_null());
    assert_eq!(old_history["history"].as_array().unwrap().len(), 1);
    assert_eq!(old_history["outbox"].as_array().unwrap().len(), 1);
    let old_project = Uuid::new_v4();
    exec(&db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,'OLD','Legacy routing',$2,$3)",
        vec![old_project.into(),owner_id.into(),Uuid::new_v4().into()]).await.unwrap();
    let old_policy_command = SetRoutingPolicy {
        expected_version: None,
        routes: routes(),
        idempotency_key: "v1-before-migration".into(),
    };
    let (old_policy, _) = repo
        .set_routing_policy(old_project, &owner, old_policy_command.clone())
        .await
        .unwrap();
    let old_policy_row: Json = db.query_one(statement(
        "SELECT to_jsonb(r) AS row FROM sdlc_project_routing_revisions r WHERE project_id=$1 AND version=1",
        vec![old_project.into()],
    )).await.unwrap().unwrap().try_get("", "row").unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    let retained_policy_row: Json = db.query_one(statement(
        "SELECT to_jsonb(r) AS row FROM sdlc_project_routing_revisions r WHERE project_id=$1 AND version=1",
        vec![old_project.into()],
    )).await.unwrap().unwrap().try_get("", "row").unwrap();
    assert_eq!(retained_policy_row, old_policy_row);
    let (retained_policy, replayed) = repo
        .set_routing_policy(old_project, &owner, old_policy_command)
        .await
        .unwrap();
    assert!(replayed);
    assert_eq!(retained_policy, old_policy);
    assert_eq!(
        legacy_history_snapshot(&db, legacy.task_id).await,
        old_history
    );
    let (replayed, was_replayed) = repo
        .create_draft(project, &owner, legacy_command.clone())
        .await
        .unwrap();
    assert!(was_replayed);
    assert_eq!(
        serde_json::to_value(replayed).unwrap(),
        serde_json::to_value(&legacy).unwrap()
    );
    assert_eq!(
        legacy_history_snapshot(&db, legacy.task_id).await,
        old_history
    );
    let marked = create("marked", Some(DraftFlow::SingleTaskV1));
    let before = checkpoint_counts(&db).await;
    assert!(matches!(
        disabled.create_draft(project, &owner, marked.clone()).await,
        Err(AppError::Unavailable(_))
    ));
    assert!(
        repo.create_draft(project, &owner, marked.clone())
            .await
            .is_err()
    );
    assert_eq!(checkpoint_counts(&db).await, before);
    let legacy_state = repo.read(legacy.task_id, &owner).await.unwrap();
    assert_eq!(legacy_state.draft_flow().unwrap(), None);
    assert_eq!(legacy_state.status, None);
    assert_eq!(legacy_state.draft_submode, None);
    let original: String = db
        .query_one(statement(
            "SELECT payload_hash FROM sdlc_draft_creations WHERE task_id=$1",
            vec![legacy.task_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<String>("", "payload_hash")
        .unwrap();
    assert_eq!(original,app::sdlc::canonical_hash(&json!({"operation":"create_draft","payload":{
        "title":legacy_command.title,"description":legacy_command.description,"idempotency_key":legacy_command.idempotency_key}})).unwrap());
    repo.set_routing_policy(
        project,
        &owner,
        SetRoutingPolicy {
            expected_version: None,
            routes: routes(),
            idempotency_key: "v1".into(),
        },
    )
    .await
    .unwrap();
    let before = checkpoint_counts(&db).await;
    assert!(matches!(
        repo.create_draft(project, &owner, marked.clone()).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(checkpoint_counts(&db).await, before);
    let roles = single_task_routes();
    let (policy2, replayed) = repo
        .set_routing_policy(
            project,
            &owner,
            SetRoutingPolicy {
                expected_version: Some(1),
                routes: roles.clone(),
                idempotency_key: "v2".into(),
            },
        )
        .await
        .unwrap();
    assert!(!replayed);
    assert_eq!(policy2.contract_version, 2);
    assert_eq!(
        repo.routing_policy(project, &owner, None).await.unwrap(),
        policy2
    );
    let mut policy_payload = serde_json::to_value(&policy2).unwrap();
    policy_payload["version"] = json!(policy2.version + 1);
    for (field, value) in [
        ("contract_version", json!(1)),
        ("contract_version", json!(3)),
        ("contract_version", json!("2")),
        ("contract_version", json!(null)),
        ("native_ready", json!(true)),
        ("dispatch_allowed", json!(true)),
        ("project_id", json!(Uuid::new_v4())),
        ("tracker_instance_id", json!("foreign-instance")),
        ("version", json!(99)),
        ("routing_hash", json!("f".repeat(64))),
    ] {
        let mut wrong = policy_payload.clone();
        wrong[field] = value;
        assert_policy_constraint_rejects(&db, &policy2, wrong).await;
    }
    for (field, value) in [
        ("instruction_profile", json!("single-task-v2")),
        ("instruction_manifest_sha256", json!(null)),
        ("instruction_manifest_sha256", json!("A".repeat(64))),
        ("instruction_manifest_sha256", json!("d".repeat(64))),
        (
            "package_commit",
            json!(domain::sdlc_routing::PACKAGE_COMMIT),
        ),
        ("agent_id", json!(roles.project_manager.agent_id)),
        ("namespace_id", json!(roles.project_manager.namespace_id)),
        ("workflow_id", json!(roles.project_manager.workflow_id)),
        ("fleet_config_revision", json!(9007199254740992i64)),
        ("workflow_key", json!("hermes-sdlc:analyst")),
        ("profile", json!("single-task-v1-developer")),
        ("workflow_catalog_version", json!(3)),
        ("workflow_catalog_sha256", json!("f".repeat(64))),
        ("unknown", json!(true)),
    ] {
        let mut wrong = policy_payload.clone();
        wrong["routes"]["analyst"][field] = value;
        assert_policy_constraint_rejects(&db, &policy2, wrong).await;
    }
    let mut mixed = policy_payload.clone();
    mixed["routes"]["developer"] = serde_json::to_value(routes().developer).unwrap();
    assert_policy_constraint_rejects(&db, &policy2, mixed).await;
    let mut missing = policy_payload;
    missing["routes"].as_object_mut().unwrap().remove("devops");
    assert_policy_constraint_rejects(&db, &policy2, missing).await;
    let (draft, replayed) = repo
        .create_draft(project, &owner, marked.clone())
        .await
        .unwrap();
    assert!(!replayed);
    let task = draft.task_id;
    let state = repo.read(task, &owner).await.unwrap();
    assert_eq!(state.draft_flow().unwrap(), Some(DraftFlow::SingleTaskV1));
    assert_eq!(state.root_task_id, task);
    assert_eq!(state.status, Some(LifecycleStatus::Ready));
    assert_eq!(state.draft_submode, Some(DraftSubmode::Brainstorming));
    let before = checkpoint_counts(&db).await;
    assert!(
        repo.create_draft(project, &owner, marked.clone())
            .await
            .unwrap()
            .1
    );
    assert_eq!(
        repo.draft_creation_operation(project, &owner, "marked")
            .await
            .unwrap()
            .task_id,
        task
    );
    let mut changed = marked.clone();
    changed.flow = None;
    assert!(matches!(
        repo.create_draft(project, &owner, changed).await,
        Err(AppError::Conflict(_))
    ));
    let mut retrofit = legacy_command.clone();
    retrofit.flow = Some(DraftFlow::SingleTaskV1);
    assert!(matches!(
        repo.create_draft(project, &owner, retrofit).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(checkpoint_counts(&db).await, before);
    for (target, sql) in [
        (
            task,
            "UPDATE sdlc_tasks SET state=state-'flow' WHERE task_id=$1",
        ),
        (
            task,
            "UPDATE sdlc_tasks SET state=jsonb_set(state,'{flow}','null'::jsonb) WHERE task_id=$1",
        ),
        (
            legacy.task_id,
            "UPDATE sdlc_tasks SET state=jsonb_set(state,'{flow}','\"single-task-v1\"'::jsonb) WHERE task_id=$1",
        ),
    ] {
        assert!(
            db.execute(statement(sql, vec![target.into()]))
                .await
                .is_err()
        );
    }
    assert_eq!(
        serde_json::to_value(repo.read(task, &owner).await.unwrap()).unwrap(),
        serde_json::to_value(&state).unwrap()
    );
    // A raw/bind-like marked insertion without immutable creation provenance cannot commit.
    let tx = db.begin().await.unwrap();
    let fake = Uuid::new_v4();
    exec(&tx,"INSERT INTO issues(id,project_id,key,issue_type,status_id,summary,description,reporter_id,priority,labels,position,time_spent_seconds)
        SELECT $1,$2,'DFT-9999','task',id,'Invalid origin','',$3,'medium','[]',0,0 FROM statuses WHERE name='SDLC Ready' LIMIT 1",
        vec![fake.into(),project.into(),owner_id.into()]).await.unwrap();
    let mut fake_state = state.clone();
    fake_state.task_id = fake;
    fake_state.root_task_id = fake;
    exec(&tx,"INSERT INTO sdlc_tasks(task_id,tracker_instance_id,project_id,root_task_id,owner_subject,state) VALUES($1,'draft-flow-test',$2,$1,'owner',$3)",
        vec![fake.into(),project.into(),json_value(&fake_state).unwrap()]).await.unwrap();
    assert!(tx.commit().await.is_err());
    assert_eq!(checkpoint_counts(&db).await, before);
    let reserve = parse::<domain::sdlc_pm_draft::ReservePmDraft>(
        json!({"requested_agent_id":roles.project_manager.agent_id,
        "expected_owner_version":0,"expected_assignment_version":null,"idempotency_key":"reserve"}),
    );
    assert!(matches!(
        disabled
            .reserve_pm_draft(task, &owner, reserve.clone())
            .await,
        Err(AppError::Unavailable(_))
    ));
    let mut wrong = reserve.clone();
    wrong.requested_agent_id = Uuid::new_v4();
    wrong.idempotency_key = "wrong-default".into();
    assert!(matches!(
        repo.reserve_pm_draft(task, &owner, wrong).await,
        Err(AppError::Conflict(_))
    ));
    let (reservation, _) = repo
        .reserve_pm_draft(task, &owner, reserve.clone())
        .await
        .unwrap();
    let pm = actor(
        "pm",
        false,
        vec![
            "task-tracker:read".into(),
            reservation.assignment.scope(task),
        ],
    );
    let fence = MachineFence {
        assignment_id: reservation.assignment.assignment_id,
        execution_id: reservation.assignment.execution_id,
        agent_id: reservation.assignment.agent_id,
        assignment_version: reservation.assignment.version,
    };
    repo.claim_execution_lease(
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
    let document: RequirementsDocument = parse(
        json!({"goal":"Atomic flow","scope":["Scope"],"exclusions":[],"scenarios":["Scenario"],
        "acceptance_criteria":["Accept"],"constraints":[],"dependencies":[],"assumptions":[],"checklist":["check"],"prerequisites":["prerequisite"]}),
    );
    let hash = app::sdlc::canonical_hash(&document).unwrap();
    repo.execute_verified(
        task,
        &pm,
        SdlcCommand::PublishRevision(PublishRevision {
            fence: fence.clone(),
            expected_requirement_revision: None,
            document: document.clone(),
            idempotency_key: "publish".into(),
        }),
        reservation.clone(),
        None,
    )
    .await
    .unwrap();
    let confirm = SdlcCommand::Confirm {
        revision: 1,
        command: ConfirmCommand {
            content_hash: hash.clone(),
            idempotency_key: "confirm".into(),
            expected_routing_policy_version: None,
        },
    };
    assert!(matches!(
        repo.execute_verified(task, &owner, confirm.clone(), reservation.clone(), None)
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
        repo.execute_verified(
            task,
            &verifier,
            SdlcCommand::Evidence(EvidenceCommand {
                fence: fence.clone(),
                requirement_revision: 1,
                content_hash: hash.clone(),
                check_id: check.into(),
                evidence_reference: format!("fixture://{check}"),
                idempotency_key: check.into(),
            }),
            reservation.clone(),
            None,
        )
        .await
        .unwrap();
    }
    let before = checkpoint_counts(&db).await;
    let before_state = serde_json::to_value(repo.read(task, &owner).await.unwrap()).unwrap();
    assert!(matches!(
        repo.execute_verified(task, &member, confirm.clone(), reservation.clone(), None)
            .await,
        Err(AppError::Forbidden)
    ));
    for (revision, content_hash, version) in [
        (2, hash.clone(), None),
        (1, "f".repeat(64), None),
        (1, hash.clone(), Some(1)),
    ] {
        let wrong = SdlcCommand::Confirm {
            revision,
            command: ConfirmCommand {
                content_hash,
                idempotency_key: "wrong-confirm".into(),
                expected_routing_policy_version: version,
            },
        };
        assert!(matches!(
            repo.execute_verified(task, &owner, wrong, reservation.clone(), None)
                .await,
            Err(AppError::Conflict(_))
        ));
    }
    assert_eq!(checkpoint_counts(&db).await, before);
    repo.set_routing_policy(
        project,
        &owner,
        SetRoutingPolicy {
            expected_version: Some(2),
            routes: routes(),
            idempotency_key: "v1-again".into(),
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        repo.execute_verified(task, &owner, confirm.clone(), reservation.clone(), None)
            .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(checkpoint_counts(&db).await, before);
    let new_roles = single_task_routes();
    let (policy4, _) = repo
        .set_routing_policy(
            project,
            &owner,
            SetRoutingPolicy {
                expected_version: Some(3),
                routes: new_roles,
                idempotency_key: "v2-again".into(),
            },
        )
        .await
        .unwrap();
    db.execute_unprepared("CREATE FUNCTION fixture_flow_fail_outbox() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        IF NEW.event_type='requirements.confirmed' THEN RAISE EXCEPTION 'fixture outbox failure'; END IF; RETURN NEW; END $$;
        CREATE TRIGGER fixture_flow_fail_outbox BEFORE INSERT ON sdlc_outbox FOR EACH ROW EXECUTE FUNCTION fixture_flow_fail_outbox();").await.unwrap();
    assert!(
        repo.execute_verified(task, &owner, confirm.clone(), reservation.clone(), None)
            .await
            .is_err()
    );
    assert_eq!(checkpoint_counts(&db).await, before);
    assert_eq!(
        serde_json::to_value(repo.read(task, &owner).await.unwrap()).unwrap(),
        before_state
    );
    db.execute_unprepared("DROP TRIGGER fixture_flow_fail_outbox ON sdlc_outbox; DROP FUNCTION fixture_flow_fail_outbox();").await.unwrap();
    let next_policy = SetRoutingPolicy {
        expected_version: Some(4),
        routes: single_task_routes(),
        idempotency_key: "concurrent-policy".into(),
    };
    let (accepted, policy5) = tokio::join!(
        repo.execute_verified(task, &owner, confirm.clone(), reservation.clone(), None),
        repo.set_routing_policy(project, &owner, next_policy)
    );
    let confirmation: Confirmation = parse(accepted.unwrap());
    let policy5 = policy5.unwrap().0;
    let snapshot = repo.task_routing_snapshot(task, &owner).await.unwrap();
    assert_eq!(snapshot.contract_version, 2);
    assert_eq!(snapshot.confirmation_id, confirmation.id);
    assert_eq!(snapshot.requirement_revision, 1);
    assert_eq!(snapshot.content_hash, hash);
    assert!(snapshot.policy == policy4 || snapshot.policy == policy5);
    let intent = repo.analysis_intent(task, &owner).await.unwrap();
    assert_eq!(intent.workflow, "single-task-v1:analyst");
    for version in [json!(1), json!(3), json!("2"), json!(null)] {
        let mut wrong = serde_json::to_value(&snapshot).unwrap();
        wrong["contract_version"] = version;
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(
            "CREATE TEMP TABLE fixture_routing_snapshot_shape
            (LIKE sdlc_task_routing_snapshots INCLUDING CONSTRAINTS) ON COMMIT DROP",
        )
        .await
        .unwrap();
        tx.execute(statement("INSERT INTO fixture_routing_snapshot_shape SELECT * FROM sdlc_task_routing_snapshots WHERE task_id=$1",
            vec![task.into()])).await.unwrap();
        let inserted = tx.execute(statement("INSERT INTO fixture_routing_snapshot_shape
            (snapshot_id,task_id,project_id,policy_version,confirmation_id,requirement_revision,content_hash,payload)
            SELECT snapshot_id,task_id,project_id,policy_version,confirmation_id,requirement_revision,content_hash,$2
            FROM sdlc_task_routing_snapshots WHERE task_id=$1",vec![task.into(),wrong.into()])).await;
        tx.rollback().await.unwrap();
        assert!(
            inserted
                .unwrap_err()
                .to_string()
                .contains("sdlc_task_routing_snapshots_check")
        );
    }
    let mut wrong = serde_json::to_value(&intent).unwrap();
    wrong["workflow"] = json!("hermes-sdlc:analyst");
    let tx = db.begin().await.unwrap();
    let inserted = tx.execute(statement("INSERT INTO sdlc_analysis_intents
        (intent_id,task_id,requirement_revision,content_hash,confirmation_id,operation_key,payload,routing_snapshot_id)
        SELECT intent_id,task_id,requirement_revision,content_hash,confirmation_id,operation_key,$2,routing_snapshot_id
        FROM sdlc_analysis_intents WHERE task_id=$1",vec![task.into(),wrong.into()])).await;
    tx.rollback().await.unwrap();
    assert!(
        inserted
            .unwrap_err()
            .to_string()
            .contains("snapshot/workflow mismatch")
    );
    let state = repo.read(task, &owner).await.unwrap();
    assert_eq!(state.stage, Stage::Analysis);
    assert_eq!(state.status, Some(LifecycleStatus::Ready));
    assert_eq!(state.assignment.as_ref().unwrap(), &reservation.assignment);
    assert_eq!(state.draft_flow().unwrap(), Some(DraftFlow::SingleTaskV1));
    let before = checkpoint_counts(&db).await;
    repo.set_routing_policy(
        project,
        &owner,
        SetRoutingPolicy {
            expected_version: Some(5),
            routes: routes(),
            idempotency_key: "legacy-after-confirm".into(),
        },
    )
    .await
    .unwrap();
    let replay = disabled
        .execute_verified(task, &owner, confirm.clone(), reservation.clone(), None)
        .await
        .unwrap();
    assert_eq!(replay, serde_json::to_value(&confirmation).unwrap());
    assert_eq!(
        disabled.task_routing_snapshot(task, &owner).await.unwrap(),
        snapshot
    );
    assert_eq!(
        disabled.confirmation_read(task, &owner, 1).await.unwrap(),
        confirmation
    );
    assert!(
        disabled
            .create_draft(project, &owner, marked.clone())
            .await
            .unwrap()
            .1
    );
    assert_eq!(
        disabled
            .draft_creation_operation(project, &owner, "marked")
            .await
            .unwrap()
            .task_id,
        task
    );
    assert!(
        disabled
            .reserve_pm_draft(task, &owner, reserve)
            .await
            .unwrap()
            .1
    );
    assert_eq!(checkpoint_counts(&db).await, before);
    let mut changed = confirm;
    if let SdlcCommand::Confirm { command, .. } = &mut changed {
        command.expected_routing_policy_version = Some(4);
    }
    assert!(matches!(
        disabled
            .execute_verified(task, &owner, changed, reservation, None)
            .await,
        Err(AppError::Conflict(_))
    ));
    assert!(
        disabled
            .create_draft(project, &owner, legacy_command)
            .await
            .unwrap()
            .1
    );
    assert_eq!(
        disabled
            .draft_creation_operation(project, &owner, "legacy")
            .await
            .unwrap()
            .task_id,
        legacy.task_id
    );
    assert_eq!(
        repo.read(legacy.task_id, &owner)
            .await
            .unwrap()
            .draft_flow()
            .unwrap(),
        None
    );
    // Existing legacy Draft/PM reservation still confirms without automatic snapshot.
    let legacy_task = legacy.task_id;
    let policy = repo.routing_policy(project, &owner, None).await.unwrap();
    let (old_pm, _) = disabled
        .reserve_pm_draft(
            legacy_task,
            &owner,
            parse(json!({
        "requested_agent_id":policy.routes.project_manager.agent_id,"expected_owner_version":0,
        "expected_assignment_version":null,"idempotency_key":"legacy-reserve"})),
        )
        .await
        .unwrap();
    let old_fence = MachineFence {
        assignment_id: old_pm.assignment.assignment_id,
        execution_id: old_pm.assignment.execution_id,
        agent_id: old_pm.assignment.agent_id,
        assignment_version: old_pm.assignment.version,
    };
    let old_actor = actor(
        "pm",
        false,
        vec![
            "task-tracker:read".into(),
            old_pm.assignment.scope(legacy_task),
        ],
    );
    disabled
        .claim_execution_lease(
            legacy_task,
            &old_actor,
            ClaimExecutionLease {
                expected_owner_version: 1,
                fence: old_fence.clone(),
                idempotency_key: "legacy-claim".into(),
            },
        )
        .await
        .unwrap();
    disabled
        .execute_verified(
            legacy_task,
            &old_actor,
            SdlcCommand::PublishRevision(PublishRevision {
                fence: old_fence.clone(),
                expected_requirement_revision: None,
                document,
                idempotency_key: "legacy-publish".into(),
            }),
            old_pm.clone(),
            None,
        )
        .await
        .unwrap();
    let old_verifier = actor(
        "verifier",
        false,
        vec![format!(
            "task-tracker:sdlc:evidence:{legacy_task}:{}",
            old_fence.assignment_version
        )],
    );
    for check in ["check", "prerequisite"] {
        disabled
            .execute_verified(
                legacy_task,
                &old_verifier,
                SdlcCommand::Evidence(EvidenceCommand {
                    fence: old_fence.clone(),
                    requirement_revision: 1,
                    content_hash: hash.clone(),
                    check_id: check.into(),
                    evidence_reference: format!("fixture://legacy-{check}"),
                    idempotency_key: check.into(),
                }),
                old_pm.clone(),
                None,
            )
            .await
            .unwrap();
    }
    let snapshot_count = count(&db, "sdlc_task_routing_snapshots").await;
    disabled
        .execute_verified(
            legacy_task,
            &owner,
            SdlcCommand::Confirm {
                revision: 1,
                command: ConfirmCommand {
                    content_hash: hash,
                    idempotency_key: "legacy-confirm".into(),
                    expected_routing_policy_version: None,
                },
            },
            old_pm,
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        count(&db, "sdlc_task_routing_snapshots").await,
        snapshot_count
    );
    assert_eq!(
        disabled
            .analysis_intent(legacy_task, &owner)
            .await
            .unwrap()
            .workflow,
        "hermes-sdlc:analyst"
    );
    let old_state = disabled.read(legacy_task, &owner).await.unwrap();
    assert_eq!(old_state.draft_flow().unwrap(), None);
    assert_eq!(old_state.status, None);
    assert_eq!(
        disabled
            .draft_creation_operation(project, &owner, "legacy")
            .await
            .unwrap()
            .task_id,
        legacy_task
    );
    exec(
        &db,
        "UPDATE users SET is_active=false WHERE central_sub='owner'",
        vec![],
    )
    .await
    .unwrap();
    assert!(matches!(
        disabled.create_draft(project, &owner, marked).await,
        Err(AppError::Forbidden)
    ));
    assert!(matches!(
        disabled
            .draft_creation_operation(project, &owner, "marked")
            .await,
        Err(AppError::Forbidden)
    ));
}
