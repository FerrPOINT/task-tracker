//! Independent archive is tested on the full real product schema, not the
//! focused M:N fixture. No live database, user data or external service is used.
#[path = "support/write_context.rs"]
mod write_context;
use app::space_access::with_request_context;
use domain::project_spaces::{ChangeProjectLifecycle, SpaceWritePermit};
use infra::repos::{SeaOrmRepositories, to_domain_repositories};
use migration::MigratorTrait;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TransactionTrait};
use shared::{
    AppError, BoardId, ProjectId, ProjectKey, UserId, space_access::*, space_lifecycle::*,
};
use std::sync::Arc;
use uuid::Uuid;
fn permit(user: UserId, spaces: &[SpaceRef], seconds: i64) -> SpaceWritePermit {
    let now = shared::now().timestamp();
    let identity = CentralIdentity {
        issuer: "https://auth.example.test".into(),
        subject: user.to_string(),
    };
    let admission = SpaceAdmission {
        schema_version: 1,
        identity: identity.clone(),
        identity_active: true,
        policy_revision: 1,
        policy: SpaceAccessPolicy::Selected {
            spaces: spaces.to_vec(),
        },
        validity: AdmissionValidity::confirmed(now, now + seconds, i64::MAX).unwrap(),
        spaces: spaces
            .iter()
            .map(|s| ConfirmedSpace {
                space: s.clone(),
                revision: 1,
                state: SpaceState::Active,
            })
            .collect(),
    };
    SpaceWritePermit::from_admission(user, &identity, &admission, now + seconds).unwrap()
}
fn change(revision: i64, archived: bool) -> ChangeProjectLifecycle {
    ChangeProjectLifecycle {
        operation_id: Uuid::new_v4(),
        expected_revision: revision,
        archived,
    }
}
#[tokio::test]
#[ignore = "requires fresh owned *_pdlc4_space_test PostgreSQL instance"]
async fn project_archive_is_independent_replay_safe_and_serializes_content_writes() {
    let original =
        std::env::var("TT_UNIFIED_SPACE_TEST_DATABASE_URL").expect("owned test database");
    let root = Database::connect(&original).await.unwrap();
    let name = root
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<String>("", "name")
        .unwrap();
    assert_eq!(name, "unified_pdlc4_space_test");
    root.execute_unprepared("CREATE DATABASE archive_pdlc4_space_test")
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&original).unwrap();
    url.set_path("/archive_pdlc4_space_test");
    let db = Arc::new(Database::connect(url.to_string()).await.unwrap());
    migration::Migrator::up(db.as_ref(), None).await.unwrap();
    let repo_db = Database::connect(url.to_string()).await.unwrap();
    let repo_pool = repo_db.get_postgres_connection_pool().clone();
    let repos = Arc::new(to_domain_repositories(SeaOrmRepositories::new(repo_db)));
    let owner_subject = Uuid::new_v4();
    let owner = repos
        .users
        .find_or_create_central_user(&owner_subject.to_string(), "owner@example.test", "Owner")
        .await
        .unwrap();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE users SET central_sub=$1 WHERE id=$2",
        [owner.id.to_string().into(), owner.id.as_uuid().into()],
    ))
    .await
    .unwrap();
    let auth = write_context::Fixture::start().await;
    let mut options = sea_orm::ConnectOptions::new(url.to_string());
    options.max_connections(1).min_connections(1);
    let guarded_connection = Database::connect(options).await.unwrap();
    let guarded_pool = guarded_connection.get_postgres_connection_pool().clone();
    let guarded = infra::guarded_db::GuardedDb::new(guarded_connection);
    let p = domain::Project {
        id: ProjectId::new(),
        key: ProjectKey::new("ARCHIVE"),
        name: "Preserved project".into(),
        description: None,
        owner_id: owner.id,
        default_board_id: BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    let status = repos.statuses.get_default().await.unwrap();
    repos
        .projects
        .save_with_board(
            &p,
            &domain::Board {
                id: p.default_board_id,
                project_id: p.id,
                name: "Board".into(),
                columns: vec![domain::BoardColumn {
                    id: status.id,
                    name: status.name,
                    category: status.category,
                    wip_limit: None,
                    position: 0,
                }],
            },
        )
        .await
        .unwrap();
    let registry = std::env::var("TT_NAMESPACE__REGISTRY_INSTANCE_ID")
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    let a = SpaceRef {
        registry_instance_id: registry,
        space_id: Uuid::new_v4(),
    };
    let b = SpaceRef {
        space_id: Uuid::new_v4(),
        ..a.clone()
    };
    for space in [&a, &b] {
        db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO tracker_space_catalog(registry_instance_id,space_id,admin_revision,state) VALUES($1,$2,1,'active')",[space.registry_instance_id.into(),space.space_id.into()])).await.unwrap();
        db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id) VALUES($1,$2,$3)",[p.id.as_uuid().into(),space.registry_instance_id.into(),space.space_id.into()])).await.unwrap();
    }
    db.execute_unprepared("UPDATE tracker_space_admission_state SET active=true")
        .await
        .unwrap();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE tracker_space_catalog SET state='archived' WHERE space_id=$1",
        [a.space_id.into()],
    ))
    .await
    .unwrap();
    let frame = auth
        .frame(
            owner.id,
            owner.id.as_uuid(),
            &[(b.clone(), SpaceState::Active)],
            60,
            true,
        )
        .await;
    with_request_context(frame,async {
    let initial = repos.projects.project_lifecycle(p.id).await.unwrap();
    assert!(!initial.archived);
    let original_links = repos.projects.project_spaces(p.id).await.unwrap();
    assert!(
        matches!(
            repos
                .projects
                .change_project_lifecycle(
                    p.id,
                    &change(1, true),
                    &permit(owner.id, &[a.clone()], 30)
                )
                .await,
            Err(AppError::Forbidden)
        ),
        "hidden active B cannot be borrowed"
    );
    assert!(matches!(
        repos
            .projects
            .change_project_lifecycle(
                p.id,
                &change(1, true),
                &permit(UserId::new(), &[b.clone()], 30)
            )
            .await,
        Err(AppError::Forbidden)
    ));
    let archive = change(initial.revision, true);
    let archived = repos
        .projects
        .change_project_lifecycle(p.id, &archive, &permit(owner.id, &[b.clone()], 30))
        .await
        .unwrap();
    assert!(archived.archived);
    assert_eq!(archived.revision, initial.revision + 1);
    // Actual Space owner restoration never clears independent Project archive.
    let now = shared::now().timestamp();
    let authority = SpaceOwnerAuthority::confirmed(
        CentralIdentity {
            issuer: "https://auth.example.test".into(),
            subject: "catalog-reader".into(),
        },
        AdmissionValidity::confirmed(now, now + 30, i64::MAX).unwrap(),
    )
    .unwrap();
    let restore_space = SpaceOwnerCommandV2 {
        schema_version: 2,
        space: a.clone(),
        participant_instance_id: std::env::var("TT_NAMESPACE__INSTANCE_ID")
            .unwrap()
            .parse()
            .unwrap(),
        operation_id: Uuid::new_v4(),
        revision: 2,
        desired_state: DesiredSpaceState::Active,
        metadata: SpaceMetadata {
            slug: "archive-qa".into(),
            name: "QA Space".into(),
            description: "".into(),
            responsible_subject: owner.id.to_string(),
        },
    };
    assert!(
        infra::space_owner::apply(db.as_ref(), &restore_space, &authority)
            .await
            .unwrap()
            .matches(&restore_space)
    );
    assert!(
        repos
            .projects
            .project_lifecycle(p.id)
            .await
            .unwrap()
            .archived
    );
    let mut stale = p.clone();
    stale.name = "Stale overwrite".into();
    assert!(repos.projects.save(&stale).await.is_err());
    assert_eq!(
        repos.projects.get_by_id(p.id).await.unwrap().name.as_ref(),
        "Preserved project"
    );
    assert!(db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"UPDATE projects SET archived_at=NULL,project_lifecycle_revision=project_lifecycle_revision+1 WHERE id=$1",[p.id.as_uuid().into()])).await.is_err(),"direct metadata bypass requires original receipt");
    let label = domain::Label {
        id: shared::LabelId::new(),
        project_id: p.id,
        name: "Archived content".into(),
        color: "#123456".into(),
    };
    assert!(repos.labels.save(&label).await.is_err());
    assert!(repos.projects.require_writable(p.id).await.is_err());
    assert!(repos.projects.delete(p.id).await.is_err());
    let restored = repos
        .projects
        .change_project_lifecycle(
            p.id,
            &change(archived.revision, false),
            &permit(owner.id, &[b.clone()], 30),
        )
        .await
        .unwrap();
    assert!(!restored.archived);
    repos.projects.require_writable(p.id).await.unwrap();
    assert!(
        !repos
            .projects
            .change_project_lifecycle(p.id, &archive, &permit(owner.id, &[b.clone()], 30))
            .await
            .unwrap()
            .archived,
        "old archive request cannot re-archive after restore"
    );
    let conflict = ChangeProjectLifecycle {
        archived: false,
        ..archive.clone()
    };
    assert!(matches!(
        repos
            .projects
            .change_project_lifecycle(p.id, &conflict, &permit(owner.id, &[b.clone()], 30))
            .await,
        Err(AppError::Conflict(_))
    ));
    let same = repos
        .projects
        .change_project_lifecycle(
            p.id,
            &change(restored.revision, false),
            &permit(owner.id, &[b.clone()], 30),
        )
        .await
        .unwrap();
    assert_eq!(same.revision, restored.revision);
    assert_eq!(
        repos.projects.project_spaces(p.id).await.unwrap().spaces,
        original_links.spaces
    );
    // An active B admission and the current owner role permit a real content write.
    repos.labels.save(&domain::Label{id:shared::LabelId::new(),project_id:p.id,name:"Allowed restored content".into(),color:"#123456".into()}).await.unwrap();
    // Even a no-op receipt cannot commit after its original authority expires.
    let tx = db.begin().await.unwrap();
    let expired_operation = Uuid::new_v4();
    let expiry = shared::now().timestamp() + 1;
    tx.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO tracker_project_lifecycle_operations(operation_id,project_id,actor_identity,command,authority_valid_until) VALUES($1,$2,$3,$4,$5)",[expired_operation.into(),p.id.as_uuid().into(),serde_json::json!({"issuer":"https://auth.example.test","subject":"project-owner"}).into(),serde_json::to_value(ChangeProjectLifecycle{operation_id:expired_operation,expected_revision:restored.revision,archived:false}).unwrap().into(),expiry.into()])).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert!(tx.commit().await.is_err());
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT count(*) AS n FROM tracker_project_lifecycle_operations WHERE operation_id=$1",
            [expired_operation.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "n").unwrap(), 0);
    // A child content write that already owns the project shared barrier commits
    // before archive. Once archived, the same legitimate edit is refused by SQL.
    let tx = guarded.begin().await.unwrap();
    tx.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO labels(id,project_id,name,color,created_at) VALUES($1,$2,'before archive','#123456',now())",[label.id.as_uuid().into(),p.id.as_uuid().into()])).await.unwrap();
    let writer = repos.clone();
    let next = change(same.revision, true);
    let fresh = permit(owner.id, &[b.clone()], 30);
    let project = p.id;
    let pending = tokio::spawn(async move {
        writer
            .projects
            .change_project_lifecycle(project, &next, &fresh)
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(75)).await;
    assert!(!pending.is_finished());
    tx.commit().await.unwrap();
    let again = pending.await.unwrap().unwrap();
    assert!(again.archived);
    assert!(
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE labels SET name='after archive' WHERE id=$1",
            [label.id.as_uuid().into()]
        ))
        .await
        .is_err()
    );
    assert_eq!(
        repos
            .labels
            .get_by_id(label.id)
            .await
            .unwrap()
            .name
            .as_ref(),
        "before archive"
    );
    let restored = repos
        .projects
        .change_project_lifecycle(
            p.id,
            &change(again.revision, false),
            &permit(owner.id, &[b.clone()], 30),
        )
        .await
        .unwrap();
    // Waiting for a row lock cannot renew the original short decision.
    let lock = db.begin().await.unwrap();
    lock.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id FROM projects WHERE id=$1 FOR UPDATE",
        [p.id.as_uuid().into()],
    ))
    .await
    .unwrap();
    let writer = repos.clone();
    let project = p.id;
    let next = change(restored.revision, true);
    let short = permit(owner.id, &[b.clone()], 2);
    let pending = tokio::spawn(async move {
        writer
            .projects
            .change_project_lifecycle(project, &next, &short)
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    lock.commit().await.unwrap();
    assert!(matches!(pending.await.unwrap(), Err(AppError::Forbidden)));
    assert_eq!(
        repos.projects.project_lifecycle(p.id).await.unwrap(),
        restored
    );
    // Actual catalog state after waiting wins over a stale positive admission.
    let lock = db.begin().await.unwrap();
    lock.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE tracker_space_catalog SET state='archived' WHERE space_id=$1",
        [b.space_id.into()],
    ))
    .await
    .unwrap();
    let writer = repos.clone();
    let project = p.id;
    let next = change(restored.revision, true);
    let fresh = permit(owner.id, &[b.clone()], 30);
    let pending = tokio::spawn(async move {
        writer
            .projects
            .change_project_lifecycle(project, &next, &fresh)
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(75)).await;
    lock.commit().await.unwrap();
    assert!(matches!(pending.await.unwrap(), Err(AppError::Forbidden)));
    assert_eq!(
        repos.projects.project_lifecycle(p.id).await.unwrap(),
        restored
    );
    }).await;
    // Transaction-local actor must never remain on a returned pooled connection.
    assert!(guarded.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO labels(id,project_id,name,color,created_at) VALUES($1,$2,'unbound','#123456',now())",[Uuid::new_v4().into(),p.id.as_uuid().into()])).await.is_err());
    drop(guarded);
    guarded_pool.close().await;
    drop(auth);
    drop(repos);
    repo_pool.close().await;
    Arc::try_unwrap(db).ok().unwrap().close().await.unwrap();
    root.execute_unprepared("DROP DATABASE archive_pdlc4_space_test")
        .await
        .unwrap();
    root.close().await.unwrap();
}
