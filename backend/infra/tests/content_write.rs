//! The real repository -> adapter -> PostgreSQL boundary on a full disposable schema.
#[path = "support/write_context.rs"]
mod write_context;
use app::space_access::with_request_context;
use infra::repos::{SeaOrmRepositories, to_domain_repositories};
use migration::MigratorTrait;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TransactionTrait};
use shared::{AppError, BoardId, ProjectId, ProjectKey, space_access::*};
use std::sync::Arc;
use uuid::Uuid;
fn statement(query: &str, args: impl IntoIterator<Item = sea_orm::Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, query, args)
}
fn label(project: ProjectId, name: &str) -> domain::Label {
    domain::Label {
        id: shared::LabelId::new(),
        project_id: project,
        name: name.into(),
        color: "#123456".into(),
    }
}
#[tokio::test]
#[ignore = "requires fresh owned *_pdlc4_space_test PostgreSQL instance"]
async fn content_writes_require_current_actor_role_and_active_admitted_space_through_commit() {
    let original = std::env::var("TT_UNIFIED_SPACE_TEST_DATABASE_URL").unwrap();
    let root = Database::connect(&original).await.unwrap();
    assert_eq!(
        root.query_one(statement("SELECT current_database() AS n", []))
            .await
            .unwrap()
            .unwrap()
            .try_get::<String>("", "n")
            .unwrap(),
        "unified_pdlc4_space_test"
    );
    root.execute_unprepared("CREATE DATABASE writes_pdlc4_space_test")
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&original).unwrap();
    url.set_path("/writes_pdlc4_space_test");
    let db = Database::connect(url.to_string()).await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    let connection = Database::connect(url.to_string()).await.unwrap();
    let pool = connection.get_postgres_connection_pool().clone();
    let repos = Arc::new(to_domain_repositories(SeaOrmRepositories::new(connection)));
    let mut users = Vec::new();
    for name in ["owner", "reader", "other-owner"] {
        let user = repos
            .users
            .find_or_create_central_user(
                &Uuid::new_v4().to_string(),
                &format!("{name}@example.test"),
                name,
            )
            .await
            .unwrap();
        db.execute(statement(
            "UPDATE users SET central_sub=$1 WHERE id=$2",
            [user.id.to_string().into(), user.id.as_uuid().into()],
        ))
        .await
        .unwrap();
        users.push(user);
    }
    let owner = users[0].id;
    let reader = users[1].id;
    let status = repos.statuses.get_default().await.unwrap();
    let mut projects = Vec::new();
    let mut issues = Vec::new();
    for (key, who) in [("SOURCE", owner), ("TARGET", users[2].id)] {
        let project = domain::Project {
            id: ProjectId::new(),
            key: ProjectKey::new(key),
            name: key.into(),
            description: None,
            owner_id: who,
            default_board_id: BoardId::new(),
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        repos
            .projects
            .save_with_board(
                &project,
                &domain::Board {
                    id: project.default_board_id,
                    project_id: project.id,
                    name: "Board".into(),
                    columns: vec![],
                },
            )
            .await
            .unwrap();
        let issue = domain::Issue::create(
            &project,
            1,
            shared::IssueType::Task,
            status.id,
            "Preserved issue",
            None,
            who,
            shared::Priority::Medium,
        );
        repos.issues.save(&issue).await.unwrap();
        issues.push(issue);
        projects.push(project);
    }
    let registry = std::env::var("TT_NAMESPACE__REGISTRY_INSTANCE_ID")
        .unwrap()
        .parse()
        .unwrap();
    let a = SpaceRef {
        registry_instance_id: registry,
        space_id: Uuid::new_v4(),
    };
    let b = SpaceRef {
        space_id: Uuid::new_v4(),
        ..a.clone()
    };
    let c = SpaceRef {
        space_id: Uuid::new_v4(),
        ..a.clone()
    };
    for space in [&a, &b, &c] {
        db.execute(statement("INSERT INTO tracker_space_catalog(registry_instance_id,space_id,admin_revision,state) VALUES($1,$2,1,'active')",[space.registry_instance_id.into(),space.space_id.into()])).await.unwrap();
    }
    for (project, space) in [
        (projects[0].id, &a),
        (projects[0].id, &b),
        (projects[1].id, &c),
    ] {
        db.execute(statement("INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id) VALUES($1,$2,$3)",[project.as_uuid().into(),space.registry_instance_id.into(),space.space_id.into()])).await.unwrap();
    }
    let neighbor = domain::Issue::create(
        &projects[1],
        2,
        shared::IssueType::Task,
        status.id,
        "Unrelated preserved issue",
        None,
        users[2].id,
        shared::Priority::Medium,
    );
    repos.issues.save(&neighbor).await.unwrap();
    for target in [issues[0].id, neighbor.id] {
        repos
            .issue_links
            .save(&domain::IssueLink {
                id: shared::IssueLinkId::new(),
                source_id: issues[1].id,
                target_id: target,
                link_type: domain::LinkType::Blocks,
            })
            .await
            .unwrap();
    }
    // Preserve an actual historical 1:1 binding. Active B must no longer inherit
    // archived A's old write restriction when the actor is admitted to B.
    let command = serde_json::json!({"schema_version":1,"operation_id":Uuid::new_v4(),"namespace":{"registry_instance_id":registry,"namespace_id":a.space_id},"resource":{"kind":"tracker_project","instance_id":std::env::var("TT_NAMESPACE__INSTANCE_ID").unwrap(),"resource_id":projects[0].id.as_uuid()},"generation":1,"state":"archived"});
    db.execute(statement("INSERT INTO tracker_namespace_bindings(resource_id,registry_instance_id,namespace_id,generation,state,command) VALUES($1,$2,$3,1,'archived',$4)",[projects[0].id.as_uuid().into(),registry.into(),a.space_id.into(),command.into()])).await.unwrap();
    db.execute_unprepared("UPDATE tracker_space_admission_state SET active=true")
        .await
        .unwrap();
    for space in [&a, &c] {
        db.execute(statement(
            "UPDATE tracker_space_catalog SET state='archived' WHERE space_id=$1",
            [space.space_id.into()],
        ))
        .await
        .unwrap();
    }
    let auth = write_context::Fixture::start().await;
    let allowed = [
        (b.clone(), SpaceState::Active),
        (c.clone(), SpaceState::Archived),
    ];
    let frame = auth.frame(owner, owner.as_uuid(), &allowed, 60, true).await;
    with_request_context(frame, async {
        let created = domain::Project {
            id: ProjectId::new(),
            key: ProjectKey::new("NEWV2"),
            name: "No primary space".into(),
            description: None,
            owner_id: owner,
            default_board_id: BoardId::new(),
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        let actor = app::space_access::current_request_for(owner)
            .unwrap()
            .unwrap();
        let permit = actor.spaces().unwrap().permit().unwrap();
        repos
            .projects
            .save_with_board_and_spaces(
                &created,
                &domain::Board {
                    id: created.default_board_id,
                    project_id: created.id,
                    name: "Board".into(),
                    columns: vec![],
                },
                &[b.clone()],
                &permit,
            )
            .await
            .unwrap();
        assert_eq!(
            repos
                .projects
                .project_spaces(created.id)
                .await
                .unwrap()
                .spaces,
            vec![b.clone()]
        );
        // A failing initial set must not leave a project or board behind.
        let rejected = domain::Project {
            id: ProjectId::new(),
            key: ProjectKey::new("UNLINKED"),
            default_board_id: BoardId::new(),
            ..created.clone()
        };
        assert!(
            repos
                .projects
                .save_with_board_and_spaces(
                    &rejected,
                    &domain::Board {
                        id: rejected.default_board_id,
                        project_id: rejected.id,
                        name: "Board".into(),
                        columns: vec![]
                    },
                    &[],
                    &permit
                )
                .await
                .is_err()
        );
        assert!(repos.projects.get_by_id(rejected.id).await.is_err());
        repos
            .labels
            .save(&label(projects[0].id, "active B writer"))
            .await
            .unwrap();
        // Target access is read-only, not membership or target editing authority.
        repos
            .issue_links
            .save(&domain::IssueLink {
                id: shared::IssueLinkId::new(),
                source_id: issues[0].id,
                target_id: issues[1].id,
                link_type: domain::LinkType::Relates,
            })
            .await
            .unwrap();
    })
    .await;
    assert!(matches!(
        repos.labels.save(&label(projects[0].id, "unbound")).await,
        Err(AppError::Forbidden)
    ));
    let hidden = auth
        .frame(
            owner,
            owner.as_uuid(),
            &[(b.clone(), SpaceState::Active)],
            60,
            true,
        )
        .await;
    with_request_context(hidden, async {
        assert!(
            matches!(
                repos
                    .issue_links
                    .save(&domain::IssueLink {
                        id: shared::IssueLinkId::new(),
                        source_id: issues[0].id,
                        target_id: issues[1].id,
                        link_type: domain::LinkType::Blocks
                    })
                    .await,
                Err(AppError::Forbidden)
            ),
            "target C is not visible through B"
        );
    })
    .await;
    let observer = auth
        .frame(
            reader,
            reader.as_uuid(),
            &[(b.clone(), SpaceState::Active)],
            60,
            true,
        )
        .await;
    with_request_context(observer, async {
        assert!(matches!(
            repos
                .labels
                .save(&label(projects[0].id, "reader edit"))
                .await,
            Err(AppError::Forbidden)
        ));
        repos.watchers.add(issues[0].id, reader).await.unwrap();
        repos.votes.add(issues[0].id, reader).await.unwrap();
        assert!(
            matches!(
                repos.watchers.add(issues[0].id, owner).await,
                Err(AppError::Forbidden)
            ),
            "a personal action cannot impersonate another user"
        );
    })
    .await;
    let readonly = auth
        .frame(
            owner,
            owner.as_uuid(),
            &[(b.clone(), SpaceState::Active)],
            60,
            false,
        )
        .await;
    with_request_context(readonly, async {
        assert!(matches!(
            repos
                .labels
                .save(&label(projects[0].id, "read-only PAT"))
                .await,
            Err(AppError::Forbidden)
        ));
    })
    .await;
    let archived = auth
        .frame(
            owner,
            owner.as_uuid(),
            &[(a.clone(), SpaceState::Archived)],
            60,
            true,
        )
        .await;
    with_request_context(archived, async {
        assert!(matches!(
            repos
                .labels
                .save(&label(projects[0].id, "archived A cannot borrow B"))
                .await,
            Err(AppError::Forbidden)
        ));
    })
    .await;
    let wrong_binding = auth
        .frame(
            owner,
            reader.as_uuid(),
            &[(b.clone(), SpaceState::Active)],
            60,
            true,
        )
        .await;
    with_request_context(wrong_binding, async {
        assert!(matches!(
            repos
                .labels
                .save(&label(projects[0].id, "wrong binding"))
                .await,
            Err(AppError::Forbidden)
        ));
    })
    .await;
    // No SQL text heuristic: execute, query-returning and explicit TX all use the
    // same proof, including rollback and expiry after a successful statement.
    let mut options = sea_orm::ConnectOptions::new(url.to_string());
    options.max_connections(1).min_connections(1);
    let conn = Database::connect(options).await.unwrap();
    let direct_pool = conn.get_postgres_connection_pool().clone();
    let guarded = infra::guarded_db::GuardedDb::new(conn);
    let short = auth
        .frame(
            owner,
            owner.as_uuid(),
            &[(b.clone(), SpaceState::Active)],
            2,
            true,
        )
        .await;
    let expired_id = Uuid::new_v4();
    with_request_context(short,async{
  let tx=guarded.begin().await.unwrap();
  tx.query_one(statement("INSERT INTO labels(id,project_id,name,color,created_at) VALUES($1,$2,'expires before commit','#123456',now()) RETURNING id",[expired_id.into(),projects[0].id.as_uuid().into()])).await.unwrap().unwrap();
  tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
  assert!(matches!(tx.commit().await.map_err(infra::db_error::map),Err(AppError::Forbidden)));
 }).await;
    assert_eq!(
        db.query_one(statement(
            "SELECT count(*) AS n FROM labels WHERE id=$1",
            [expired_id.into()]
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "n")
        .unwrap(),
        0
    );
    assert!(guarded.execute(statement("INSERT INTO labels(id,project_id,name,color,created_at) VALUES($1,$2,'pool leak','#123456',now())",[Uuid::new_v4().into(),projects[0].id.as_uuid().into()])).await.is_err(),"SET LOCAL must be absent after the rejected commit");
    // Missing kind/user context cannot use SQL NULL to bypass validation.
    for bad in [
        serde_json::json!({}),
        serde_json::json!({"schema_version":1,"writable":true,"identity":{"issuer":"x","subject":owner.to_string()},"active_spaces":[],"readable_spaces":[],"policy_revision":1,"valid_until":shared::now().timestamp()+30}),
    ] {
        let tx = db.begin().await.unwrap();
        tx.execute(statement(
            "SELECT set_config('pdlc.tracker.write_actor',$1,true)",
            [bad.to_string().into()],
        ))
        .await
        .unwrap();
        assert!(
            tx.query_one(statement("SELECT tracker_current_write_actor()", []))
                .await
                .is_err()
        );
        let _ = tx.rollback().await;
    }
    let cleanup = auth
        .frame(
            owner,
            owner.as_uuid(),
            &[(b.clone(), SpaceState::Active)],
            60,
            true,
        )
        .await;
    with_request_context(cleanup, async {
        assert!(
            matches!(
                repos.watchers.remove(issues[0].id, reader).await,
                Err(AppError::Forbidden)
            ),
            "normal unsubscribe remains personal"
        );
        repos.issues.delete(issues[0].id).await.unwrap();
        repos.issues.purge(issues[0].id).await.unwrap();
        assert!(
            repos
                .issues
                .get_by_id_include_deleted(issues[0].id)
                .await
                .is_err()
        );
    })
    .await;
    assert!(repos.issues.get_by_id(issues[1].id).await.is_ok());
    assert!(repos.issues.get_by_id(neighbor.id).await.is_ok());
    let remaining = db
        .query_one(statement(
            "SELECT count(*) AS n FROM issue_links WHERE source_id=$1 AND target_id=$2",
            [issues[1].id.as_uuid().into(), neighbor.id.as_uuid().into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "n")
        .unwrap();
    assert_eq!(
        remaining, 1,
        "purge does not touch unrelated hidden-project links"
    );
    drop(guarded);
    direct_pool.close().await;
    drop(repos);
    pool.close().await;
    drop(auth);
    db.close().await.unwrap();
    root.execute_unprepared("DROP DATABASE writes_pdlc4_space_test")
        .await
        .unwrap();
    root.close().await.unwrap();
}
