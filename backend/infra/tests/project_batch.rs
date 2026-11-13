use infra::{entities::project, repos::SeaOrmRepositories};
use sea_orm::{DatabaseBackend, DatabaseConnection, DbErr, MockDatabase, RuntimeErr};
use shared::{AppError, ProjectId};
use uuid::Uuid;
fn observed(db: &DatabaseConnection) -> DatabaseConnection {
    let DatabaseConnection::MockDatabaseConnection(inner) = db else {
        panic!("mock fixture")
    };
    DatabaseConnection::MockDatabaseConnection(inner.clone())
}
fn row() -> project::Model {
    project::Model {
        id: Uuid::new_v4(),
        key: "BATCH".into(),
        name: "Имя проекта".into(),
        description: None,
        owner_id: Uuid::new_v4(),
        default_board_id: Uuid::new_v4(),
        created_at: shared::now(),
        updated_at: shared::now(),
    }
}
#[tokio::test]
async fn current_selection_uses_one_project_query_and_keeps_errors() {
    for count in [1, 50, 200] {
        let rows: Vec<_> = (0..count).map(|_| row()).collect();
        let ids: Vec<_> = rows.iter().map(|p| ProjectId::from_uuid(p.id)).collect();
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([rows])
            .into_connection();
        let observer = observed(&db);
        let projects = SeaOrmRepositories::new(db)
            .projects
            .get_by_ids(&ids)
            .await
            .unwrap();
        assert_eq!(projects.len(), count);
        assert!(projects.iter().all(|p| ids.contains(&p.id)));
        assert_eq!(observer.into_transaction_log().len(), 1);
    }
    let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
    let observer = observed(&db);
    assert!(
        SeaOrmRepositories::new(db)
            .projects
            .get_by_ids(&[])
            .await
            .unwrap()
            .is_empty()
    );
    assert!(observer.into_transaction_log().is_empty());
    let missing = ProjectId::new();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<project::Model>::new()])
        .into_connection();
    assert!(matches!(
        SeaOrmRepositories::new(db)
            .projects
            .get_by_ids(&[missing])
            .await,
        Err(AppError::NotFound(_))
    ));
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_errors([DbErr::Conn(RuntimeErr::Internal(
            "fixture unavailable".into(),
        ))])
        .into_connection();
    assert!(matches!(
        SeaOrmRepositories::new(db)
            .projects
            .get_by_ids(&[missing])
            .await,
        Err(AppError::Database(_))
    ));
}
