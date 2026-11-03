//! Manual regression for the production worklog repository on a guarded synthetic database.
use domain::Worklog;
use infra::repos::SeaOrmRepositories;
use sea_orm::{Database, sqlx};
use shared::{IssueId, UserId, WorklogId};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

fn item(issue: Uuid, duration: i64) -> Worklog {
    Worklog {
        id: WorklogId::new(),
        issue_id: IssueId::from_uuid(issue),
        author_id: UserId::new(),
        started_at: shared::now(),
        duration_seconds: duration,
        description: None,
        created_at: shared::now(),
        updated_at: shared::now(),
    }
}
#[tokio::test]
#[ignore = "manual owned tracker_attachment_test PostgreSQL worklog regression"]
async fn worklog_and_aggregate_are_atomic_and_stale_issue_writes_preserve_spent_time() {
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
    let schema = format!("worklogs_{}", Uuid::new_v4().simple());
    sqlx::raw_sql(&format!(r#"CREATE SCHEMA {schema};
      CREATE TABLE {schema}.issues(
        id uuid PRIMARY KEY, project_id uuid NOT NULL, key text NOT NULL,
        issue_type text NOT NULL DEFAULT 'Task',status_id uuid NOT NULL,
        summary text NOT NULL DEFAULT 'original',description text,assignee_id uuid,
        reporter_id uuid NOT NULL,priority text NOT NULL DEFAULT 'Medium',
        labels jsonb NOT NULL DEFAULT '[]',sprint_id uuid,component_id uuid,
        affected_version_id uuid,fix_version_id uuid,position float8 NOT NULL DEFAULT 0,
        due_date timestamptz,original_estimate_seconds bigint DEFAULT 1000,
        remaining_estimate_seconds bigint DEFAULT 1000,time_spent_seconds bigint NOT NULL DEFAULT 0,
        created_at timestamptz NOT NULL DEFAULT now(),updated_at timestamptz NOT NULL DEFAULT now(),deleted_at timestamptz);
      CREATE TABLE {schema}.worklogs(id uuid PRIMARY KEY,issue_id uuid NOT NULL REFERENCES {schema}.issues(id),
        author_id uuid NOT NULL,started_at timestamptz NOT NULL,duration_seconds bigint NOT NULL,
        description text,created_at timestamptz NOT NULL,updated_at timestamptz NOT NULL);"#))
      .execute(&root).await.unwrap();
    let scoped = format!("{url}&options=-csearch_path%3D{schema}");
    let db = Database::connect(&scoped).await.unwrap();
    let connection_pool = db.get_postgres_connection_pool().clone();
    let repos = SeaOrmRepositories::new(db);
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&scoped)
        .await
        .unwrap();
    let issue = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO issues(id,project_id,key,status_id,reporter_id) VALUES($1,$2,'QA-1',$3,$4)",
    )
    .bind(issue)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .unwrap();
    let mut stale = repos
        .issues
        .get_by_id(IssueId::from_uuid(issue))
        .await
        .unwrap();
    let first = item(issue, 100);
    let second = item(issue, 200);
    let (a, b) = tokio::join!(repos.worklogs.save(&first), repos.worklogs.save(&second));
    a.unwrap();
    b.unwrap();
    let actual: (i64, Option<i64>) = sqlx::query_as(
        "SELECT time_spent_seconds,remaining_estimate_seconds FROM issues WHERE id=$1",
    )
    .bind(issue)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        actual,
        (300, Some(700)),
        "worklog writes must atomically maintain the aggregate"
    );
    stale.summary = "concurrent summary".into();
    repos.issues.save(&stale).await.unwrap();
    let current = repos.issues.get_by_id(stale.id).await.unwrap();
    assert_eq!(
        current.time_spent_seconds, 300,
        "an old issue snapshot cannot reset spent time"
    );
    assert_eq!(current.remaining_estimate_seconds, Some(700));
    assert_eq!(current.summary.as_ref(), "concurrent summary");
    // Fail the aggregate update after the worklog mutation: the whole transaction must roll back.
    sqlx::raw_sql(
        "ALTER TABLE issues ADD CONSTRAINT qa_reject_aggregate CHECK(time_spent_seconds<=300)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let rejected = item(issue, 400);
    assert!(repos.worklogs.save(&rejected).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM worklogs WHERE id=$1")
            .bind(rejected.id.as_uuid())
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let mut changed = first.clone();
    changed.duration_seconds = 500;
    assert!(repos.worklogs.save(&changed).await.is_err());
    assert_eq!(
        repos
            .worklogs
            .get_by_id(first.id)
            .await
            .unwrap()
            .duration_seconds,
        100
    );
    // An ordinary update and delete keep the same owner and exact sum.
    changed.duration_seconds = 50;
    repos.worklogs.save(&changed).await.unwrap();
    repos.worklogs.delete(second.id).await.unwrap();
    let current = repos.issues.get_by_id(stale.id).await.unwrap();
    assert_eq!(
        (
            current.time_spent_seconds,
            current.remaining_estimate_seconds
        ),
        (50, Some(950))
    );
    assert_eq!(current.summary.as_ref(), "concurrent summary");
    // Parent deletion is checked before mutation; no orphan worklog can be inserted.
    sqlx::query("UPDATE issues SET deleted_at=now() WHERE id=$1")
        .bind(issue)
        .execute(&pool)
        .await
        .unwrap();
    let deleted = item(issue, 10);
    assert!(repos.worklogs.save(&deleted).await.is_err());
    drop(repos);
    connection_pool.close().await;
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&root)
        .await
        .unwrap();
}
