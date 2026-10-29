//! Owned synthetic PostgreSQL only. Runtime/Workflow observations are not live evidence.
use super::*;

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending _candidates_test"]
async fn forward_start_migration_preserves_filled_110_history_without_auto_start() {
    Box::pin(async {
        let migrations = migration::Migrator::migrations();
        assert_eq!(migrations.last().unwrap().name(), "m20261029_0000111_configured_analysis_start");
        let f = Box::pin(Fixture::new_with_migration_steps(Some((migrations.len()-1) as u32))).await;
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
