//! DB-backed regression tests for core FK constraints (docker Postgres required).
//!
//! Run: cargo test -p infra --test fk_regression -- --include-ignored --test-threads=1
use migration::MigratorTrait;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};

async fn db() -> DatabaseConnection {
    let base_url = std::env::var("TT_TEST_DATABASE_URL").unwrap_or_else(|_| {
        std::fs::read_to_string("/root/.tt_db_url")
            .expect("read /root/.tt_db_url")
            .trim()
            .to_string()
    });
    let base_url = base_url
        .rsplit_once('/')
        .map(|(h, _)| h.to_string())
        .unwrap_or(base_url);
    let db_url = format!("{}/{}", base_url, "tasktracker_infra_test");
    let db = Database::connect(&db_url)
        .await
        .expect("connect to test db");
    migration::Migrator::up(&db, None)
        .await
        .expect("run migrations");
    db
}

/// Orphan comment (issue_id pointing nowhere) must be rejected by the FK.
#[tokio::test]
#[ignore = "requires docker test stack"]
async fn fk_rejects_orphan_comment() {
    let db = db().await;
    let err = db
        .execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "INSERT INTO comments (id, issue_id, author_id, body, created_at, updated_at) VALUES (gen_random_uuid(), '00000000-0000-0000-0000-00000000dead', '00000000-0000-0000-0000-00000000beef', 'x', NOW(), NOW())",
        ))
        .await;
    assert!(err.is_err(), "orphan comment must be rejected");
    let msg = err.err().unwrap().to_string();
    assert!(
        msg.contains("fk_comments_issue"),
        "expected FK violation, got: {msg}"
    );
}

/// Issue referencing a missing project must be rejected.
#[tokio::test]
#[ignore = "requires docker test stack"]
async fn fk_rejects_orphan_issue_project() {
    let db = db().await;
    let err = db
        .execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "INSERT INTO issues (id, project_id, key, issue_type, status_id, summary, priority, reporter_id, position, time_spent_seconds, created_at, updated_at, labels) VALUES (gen_random_uuid(), '00000000-0000-0000-0000-00000000dead', 'ZZZ-1', 'task', '00000000-0000-0000-0000-000000000001', 'x', 'medium', '00000000-0000-0000-0000-00000000beef', 0, 0, NOW(), NOW(), '{}')",
        ))
        .await;
    assert!(err.is_err(), "orphan issue must be rejected");
    let msg = err.err().unwrap().to_string();
    assert!(
        msg.contains("fk_issues_project")
            || msg.contains("fk_issues_reporter")
            || msg.contains("fk_issues_status"),
        "expected FK violation, got: {msg}"
    );
}

/// All core FK constraints exist and are validated after migration.
#[tokio::test]
#[ignore = "requires docker test stack"]
async fn core_fk_constraints_present() {
    let db = db().await;
    let rows = db
        .query_all(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            r#"SELECT conname FROM pg_constraint
               WHERE contype='f' AND convalidated
               AND conname IN ('fk_issues_project','fk_issues_status','fk_issues_assignee','fk_issues_reporter','fk_issues_sprint',
                               'fk_comments_issue','fk_comments_author','fk_worklogs_issue','fk_worklogs_author',
                               'fk_attachments_issue','fk_attachments_author','fk_sprints_project','fk_boards_project',
                               'fk_members_project','fk_members_user','fk_history_issue')
               GROUP BY conname"#,
        ))
        .await
        .unwrap();
    let found: Vec<String> = rows
        .into_iter()
        .map(|r| r.try_get::<String>("", "conname").unwrap())
        .collect();
    assert_eq!(
        found.len(),
        16,
        "all 16 core FKs must exist and be validated: {found:?}"
    );
}
