//! Session publication updates no identity fields and validates the credential atomically.
use infra::repos::SeaOrmRepositories;
use sea_orm::{Database, sqlx};
use shared::{AppError, UserId};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
#[ignore = "manual owned tracker_attachment_test PostgreSQL session issuance"]
async fn session_write_preserves_identity_and_waits_for_credential_changes() {
    let url = std::env::var("TRACKER_ATTACHMENT_TEST_DATABASE_URL").expect("owned fixture URL");
    let root = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT current_database()")
            .fetch_one(&root)
            .await
            .unwrap(),
        "tracker_attachment_test"
    );
    let schema = format!("session_{}", Uuid::new_v4().simple());
    sqlx::raw_sql(&format!(
        "CREATE SCHEMA {schema};
        CREATE TABLE {schema}.users(id uuid PRIMARY KEY,email text NOT NULL,central_sub text,
          username text NOT NULL,display_name text NOT NULL,password_hash text NOT NULL,
          refresh_token_hash text,is_system_admin boolean NOT NULL DEFAULT false,
          is_active boolean NOT NULL DEFAULT true,created_at timestamptz NOT NULL DEFAULT now(),
          updated_at timestamptz NOT NULL DEFAULT now());"
    ))
    .execute(&root)
    .await
    .unwrap();
    let mut scoped = reqwest::Url::parse(&url).unwrap();
    scoped
        .query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    let db = Database::connect(scoped.to_string()).await.unwrap();
    let pool = db.get_postgres_connection_pool().clone();
    let repo = SeaOrmRepositories::new(db).users;
    let id = UserId::new();
    sqlx::query("INSERT INTO users(id,email,username,display_name,password_hash,refresh_token_hash,is_system_admin) VALUES($1,'session@example.test','session','Current profile','current-password','old-session',false)")
        .bind(id.as_uuid()).execute(&pool).await.unwrap();
    let current = repo
        .set_refresh_token_if_password_matches(id, "current-password", "new-session")
        .await
        .unwrap();
    assert_eq!(current.display_name.as_ref(), "Current profile");
    assert!(!current.is_system_admin);
    assert_eq!(current.password_hash.as_ref(), "current-password");
    assert_eq!(current.refresh_token_hash.as_deref(), Some("new-session"));
    assert!(matches!(
        repo.set_refresh_token_if_password_matches(id, "old-password", "bad-session")
            .await,
        Err(AppError::Unauthorized)
    ));
    assert!(matches!(
        repo.set_refresh_token_if_password_matches(
            UserId::new(),
            "current-password",
            "bad-session"
        )
        .await,
        Err(AppError::Unauthorized)
    ));
    // A committed block that is in flight when session publication starts wins.
    let mut block = pool.begin().await.unwrap();
    sqlx::query("UPDATE users SET is_active=false WHERE id=$1")
        .bind(id.as_uuid())
        .execute(&mut *block)
        .await
        .unwrap();
    let issuer = repo.clone();
    let mut pending = tokio::spawn(async move {
        issuer
            .set_refresh_token_if_password_matches(id, "current-password", "must-not-publish")
            .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut pending)
            .await
            .is_err(),
        "session write must wait for the row owner"
    );
    block.commit().await.unwrap();
    assert!(matches!(
        pending.await.unwrap(),
        Err(AppError::Unauthorized)
    ));
    let stored = repo.get_by_id(id).await.unwrap();
    assert!(!stored.is_active);
    assert_eq!(stored.refresh_token_hash.as_deref(), Some("new-session"));
    assert_eq!(stored.password_hash.as_ref(), "current-password");
    drop(repo);
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&root)
        .await
        .unwrap();
}
