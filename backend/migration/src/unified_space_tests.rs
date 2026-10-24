use super::*;
use sea_orm::{Database, DatabaseConnection, DbBackend, Statement, TransactionTrait};

async fn value(db: &DatabaseConnection, query: &str) -> i64 {
    db.query_one(Statement::from_string(
        DbBackend::Postgres,
        query.to_owned(),
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "count")
    .unwrap()
}

#[tokio::test]
#[ignore = "requires a fresh owned *_pdlc4_space_test PostgreSQL database"]
async fn unified_space_commit_guards_preserve_links_and_serialize_read_committed_and_repeatable_read()
 {
    let url = std::env::var("TT_UNIFIED_SPACE_TEST_DATABASE_URL")
        .expect("disposable test database required");
    let db = Database::connect(url).await.unwrap();
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name,to_regclass('public.projects') IS NULL AS fresh"
                .to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_pdlc4_space_test"),
        "refuse working database"
    );
    assert!(row.try_get::<bool>("", "fresh").unwrap());
    db.execute_unprepared(include_str!("unified_spaces_fixture.sql"))
        .await
        .unwrap();
    m20261019_0000099_unified_spaces::Migration
        .up(&SchemaManager::new(&db))
        .await
        .unwrap();
    db.execute_unprepared(include_str!("unified_spaces_assertions.sql"))
        .await
        .unwrap();
    for isolation in ["READ COMMITTED", "REPEATABLE READ"] {
        let first = db.begin().await.unwrap();
        let second = db.begin().await.unwrap();
        let statement = format!("SET TRANSACTION ISOLATION LEVEL {isolation}");
        first.execute_unprepared(&statement).await.unwrap();
        second.execute_unprepared(&statement).await.unwrap();
        // Both transactions observe the original full set before either delete.
        for tx in [&first, &second] {
            tx.query_one(Statement::from_string(
                DbBackend::Postgres,
                "SELECT count(*) AS count FROM tracker_project_spaces".to_owned(),
            ))
            .await
            .unwrap();
        }
        first.execute_unprepared("DELETE FROM tracker_project_spaces WHERE project_id='00000000-0000-4000-8000-000000000001' AND space_id='00000000-0000-4000-8000-000000000011'").await.unwrap();
        let competing = tokio::spawn(async move {
            second.execute_unprepared("DELETE FROM tracker_project_spaces WHERE project_id='00000000-0000-4000-8000-000000000001' AND space_id='00000000-0000-4000-8000-000000000012'").await?;
            second.commit().await
        });
        first.commit().await.unwrap();
        let error = competing
            .await
            .unwrap()
            .expect_err("two deletes removed the last link");
        let expected = if isolation == "READ COMMITTED" {
            "project_requires_space"
        } else {
            "could not serialize access"
        };
        assert!(
            error.to_string().contains(expected),
            "wrong failure under {isolation}: {error}"
        );
        assert_eq!(value(&db,"SELECT count(*) AS count FROM tracker_project_spaces WHERE project_id='00000000-0000-4000-8000-000000000001'").await,1);
        db.execute_unprepared("INSERT INTO tracker_project_spaces VALUES ('00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000010','00000000-0000-4000-8000-000000000011',now())").await.unwrap();
    }
}
