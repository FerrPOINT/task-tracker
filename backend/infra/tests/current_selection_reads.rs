//! Manual SQL-adapter regression for current-page users and project history.
use infra::repos::SeaOrmRepositories;
use sea_orm::{Database, sqlx};
use shared::{ProjectId, UserId};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
#[ignore = "manual owned tracker_attachment_test PostgreSQL read adapters"]
async fn sparse_user_batches_and_project_history_keep_identity_errors_and_scope() {
    let url = std::env::var("TRACKER_ATTACHMENT_TEST_DATABASE_URL").expect("owned fixture URL");
    let root = PgPoolOptions::new()
        .max_connections(3)
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
    let schema = format!("selection_{}", Uuid::new_v4().simple());
    sqlx::raw_sql(&format!(
        "CREATE SCHEMA {schema};
        CREATE TABLE {schema}.users(id uuid PRIMARY KEY,email text NOT NULL,central_sub text,
          username text NOT NULL,display_name text NOT NULL,password_hash text NOT NULL,
          refresh_token_hash text,is_system_admin boolean NOT NULL DEFAULT false,
          is_active boolean NOT NULL DEFAULT true,created_at timestamptz NOT NULL DEFAULT now(),
          updated_at timestamptz NOT NULL DEFAULT now());
        CREATE TABLE {schema}.issues(id uuid PRIMARY KEY,project_id uuid NOT NULL);
        CREATE TABLE {schema}.issue_status_history(id uuid PRIMARY KEY,issue_id uuid NOT NULL,
          from_status_id uuid,to_status_id uuid NOT NULL,changed_by_id uuid NOT NULL,
          created_at timestamptz NOT NULL);"
    ))
    .execute(&root)
    .await
    .unwrap();
    let mut scoped = reqwest::Url::parse(&url).unwrap();
    scoped
        .query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    let scoped = scoped.to_string();
    let db = Database::connect(&scoped).await.unwrap();
    let pool = db.get_postgres_connection_pool().clone();
    let repo = SeaOrmRepositories::new(db);
    let users: Vec<_> = (0..100).map(|_| Uuid::new_v4()).collect();
    sqlx::query("INSERT INTO users(id,email,username,display_name,password_hash) SELECT id,id::text||'@example.test',id::text,'Selected identity','synthetic' FROM unnest($1::uuid[]) id")
        .bind(&users).execute(&pool).await.unwrap();
    let selected = repo
        .users
        .get_by_ids(&[
            UserId::from_uuid(users[1]),
            UserId::from_uuid(users[9]),
            UserId::from_uuid(users[1]),
            UserId::new(),
        ])
        .await
        .unwrap();
    let actual: std::collections::HashSet<_> =
        selected.into_iter().map(|user| user.id.as_uuid()).collect();
    assert_eq!(
        actual,
        std::collections::HashSet::from([users[1], users[9]])
    );
    let project = Uuid::new_v4();
    let other = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let foreign = Uuid::new_v4();
    for (issue, owner) in [(a, project), (b, project), (foreign, other)] {
        sqlx::query("INSERT INTO issues(id,project_id) VALUES($1,$2)")
            .bind(issue)
            .bind(owner)
            .execute(&pool)
            .await
            .unwrap();
    }
    let mut expected = Vec::new();
    for (issue, day) in [(a, 3), (foreign, 1), (b, 2), (a, 1)] {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO issue_status_history(id,issue_id,to_status_id,changed_by_id,created_at) VALUES($1,$2,$3,$4,'2026-01-01'::timestamptz + make_interval(days=>$5))")
            .bind(id).bind(issue).bind(Uuid::new_v4()).bind(users[1]).bind(day).execute(&pool).await.unwrap();
        if issue != foreign {
            expected.push((day, id));
        }
    }
    expected.sort();
    let history = repo
        .issue_status_history
        .list_by_project(ProjectId::from_uuid(project))
        .await
        .unwrap();
    assert_eq!(
        history
            .iter()
            .map(|row| row.id.as_uuid())
            .collect::<Vec<_>>(),
        expected.into_iter().map(|(_, id)| id).collect::<Vec<_>>()
    );
    assert!(
        repo.issue_status_history
            .list_by_project(ProjectId::new())
            .await
            .unwrap()
            .is_empty()
    );
    sqlx::query("DROP TABLE users")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        repo.users.get_by_ids(&[]).await.unwrap().is_empty(),
        "empty selection does no directory IO"
    );
    assert!(
        repo.users
            .get_by_ids(&[UserId::from_uuid(users[1])])
            .await
            .is_err(),
        "database errors must not become missing historical identities"
    );
    drop(repo);
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&root)
        .await
        .unwrap();
    root.close().await;
}
