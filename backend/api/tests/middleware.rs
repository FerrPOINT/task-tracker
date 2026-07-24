use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

fn test_config() -> Arc<shared::AppConfig> {
    Arc::new(shared::AppConfig {
        database: shared::DatabaseConfig::default(),
        server: shared::ServerConfig::default(),
        auth: shared::AuthConfig {
            jwt_secret: "test-secret".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
        },
    })
}

async fn ctx_with_user() -> Arc<app::context::AppContext> {
    let users = Arc::new(domain::MemoryUserRepository::default());
    let projects = Arc::new(domain::MemoryProjectRepository::default());
    let issues = Arc::new(domain::MemoryIssueRepository::default());
    let boards = Arc::new(domain::MemoryBoardRepository::default());
    let sprints = Arc::new(domain::MemorySprintRepository::default());
    let repos = Arc::new(domain::Repositories {
        users: users.clone(),
        projects: projects.clone(),
        issues: issues.clone(),
        boards: boards.clone(),
        sprints: sprints.clone(),
    });
    let ctx = Arc::new(app::context::AppContext::new(test_config(), repos));
    ctx.services
        .auth
        .register(app::commands::RegisterCommand {
            email: "demo@example.com".to_string(),
            username: "demo".to_string(),
            name: "Demo".to_string(),
            password: "secret123".to_string(),
        })
        .await
        .unwrap();
    ctx
}

async fn login_token(ctx: &app::context::AppContext) -> String {
    ctx.services
        .auth
        .login(app::commands::LoginCommand {
            email: "demo@example.com".to_string(),
            password: "secret123".to_string(),
        })
        .await
        .unwrap()
        .token
}

#[tokio::test]
async fn middleware_rejects_missing_auth() {
    let ctx = ctx_with_user().await;
    let app = api::router(ctx.clone()).with_state(ctx);
    let req = Request::builder()
        .uri("/api/v1/dashboard")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn middleware_rejects_invalid_token() {
    let ctx = ctx_with_user().await;
    let app = api::router(ctx.clone()).with_state(ctx);
    let req = Request::builder()
        .uri("/api/v1/dashboard")
        .header("authorization", "Bearer invalid-token")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn middleware_accepts_valid_token() {
    let ctx = ctx_with_user().await;
    let token = login_token(&ctx).await;
    let app = api::router(ctx.clone()).with_state(ctx);
    let req = Request::builder()
        .uri("/api/v1/dashboard")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn create_project_requires_auth() {
    let ctx = ctx_with_user().await;
    let app = api::router(ctx.clone()).with_state(ctx);
    let body = serde_json::json!({
        "key": "NEW",
        "name": "New Project",
    });
    let req = Request::builder()
        .uri("/api/v1/projects")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_project_creates_when_authenticated() {
    let ctx = ctx_with_user().await;
    let token = login_token(&ctx).await;
    let app = api::router(ctx.clone()).with_state(ctx);
    let body = serde_json::json!({
        "key": "NEW",
        "name": "New Project",
    });
    let req = Request::builder()
        .uri("/api/v1/projects")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn get_project_not_found() {
    let ctx = ctx_with_user().await;
    let token = login_token(&ctx).await;
    let app = api::router(ctx.clone()).with_state(ctx);
    let req = Request::builder()
        .uri("/api/v1/projects/NONEXISTENT")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
