//! Owned synthetic PostgreSQL only. Runtime/Workflow observations are not live evidence.
use super::*;

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn forward_start_migration_preserves_filled_110_history_without_auto_start() {
    Box::pin(async {
        let migrations = migration::Migrator::migrations();
        let checkpoint = migrations.iter().position(|migration| migration.name() == "m20261029_0000111_configured_analysis_start").unwrap();
        let f = Box::pin(Fixture::new_with_migration_steps(Some(checkpoint as u32))).await;
        let historical = Box::pin(f.queued_analysis("historical", true)).await;
        f.repo.lifecycle_command(historical, &f.writer, LifecycleCommand::EnsureQueued {
            idempotency_key: "historical-queue".into(),
        }).await.unwrap();
        let pending = Box::pin(f.confirmed("upgrade-backlog", true)).await;
        let before = f.snapshot().await;
        let old_rows: Json = f.db.query_one(statement("SELECT jsonb_agg(to_jsonb(o) ORDER BY task_id,actor_subject,idempotency_key) AS rows FROM sdlc_delivery_operations o",vec![])).await.unwrap().unwrap().try_get("","rows").unwrap();
        migration::Migrator::up(&f.db, None).await.unwrap();
        assert_eq!(f.snapshot().await, before);
        let retained: Json = f.db.query_one(statement("SELECT jsonb_agg(to_jsonb(o)-ARRAY['command','transaction_id'] ORDER BY task_id,actor_subject,idempotency_key) AS rows FROM sdlc_delivery_operations o",vec![])).await.unwrap().unwrap().try_get("","rows").unwrap();
        assert_eq!(retained, old_rows);
        f.assert_backlog_without_analysis(pending).await;
        assert!(f.repo.read_delivery_candidates(&f.reader, f.query(None,100)).await.unwrap().items.is_empty());
        let cmd = command(&f,pending,"after-upgrade").await;
        f.repo.lifecycle_command(pending,&f.owner,cmd).await.unwrap();
        let page = f.repo.read_delivery_candidates(&f.reader,f.query(None,100)).await.unwrap();
        assert_eq!(page.items.len(),1);
        assert_eq!(page.items[0].task_id,pending);
    }).await;
}

async fn command(f: &Fixture, task: Uuid, key: &str) -> LifecycleCommand {
    let completion = f
        .repo
        .configured_pm_completion(task, &f.owner, None)
        .await
        .unwrap()
        .unwrap();
    LifecycleCommand::StartAnalysis {
        pm_completion_id: completion.result.completion_id,
        idempotency_key: key.into(),
    }
}

#[tokio::test]
#[serial_test::serial]
#[ignore = "requires isolated process and fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn configured_scheduler_reserve_and_bind_keep_native_owner_and_space_guards() {
    use app::native_space_access::{NativeRequest, with_request};
    use axum::{
        Json as HttpJson, Router,
        extract::State,
        http::HeaderMap,
        routing::{get, post},
    };
    use std::sync::RwLock;

    // Seed through the existing business fixture before enabling the serving policies.
    // Run this selector alone: the real central bridge keeps its configuration per process.
    assert!(std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_none());
    assert!(std::env::var_os("TT_AUTH__CENTRAL_APPLICATION_ACCESS_REQUIRED").is_none());
    let mut f = Box::pin(Fixture::new()).await;
    // Create fresh identities and membership; existing identity aliases are immutable.
    let owner_identity = crate::sdlc::configured_test_support::service(&f.repo, f.project).await;
    let scheduler_identity =
        crate::sdlc::configured_test_support::service(&f.repo, f.project).await;
    let owner = Uuid::parse_str(&owner_identity.subject).unwrap();
    let scheduler = Uuid::parse_str(&scheduler_identity.subject).unwrap();
    f.owner = actor(&owner_identity.subject, true, &[]);
    f.writer = actor(&scheduler_identity.subject, false, &["task-tracker:write"]);
    f.reader = actor(&scheduler_identity.subject, false, &["task-tracker:read"]);
    f.repo.config.reservation_scheduler_subject = scheduler.to_string();
    let foreign = crate::sdlc::configured_test_support::service(&f.repo, f.project).await;
    let producer = crate::sdlc::configured_test_support::service(&f.repo, f.project).await;
    let task = Box::pin(f.confirmed("native-scheduler", true)).await;
    let start = command(&f, task, "native-scheduler-start").await;
    let initial = f
        .repo
        .lifecycle_command(task, &f.owner, start)
        .await
        .unwrap()
        .0;
    // Activation validates every project, including those seeded by migrations.
    // Preserve existing bindings and add the missing synthetic owner receipts.
    let unbound =
        f.db.query_all(statement(
            "SELECT id FROM projects p WHERE NOT EXISTS
        (SELECT 1 FROM tracker_namespace_bindings b WHERE b.resource_id=p.id)",
            vec![],
        ))
        .await
        .unwrap();
    for row in unbound {
        let project: Uuid = row.try_get("", "id").unwrap();
        let registry = Uuid::new_v4();
        let namespace = Uuid::new_v4();
        let binding = json!({"schema_version":1,
            "namespace":{"registry_instance_id":registry,"namespace_id":namespace},
            "resource":{"kind":"tracker_project","instance_id":Uuid::new_v4(),"resource_id":project},
            "operation_id":Uuid::new_v4(),"generation":1,"state":"active"});
        exec(&f.db, "INSERT INTO tracker_namespace_bindings(resource_id,registry_instance_id,namespace_id,generation,state,command)
            VALUES($1,$2,$3,1,'active',$4)",
            vec![project.into(),registry.into(),namespace.into(),binding.into()]).await.unwrap();
    }
    exec(
        &f.db,
        "INSERT INTO tracker_space_catalog(registry_instance_id,space_id,admin_revision,state)
        SELECT registry_instance_id,namespace_id,1,state FROM tracker_namespace_bindings
        ON CONFLICT(registry_instance_id,space_id) DO NOTHING",
        vec![],
    )
    .await
    .unwrap();
    exec(
        &f.db,
        "UPDATE tracker_space_catalog SET admin_revision=1 WHERE admin_revision IS NULL",
        vec![],
    )
    .await
    .unwrap();
    exec(
        &f.db,
        "INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id)
        SELECT resource_id,registry_instance_id,namespace_id FROM tracker_namespace_bindings
        ON CONFLICT(project_id,registry_instance_id,space_id) DO NOTHING",
        vec![],
    )
    .await
    .unwrap();
    exec(
        &f.db,
        "UPDATE tracker_space_admission_state SET active=true",
        vec![],
    )
    .await
    .unwrap();

    #[derive(Clone)]
    struct Authority {
        issuer: String,
        owner: Uuid,
        scheduler: Uuid,
        foreign: String,
        owner_permitted: bool,
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let authority = Arc::new(RwLock::new(Authority {
        issuer: issuer.clone(),
        owner,
        scheduler,
        foreign: foreign.subject.clone(),
        owner_permitted: true,
    }));
    let router = Router::new()
        .route("/oidc/jwks", get(|| async { HttpJson(json!({"keys":[]})) }))
        .route("/auth/tokens/introspect", get(
            |State(state): State<Arc<RwLock<Authority>>>, headers: HeaderMap| async move {
                let state = state.read().unwrap().clone();
                let (subject, scopes) = match headers["authorization"].to_str().unwrap() {
                    "Bearer sdlc_pat_scheduler_scope_write" => (state.scheduler.to_string(), vec!["task-tracker:write"]),
                    "Bearer sdlc_pat_scheduler_scope_read" => (state.scheduler.to_string(), vec!["task-tracker:read"]),
                    "Bearer sdlc_pat_scheduler_scope_mixed" => (state.scheduler.to_string(), vec!["task-tracker:read", "task-tracker:write"]),
                    "Bearer sdlc_pat_scheduler_scope_foreign" => (state.foreign, vec!["task-tracker:write"]),
                    _ => panic!("unexpected synthetic credential"),
                };
                let now = chrono::Utc::now();
                HttpJson(json!({"sub":subject,"email":"scheduler-scope@example.test","subject_role":"service_account",
                    "scopes":scopes,"observed_at":now,"expires_at":now+chrono::Duration::seconds(300)}))
            }))
        .route("/auth/access/decisions", post(
            |State(state): State<Arc<RwLock<Authority>>>, headers: HeaderMap, HttpJson(request): HttpJson<Json>| async move {
                assert_eq!(headers["authorization"], "Bearer scheduler-scope-owner-reader");
                let state = state.read().unwrap().clone();
                assert_eq!(request, json!({"schema_version":1,"subjects":[state.owner]}));
                let applications = if state.owner_permitted { vec!["task-tracker", "fleet-control"] } else { vec!["task-tracker"] };
                let now = chrono::Utc::now();
                HttpJson(json!({"schema_version":1,"decisions":[{"schema_version":1,"issuer":state.issuer,
                    "subject":state.owner,"role":"user","applications":applications,"revision":1,"enforced":true,
                    "observed_at":now,"expires_at":now+chrono::Duration::seconds(30)}]}))
            }))
        .with_state(authority.clone());
    unsafe {
        std::env::set_var("TT_AUTH__CENTRAL_JWKS_URI", format!("{issuer}/oidc/jwks"));
        std::env::set_var("TT_AUTH__CENTRAL_ISSUER", &issuer);
        std::env::set_var("TT_AUTH__CENTRAL_APPLICATION_ACCESS_REQUIRED", "true");
        std::env::set_var(
            "TT_AUTH__CENTRAL_APPLICATION_ACCESS_READER_TOKEN",
            "scheduler-scope-owner-reader",
        );
    }
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let bridge = &app::space_access::CENTRAL_BRIDGE;
    let write = bridge
        .try_verified_token("sdlc_pat_scheduler_scope_write")
        .await
        .unwrap();
    let read = bridge
        .try_verified_token("sdlc_pat_scheduler_scope_read")
        .await
        .unwrap();
    let mixed = bridge
        .try_verified_token("sdlc_pat_scheduler_scope_mixed")
        .await
        .unwrap();
    let other = bridge
        .try_verified_token("sdlc_pat_scheduler_scope_foreign")
        .await
        .unwrap();
    assert!(
        app::native_space_access::application_policy_required()
            .await
            .unwrap()
    );
    assert!(
        matches!(
            NativeRequest::verified(&write, true, false),
            Err(AppError::Forbidden)
        ),
        "the separate write-only credential cannot enter a GET request"
    );
    let scope = |proof| NativeRequest::verified(proof, true, true).unwrap();

    async fn denied(
        label: &str,
        f: &Fixture,
        task: Uuid,
        actor: &Principal,
        native: NativeRequest,
        command: LifecycleCommand,
    ) {
        let before = f.snapshot().await;
        let outcome = Box::pin(with_request(
            native,
            f.repo.lifecycle_command(task, actor, command),
        ))
        .await;
        assert!(
            matches!(&outcome, Err(AppError::Forbidden)),
            "{label}: expected Forbidden, got {outcome:?}"
        );
        assert_eq!(
            f.snapshot().await,
            before,
            "{label}: denial must not leave a command, hold or assignment"
        );
    }
    async fn scheduler_membership(f: &Fixture, present: bool) {
        // Synthetic owner admission for fixture administration, as in the
        // unified-space SQL tests; the normal owner/space/clock guards still run.
        let tx = f.db.begin().await.unwrap();
        let row = tx.query_one(statement("SELECT jsonb_build_object(
            'schema_version',1,'kind','human','user_id',u.id,
            'identity',jsonb_build_object('issuer','https://auth.example.test','subject',u.central_sub),
            'active_spaces',s.spaces,'readable_spaces',s.spaces,'policy_revision',1,'writable',true,
            'valid_until',floor(extract(epoch FROM clock_timestamp()))::bigint+30) AS actor
            FROM projects p JOIN users u ON u.id=p.owner_id AND u.is_active
            CROSS JOIN LATERAL (SELECT jsonb_agg(jsonb_build_object(
                'registry_instance_id',c.registry_instance_id,'space_id',c.space_id)) AS spaces
                FROM tracker_project_spaces l JOIN tracker_space_catalog c USING(registry_instance_id,space_id)
                WHERE l.project_id=p.id AND c.state='active' AND c.admin_revision IS NOT NULL) s
            WHERE p.id=$1", vec![f.project.into()])).await.unwrap().unwrap();
        let owner: Json = row.try_get("", "actor").unwrap();
        crate::guarded_db::bind_commit_clock(&tx, owner["valid_until"].as_i64().unwrap())
            .await
            .unwrap();
        exec(
            &tx,
            "SELECT set_config('pdlc.tracker.write_actor',$1,true)",
            vec![owner.to_string().into()],
        )
        .await
        .unwrap();
        let sql = if present {
            "INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub=$2"
        } else {
            "DELETE FROM project_members WHERE project_id=$1 AND user_id=(SELECT id FROM users WHERE central_sub=$2)"
        };
        exec(
            &tx,
            sql,
            vec![f.project.into(), f.writer.subject.clone().into()],
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    let mut next = LifecycleCommand::Reserve {
        expected_task_version: initial.task.version,
        machine_subject: Some(producer.subject.clone()),
        idempotency_key: "native-scheduler-reserve".into(),
    };
    for phase in ["reserve", "bind"] {
        // The same subject's legitimate GET frame cannot borrow its separate write credential.
        denied(
            &format!("{phase}/read_frame"),
            &f,
            task,
            &f.writer,
            NativeRequest::verified(&read, true, false).unwrap(),
            next.clone(),
        )
        .await;
        let mut unregistered = foreign.clone();
        unregistered.scopes = ["task-tracker:write".into()].into();
        denied(
            &format!("{phase}/unregistered_scheduler"),
            &f,
            task,
            &unregistered,
            scope(&other),
            next.clone(),
        )
        .await;
        let mut combined = f.writer.clone();
        combined.scopes.insert("task-tracker:read".into());
        denied(
            &format!("{phase}/mixed_scopes"),
            &f,
            task,
            &combined,
            scope(&mixed),
            next.clone(),
        )
        .await;

        scheduler_membership(&f, false).await;
        denied(
            &format!("{phase}/revoked_membership"),
            &f,
            task,
            &f.writer,
            scope(&write),
            next.clone(),
        )
        .await;
        scheduler_membership(&f, true).await;

        exec(
            &f.db,
            "UPDATE tracker_space_catalog SET state='restoring'",
            vec![],
        )
        .await
        .unwrap();
        denied(
            &format!("{phase}/restoring_space"),
            &f,
            task,
            &f.writer,
            scope(&write),
            next.clone(),
        )
        .await;
        exec(
            &f.db,
            "UPDATE tracker_space_catalog SET state='active'",
            vec![],
        )
        .await
        .unwrap();
        authority.write().unwrap().owner_permitted = false;
        denied(
            &format!("{phase}/revoked_owner_application"),
            &f,
            task,
            &f.writer,
            scope(&write),
            next.clone(),
        )
        .await;
        authority.write().unwrap().owner_permitted = true;

        let native = scope(&write);
        let original_deadline = native.deadline();
        let retained_clock = native.clone();
        let (saved, replay) = Box::pin(with_request(
            native,
            f.repo.lifecycle_command(task, &f.writer, next),
        ))
        .await
        .unwrap();
        assert!(!replay);
        assert!(
            retained_clock.deadline() <= original_deadline,
            "owner proof cannot renew the native clock"
        );
        if phase == "reserve" {
            assert_eq!(saved.task.runtime, RuntimeState::Reserved);
            assert_eq!(
                saved
                    .assignment
                    .as_ref()
                    .unwrap()
                    .machine_subject
                    .as_deref(),
                Some(producer.subject.as_str())
            );
            next = LifecycleCommand::BindRun {
                fence: fence(&saved),
                binding: RunBinding {
                    chat_id: Uuid::new_v4(),
                    run_id: Uuid::new_v4(),
                    machine_subject: producer.subject.clone(),
                    dispatch_receipt: ArtifactReceipt {
                        reference: "fixture://scheduler-native-acceptance".into(),
                        sha256: "a".repeat(64),
                    },
                },
                idempotency_key: "native-scheduler-bind".into(),
            };
        } else {
            assert_eq!(saved.task.runtime, RuntimeState::Running);
            assert_eq!(saved.binding.unwrap().machine_subject, producer.subject);
            break;
        }
    }
    let before_read = f.snapshot().await;
    let context = Box::pin(with_request(
        NativeRequest::verified(&read, true, false).unwrap(),
        f.repo.context(task, &f.reader),
    ))
    .await
    .unwrap();
    assert_eq!(context.task_id, task);
    assert_eq!(
        f.snapshot().await,
        before_read,
        "the separate reader keeps current context read-only"
    );
    server.abort();
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn explicit_start_is_atomic_discovered_once_and_replays_original_after_advancement() {
    Box::pin(async {
        let f = Box::pin(Fixture::new()).await;
        let task = Box::pin(f.confirmed("explicit", true)).await;
        Box::pin(f.confirmed("not-started", true)).await;
        assert!(f.repo.read_delivery_candidates(&f.reader, f.query(None, 100)).await.unwrap().items.is_empty());
        let original_command = command(&f, task, "start").await;
        let (initial, replay) = f.repo.lifecycle_command(task, &f.owner, original_command.clone()).await.unwrap();
        assert!(!replay);
        assert_eq!(initial.task.stage, LifecycleStage::Analysis);
        assert_eq!(initial.task.runtime, RuntimeState::Queued);
        assert_eq!(initial.task.version, 1);
        let intent = f.repo.analysis_intent(task, &f.owner).await.unwrap();
        assert_eq!(intent.confirmation_id, initial.task.routing.confirmation_id);
        assert_eq!(intent.workflow, initial.task.routing.policy.routes.analyst.workflow_key);
        let state = f.repo.read(task, &f.owner).await.unwrap();
        let completed = f.repo.configured_pm_completion(task, &f.owner, None).await.unwrap().unwrap().result;
        assert!(app::sdlc::analysis_intent(&state, state.current_confirmation().unwrap(), intent.intent_id).is_err());
        for field in ["task_id", "owner_subject", "version", "stage", "runtime"] {
            let mut raw = json!(initial);
            raw["task"][field] = match field {
                "task_id" => json!(Uuid::new_v4()),
                "version" => json!(2),
                "stage" => json!("Backlog"),
                "runtime" => json!("running"),
                _ => json!("foreign"),
            };
            let wrong: LifecycleReceipt = parse(raw);
            assert!(app::sdlc_lifecycle::configured_started_intent(&state, &completed, &wrong, intent.intent_id).is_err(), "{field}");
        }
        let before_discovery = f.snapshot().await;
        let page = f.repo.read_delivery_candidates(&f.reader, f.query(None, 1)).await.unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].task_id, task);
        assert_eq!(f.snapshot().await, before_discovery);
        assert_eq!(page.items[0].next_operation, domain::sdlc_candidates::CandidateOperation::Reserve);
        let counts = f.db.query_one(statement("SELECT
            (SELECT count(*) FROM sdlc_analysis_intents WHERE task_id=$1) AS intents,
            (SELECT count(*) FROM sdlc_delivery_operations WHERE task_id=$1 AND command IS NOT NULL) AS starts,
            (SELECT count(*) FROM sdlc_outbox WHERE task_id=$1 AND event_type IN ('analysis.intent_created','delivery.queued')) AS events,
            (SELECT count(*) FROM sdlc_delivery_projection_permits WHERE task_id=$1) AS permits",
            vec![task.into()])).await.unwrap().unwrap();
        assert_eq!(counts.try_get::<i64>("", "intents").unwrap(), 1);
        assert_eq!(counts.try_get::<i64>("", "starts").unwrap(), 1);
        assert_eq!(counts.try_get::<i64>("", "events").unwrap(), 2);
        assert_eq!(counts.try_get::<i64>("", "permits").unwrap(), 0);
        // Existing configured role-result path advances the same task without another human action.
        let producer_user = Uuid::new_v4();
        exec(&f.db, "INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active)
            VALUES($1,'start-agent@example.test','start-agent','Start Agent','!','77777777-7777-4777-8777-777777777777',true)",
            vec![producer_user.into()]).await.unwrap();
        exec(&f.db, "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'developer')",
            vec![f.project.into(), producer_user.into()]).await.unwrap();
        let storage = domain::InMemoryStorage::default();
        let running = Box::pin(output(&f, task, Box::pin(bind(&f, task)).await, &storage)).await;
        let terminal = Box::pin(proposal(&f, &running)).await;
        let advanced = f.repo.lifecycle_command(task, &f.writer, LifecycleCommand::Complete {
            fence: fence(&running.head), terminal, result: result(&running), idempotency_key: "accepted-analysis".into(),
        }).await.unwrap().0;
        assert_eq!(advanced.task.task_id, task);
        assert_eq!(advanced.task.stage, LifecycleStage::Architecture);
        assert_eq!(advanced.task.runtime, RuntimeState::Queued);
        let before = f.snapshot().await;
        let (saved, replay) = f.repo.lifecycle_command(task, &f.owner, original_command).await.unwrap();
        assert!(replay);
        assert_eq!(json!(saved), json!(initial));
        assert_eq!(json!(f.repo.lifecycle_operation(task, &f.owner, "start").await.unwrap()), json!(initial));
        assert_eq!(f.snapshot().await, before);
        let current = f.repo.lifecycle_read(task, &f.reader).await.unwrap().receipt;
        assert_eq!(json!(current), json!(advanced));
        let page = f.repo.read_delivery_candidates(&f.reader, f.query(None, 100)).await.unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].task_id, task);
        assert_eq!(f.snapshot().await, before);
    }).await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn start_refuses_nonowners_no_opt_in_missing_consent_stale_and_foreign_receipts() {
    Box::pin(async {
        let mut f = Box::pin(Fixture::new()).await;
        let task = Box::pin(f.confirmed("guarded", true)).await;
        let cmd = command(&f, task, "start").await;
        let state = f.repo.read(task, &f.owner).await.unwrap();
        let completed = f
            .repo
            .configured_pm_completion(task, &f.owner, None)
            .await
            .unwrap()
            .unwrap();
        let completed = completed.result;
        app::sdlc_lifecycle::validate_start(&state, &completed).unwrap();
        for (field, value) in [
            ("owner_subject", json!("foreign")),
            ("project_id", json!(Uuid::new_v4())),
            ("confirmation_revision", json!(2)),
            ("confirmations", json!([])),
        ] {
            let mut raw = json!(state);
            raw[field] = value;
            let stale: TaskState = parse(raw);
            assert!(
                app::sdlc_lifecycle::validate_start(&stale, &completed).is_err(),
                "{field}"
            );
        }
        let mut stale = state.clone();
        stale.revisions.last_mut().unwrap().content_hash = "d".repeat(64);
        assert!(app::sdlc_lifecycle::validate_start(&stale, &completed).is_err());
        let mut stale = state.clone();
        stale.assignment.as_mut().unwrap().version += 1;
        assert!(app::sdlc_lifecycle::validate_start(&stale, &completed).is_err());
        let before = f.snapshot().await;
        for principal in [
            f.writer.clone(),
            actor("owner", false, &[]),
            actor("foreign", true, &["admin"]),
            actor("scheduler", true, &["admin"]),
        ] {
            assert!(matches!(
                f.repo
                    .lifecycle_command(task, &principal, cmd.clone())
                    .await,
                Err(AppError::Forbidden)
            ));
        }
        f.repo.config.single_task_project_ids.clear();
        assert!(matches!(
            f.repo.lifecycle_command(task, &f.owner, cmd.clone()).await,
            Err(AppError::Unavailable(_))
        ));
        f.repo.config.single_task_project_ids.insert(f.project);
        let wrong = LifecycleCommand::StartAnalysis {
            pm_completion_id: Uuid::new_v4(),
            idempotency_key: "wrong-receipt".into(),
        };
        assert!(matches!(
            f.repo.lifecycle_command(task, &f.owner, wrong).await,
            Err(AppError::Conflict(_))
        ));
        assert_eq!(f.snapshot().await, before);
        let unconfirmed = f.draft("no-consent", Some(DraftFlow::SingleTaskV1)).await;
        let original_project = f.project;
        f.project = Uuid::new_v4();
        f.repo.config.single_task_project_ids.insert(f.project);
        exec(&f.db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) SELECT $1,'OTHER','Other synthetic project',owner_id,$2 FROM projects WHERE id=$3",
            vec![f.project.into(),Uuid::new_v4().into(),original_project.into()]).await.unwrap();
        exec(&f.db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Other','[]' FROM projects WHERE id=$1",vec![f.project.into()]).await.unwrap();
        crate::sdlc::source::configure_test_source(&f.repo, f.project, &f.owner).await;
        f.repo.set_routing_policy(f.project,&f.owner,SetRoutingPolicy {
            expected_version:None,routes:f.roles.clone(),idempotency_key:"other-policy".into(),
        }).await.unwrap();
        let foreign = Box::pin(f.confirmed("another-project-receipt", true)).await;
        let foreign_command = command(&f, foreign, "foreign-receipt").await;
        f.project = original_project;
        let before = f.snapshot().await;
        for target in [task, unconfirmed] {
            assert!(matches!(
                f.repo
                    .lifecycle_command(target, &f.owner, foreign_command.clone())
                    .await,
                Err(AppError::Conflict(_))
            ));
        }
        assert_eq!(f.snapshot().await, before);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn start_race_payload_conflict_and_current_access_readback_do_not_duplicate_effects() {
    Box::pin(async {
        let f = Box::pin(Fixture::new()).await;
        let task = Box::pin(f.confirmed("race", true)).await;
        let cmd = command(&f, task, "race-key").await;
        let (a, b) = tokio::join!(
            f.repo.lifecycle_command(task, &f.owner, cmd.clone()),
            f.repo.lifecycle_command(task, &f.owner, cmd.clone())
        );
        let (a, ar) = a.unwrap();
        let (b, br) = b.unwrap();
        assert_ne!(ar, br);
        assert_eq!(json!(a), json!(b));
        let other = Box::pin(f.confirmed("different-key-race", true)).await;
        let first = command(&f, other, "first").await;
        let second = command(&f, other, "second").await;
        let (first, second) = tokio::join!(
            f.repo.lifecycle_command(other, &f.owner, first),
            f.repo.lifecycle_command(other, &f.owner, second)
        );
        assert_ne!(first.is_ok(), second.is_ok());
        let rejected = if first.is_err() { first } else { second };
        assert!(matches!(rejected, Err(AppError::Conflict(_))));
        let before = f.snapshot().await;
        let changed = LifecycleCommand::StartAnalysis {
            pm_completion_id: Uuid::new_v4(),
            idempotency_key: "race-key".into(),
        };
        assert!(matches!(
            f.repo.lifecycle_command(task, &f.owner, changed).await,
            Err(AppError::Conflict(_))
        ));
        let new_key = command(&f, task, "second-key").await;
        assert!(matches!(
            f.repo.lifecycle_command(task, &f.owner, new_key).await,
            Err(AppError::Conflict(_))
        ));
        assert_eq!(f.snapshot().await, before);
        exec(&f.db,"UPDATE projects SET owner_id=(SELECT id FROM users WHERE central_sub='foreign') WHERE id=$1",vec![f.project.into()]).await.unwrap();
        assert!(matches!(f.repo.lifecycle_operation(task,&f.owner,"race-key").await,Err(AppError::Forbidden)));
        assert!(matches!(f.repo.lifecycle_command(task,&f.owner,cmd.clone()).await,Err(AppError::Forbidden)));
        exec(&f.db,"UPDATE projects SET owner_id=(SELECT id FROM users WHERE central_sub='owner') WHERE id=$1",vec![f.project.into()]).await.unwrap();
        exec(
            &f.db,
            "UPDATE users SET is_active=false WHERE central_sub='owner'",
            vec![],
        )
        .await
        .unwrap();
        assert!(matches!(
            f.repo.lifecycle_operation(task, &f.owner, "race-key").await,
            Err(AppError::Forbidden)
        ));
        assert!(matches!(
            f.repo.lifecycle_command(task, &f.owner, cmd).await,
            Err(AppError::Forbidden)
        ));
        assert_eq!(f.snapshot().await, before);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn start_sql_rejects_detached_command_and_direct_permit_bypass() {
    Box::pin(async {
        let f = Box::pin(Fixture::new()).await;
        let task = Box::pin(f.confirmed("sql-guard", true)).await;
        let cmd = command(&f, task, "start").await;
        let (receipt, _) = f.repo.lifecycle_command(task, &f.owner, cmd.clone()).await.unwrap();
        let detached = Box::pin(f.confirmed("detached", true)).await;
        let detached_cmd = command(&f, detached, "detached-key").await;
        let mut forged = receipt.clone();
        forged.task.task_id = detached;
        let state = f.repo.read(detached, &f.owner).await.unwrap();
        let before = f.snapshot().await;
        let tx = f.db.begin().await.unwrap();
        assert!(PostgresSdlcRepository::save_delivery_projection(&tx, &f.owner, &forged, true).await.is_err());
        tx.rollback().await.unwrap();
        // A plausible operation without its exact queued projection/intent/outbox cannot commit.
        let row = f.db.query_one(statement("SELECT payload FROM sdlc_task_routing_snapshots WHERE task_id=$1", vec![detached.into()])).await.unwrap().unwrap();
        forged.task.routing = parse(row.try_get("", "payload").unwrap());
        let tx = f.db.begin().await.unwrap();
        exec(&tx, "INSERT INTO sdlc_delivery_operations(task_id,actor_subject,idempotency_key,payload_hash,result,event_id,command,transaction_id)
            VALUES($1,$2,$3,$4,$5,$6,$7,pg_current_xact_id())", vec![detached.into(), f.owner.subject.clone().into(), detached_cmd.key().into(),
            app::sdlc::canonical_hash(&detached_cmd).unwrap().into(), json_value(&forged).unwrap(), Uuid::new_v4().into(), json_value(&detached_cmd).unwrap()]).await.unwrap();
        assert!(tx.commit().await.is_err());
        assert_eq!(f.snapshot().await, before);
        assert_eq!(state.stage, Stage::Backlog);
    }).await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn pm_completion_automatically_queues_once_without_impersonating_owner() {
    Box::pin(async {
        let f = Box::pin(Fixture::new()).await;
        let task = Box::pin(f.confirmed_with_rollout("automatic", true, true)).await;
        let state = f.repo.read(task, &f.owner).await.unwrap();
        assert_eq!(state.stage, Stage::Analysis);
        assert_eq!(state.status, Some(LifecycleStatus::Ready));
        let receipt = f.repo.lifecycle_read(task, &f.reader).await.unwrap().receipt;
        assert_eq!(receipt.task.runtime, RuntimeState::Queued);
        assert!(receipt.assignment.is_none());
        let completion = f.repo.configured_pm_completion(task, &f.owner, None).await.unwrap().unwrap();
        assert_eq!(completion.result.stage, Stage::Backlog); // Immutable PM fact, not the current cursor.
        let intent = f.repo.analysis_intent(task, &f.owner).await.unwrap();
        assert_eq!(intent.content_hash, completion.result.content_hash);
        assert_eq!(intent.confirmation_id, completion.result.confirmation_id);
        let row = f.db.query_one(statement("SELECT actor_subject,transaction_id=(SELECT transaction_id FROM sdlc_configured_pm_completions WHERE completion_id=$2) AS same_transaction FROM sdlc_delivery_operations WHERE task_id=$1 AND command IS NOT NULL", vec![task.into(),completion.result.completion_id.into()])).await.unwrap().unwrap();
        assert_ne!(row.try_get::<String>("", "actor_subject").unwrap(), f.owner.subject);
        let pm_subject = state.assignment.unwrap().machine_subject;
        assert_eq!(row.try_get::<String>("", "actor_subject").unwrap(), pm_subject);
        let pm = actor(&pm_subject, false, &["task-tracker:read", "task-tracker:write"]);
        assert!(f.repo.read(task, &pm).await.is_err());
        let saved = f.repo.configured_pm_completion(task, &pm, None).await.unwrap().unwrap();
        assert_eq!(saved.result.completion_id, completion.result.completion_id);
        assert!(f.repo.pm_completion_state(task, &pm).await.is_ok());
        let foreign = actor(&Uuid::new_v4().to_string(), false, &["task-tracker:read", "task-tracker:write"]);
        assert!(f.repo.configured_pm_completion(task, &foreign, None).await.is_err());
        assert!(row.try_get::<bool>("", "same_transaction").unwrap());
        let before = f.snapshot().await;
        for _ in 0..2 {
            let current = f.repo.read_delivery_candidates(&f.reader, f.query(None,100)).await.unwrap();
            assert_eq!(current.items.len(),1);
            assert_eq!(current.items[0].task_id,task);
        }
        assert_eq!(before,f.snapshot().await);
        assert!(matches!(f.repo.lifecycle_command(task,&f.owner,command(&f,task,"manual-after-auto").await).await,Err(AppError::Conflict(_))));
        assert_eq!(before,f.snapshot().await);
    }).await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn automatic_start_requires_completion_and_respects_disabled_project() {
    Box::pin(async {
        let f = Box::pin(Fixture::new()).await;
        let pending = Box::pin(f.confirmed_with_rollout("consent-only",false,true)).await;
        assert_eq!(f.repo.read(pending,&f.owner).await.unwrap().stage,Stage::Draft);
        assert!(f.repo.analysis_intent(pending,&f.owner).await.is_err());
        let retained = Box::pin(f.confirmed("disabled",true)).await;
        f.assert_backlog_without_analysis(retained).await;
        let before=f.snapshot().await;
        let tx=f.db.begin().await.unwrap();
        assert!(tx.execute(statement("UPDATE sdlc_tasks SET state=state||'{\"stage\":\"Analysis\",\"status\":\"Ready\"}'::jsonb WHERE task_id=$1",vec![pending.into()])).await.is_err());
        tx.rollback().await.unwrap();
        assert_eq!(before,f.snapshot().await);
    }).await;
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn forward_auto_start_preserves_completed_backlog_and_manual_start_history() {
    Box::pin(async {
        let migrations=migration::Migrator::migrations();
        let checkpoint=migrations.iter().position(|migration| migration.name() == "m20261103_0000112_configured_analysis_auto_start").unwrap();
        let f=Box::pin(Fixture::new_with_migration_steps(Some(checkpoint as u32))).await;
        let retained=Box::pin(f.confirmed("pre-auto",true)).await;
        // Historical images can have LF function bodies while the new source uses CRLF.
        for function in ["sdlc_critical_pm_lifecycle_insert_gate()", "sdlc_critical_pm_lifecycle_commit_gate()", "sdlc_configured_cli_completion_transition()"] {
            let row=f.db.query_one(statement("SELECT replace(pg_get_functiondef($1::regprocedure),chr(13)||chr(10),chr(10)) AS definition",vec![function.into()])).await.unwrap().unwrap();
            f.db.execute_unprepared(&row.try_get::<String>("","definition").unwrap()).await.unwrap();
        }
        let before=f.snapshot().await;
        let rows:Json=f.db.query_one(statement("SELECT jsonb_agg(to_jsonb(c) ORDER BY completion_id) AS rows FROM sdlc_configured_pm_completions c",vec![])).await.unwrap().unwrap().try_get("","rows").unwrap();
        migration::Migrator::up(&f.db,None).await.unwrap();
        assert_eq!(before,f.snapshot().await);
        let after:Json=f.db.query_one(statement("SELECT jsonb_agg(to_jsonb(c)-'transaction_id' ORDER BY completion_id) AS rows FROM sdlc_configured_pm_completions c",vec![])).await.unwrap().unwrap().try_get("","rows").unwrap();
        assert_eq!(rows,after);
        f.assert_backlog_without_analysis(retained).await;
        assert!(f.repo.read_delivery_candidates(&f.reader,f.query(None,100)).await.unwrap().items.is_empty());
    }).await;
}
