//! Existing PostgreSQL owner ledgers; Fleet/Workflow observations below are explicitly synthetic.
use super::*;
#[path = "attachment_artifacts_pg_tests.rs"]
mod artifact_tests;
#[path = "sdlc_critical_blockers_pg_tests.rs"]
mod critical_blocker_tests;
use domain::sdlc_execution_lease::ClaimExecutionLease;
use domain::sdlc_lifecycle::*;
use domain::sdlc_reservation::CompletePmDraft;
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
        migration::Migrator::up(&db, None).await.unwrap();
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
        let (reservation, _) = self
            .repo
            .reserve_pm_draft(
                task,
                &self.owner,
                parse(json!({
            "requested_agent_id":self.roles.project_manager.agent_id,"expected_owner_version":0,
            "expected_assignment_version":null,"idempotency_key":"pm-reserve"})),
            )
            .await
            .unwrap();
        let mut pm = actor("pm", false, &["task-tracker:read", "task-tracker:write"]);
        pm.scopes.insert(reservation.assignment.scope(task));
        let fence = MachineFence {
            assignment_id: reservation.assignment.assignment_id,
            execution_id: reservation.assignment.execution_id,
            agent_id: reservation.assignment.agent_id,
            assignment_version: reservation.assignment.version,
        };
        let (lease, _) = self
            .repo
            .claim_execution_lease(
                task,
                &pm,
                ClaimExecutionLease {
                    expected_owner_version: reservation.owner_cas.version,
                    fence: fence.clone(),
                    idempotency_key: "claim".into(),
                },
            )
            .await
            .unwrap();
        let document: RequirementsDocument = parse(
            json!({"goal":"Candidate fixture","scope":["scope"],"exclusions":[],
            "scenarios":["scenario"],"acceptance_criteria":["criterion"],"constraints":[],"dependencies":[],"assumptions":[],
            "checklist":["check"],"prerequisites":["pre"]}),
        );
        let hash = app::sdlc::canonical_hash(&document).unwrap();
        self.repo
            .execute_verified(
                task,
                &pm,
                SdlcCommand::PublishRevision(PublishRevision {
                    fence: fence.clone(),
                    expected_requirement_revision: None,
                    document,
                    idempotency_key: "publish".into(),
                }),
                reservation.clone(),
                None,
            )
            .await
            .unwrap();
        let mut verifier = actor("verifier", false, &["task-tracker:write"]);
        verifier
            .scopes
            .insert(format!("task-tracker:sdlc:evidence:{task}:1"));
        for check in ["check", "pre"] {
            self.repo
                .execute_verified(
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
        self.repo
            .execute_verified(
                task,
                &self.owner,
                SdlcCommand::Confirm {
                    revision: 1,
                    command: ConfirmCommand {
                        content_hash: hash.clone(),
                        idempotency_key: "confirm".into(),
                        expected_routing_policy_version: Some(1),
                    },
                },
                reservation,
                None,
            )
            .await
            .unwrap();
        if completed {
            self.repo
                .complete_pm_draft(
                    task,
                    &self.writer,
                    CompletePmDraft {
                        contract_version: 1,
                        fence,
                        lease_id: lease.lease.lease_id,
                        lease_version: lease.lease.version,
                        requirement_revision: 1,
                        content_hash: hash,
                        terminal_receipt_ref: "fixture://synthetic-native-terminal".into(),
                        terminal_receipt_sha256: "a".repeat(64),
                        workflow_receipt_ref: "fixture://synthetic-workflow-complete".into(),
                        workflow_receipt_sha256: "b".repeat(64),
                        idempotency_key: "complete".into(),
                    },
                )
                .await
                .unwrap();
        }
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
async fn actual_owner_discovery_is_read_only_paginated_and_pm_terminal_gated() {
    let mut f = Fixture::new().await;
    let mut expected = vec![];
    for key in ["first", "second", "third"] {
        expected.push(f.confirmed(key, true).await);
    }
    let pending = f.confirmed("consent-without-completion", false).await;
    f.draft("unconfirmed", Some(DraftFlow::SingleTaskV1)).await;
    f.draft("legacy", None).await;
    expected.sort();
    let before = f.snapshot().await;
    let mut cursor = None;
    let mut actual = vec![];
    loop {
        let page = f
            .repo
            .read_delivery_candidates(&f.reader, f.query(cursor, 1))
            .await
            .unwrap();
        assert_eq!(page.contract_version, 1);
        assert_eq!(page.tracker_instance_id, "candidates-test");
        assert_eq!(page.project_id, f.project);
        assert_eq!(page.items.len(), 1);
        let item = &page.items[0];
        assert_eq!(item.root_task_id, item.task_id);
        assert_eq!(item.requirement_revision, 1);
        assert_eq!(item.next_operation, CandidateOperation::EnsureQueued);
        assert_eq!(item.task_version, None);
        assert_ne!(item.task_id, pending);
        actual.push(item.task_id);
        if page.next_cursor.is_none() {
            break;
        }
        assert_eq!(page.next_cursor, Some(item.task_id));
        cursor = page.next_cursor;
        assert!(actual.len() < 4, "pagination must advance");
    }
    assert_eq!(actual, expected);
    let empty = f
        .repo
        .read_delivery_candidates(&f.reader, f.query(actual.last().copied(), 100))
        .await
        .unwrap();
    assert!(empty.items.is_empty());
    assert_eq!(empty.next_cursor, None);
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
    let task = actual[0];
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
    let candidate = page.items.iter().find(|item| item.task_id == task).unwrap();
    assert_eq!(candidate.next_operation, CandidateOperation::Reserve);
    assert_eq!(candidate.task_version, Some(queued.task.version));
    assert_eq!(f.snapshot().await, before);
    assert!(
        f.repo
            .lifecycle_command(
                task,
                &f.writer,
                LifecycleCommand::Reserve {
                    expected_task_version: queued.task.version + 1,
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
    assert!(
        !page
            .items
            .iter()
            .any(|item| item.task_id == task || item.task_id == pending)
    );
    let state = f.repo.read(actual[1], &f.owner).await.unwrap();
    let snapshot = f
        .repo
        .task_routing_snapshot(actual[1], &f.reader)
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
