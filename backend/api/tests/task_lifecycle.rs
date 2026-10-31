//! Actual owner router + PostgreSQL; local fixture auth, not central live acceptance.
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use migration::MigratorTrait;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

const OWNER: &str = "10000000-0000-4000-8000-000000000001";
const VIEWER: &str = "10000000-0000-4000-8000-000000000002";
const TASK: &str = "50000000-0000-4000-8000-000000000001";
const JWT: &str = "isolated-lifecycle-fixture-only";
fn token(actor: &str) -> String {
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &app::auth::UserClaims {
            sub: actor.into(),
            exp: (shared::now().timestamp() + 120) as usize,
            typ: Some("access".into()),
            jti: None,
        },
        &jsonwebtoken::EncodingKey::from_secret(JWT.as_bytes()),
    )
    .unwrap()
}
async fn call(
    router: &Router,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = bearer {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(if body.is_null() {
                    Body::empty()
                } else {
                    Body::from(body.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
#[tokio::test]
#[ignore = "owned fresh *_task_lifecycle_test PostgreSQL required"]
async fn owner_http_keeps_manual_model_without_execution_and_rejects_old_writers() {
    let url =
        std::env::var("TT_TASK_LIFECYCLE_TEST_DATABASE_URL").expect("owned database required");
    let db = Database::connect(url.as_str()).await.unwrap();
    let name = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name,to_regclass('public.issues') IS NULL AS fresh"
                .to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(
        name.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_task_lifecycle_test")
    );
    assert!(name.try_get::<bool>("", "fresh").unwrap());
    migration::Migrator::up(&db, None).await.unwrap();
    db.execute_unprepared(include_str!(
        "../../migration/src/task_lifecycle_fixture.sql"
    ))
    .await
    .unwrap();
    db.execute_unprepared("INSERT INTO users(id,email,username,display_name,password_hash) VALUES('10000000-0000-4000-8000-000000000002','viewer@example.test','viewer','Viewer','fixture')").await.unwrap();
    let config = Arc::new(shared::AppConfig {
        database: shared::DatabaseConfig {
            url: url.clone(),
            max_connections: 4,
            min_connections: 0,
            ..Default::default()
        },
        server: shared::ServerConfig::default(),
        auth: shared::AuthConfig {
            jwt_secret: JWT.into(),
            ..Default::default()
        },
        storage: shared::StorageConfig::default(),
        email: shared::EmailConfig::default(),
        metrics: shared::MetricsConfig::default(),
    });
    let repos = Arc::new(
        infra::build_repositories(config.database.clone())
            .await
            .unwrap(),
    );
    let ctx = Arc::new(app::AppContext::new(
        config,
        repos,
        Arc::new(domain::InMemoryStorage::default()),
    ));
    assert!(ctx.sdlc.is_none());
    let router = api::router(ctx.clone()).with_state(ctx);
    let owner = token(OWNER);
    let viewer = token(VIEWER);
    // A compatible backend can be installed before the global data cutover.
    // It must not create a nine-column board with issues still using four IDs.
    let (status, project) = call(
        &router,
        "POST",
        "/api/v1/projects",
        Some(&owner),
        json!({"key":"ROLL","name":"Rollout compatibility","description":null}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{project}");
    let (status, issue)=call(&router,"POST","/api/v1/issues",Some(&owner),
        json!({"project_key":"ROLL","summary":"Visible during cutover","issue_type":"task","description":"Retained","priority":"medium"})).await;
    assert_eq!(status, StatusCode::CREATED, "{issue}");
    let (status, before) = call(
        &router,
        "GET",
        "/api/v1/projects/ROLL/board",
        Some(&owner),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{before}");
    assert_eq!(before["columns"].as_array().unwrap().len(), 4);
    assert_eq!(
        before["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["issue_ids"].as_array().unwrap().len())
            .sum::<usize>(),
        1
    );
    assert_eq!(
        call(
            &router,
            "DELETE",
            &format!("/api/v1/issues/{}", issue["id"].as_str().unwrap()),
            Some(&owner),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    db.execute_unprepared("SELECT task_lifecycle_activate('10000000-0000-4000-8000-000000000001',task_lifecycle_inventory_snapshot())").await.unwrap();
    let (status, after) = call(
        &router,
        "GET",
        "/api/v2/projects/ROLL/board",
        Some(&owner),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{after}");
    assert_eq!(after["columns"].as_array().unwrap().len(), 9);
    assert!(after["issues"].as_array().unwrap().is_empty());
    let (status, project) = call(
        &router,
        "POST",
        "/api/v1/projects",
        Some(&owner),
        json!({"key":"NEW","name":"Mandatory stages","description":null}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{project}");
    let (status, empty) = call(
        &router,
        "GET",
        "/api/v2/projects/NEW/board",
        Some(&owner),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["columns"].as_array().unwrap().len(), 9);
    assert!(empty["issues"].as_array().unwrap().is_empty());

    assert_eq!(
        call(&router, "GET", "/api/v1/task-lifecycle", None, Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, catalog) = call(
        &router,
        "GET",
        "/api/v1/task-lifecycle",
        Some(&owner),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(catalog["stages"].as_array().unwrap().len(), 9);
    // Saved baseline filters must retain the agreed imported state, never silently
    // become an empty or unfiltered search after the canonical projection changes.
    for (filter, stage, phase) in [
        ("todo", "Backlog", "Ready"),
        ("in_progress", "Development", "InProgress"),
        ("review", "Review", "Ready"),
        ("done", "Deployment", "Completed"),
    ] {
        for endpoint in ["search", "issues"] {
            let path = format!("/api/v1/{endpoint}?project_key=CORE&status={filter}&limit=1");
            let (status, result) = call(&router, "GET", &path, Some(&owner), Value::Null).await;
            assert_eq!(status, StatusCode::OK, "{path}: {result}");
            assert_eq!(
                result["issues"].as_array().unwrap().len(),
                1,
                "{path}: {result}"
            );
            assert_eq!(result["issues"][0]["lifecycle"]["stage"], stage);
            assert_eq!(result["issues"][0]["lifecycle"]["status"], phase);
        }
    }
    for (query, count) in [
        (
            "stage%20%3D%20Review%20AND%20lifecycleStatus%20%3D%20Ready",
            1,
        ),
        ("status%20%3D%20Done", 1),
        ("status%20%21%3D%20Done", 3),
        (
            "stage%20%3D%20Development%20AND%20lifecycleStatus%20%3D%20Completed",
            0,
        ),
    ] {
        let (status, result) = call(
            &router,
            "GET",
            &format!("/api/v1/search?project_key=CORE&jql={query}"),
            Some(&owner),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{query}: {result}");
        assert_eq!(
            result["issues"].as_array().unwrap().len(),
            count,
            "{query}: {result}"
        );
    }
    let path = format!("/api/v1/issues/{TASK}/lifecycle");
    let (status, state) = call(&router, "GET", &path, Some(&owner), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        state["state"],
        json!({"stage":"Backlog","status":"Ready","revision":1})
    );
    for path in [
        "/api/v1/search?lifecycle_status=Ready&limit=1&offset=0",
        "/api/v1/issues?lifecycle_stage=Backlog&lifecycle_status=Ready&limit=1",
    ] {
        let (status, result) = call(&router, "GET", path, Some(&owner), Value::Null).await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["issues"].as_array().unwrap().len(), 1);
        assert_eq!(result["issues"][0]["lifecycle"]["status"], "Ready");
        assert_ne!(result["issues"][0]["status"], "Unknown");
    }
    let (status, listed) = call(
        &router,
        "GET",
        "/api/v1/search?lifecycle_status=Ready&limit=1&offset=1",
        Some(&owner),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["issues"].as_array().unwrap().len(), 1);
    let (status, listed) = call(
        &router,
        "GET",
        "/api/v1/search?lifecycle_status=Ready&limit=1&offset=2",
        Some(&owner),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(listed["issues"].as_array().unwrap().is_empty());
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/v1/search?lifecycle_status=unknown",
            Some(&owner),
            Value::Null
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &router,
            "GET",
            "/api/v1/search?status=todo&lifecycle_status=Ready",
            Some(&owner),
            Value::Null
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    // Internal pair IDs never become another set of user-selectable statuses.
    let (status, listed) = call(
        &router,
        "GET",
        "/api/v1/statuses",
        Some(&owner),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!listed.to_string().contains("Task Draft / Ready"));
    let command = json!({"action":"start","expected_revision":1,"idempotency_key":"http-human"});
    let transitions = format!("{path}/transitions");
    assert_eq!(
        call(
            &router,
            "POST",
            &transitions,
            Some(&viewer),
            command.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&router, "POST", &transitions, Some(&owner), command.clone())
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&router, "POST", &transitions, Some(&owner), command)
            .await
            .1["state"]["revision"],
        2
    );
    let stale = json!({"action":"complete","expected_revision":1,"idempotency_key":"stale-http"});
    assert_eq!(
        call(&router, "POST", &transitions, Some(&owner), stale)
            .await
            .0,
        StatusCode::CONFLICT
    );
    for path in [
        format!("/api/v1/issues/{TASK}"),
        format!("/api/v1/issues/{TASK}/transition"),
        "/api/v1/projects/CORE/board/move".into(),
    ] {
        let (method, body) = if path.ends_with("/transition") {
            (
                "POST",
                json!({"target_status_id":"00000000-0000-0000-0000-000000000003"}),
            )
        } else if path.ends_with("/move") {
            (
                "POST",
                json!({"issue_id":TASK,"status_id":"00000000-0000-0000-0000-000000000003"}),
            )
        } else {
            (
                "PATCH",
                json!({"status_id":"00000000-0000-0000-0000-000000000003"}),
            )
        };
        assert_eq!(
            call(&router, method, &path, Some(&owner), body).await.0,
            StatusCode::CONFLICT,
            "{path}"
        );
    }
    let (status, board) = call(
        &router,
        "GET",
        "/api/v2/projects/CORE/board",
        Some(&owner),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(board["columns"].as_array().unwrap().len(), 9);
    assert_eq!(board["issues"].as_array().unwrap().len(), 4);
    let (status, legacy) = call(
        &router,
        "GET",
        "/api/v1/projects/CORE/board",
        Some(&owner),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        legacy["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["issue_ids"].as_array().unwrap().len())
            .sum::<usize>(),
        4
    );
    let (status,fresh)=call(&router,"POST","/api/v1/issues",Some(&owner),json!({"project_key":"CORE","summary":"Обычная новая задача","issue_type":"task","description":"Материалы","priority":"medium"})).await;
    assert_eq!(status, StatusCode::CREATED, "{fresh}");
    let fresh = call(
        &router,
        "GET",
        &format!("/api/v1/issues/{}/lifecycle", fresh["id"].as_str().unwrap()),
        Some(&owner),
        Value::Null,
    )
    .await
    .1;
    assert_eq!(
        fresh["state"],
        json!({"stage":"Draft","status":"Ready","revision":1})
    );
}
