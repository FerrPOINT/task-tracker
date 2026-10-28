//! Existing PostgreSQL owner ledgers; Fleet/Workflow observations below are explicitly synthetic.
use super::*;
#[path = "attachment_artifacts_pg_tests.rs"]
mod artifact_tests;
#[path = "sdlc_configured_cli_pg_tests.rs"]
mod configured_cli_tests;
#[path = "sdlc_critical_blockers_pg_tests.rs"]
mod critical_blocker_tests;
use domain::sdlc_lifecycle::*;
use domain::sdlc_routing::{RoleRoutes, SetRoutingPolicy};
use sea_orm_migration::MigratorTrait;
use serde::de::DeserializeOwned;

fn parse<T: DeserializeOwned>(value: Json) -> T {
    serde_json::from_value(value).unwrap()
}

fn actor(subject: &str, human: bool, scopes: &[&str]) -> Principal {
    Principal {
        subject: subject.into(),
        human_session: human,
        trusted_human: human,
        scopes: scopes.iter().map(|scope| (*scope).into()).collect(),
    }
}

fn routes() -> RoleRoutes {
    let mut value = json!({});
    for (n, role) in [
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
        value[*role] = json!({"agent_id":Uuid::new_v4(),"fleet_config_revision":1,
            "instruction_profile":"single-task-v1","instruction_manifest_sha256":"a".repeat(64),
            "namespace_id":(n+1).to_string(),"namespace_name":format!("single-task-{suffix}"),
            "workflow_id":(n+11).to_string(),"workflow_key":format!("single-task-v1:{role}"),
            "profile":format!("single-task-v1-{suffix}"),"workflow_catalog_version":1,"workflow_catalog_sha256":"b".repeat(64)});
    }
    parse(value)
}

struct Fixture {
    db: DatabaseConnection,
    repo: PostgresSdlcRepository,
    project: Uuid,
    owner: Principal,
    writer: Principal,
    reader: Principal,
    roles: RoleRoutes,
}

impl Fixture {
    async fn new() -> Self {
        Self::new_with_migration_steps(None).await
    }

    async fn new_with_migration_steps(steps: Option<u32>) -> Self {
        let url = std::env::var("TT_CANDIDATES_TEST_DATABASE_URL")
            .expect("fresh disposable candidates DB required");
        let db = Database::connect(&url).await.unwrap();
        let row = db.query_one(statement("SELECT current_database() AS name,to_regclass('public.seaql_migrations') IS NULL AS fresh",vec![]))
            .await.unwrap().unwrap();
        assert!(
            row.try_get::<String>("", "name")
                .unwrap()
                .ends_with("_candidates_test")
        );
        assert!(
            row.try_get::<bool>("", "fresh").unwrap(),
            "use a fresh disposable database"
        );
        migration::Migrator::up(&db, steps).await.unwrap();
        let owner_id = Uuid::new_v4();
        let project = Uuid::new_v4();
        for (subject, id) in [
            ("owner", owner_id),
            ("pm", Uuid::new_v4()),
            ("verifier", Uuid::new_v4()),
            ("scheduler", Uuid::new_v4()),
            ("foreign", Uuid::new_v4()),
        ] {
            exec(&db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,$2,$3,$3,'!',$3,true)",
                vec![id.into(),format!("{subject}@example.test").into(),subject.into()]).await.unwrap();
        }
        exec(&db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,'CAND','Candidates',$2,$3)",
            vec![project.into(),owner_id.into(),Uuid::new_v4().into()]).await.unwrap();
        exec(&db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Candidates','[]' FROM projects WHERE id=$1",
            vec![project.into()]).await.unwrap();
        exec(&db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub IN ('pm','verifier','scheduler')",
            vec![project.into()]).await.unwrap();
        let repo = PostgresSdlcRepository::connect(
            &url,
            SdlcConfig {
                instance_id: "candidates-test".into(),
                orchestrator_subject: "pm".into(),
                verifier_subject: "verifier".into(),
                reservation_scheduler_subject: "scheduler".into(),
                single_task_project_ids: [project].into_iter().collect(),
            },
        )
        .await
        .unwrap();
        let roles = routes();
        let owner = actor("owner", true, &[]);
        super::super::source::configure_test_source(&repo, project, &owner).await;
        repo.set_routing_policy(
            project,
            &owner,
            SetRoutingPolicy {
                expected_version: None,
                routes: roles.clone(),
                idempotency_key: "policy".into(),
            },
        )
        .await
        .unwrap();
        Self {
            db,
            repo,
            project,
            owner,
            roles,
            writer: actor("scheduler", false, &["task-tracker:write"]),
            reader: actor("scheduler", false, &["task-tracker:read"]),
        }
    }

    fn query(&self, after: Option<Uuid>, limit: u16) -> DeliveryCandidatesQuery {
        DeliveryCandidatesQuery {
            project_id: self.project,
            after,
            limit,
        }
    }

    async fn draft(&self, key: &str, flow: Option<DraftFlow>) -> Uuid {
        self.repo
            .create_draft(
                self.project,
                &self.owner,
                CreateDraftCommand {
                    interaction_protocol: None,
                    flow,
                    title: key.into(),
                    description: "Synthetic test input, not live execution".into(),
                    idempotency_key: key.into(),
                },
            )
            .await
            .unwrap()
            .0
            .task_id
    }

    async fn confirmed(&self, key: &str, completed: bool) -> Uuid {
        let task = self.draft(key, Some(DraftFlow::SingleTaskV1)).await;
        let document: RequirementsDocument = parse(json!({
            "goal":"Candidate fixture","scope":["scope"],"exclusions":[],
            "scenarios":["scenario"],"acceptance_criteria":["criterion"],"constraints":[],
            "dependencies":[],"assumptions":[],"checklist":["check"],"prerequisites":["pre"]}));
        crate::sdlc::configured_test_support::confirmed_pm(
            &self.repo,
            task,
            &self.owner,
            self.roles.project_manager.agent_id,
            document,
            completed,
        )
        .await;
        if completed {
            self.assert_backlog_without_analysis(task).await;
        }
        task
    }

    async fn assert_backlog_without_analysis(&self, task: Uuid) {
        let state = self.repo.read(task, &self.owner).await.unwrap();
        assert_eq!(state.stage, Stage::Backlog);
        assert_eq!(state.status, Some(LifecycleStatus::Ready));
        assert!(state.questions.is_empty());
        assert_eq!(state.confirmations.len(), 1);
        assert!(!state.context(&self.owner).permissions.can_confirm);
        assert!(matches!(
            self.repo.analysis_intent(task, &self.owner).await,
            Err(AppError::NotFound(_))
        ));
        let row = self.db.query_one(statement("SELECT
            (SELECT count(*) FROM sdlc_analysis_intents WHERE task_id=$1) AS intents,
            (SELECT count(*) FROM sdlc_analysis_reservations WHERE task_id=$1) AS reservations,
            (SELECT count(*) FROM sdlc_delivery_assignments WHERE task_id=$1) AS assignments,
            (SELECT count(*) FROM sdlc_outbox WHERE task_id=$1 AND event_type='analysis.intent_created') AS events",
            vec![task.into()])).await.unwrap().unwrap();
        assert_eq!(row.try_get::<i64>("", "intents").unwrap(), 0);
        assert_eq!(row.try_get::<i64>("", "reservations").unwrap(), 0);
        assert_eq!(row.try_get::<i64>("", "assignments").unwrap(), 0);
        assert_eq!(row.try_get::<i64>("", "events").unwrap(), 0);
    }

    async fn queued_analysis(&self, key: &str, completed: bool) -> Uuid {
        let task = self.confirmed(key, completed).await;
        seed_analysis_fixture(&self.db, task, &self.owner).await;
        task
    }

    async fn snapshot(&self) -> Json {
        self.db.query_one(statement("SELECT jsonb_build_object(
            'tasks',(SELECT jsonb_agg(to_jsonb(t) ORDER BY task_id) FROM sdlc_tasks t),
            'heads',(SELECT jsonb_agg(to_jsonb(h) ORDER BY task_id) FROM sdlc_delivery_heads h),
            'pm_leases',(SELECT jsonb_agg(to_jsonb(l) ORDER BY lease_id) FROM sdlc_pm_execution_leases l),
            'assignments',(SELECT count(*) FROM sdlc_delivery_assignments),
            'holds',(SELECT count(*) FROM sdlc_delivery_holds),
            'operations',(SELECT count(*) FROM sdlc_delivery_operations),
            'outbox',(SELECT count(*) FROM sdlc_outbox),
            'issue_history',(SELECT count(*) FROM issue_status_history)) AS snapshot", vec![]))
            .await.unwrap().unwrap().try_get("", "snapshot").unwrap()
    }
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending in _candidates_test; no live agents"]
async fn pm_publication_stops_at_backlog_without_analysis_on_confirmation_completion_or_replay() {
    let f = Fixture::new().await;
    let task = f.confirmed("backlog-only", true).await;
    f.assert_backlog_without_analysis(task).await;
    let completion = f
        .repo
        .configured_pm_completion(task, &f.reader, None)
        .await
        .unwrap();
    assert!(completion.is_some());
    let before = f.snapshot().await;
    assert!(
        f.repo
            .read_delivery_candidates(&f.reader, f.query(None, 100))
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(matches!(
        f.repo
            .lifecycle_command(
                task,
                &f.writer,
                LifecycleCommand::EnsureQueued {
                    idempotency_key: "must-not-queue-analysis".into()
                }
            )
            .await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(f.snapshot().await, before);
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_CANDIDATES_TEST_DATABASE_URL ending in _candidates_test; no live agents"]
async fn actual_configured_discovery_is_read_only_and_never_auto_starts_downstream() {
    let mut f = Fixture::new().await;
    let mut expected = vec![];
    for key in ["first", "second", "third"] {
        expected.push(f.queued_analysis(key, true).await);
    }
    f.confirmed("consent-without-completion", false).await;
    f.draft("unconfirmed", Some(DraftFlow::SingleTaskV1)).await;
    f.draft("legacy", None).await;
    expected.sort();
    let before = f.snapshot().await;
    // Current CLI assignments do not opt into the legacy auto-dispatcher,
    // even when a standalone technical test imports a queued snapshot.
    for after in std::iter::once(None).chain(expected.iter().copied().map(Some)) {
        for limit in [1, 2, 100] {
            let page = f
                .repo
                .read_delivery_candidates(&f.reader, f.query(after, limit))
                .await
                .unwrap();
            assert_eq!(page.contract_version, 1);
            assert_eq!(page.tracker_instance_id, "candidates-test");
            assert_eq!(page.project_id, f.project);
            assert!(page.items.is_empty());
            assert_eq!(page.next_cursor, None);
        }
    }
    assert_eq!(f.snapshot().await, before);
    let foreign_project = Uuid::new_v4();
    exec(
        &f.db,
        "INSERT INTO projects(id,key,name,owner_id,default_board_id)
        SELECT $1,'FOREIGN','Foreign project',id,$2 FROM users WHERE central_sub='foreign'",
        vec![foreign_project.into(), Uuid::new_v4().into()],
    )
    .await
    .unwrap();
    let foreign_query = DeliveryCandidatesQuery {
        project_id: foreign_project,
        ..f.query(None, 50)
    };
    assert!(matches!(
        f.repo
            .read_delivery_candidates(&f.reader, foreign_query)
            .await,
        Err(AppError::Forbidden)
    ));
    for principal in [
        &f.owner,
        &f.writer,
        &actor("foreign", false, &["task-tracker:read"]),
    ] {
        assert!(matches!(
            f.repo
                .read_delivery_candidates(principal, f.query(None, 50))
                .await,
            Err(AppError::Forbidden)
        ));
    }
    let enabled_projects = std::mem::take(&mut f.repo.config.single_task_project_ids);
    assert!(matches!(
        f.repo
            .read_delivery_candidates(&f.reader, f.query(None, 50))
            .await,
        Err(AppError::Unavailable(_))
    ));
    f.repo.config.single_task_project_ids = enabled_projects;
    let task = expected[0];
    let machine_subject = f
        .repo
        .read(task, &f.owner)
        .await
        .unwrap()
        .assignment
        .unwrap()
        .machine_subject;
    let (queued, _) = f
        .repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::EnsureQueued {
                idempotency_key: "queue".into(),
            },
        )
        .await
        .unwrap();
    let before = f.snapshot().await;
    let page = f
        .repo
        .read_delivery_candidates(&f.reader, f.query(None, 100))
        .await
        .unwrap();
    assert!(page.items.is_empty());
    assert_eq!(page.next_cursor, None);
    assert_eq!(f.snapshot().await, before);
    assert!(
        f.repo
            .lifecycle_command(
                task,
                &f.writer,
                LifecycleCommand::Reserve {
                    expected_task_version: queued.task.version + 1,

                    machine_subject: Some(machine_subject.clone()),
                    idempotency_key: "stale-reserve".into(),
                }
            )
            .await
            .is_err()
    );
    assert_eq!(f.snapshot().await, before);
    f.repo
        .lifecycle_command(
            task,
            &f.writer,
            LifecycleCommand::Reserve {
                expected_task_version: queued.task.version,

                machine_subject: Some(machine_subject),
                idempotency_key: "reserve".into(),
            },
        )
        .await
        .unwrap();
    let page = f
        .repo
        .read_delivery_candidates(&f.reader, f.query(None, 100))
        .await
        .unwrap();
    assert!(page.items.is_empty());
    assert_eq!(page.next_cursor, None);
    let state = f.repo.read(expected[1], &f.owner).await.unwrap();
    let snapshot = f
        .repo
        .task_routing_snapshot(expected[1], &f.reader)
        .await
        .unwrap();
    let mut stale = snapshot.clone();
    stale.requirement_revision += 1;
    assert!(super::super::lifecycle::exact_snapshot(&state, &stale).is_err());
    stale = snapshot;
    stale.content_hash = "f".repeat(64);
    assert!(super::super::lifecycle::exact_snapshot(&state, &stale).is_err());
    exec(&f.db,"DELETE FROM project_members WHERE project_id=$1 AND user_id=(SELECT id FROM users WHERE central_sub='scheduler')",
        vec![f.project.into()]).await.unwrap();
    assert!(matches!(
        f.repo
            .read_delivery_candidates(&f.reader, f.query(None, 50))
            .await,
        Err(AppError::Forbidden)
    ));
    exec(&f.db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub='scheduler'",
        vec![f.project.into()]).await.unwrap();
    exec(
        &f.db,
        "UPDATE users SET is_active=false WHERE central_sub='scheduler'",
        vec![],
    )
    .await
    .unwrap();
    assert!(matches!(
        f.repo
            .read_delivery_candidates(&f.reader, f.query(None, 50))
            .await,
        Err(AppError::Forbidden)
    ));
}
