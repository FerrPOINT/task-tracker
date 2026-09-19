use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};

#[tokio::test]
async fn central_subject_does_not_reuse_historical_email() {
    let Ok(url) = std::env::var("TT_MIGRATION_TEST_DATABASE_URL") else {
        return;
    };
    let db = Database::connect(&url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    db.execute_unprepared(
        "INSERT INTO users (id, email, username, display_name, password_hash) \
         VALUES (gen_random_uuid(), 'same@example.test', 'historical', 'Historical', 'hash')",
    )
    .await
    .unwrap();
    db.execute_unprepared(
        "INSERT INTO users (id, email, username, display_name, password_hash, central_sub) \
         VALUES (gen_random_uuid(), 'same@example.test', 'central-user', 'New', '!', 'central-sub-1')"
    ).await.unwrap();
    let count = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT count(*) AS n FROM users WHERE email = 'same@example.test'".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(count.try_get::<i64>("", "n").unwrap(), 2);
}
