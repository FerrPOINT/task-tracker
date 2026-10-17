//! Synthetic disposable PG qualification; not production/source-readback evidence.
use super::*;
use sea_orm_migration::MigratorTrait;

fn actor(subject: &str, human: bool) -> Principal {
    Principal {
        subject: subject.into(),
        human_session: human,
        trusted_human: false,
        scopes: if human {
            Default::default()
        } else {
            ["task-tracker:read".into()].into_iter().collect()
        },
    }
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_SOURCE_TEST_DATABASE_URL ending in _source_test"]
async fn source_owner_cas_replay_owner_disable_and_no_historical_enrollment() {
    let url =
        std::env::var("TT_SOURCE_TEST_DATABASE_URL").expect("owned disposable database required");
    let db = Database::connect(&url).await.unwrap();
    let row = db.query_one(statement("SELECT current_database() AS name,to_regclass('public.seaql_migrations') IS NULL AS fresh",vec![])).await.unwrap().unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_source_test")
    );
    assert!(row.try_get::<bool>("", "fresh").unwrap());
    migration::Migrator::up(&db, None).await.unwrap();
    let owner_id = Uuid::new_v4();
    let member_id = Uuid::new_v4();
    let project = Uuid::new_v4();
    let instance = Uuid::new_v4();
    for (id, sub) in [(owner_id, "source-owner"), (member_id, "source-member")] {
        exec(&db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,$2,$3,$3,'!',$3,true)",
            vec![id.into(),format!("{sub}@example.test").into(),sub.into()]).await.unwrap();
    }
    exec(&db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,'SRC','Synthetic source fixture',$2,$3)",
        vec![project.into(),owner_id.into(),Uuid::new_v4().into()]).await.unwrap();
    exec(&db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Synthetic source','[]' FROM projects WHERE id=$1",vec![project.into()]).await.unwrap();
    exec(
        &db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'developer')",
        vec![project.into(), member_id.into()],
    )
    .await
    .unwrap();
    let namespace = NamespaceRef {
        registry_instance_id: Uuid::new_v4(),
        namespace_id: Uuid::new_v4(),
    };
    let command = json!({"schema_version":1,"namespace":namespace,"resource":{"kind":"tracker_project","instance_id":instance,"resource_id":project},
        "operation_id":Uuid::new_v4(),"generation":1,"state":"active"});
    exec(&db,"INSERT INTO tracker_namespace_bindings(resource_id,registry_instance_id,namespace_id,generation,state,command) VALUES($1,$2,$3,1,'active',$4)",
        vec![project.into(),namespace.registry_instance_id.into(),namespace.namespace_id.into(),command.into()]).await.unwrap();
    let repo = PostgresSdlcRepository::connect(
        &url,
        SdlcConfig {
            instance_id: instance.to_string(),
            orchestrator_subject: "source-pm".into(),
            verifier_subject: "source-verifier".into(),
            reservation_scheduler_subject: "source-scheduler".into(),
            single_task_project_ids: [project].into_iter().collect(),
        },
    )
    .await
    .unwrap();
    let owner = actor("source-owner", true);
    let source = SourcePolicySpec {
        forge_instance_id: Uuid::new_v4(),
        repository_id: Uuid::new_v4(),
        base_ref: "refs/heads/main".into(),
        base_commit: "a".repeat(40),
    };
    let proof = VerifiedSource {
        public_name: "fixture/repository".into(),
        namespace,
        forge_instance_id: source.forge_instance_id,
        repository_id: source.repository_id,
        base_ref: source.base_ref.clone(),
        base_commit: source.base_commit.clone(),
        default_ref: "refs/heads/main".into(),
        protected_refs: vec!["refs/heads/release".into()],
        observed_at: chrono::Utc::now(),
    };
    let input = SetSourcePolicy {
        expected_source_revision: None,
        source,
        idempotency_key: "source-policy:1".into(),
    };
    for denied in [
        actor("source-member", true),
        actor(&owner_id.to_string(), true),
        actor("source-owner", false),
    ] {
        assert!(matches!(
            repo.set_source_policy(project, &denied, input.clone(), proof.clone())
                .await,
            Err(AppError::Forbidden)
        ));
    }
    let (first, replay) = repo
        .set_source_policy(project, &owner, input.clone(), proof.clone())
        .await
        .unwrap();
    assert!(!replay);
    assert_eq!(first.source_revision, 1);
    let mut expired = proof.clone();
    expired.observed_at -= chrono::Duration::hours(1);
    let (same, replay) = repo
        .set_source_policy(project, &owner, input.clone(), expired)
        .await
        .unwrap();
    assert!(replay);
    assert_eq!(first, same);
    let mut changed = input.clone();
    changed.source.base_commit = "b".repeat(40);
    assert!(matches!(
        repo.set_source_policy(project, &owner, changed, proof.clone())
            .await,
        Err(AppError::Conflict(_))
    ));
    let mut next = input.clone();
    next.expected_source_revision = Some(1);
    next.idempotency_key = "source-policy:2a".into();
    let mut other = next.clone();
    other.idempotency_key = "source-policy:2b".into();
    let (left, right) = tokio::join!(
        repo.set_source_policy(project, &owner, next, proof.clone()),
        repo.set_source_policy(project, &owner, other, proof.clone())
    );
    assert_ne!(left.is_ok(), right.is_ok());
    assert_eq!(
        repo.source_policy(project, &owner, None)
            .await
            .unwrap()
            .source_revision,
        2
    );
    let original = repo
        .source_policy_operation(project, &owner, &input.idempotency_key)
        .await
        .unwrap();
    assert_eq!(original, first);
    let tx = db.begin().await.unwrap();
    assert!(
        tx.execute(statement(
            "UPDATE sdlc_source_policy_revisions SET source_hash=$2 WHERE project_id=$1",
            vec![project.into(), "f".repeat(64).into()]
        ))
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    let (draft, _) = repo
        .create_draft(
            project,
            &owner,
            CreateDraftCommand {
                flow: None,
                title: "Synthetic legacy Draft".into(),
                description: String::new(),
                idempotency_key: "legacy-source-absence".into(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        repo.task_source(draft.task_id, &owner).await.unwrap(),
        TaskSourceReadback::Blocked {
            reason: SourceBlockReason::MissingBinding,
            ..
        }
    ));
    let bindings = db
        .query_one(statement(
            "SELECT count(*) AS n FROM sdlc_task_source_bindings",
            vec![],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(bindings.try_get::<i64>("", "n").unwrap(), 0);
    let mut routes = json!({});
    for (i, role) in [
        "project_manager",
        "analyst",
        "architect",
        "developer",
        "reviewer",
        "tester",
        "devops",
    ]
    .iter()
    .enumerate()
    {
        let suffix = role.replace('_', "-");
        routes[*role] = json!({"agent_id":Uuid::new_v4(),"fleet_config_revision":4,
            "instruction_profile":"single-task-v1","instruction_manifest_sha256":"b".repeat(64),
            "namespace_id":(i+1).to_string(),"namespace_name":format!("single-task-{suffix}"),
            "workflow_id":(i+11).to_string(),"workflow_key":format!("single-task-v1:{role}"),
            "profile":format!("single-task-v1-{suffix}"),"workflow_catalog_version":1,"workflow_catalog_sha256":"c".repeat(64)});
    }
    repo.set_routing_policy(
        project,
        &owner,
        domain::sdlc_routing::SetRoutingPolicy {
            expected_version: None,
            routes: serde_json::from_value(routes).unwrap(),
            idempotency_key: "routing:1".into(),
        },
    )
    .await
    .unwrap();
    let (new_draft, _) = repo
        .create_draft(
            project,
            &owner,
            CreateDraftCommand {
                flow: Some(DraftFlow::SingleTaskV1),
                title: "Synthetic selected Draft".into(),
                description: String::new(),
                idempotency_key: "selected-source".into(),
            },
        )
        .await
        .unwrap();
    let selected = db
        .query_one(statement(
            "SELECT source_revision FROM sdlc_task_source_selections WHERE task_id=$1",
            vec![new_draft.task_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.try_get::<i64>("", "source_revision").unwrap(), 2);
    let mut third = input.clone();
    third.idempotency_key = "source-policy:3".into();
    third.expected_source_revision = Some(2);
    let mut fresh_proof = proof;
    fresh_proof.observed_at = chrono::Utc::now();
    repo.set_source_policy(project, &owner, third, fresh_proof)
        .await
        .unwrap();
    let selected = db
        .query_one(statement(
            "SELECT source_revision FROM sdlc_task_source_selections WHERE task_id=$1",
            vec![new_draft.task_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.try_get::<i64>("", "source_revision").unwrap(), 2);
    let legacy = db
        .query_one(statement(
            "SELECT count(*) AS n FROM sdlc_task_source_selections WHERE task_id=$1",
            vec![draft.task_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(legacy.try_get::<i64>("", "n").unwrap(), 0);
    // Normal synthetic PM/owner confirmation; no caller ref and no Git/API effect.
    for sub in ["source-pm", "source-verifier", "source-scheduler"] {
        let id = Uuid::new_v4();
        exec(&db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,$2,$3,$3,'!',$3,true)",
            vec![id.into(),format!("{sub}@example.test").into(),sub.into()]).await.unwrap();
        exec(
            &db,
            "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'developer')",
            vec![project.into(), id.into()],
        )
        .await
        .unwrap();
    }
    assert_legacy_routed_confirmation(&db, &repo, &owner, draft.task_id).await;
    let task = new_draft.task_id;
    let policy = repo.routing_policy(project, &owner, None).await.unwrap();
    let (reservation, _) = repo
        .reserve_pm_draft(
            task,
            &owner,
            serde_json::from_value(json!({
        "requested_agent_id":policy.routes.project_manager.agent_id,"expected_owner_version":0,
        "expected_assignment_version":null,"idempotency_key":"source-pm:reserve"}))
            .unwrap(),
        )
        .await
        .unwrap();
    let mut pm = actor("source-pm", false);
    pm.scopes = [
        "task-tracker:read".into(),
        "task-tracker:write".into(),
        reservation.assignment.scope(task),
    ]
    .into_iter()
    .collect();
    let fence = MachineFence {
        assignment_id: reservation.assignment.assignment_id,
        execution_id: reservation.assignment.execution_id,
        agent_id: reservation.assignment.agent_id,
        assignment_version: 1,
    };
    repo.claim_execution_lease(
        task,
        &pm,
        domain::sdlc_execution_lease::ClaimExecutionLease {
            expected_owner_version: 1,
            fence: fence.clone(),
            idempotency_key: "source-pm:claim".into(),
        },
    )
    .await
    .unwrap();
    let document: RequirementsDocument = serde_json::from_value(json!({"goal":"Synthetic source test",
        "scope":["One root"],"exclusions":[],"scenarios":["Source freeze"],"acceptance_criteria":["Exact binding"],
        "constraints":[],"dependencies":[],"assumptions":[],"checklist":["source-check"],"prerequisites":["source-prerequisite"]})).unwrap();
    let content_hash = app::sdlc::canonical_hash(&document).unwrap();
    repo.execute_verified(
        task,
        &pm,
        SdlcCommand::PublishRevision(PublishRevision {
            fence: fence.clone(),
            expected_requirement_revision: None,
            document,
            idempotency_key: "source:requirements".into(),
        }),
        reservation.clone(),
        None,
    )
    .await
    .unwrap();
    let mut verifier = actor("source-verifier", false);
    verifier.scopes = [
        "task-tracker:write".into(),
        format!("task-tracker:sdlc:evidence:{task}:1"),
    ]
    .into_iter()
    .collect();
    for check in ["source-check", "source-prerequisite"] {
        repo.execute_verified(
            task,
            &verifier,
            SdlcCommand::Evidence(EvidenceCommand {
                fence: fence.clone(),
                requirement_revision: 1,
                content_hash: content_hash.clone(),
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
    let old_assignment = repo.read(task, &owner).await.unwrap().assignment;
    let command = SdlcCommand::Confirm {
        revision: 1,
        command: ConfirmCommand {
            content_hash: content_hash.clone(),
            idempotency_key: "source:confirm".into(),
            expected_routing_policy_version: Some(1),
        },
    };
    let confirmation = repo
        .execute_verified(task, &owner, command.clone(), reservation.clone(), None)
        .await
        .unwrap();
    let repeated = repo
        .execute_verified(task, &owner, command, reservation, None)
        .await
        .unwrap();
    assert_eq!(confirmation, repeated);
    assert_eq!(
        serde_json::to_value(repo.read(task, &owner).await.unwrap().assignment).unwrap(),
        serde_json::to_value(old_assignment).unwrap()
    );
    let source = repo.task_source(task, &owner).await.unwrap();
    let TaskSourceReadback::Blocked {
        reason: SourceBlockReason::MissingCurrentExecution,
        binding: Some(binding),
        ..
    } = source
    else {
        panic!("source must be frozen without invented runtime")
    };
    assert_eq!(binding.source_revision, 2);
    assert_eq!(binding.candidate_ref, format!("refs/heads/sdlc/{task}"));
    assert_eq!(
        binding.source_hash,
        app::sdlc_source::sealed_hash(&binding).unwrap()
    );
    let count = db
        .query_one(statement(
            "SELECT count(*) AS n FROM sdlc_task_source_bindings WHERE task_id=$1",
            vec![task.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(count.try_get::<i64>("", "n").unwrap(), 1);
    assert!(repo.draft_source_preflight(task, &owner).await.is_err());
    let tx = db.begin().await.unwrap();
    assert!(
        tx.execute(statement(
            "UPDATE sdlc_task_source_bindings SET candidate_ref='refs/heads/main' WHERE task_id=$1",
            vec![task.into()]
        ))
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    for race in [false, true] {
        explicit_unconfigured_draft_fixture(
            &db, &url, &repo, project, instance, owner_id, &owner, race,
        )
        .await;
    }
    exec(
        &db,
        "UPDATE users SET is_active=false WHERE central_sub='source-owner'",
        vec![],
    )
    .await
    .unwrap();
    assert!(matches!(
        repo.source_policy_operation(project, &owner, &input.idempotency_key)
            .await,
        Err(AppError::Forbidden)
    ));
}

// Synthetic disposable original Draft. No Fleet readback, new task or runtime effect on enrollment.
#[allow(clippy::too_many_arguments)]
async fn explicit_unconfigured_draft_fixture(
    db: &DatabaseConnection,
    url: &str,
    original: &PostgresSdlcRepository,
    original_project: Uuid,
    instance: Uuid,
    owner_id: Uuid,
    owner: &Principal,
    race: bool,
) {
    let project = Uuid::new_v4();
    exec(db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,$4,'Synthetic initial enrollment',$2,$3)",
        vec![project.into(),owner_id.into(),Uuid::new_v4().into(),if race {"ENRR"} else {"ENR"}.into()]).await.unwrap();
    exec(db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Synthetic enrollment','[]' FROM projects WHERE id=$1",
        vec![project.into()]).await.unwrap();
    let original_policy = original
        .source_policy(original_project, owner, None)
        .await
        .unwrap();
    let namespace = NamespaceRef {
        registry_instance_id: Uuid::new_v4(),
        namespace_id: Uuid::new_v4(),
    };
    let command = json!({"schema_version":1,"namespace":namespace,"resource":{"kind":"tracker_project","instance_id":instance,"resource_id":project},
        "operation_id":Uuid::new_v4(),"generation":1,"state":"active"});
    exec(db,"INSERT INTO tracker_namespace_bindings(resource_id,registry_instance_id,namespace_id,generation,state,command) VALUES($1,$2,$3,1,'active',$4)",
        vec![project.into(),namespace.registry_instance_id.into(),namespace.namespace_id.into(),command.into()]).await.unwrap();
    let repo = PostgresSdlcRepository::connect(
        url,
        SdlcConfig {
            instance_id: instance.to_string(),
            orchestrator_subject: "source-pm".into(),
            verifier_subject: "source-verifier".into(),
            reservation_scheduler_subject: "source-scheduler".into(),
            single_task_project_ids: [project].into_iter().collect(),
        },
    )
    .await
    .unwrap();
    let routes = original
        .routing_policy(original_project, owner, None)
        .await
        .unwrap()
        .routes;
    repo.set_routing_policy(
        project,
        owner,
        domain::sdlc_routing::SetRoutingPolicy {
            expected_version: None,
            routes,
            idempotency_key: "fixture-routing".into(),
        },
    )
    .await
    .unwrap();
    let (draft, _) = repo
        .create_draft(
            project,
            owner,
            CreateDraftCommand {
                flow: Some(DraftFlow::SingleTaskV1),
                title: "Synthetic unconfigured original Draft".into(),
                description: String::new(),
                idempotency_key: "fixture-original-draft".into(),
            },
        )
        .await
        .unwrap();
    let task = draft.task_id;
    exec(
        db,
        "INSERT INTO project_members(project_id,user_id,role)
        SELECT $1,id,'developer' FROM users WHERE central_sub IN ('source-pm','source-verifier')",
        vec![project.into()],
    )
    .await
    .unwrap();
    let policy = repo.routing_policy(project, owner, None).await.unwrap();
    let (reservation, _) = repo
        .reserve_pm_draft(
            task,
            owner,
            serde_json::from_value(json!({
        "requested_agent_id":policy.routes.project_manager.agent_id,"expected_owner_version":0,
        "expected_assignment_version":null,"idempotency_key":"fixture-original-pm"}))
            .unwrap(),
        )
        .await
        .unwrap();
    let mut pm = actor("source-pm", false);
    pm.scopes = [
        "task-tracker:read".into(),
        "task-tracker:write".into(),
        reservation.assignment.scope(task),
    ]
    .into_iter()
    .collect();
    let fence = MachineFence {
        assignment_id: reservation.assignment.assignment_id,
        execution_id: reservation.assignment.execution_id,
        agent_id: reservation.assignment.agent_id,
        assignment_version: reservation.assignment.version,
    };
    let (lease, _) = repo
        .claim_execution_lease(
            task,
            &pm,
            domain::sdlc_execution_lease::ClaimExecutionLease {
                expected_owner_version: 1,
                fence: fence.clone(),
                idempotency_key: "enroll-active-pm-lease".into(),
            },
        )
        .await
        .unwrap();
    let confirm = prepare_enrollment_confirmation(&repo, task, &reservation, &pm, &fence).await;
    let old_state = serde_json::to_value(repo.read(task, owner).await.unwrap()).unwrap();
    let old_assignment = serde_json::to_value(&reservation).unwrap();
    assert!(repo.draft_source_preflight(task, owner).await.is_err());
    let mut verified = original_policy.verification;
    verified.namespace = namespace;
    verified.observed_at = chrono::Utc::now();
    let input = SetSourcePolicy {
        expected_source_revision: None,
        source: original_policy.source,
        idempotency_key: "fixture-source-policy".into(),
    };
    repo.set_source_policy(project, owner, input.clone(), verified.clone())
        .await
        .unwrap();
    let before = repo.draft_source_preflight(task, owner).await.unwrap();
    for denied in [
        actor("source-member", true),
        actor(&owner_id.to_string(), true),
        actor("source-owner", false),
    ] {
        assert!(matches!(
            repo.draft_source_preflight(task, &denied).await,
            Err(AppError::Forbidden)
        ));
    }
    let make_command = |p: &DraftSourceEnrollmentPreflight| EnrollDraftSource {
        expected_preflight_sha256: p.preflight_sha256.clone(),
        source_revision: p.policy.source_revision,
        policy_hash: p.policy.source_hash.clone(),
        idempotency_key: "fixture-explicit-enrollment".into(),
    };
    let mut next = input;
    next.expected_source_revision = Some(1);
    next.idempotency_key = "fixture-source-policy-2".into();
    verified.observed_at = chrono::Utc::now();
    repo.set_source_policy(project, owner, next, verified)
        .await
        .unwrap();
    assert!(
        repo.enroll_draft_source(task, owner, make_command(&before))
            .await
            .is_err()
    );
    let before = repo.draft_source_preflight(task, owner).await.unwrap();
    let command = make_command(&before);
    // Both operations take the SAME issue/aggregate lock. Confirmation cannot freeze an unselected source.
    let (enrollment, confirmation) = if race {
        let (a, b) = tokio::join!(
            repo.enroll_draft_source(task, owner, command.clone()),
            repo.execute_verified(task, owner, confirm.clone(), reservation.clone(), None)
        );
        (a, Some(b))
    } else {
        (
            repo.enroll_draft_source(task, owner, command.clone()).await,
            None,
        )
    };
    let (receipt, replay) = enrollment.unwrap();
    assert!(!replay);
    assert_eq!(receipt.task_id, task);
    if confirmation.as_ref().is_some_and(|r| r.is_ok()) {
        // Confirmation won after enrollment; even an exact enrollment replay is now closed.
        assert!(
            repo.enroll_draft_source(task, owner, command)
                .await
                .is_err()
        );
        assert_frozen_enrollment(db, &repo, task, owner, receipt.source_revision).await;
        return;
    }
    if let Some(Err(error)) = confirmation {
        assert!(
            matches!(error,AppError::Unavailable(ref message) if message.contains("sdlc_source_project_policy_missing"))
        );
    }
    let (same, replay) = repo
        .enroll_draft_source(task, owner, command.clone())
        .await
        .unwrap();
    assert!(replay);
    assert_eq!(
        serde_json::to_value(same).unwrap(),
        serde_json::to_value(&receipt).unwrap()
    );
    let mut changed = command.clone();
    changed.policy_hash = "f".repeat(64);
    assert!(
        repo.enroll_draft_source(task, owner, changed)
            .await
            .is_err()
    );
    assert!(repo.draft_source_preflight(task, owner).await.is_err());
    let current_state = repo.read(task, owner).await.unwrap();
    assert_eq!(old_state, serde_json::to_value(&current_state).unwrap());
    let tx = db.begin().await.unwrap();
    assert_eq!(
        old_assignment,
        serde_json::to_value(
            PostgresSdlcRepository::reservation(
                &tx,
                &current_state,
                reservation.assignment.execution_id
            )
            .await
            .unwrap()
        )
        .unwrap()
    );
    let row = tx.query_one(statement("SELECT
        (SELECT source_revision IS NULL FROM sdlc_task_source_selections WHERE task_id=$1) AS original_unconfigured,
        (SELECT count(*) FROM sdlc_draft_source_enrollments WHERE task_id=$1) AS enrolled,
        (SELECT count(*) FROM sdlc_task_source_bindings WHERE task_id=$1) AS published,
        (SELECT count(*) FROM audit_logs WHERE entity_id=$1 AND action='sdlc.draft_source.enrolled') AS audited",vec![task.into()])).await.unwrap().unwrap();
    assert!(row.try_get::<bool>("", "original_unconfigured").unwrap());
    assert_eq!(row.try_get::<i64>("", "enrolled").unwrap(), 1);
    assert_eq!(row.try_get::<i64>("", "published").unwrap(), 0);
    assert_eq!(row.try_get::<i64>("", "audited").unwrap(), 1);
    let event = tx.query_one(statement("SELECT count(*) AS n FROM sdlc_outbox WHERE task_id=$1 AND event_type='source.selected'",vec![task.into()])).await.unwrap().unwrap();
    assert_eq!(event.try_get::<i64>("", "n").unwrap(), 1);
    let retained_lease = PostgresSdlcRepository::lease_current(&tx, &reservation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(lease).unwrap(),
        serde_json::to_value(retained_lease).unwrap()
    );
    tx.rollback().await.unwrap();
    let current_policy = repo.source_policy(project, owner, None).await.unwrap();
    let mut verification = current_policy.verification;
    verification.observed_at = chrono::Utc::now();
    repo.set_source_policy(
        project,
        owner,
        SetSourcePolicy {
            expected_source_revision: Some(current_policy.source_revision),
            source: current_policy.source,
            idempotency_key: "enrollment-head-after-pin".into(),
        },
        verification,
    )
    .await
    .unwrap();
    // If confirmation acquired the lock first it failed missing-source and rolled back, not fake-success.
    repo.execute_verified(task, owner, confirm, reservation, None)
        .await
        .unwrap();
    assert!(
        repo.enroll_draft_source(task, owner, command)
            .await
            .is_err()
    );
    assert_frozen_enrollment(db, &repo, task, owner, receipt.source_revision).await;
}

async fn assert_legacy_routed_confirmation(
    db: &DatabaseConnection,
    repo: &PostgresSdlcRepository,
    owner: &Principal,
    task: Uuid,
) {
    let state = repo.read(task, owner).await.unwrap();
    assert_eq!(state.draft_flow().unwrap(), None);
    let policy = repo
        .routing_policy(state.project_id, owner, None)
        .await
        .unwrap();
    let (reservation, _) = repo
        .reserve_pm_draft(
            task,
            owner,
            serde_json::from_value(json!({
                "requested_agent_id":policy.routes.project_manager.agent_id,
                "expected_owner_version":0,"expected_assignment_version":null,
                "idempotency_key":"legacy-routed:reserve"}))
            .unwrap(),
        )
        .await
        .unwrap();
    let mut pm = actor("source-pm", false);
    pm.scopes.insert(reservation.assignment.scope(task));
    let fence = MachineFence {
        assignment_id: reservation.assignment.assignment_id,
        execution_id: reservation.assignment.execution_id,
        agent_id: reservation.assignment.agent_id,
        assignment_version: reservation.assignment.version,
    };
    repo.claim_execution_lease(
        task,
        &pm,
        domain::sdlc_execution_lease::ClaimExecutionLease {
            expected_owner_version: reservation.owner_cas.version,
            fence: fence.clone(),
            idempotency_key: "legacy-routed:claim".into(),
        },
    )
    .await
    .unwrap();
    let command = prepare_enrollment_confirmation(repo, task, &reservation, &pm, &fence).await;
    let before = repo.read(task, owner).await.unwrap();
    let saved = repo
        .execute_verified(task, owner, command.clone(), reservation.clone(), None)
        .await
        .unwrap();
    let snapshot = repo.task_routing_snapshot(task, owner).await.unwrap();
    assert_eq!(snapshot.policy, policy);
    assert_eq!(saved["id"], json!(snapshot.confirmation_id));
    assert_eq!(saved["stage"], json!("Backlog"));
    let intent = repo.analysis_intent(task, owner).await.unwrap();
    assert_eq!(intent.confirmation_id, snapshot.confirmation_id);
    assert_eq!(intent.workflow, snapshot.policy.routes.analyst.workflow_key);
    let after = repo.read(task, owner).await.unwrap();
    assert_eq!(after.stage, Stage::Analysis);
    assert_eq!(after.status, None);
    assert_eq!(
        serde_json::to_value(after.assignment).unwrap(),
        serde_json::to_value(before.assignment).unwrap()
    );
    let inventory = db
        .query_one(statement(
            "SELECT (SELECT count(*) FROM sdlc_task_source_selections WHERE task_id=$1) AS selections,
             (SELECT count(*) FROM sdlc_task_source_bindings WHERE task_id=$1) AS bindings,
             (SELECT count(*) FROM sdlc_task_routing_snapshots WHERE task_id=$1) AS snapshots,
             (SELECT count(*) FROM sdlc_analysis_intents WHERE task_id=$1) AS intents",
            vec![task.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(inventory.try_get::<i64>("", "selections").unwrap(), 0);
    assert_eq!(inventory.try_get::<i64>("", "bindings").unwrap(), 0);
    assert_eq!(inventory.try_get::<i64>("", "snapshots").unwrap(), 1);
    assert_eq!(inventory.try_get::<i64>("", "intents").unwrap(), 1);
    assert_eq!(
        repo.execute_verified(task, owner, command, reservation, None)
            .await
            .unwrap(),
        saved
    );
    assert_eq!(
        repo.task_routing_snapshot(task, owner).await.unwrap(),
        snapshot
    );
}

async fn prepare_enrollment_confirmation(
    repo: &PostgresSdlcRepository,
    task: Uuid,
    reservation: &domain::sdlc_pm_draft::PmDraftReservation,
    pm: &Principal,
    fence: &MachineFence,
) -> SdlcCommand {
    let document:RequirementsDocument = serde_json::from_value(json!({"goal":"Synthetic same-task source selection",
        "scope":["One original root"],"exclusions":[],"scenarios":["Enrollment/confirmation serialization"],
        "acceptance_criteria":["Pinned source"],"constraints":[],"dependencies":[],"assumptions":[],
        "checklist":["enrollment-check"],"prerequisites":["enrollment-prerequisite"]})).unwrap();
    let content_hash = app::sdlc::canonical_hash(&document).unwrap();
    repo.execute_verified(
        task,
        pm,
        SdlcCommand::PublishRevision(PublishRevision {
            fence: fence.clone(),
            expected_requirement_revision: None,
            document,
            idempotency_key: "enrollment-requirements".into(),
        }),
        reservation.clone(),
        None,
    )
    .await
    .unwrap();
    let mut verifier = actor("source-verifier", false);
    verifier.scopes = [
        "task-tracker:write".into(),
        format!("task-tracker:sdlc:evidence:{task}:1"),
    ]
    .into_iter()
    .collect();
    for check in ["enrollment-check", "enrollment-prerequisite"] {
        repo.execute_verified(
            task,
            &verifier,
            SdlcCommand::Evidence(EvidenceCommand {
                fence: fence.clone(),
                requirement_revision: 1,
                content_hash: content_hash.clone(),
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
    SdlcCommand::Confirm {
        revision: 1,
        command: ConfirmCommand {
            content_hash,
            idempotency_key: "enrollment-confirm".into(),
            expected_routing_policy_version: Some(1),
        },
    }
}

async fn assert_frozen_enrollment(
    db: &DatabaseConnection,
    repo: &PostgresSdlcRepository,
    task: Uuid,
    owner: &Principal,
    revision: i64,
) {
    let readback = repo.task_source(task, owner).await.unwrap();
    let TaskSourceReadback::Blocked {
        binding: Some(binding),
        reason: SourceBlockReason::MissingCurrentExecution,
        ..
    } = readback
    else {
        panic!("confirmation must freeze source without inventing runtime")
    };
    assert_eq!(binding.source_revision, revision);
    assert_eq!(binding.task_id, task);
    let row = db.query_one(statement("SELECT count(*) AS n FROM sdlc_outbox WHERE task_id=$1 AND event_type='source.selected'",vec![task.into()])).await.unwrap().unwrap();
    assert_eq!(row.try_get::<i64>("", "n").unwrap(), 1);
}
