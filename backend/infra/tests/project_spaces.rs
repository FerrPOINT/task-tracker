//! Real canonical repository operations in a fresh, explicitly owned PostgreSQL database.
#[path = "support/space_owner.rs"]
mod owner_v2;
#[path = "support/project_projection.rs"]
mod project_projection_http;
#[path = "support/resource_history.rs"]
mod resource_history;
use domain::project_spaces::{ReplaceProjectSpaces, SpaceWritePermit};
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use shared::{AppError, ProjectId, UserId, space_access::*};
use std::sync::Arc;
use uuid::Uuid;

fn id(suffix: &str) -> Uuid {
    format!("00000000-0000-4000-8000-{suffix:0>12}")
        .parse()
        .unwrap()
}
fn permit(user: UserId, spaces: &[SpaceRef], seconds: i64) -> SpaceWritePermit {
    let identity = CentralIdentity {
        issuer: "https://auth.example.test".into(),
        subject: "owner-subject".into(),
    };
    let now = chrono::Utc::now().timestamp();
    let admission = SpaceAdmission {
        inheritance: None,
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
            .map(|space| ConfirmedSpace {
                space: space.clone(),
                revision: 10,
                state: SpaceState::Active,
            })
            .collect(),
    };
    SpaceWritePermit::from_admission(user, &identity, &admission, now + seconds).unwrap()
}
async fn verify(db: &Arc<DatabaseConnection>) {
    let user = UserId::from_uuid(id("30"));
    let project = ProjectId::from_uuid(id("1"));
    db.execute_unprepared(include_str!(
        "../../migration/src/unified_spaces_fixture.sql"
    ))
    .await
    .unwrap();
    db.execute_unprepared("ALTER TABLE projects ADD COLUMN key text NOT NULL DEFAULT 'NEW'; ALTER TABLE projects ADD COLUMN name text NOT NULL DEFAULT 'Same display name';UPDATE projects SET key=CASE WHEN id='00000000-0000-4000-8000-000000000001' THEN 'QA1' ELSE 'QA2' END").await.unwrap();
    db.execute_unprepared(include_str!("../../migration/src/unified_spaces.sql"))
        .await
        .unwrap();
    db.execute_unprepared("UPDATE tracker_space_catalog SET admin_revision=10; INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id) VALUES ('00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000010','00000000-0000-4000-8000-000000000012'); UPDATE tracker_space_admission_state SET active=true").await.unwrap();

    let a = SpaceRef {
        registry_instance_id: id("10"),
        space_id: id("11"),
    };
    let b = SpaceRef {
        space_id: id("12"),
        ..a.clone()
    };

    let tracker: Uuid = std::env::var("TT_NAMESPACE__INSTANCE_ID")
        .unwrap()
        .parse()
        .unwrap();
    let selectors = vec![
        shared::space_access::ProjectRef {
            tracker_instance_id: tracker,
            project_id: project.as_uuid(),
        },
        shared::space_access::ProjectRef {
            tracker_instance_id: tracker,
            project_id: id("2"),
        },
        shared::space_access::ProjectRef {
            tracker_instance_id: tracker,
            project_id: Uuid::new_v4(),
        },
    ];
    let batch = infra::project_spaces::contexts(db.as_ref(), &selectors)
        .await
        .unwrap();
    assert_eq!(
        batch.len(),
        2,
        "unknown IDs are omitted without selecting a first project"
    );
    assert!(
        batch
            .iter()
            .all(|c| c.name == "Same display name" && c.valid())
    );
    assert_eq!(
        batch
            .iter()
            .find(|c| c.links.project.project_id == project.as_uuid())
            .unwrap()
            .links
            .spaces
            .len(),
        2
    );
    assert!(
        infra::project_spaces::contexts(
            db.as_ref(),
            &[shared::space_access::ProjectRef {
                tracker_instance_id: Uuid::new_v4(),
                project_id: project.as_uuid()
            }]
        )
        .await
        .is_err()
    );
    let first_page = infra::project_spaces::context_page(db.as_ref(), &b, 1, 0)
        .await
        .unwrap();
    let second_page = infra::project_spaces::context_page(db.as_ref(), &b, 1, 1)
        .await
        .unwrap();
    assert_eq!(first_page.total, 2);
    assert_eq!(second_page.total, 2);
    assert_eq!(first_page.projects.len(), 1);
    assert_eq!(second_page.projects.len(), 1);
    assert_ne!(
        first_page.projects[0].links.project, second_page.projects[0].links.project,
        "M:N does not duplicate projects across pages"
    );
    let a_page = infra::project_spaces::context_page(db.as_ref(), &a, 100, 0)
        .await
        .unwrap();
    assert_eq!(a_page.total, 1);
    assert_eq!(
        a_page.projects[0].links.project.project_id,
        project.as_uuid()
    );
    // Exercise the real DTO repository, not only the lower-level selector. A
    // legacy zero-limit helper used to collapse this two-project result to one.
    db.execute_unprepared("ALTER TABLE projects ADD COLUMN description text; ALTER TABLE projects ADD COLUMN default_board_id uuid NOT NULL DEFAULT '00000000-0000-4000-8000-000000000077'; ALTER TABLE projects ADD COLUMN created_at timestamptz NOT NULL DEFAULT now(); ALTER TABLE projects ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now()").await.unwrap();
    let repos = infra::repos::SeaOrmRepositories::new(
        Database::connect(
            std::env::var("TT_UNIFIED_SPACE_TEST_DATABASE_URL")
                .unwrap()
                .replace("/unified_pdlc4_space_test?", "/port_pdlc4_space_test?"),
        )
        .await
        .unwrap(),
    );
    let visible = repos
        .projects
        .list_space_accessible_page(&[a.clone(), b.clone()], 100, 0)
        .await
        .unwrap();
    assert_eq!(visible.len(), 2);
    let all = repos
        .projects
        .list_space_accessible_page(&[b.clone()], 0, 0)
        .await
        .unwrap();
    assert_eq!(all.len(), 2);
    let page = repos
        .projects
        .list_space_accessible_page(&[a.clone()], 1, 0)
        .await
        .unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].id, project);
    assert!(
        repos
            .projects
            .list_space_accessible_page(&[a.clone()], 1, 1)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        repos
            .projects
            .list_space_accessible_page(&[], 100, 0)
            .await
            .unwrap()
            .is_empty()
    );
    let foreign = SpaceRef {
        registry_instance_id: Uuid::new_v4(),
        space_id: b.space_id,
    };
    assert!(
        repos
            .projects
            .list_space_accessible_page(&[foreign], 100, 0)
            .await
            .unwrap()
            .is_empty()
    );
    let query = |search: &str, offset: u64| domain::project_spaces::ProjectCatalogQuery {
        working_only: false,
        limit: 1,
        offset,
        search: search.into(),
        project_key: None,
    };
    let first = repos
        .projects
        .list_space_accessible_catalog(&[a.clone(), b.clone()], &query("", 0))
        .await
        .unwrap();
    let second = repos
        .projects
        .list_space_accessible_catalog(&[a.clone(), b.clone()], &query("", 1))
        .await
        .unwrap();
    assert_eq!((first.total, second.total), (2, 2));
    assert_eq!((first.projects.len(), second.projects.len()), (1, 1));
    assert_ne!(
        first.projects[0].id, second.projects[0].id,
        "M:N never duplicates a project across pages"
    );
    let beyond = repos
        .projects
        .list_space_accessible_catalog(&[a.clone(), b.clone()], &query("", 99))
        .await
        .unwrap();
    assert_eq!(beyond.total, 2);
    assert!(beyond.projects.is_empty());
    let matched = repos
        .projects
        .list_space_accessible_catalog(&[a.clone()], &query("same DISPLAY", 0))
        .await
        .unwrap();
    assert_eq!(matched.total, 1);
    assert_eq!(matched.projects[0].id, project);
    let hidden = repos
        .projects
        .list_space_accessible_catalog(&[a.clone()], &query("QA2", 0))
        .await
        .unwrap();
    assert_eq!(hidden.total, 0);
    assert!(
        hidden.projects.is_empty(),
        "search must not reveal the hidden B project"
    );
    for literal in ["%_", "'"] {
        let page = repos
            .projects
            .list_space_accessible_catalog(&[a.clone(), b.clone()], &query(literal, 0))
            .await
            .unwrap();
        assert_eq!(
            page.total, 0,
            "search treats wildcard and shell-like characters literally"
        );
    }
    let none = repos
        .projects
        .list_space_accessible_catalog(&[], &query("", 0))
        .await
        .unwrap();
    assert_eq!(none.total, 0);
    assert!(none.projects.is_empty());
    let foreign = SpaceRef {
        registry_instance_id: Uuid::new_v4(),
        space_id: a.space_id,
    };
    let none = repos
        .projects
        .list_space_accessible_catalog(&[foreign], &query("", 0))
        .await
        .unwrap();
    assert_eq!(
        none.total, 0,
        "space UUID alone cannot match a foreign registry"
    );
    let mut exact = query("same display", 0);
    exact.project_key = Some(shared::ProjectKey::new("QA1"));
    let page = repos
        .projects
        .list_space_accessible_catalog(&[a.clone(), b.clone()], &exact)
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.projects[0].id, project);
    exact.project_key = Some(shared::ProjectKey::new("QA2"));
    let page = repos
        .projects
        .list_space_accessible_catalog(&[a.clone()], &exact)
        .await
        .unwrap();
    assert_eq!(
        page.total, 0,
        "an exact key does not reveal a hidden project"
    );
    println!("Native project catalog count/page/literal search/hidden/M:N PASS");
    resource_history::verify(&repos, db.as_ref(), user, &a, &b).await;
    drop(repos);
    let initial = infra::project_spaces::get(db.as_ref(), project)
        .await
        .unwrap();
    assert_eq!(initial.spaces.len(), 2);
    let change = ReplaceProjectSpaces {
        operation_id: Uuid::new_v4(),
        expected_revision: initial.revision,
        spaces: vec![b.clone()],
    };
    assert!(matches!(
        infra::project_spaces::replace(
            db,
            project,
            &change,
            &permit(UserId::new(), &[a.clone(), b.clone()], 60)
        )
        .await,
        Err(AppError::Forbidden)
    ));
    assert_eq!(
        infra::project_spaces::get(db.as_ref(), project)
            .await
            .unwrap()
            .revision,
        initial.revision
    );
    // Admin proof can still say active during the bounded cache interval; the local archive barrier wins.
    db.execute_unprepared("UPDATE tracker_space_catalog SET state='archived' WHERE space_id='00000000-0000-4000-8000-000000000011'").await.unwrap();
    let hidden = ReplaceProjectSpaces {
        operation_id: Uuid::new_v4(),
        expected_revision: initial.revision,
        spaces: vec![a.clone()],
    };
    assert!(matches!(
        infra::project_spaces::replace(db, project, &hidden, &permit(user, &[a.clone()], 60)).await,
        Err(AppError::Forbidden)
    ));
    let mut result =
        infra::project_spaces::replace(db, project, &change, &permit(user, &[b.clone()], 60))
            .await
            .unwrap();
    assert_eq!(result.spaces, vec![b.clone()]);
    assert!(result.revision > initial.revision);
    let replay =
        infra::project_spaces::replace(db, project, &change, &permit(user, &[b.clone()], 60))
            .await
            .unwrap();
    assert_eq!(
        replay.revision, result.revision,
        "same original key replays without another link write"
    );
    let pending = infra::project_space_outbox::current_status(db.as_ref(), project)
        .await
        .unwrap();
    assert!(!pending.confirmed);
    assert_eq!(pending.operation_id, change.operation_id);
    let mut conflicting = change.clone();
    conflicting.spaces = vec![a.clone(), b.clone()];
    assert!(
        infra::project_spaces::replace(db, project, &conflicting, &permit(user, &[b.clone()], 60))
            .await
            .is_err()
    );
    let mut fresh_key = change.clone();
    fresh_key.operation_id = Uuid::new_v4();
    assert!(
        infra::project_spaces::replace(db, project, &fresh_key, &permit(user, &[b.clone()], 60))
            .await
            .is_err(),
        "new key still obeys CAS"
    );
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT command FROM tracker_project_space_outbox WHERE operation_id=$1",
            [change.operation_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    let projection: shared::project_spaces::ProjectSpacesProjection =
        serde_json::from_value(row.try_get("", "command").unwrap()).unwrap();
    let ack =
        shared::project_spaces::ProjectSpacesProjectionReadback::confirmed(&projection).unwrap();
    let mut wrong = ack.clone();
    wrong.revision += 1;
    assert!(
        infra::project_space_outbox::accept_ack(db.as_ref(), &projection, &wrong)
            .await
            .is_err()
    );
    assert!(
        !infra::project_space_outbox::current_status(db.as_ref(), project)
            .await
            .unwrap()
            .confirmed
    );
    infra::project_space_outbox::accept_ack(db.as_ref(), &projection, &ack)
        .await
        .unwrap();
    infra::project_space_outbox::accept_ack(db.as_ref(), &projection, &ack)
        .await
        .unwrap();
    assert!(
        infra::project_space_outbox::current_status(db.as_ref(), project)
            .await
            .unwrap()
            .confirmed
    );
    result = project_projection_http::verify(db.as_ref(), project, user, &a, &b, &result).await;
    // A lock wait cannot renew the original authority or leave a partial full-set replacement.
    let lock = db.begin().await.unwrap();
    lock.execute_unprepared("SELECT 1 FROM tracker_project_space_sets WHERE project_id='00000000-0000-4000-8000-000000000001' FOR UPDATE").await.unwrap();
    let short = permit(user, &[b.clone()], 2);
    let write_db = db.clone();
    let next = ReplaceProjectSpaces {
        operation_id: Uuid::new_v4(),
        expected_revision: result.revision,
        spaces: vec![b.clone()],
    };
    let waiting = tokio::spawn(async move {
        infra::project_spaces::replace(&write_db, project, &next, &short).await
    });
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    lock.commit().await.unwrap();
    assert!(matches!(waiting.await.unwrap(), Err(AppError::Forbidden)));
    assert_eq!(
        infra::project_spaces::get(db.as_ref(), project)
            .await
            .unwrap()
            .revision,
        result.revision
    );
    // An archive committed while a writer waits for the catalog must also deny the write.
    let lock = db.begin().await.unwrap();
    lock.execute_unprepared("UPDATE tracker_space_catalog SET state='archived' WHERE space_id='00000000-0000-4000-8000-000000000012'").await.unwrap();
    let active = permit(user, &[b.clone()], 60);
    let write_db = db.clone();
    let next = ReplaceProjectSpaces {
        operation_id: Uuid::new_v4(),
        expected_revision: result.revision,
        spaces: vec![b.clone()],
    };
    let waiting = tokio::spawn(async move {
        infra::project_spaces::replace(&write_db, project, &next, &active).await
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    lock.commit().await.unwrap();
    assert!(matches!(waiting.await.unwrap(), Err(AppError::Forbidden)));
    let final_links = infra::project_spaces::get(db.as_ref(), project)
        .await
        .unwrap();
    assert_eq!(final_links.revision, result.revision);
    assert_eq!(final_links.spaces, vec![b]);
    owner_v2::verify(db.as_ref()).await;
}

#[tokio::test]
#[ignore = "requires a fresh owned *_pdlc4_space_test PostgreSQL instance"]
async fn canonical_space_replacement_enforces_owner_deadline_and_archive_barriers() {
    let url = std::env::var("TT_UNIFIED_SPACE_TEST_DATABASE_URL").expect("owned PostgreSQL URL");
    let root = Database::connect(&url).await.unwrap();
    let row = root
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        row.try_get::<String>("", "name").unwrap(),
        "unified_pdlc4_space_test",
        "refuse working instance"
    );
    root.execute_unprepared("CREATE DATABASE port_pdlc4_space_test")
        .await
        .unwrap();
    let target = url.replace("/unified_pdlc4_space_test?", "/port_pdlc4_space_test?");
    assert_ne!(target, url);
    let db = Arc::new(Database::connect(target).await.unwrap());
    verify(&db).await;
    Arc::try_unwrap(db)
        .unwrap_or_else(|_| panic!("only owned test connection"))
        .close()
        .await
        .unwrap();
    root.execute_unprepared("DROP DATABASE port_pdlc4_space_test")
        .await
        .unwrap();
    root.close().await.unwrap();
}
