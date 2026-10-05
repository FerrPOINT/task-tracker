use infra::repos::SeaOrmRepositories;
use migration::MigratorTrait;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use shared::AppError;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, SeaOrmRepositories) {
    let base = std::env::var("TT_TEST_DATABASE_URL").expect("isolated PostgreSQL URL");
    let host = base.rsplit_once('/').expect("database URL").0;
    let db = Database::connect(format!("{host}/tasktracker_infra_test"))
        .await
        .unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    db.execute_unprepared("TRUNCATE users CASCADE")
        .await
        .unwrap();
    let repos = SeaOrmRepositories::new(
        Database::connect(format!("{host}/tasktracker_infra_test"))
            .await
            .unwrap(),
    );
    (db, repos)
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL test database"]
async fn verified_name_updates_only_the_matching_subject() {
    let (db, repos) = setup().await;
    let legacy_id = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO users (id, email, username, display_name, password_hash, is_system_admin, is_active, created_at, updated_at)
         VALUES ($1, 'same@example.test', 'historic', 'Historic Author', '!', false, true, now(), now())",
        [legacy_id.into()],
    )).await.unwrap();
    let subject = Uuid::new_v4().to_string();
    let first = repos
        .users
        .find_or_create_central_user(&subject, "same@example.test", "Central Human")
        .await
        .unwrap();
    assert_ne!(first.id.as_uuid(), legacy_id);
    let renamed = repos
        .users
        .find_or_create_central_user(&subject, "different@example.test", " Renamed Human ")
        .await
        .unwrap();
    assert_eq!(renamed.id, first.id);
    assert_eq!(renamed.email, first.email);
    assert_eq!(renamed.username, first.username);
    assert_eq!(renamed.created_at, first.created_at);
    assert_eq!(renamed.display_name.as_ref(), "Renamed Human");
    assert_eq!(renamed.is_system_admin, first.is_system_admin);
    assert!(matches!(
        repos
            .users
            .find_or_create_central_user(&subject, "same@example.test", "  ")
            .await,
        Err(AppError::Unauthorized)
    ));
    let unchanged = repos.users.get_by_id(first.id).await.unwrap();
    assert_eq!(unchanged.updated_at, renamed.updated_at);
    assert_eq!(unchanged.display_name, renamed.display_name);
    let historic = repos
        .users
        .get_by_id(shared::UserId::from_uuid(legacy_id))
        .await
        .unwrap();
    assert_eq!(historic.display_name.as_ref(), "Historic Author");
    assert!(!historic.is_system_admin);
    let other = repos
        .users
        .find_or_create_central_user(
            &Uuid::new_v4().to_string(),
            "same@example.test",
            "Different Subject",
        )
        .await
        .unwrap();
    assert_ne!(other.id, first.id);
    assert_ne!(other.id.as_uuid(), legacy_id);
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL test database"]
async fn inactive_central_profile_is_rejected_without_writes() {
    let (db, repos) = setup().await;
    let subject = Uuid::new_v4().to_string();
    let first = repos
        .users
        .find_or_create_central_user(&subject, "inactive@example.test", "Before Disable")
        .await
        .unwrap();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE users SET is_active = false, is_system_admin = false WHERE id = $1",
        [first.id.as_uuid().into()],
    ))
    .await
    .unwrap();
    let before = repos.users.get_by_id(first.id).await.unwrap();
    for name in ["Before Disable", "Must Not Overwrite"] {
        assert!(matches!(
            repos
                .users
                .find_or_create_central_user(&subject, "inactive@example.test", name)
                .await,
            Err(AppError::Unauthorized)
        ));
    }
    let after = repos.users.get_by_id(first.id).await.unwrap();
    assert_eq!(after.display_name, before.display_name);
    assert_eq!(after.updated_at, before.updated_at);
    assert_eq!(after.created_at, before.created_at);
    assert!(!after.is_active);
    assert!(!after.is_system_admin);
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL test database"]
async fn unchanged_central_profile_does_not_wait_for_a_user_write_lock() {
    let (db, repos) = setup().await;
    let subject = Uuid::new_v4().to_string();
    let first = repos
        .users
        .find_or_create_central_user(&subject, "lock@example.test", "Central Human")
        .await
        .unwrap();
    let blocker = db.begin().await.unwrap();
    blocker
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id FROM users WHERE id = $1 FOR UPDATE",
            [first.id.as_uuid().into()],
        ))
        .await
        .unwrap()
        .expect("owned profile");
    let mut lookup = tokio::spawn(async move {
        repos
            .users
            .find_or_create_central_user(&subject, "lock@example.test", "Central Human")
            .await
    });
    let resolved = tokio::time::timeout(std::time::Duration::from_secs(3), &mut lookup).await;
    blocker.rollback().await.unwrap();
    let result = match resolved {
        Ok(result) => result.unwrap().unwrap(),
        Err(_) => {
            lookup.await.unwrap().unwrap();
            panic!("unchanged central profile must not acquire a user write lock");
        }
    };
    assert_eq!(result.id, first.id);
    assert_eq!(result.updated_at, first.updated_at);
}
