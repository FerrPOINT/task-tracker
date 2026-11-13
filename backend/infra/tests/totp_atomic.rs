//! Actual PostgreSQL conditional writes for standalone TOTP; no user credentials.
use domain::TotpRepository;
use infra::repos::TotpRepo;
use sea_orm::{Database, sqlx};
use shared::UserId;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
#[ignore = "manual owned tracker_attachment_test PostgreSQL MFA race regression"]
async fn one_time_factors_and_enrollment_changes_are_atomic() {
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
    let schema = format!("totp_{}", Uuid::new_v4().simple());
    sqlx::raw_sql(&format!("CREATE SCHEMA {schema};
        CREATE TABLE {schema}.totp(user_id uuid PRIMARY KEY,secret_cipher text NOT NULL,enabled boolean NOT NULL,
        confirmed_at timestamptz,last_used_step bigint NOT NULL,recovery_codes text NOT NULL,
        created_at timestamptz NOT NULL,updated_at timestamptz NOT NULL);")).execute(&root).await.unwrap();
    let mut scoped = reqwest::Url::parse(&url).unwrap();
    scoped
        .query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    let scoped = scoped.to_string();
    let db = Database::connect(&scoped).await.unwrap();
    let connections = db.get_postgres_connection_pool().clone();
    let repo = TotpRepo::new(db);
    let user = UserId::new();
    repo.upsert_unconfirmed(user, "cipher-a").await.unwrap();
    repo.confirm_enable(user, "cipher-a", r#"["hash-a","hash-b"]"#, 10)
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        repo.mark_used_step(user, "cipher-a", 11),
        repo.mark_used_step(user, "cipher-a", 11)
    );
    assert_eq!(
        usize::from(a.unwrap()) + usize::from(b.unwrap()),
        1,
        "exactly one writer consumes a step"
    );
    assert_eq!(repo.get(user).await.unwrap().last_used_step, 11);
    let (a, b) = tokio::join!(
        repo.consume_recovery_code(user, "cipher-a", "hash-a"),
        repo.consume_recovery_code(user, "cipher-a", "hash-b")
    );
    assert!(
        a.unwrap() && b.unwrap(),
        "different valid codes must both be usable"
    );
    assert_eq!(
        repo.get(user).await.unwrap().recovery_codes.as_ref(),
        "[]",
        "neither update resurrects the other code"
    );
    assert!(
        !repo
            .consume_recovery_code(user, "cipher-a", "hash-a")
            .await
            .unwrap()
    );
    let before = repo.get(user).await.unwrap();
    assert!(repo.upsert_unconfirmed(user, "replacement").await.is_err());
    let after = repo.get(user).await.unwrap();
    assert!(after.enabled);
    assert_eq!(after.secret_cipher, before.secret_cipher);
    assert_eq!(after.last_used_step, before.last_used_step);
    assert!(!repo.disable(user, "different-enrollment").await.unwrap());
    assert!(repo.disable(user, "cipher-a").await.unwrap());
    repo.upsert_unconfirmed(user, "cipher-b").await.unwrap();
    assert!(
        repo.confirm_enable(user, "cipher-a", r#"["stale"]"#, 12)
            .await
            .is_err()
    );
    repo.confirm_enable(user, "cipher-b", r#"["single"]"#, 12)
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        repo.consume_recovery_code(user, "cipher-b", "single"),
        repo.consume_recovery_code(user, "cipher-b", "single")
    );
    assert_eq!(
        usize::from(a.unwrap()) + usize::from(b.unwrap()),
        1,
        "the same recovery code has one winner"
    );
    assert!(!repo.mark_used_step(user, "cipher-a", 13).await.unwrap());
    assert!(!repo.disable(user, "cipher-a").await.unwrap());
    assert_eq!(
        repo.get(user).await.unwrap().secret_cipher.as_ref(),
        "cipher-b"
    );
    assert!(
        !repo
            .mark_used_step(UserId::new(), "cipher-b", 13)
            .await
            .unwrap()
    );
    drop(repo);
    connections.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&root)
        .await
        .unwrap();
}
