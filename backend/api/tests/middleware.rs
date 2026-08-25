use domain::InMemoryStorage;
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
            refresh_cookie_name: "refresh_token".to_string(),
            refresh_cookie_secure: true,
            refresh_cookie_same_site: "Lax".to_string(),
            refresh_cookie_domain: None,
            refresh_cookie_path: "/api/v1/auth".to_string(),
        },
        storage: shared::StorageConfig::default(),
        email: shared::EmailConfig::default(),
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
        audit_logs: Arc::new(domain::StubAuditLogRepository),
        system_settings: Arc::new(domain::StubSystemSettingRepository),
        projects: projects.clone(),
        issues: issues.clone(),
        boards: boards.clone(),
        sprints: sprints.clone(),
        comments: Arc::new(domain::StubCommentRepository),
        worklogs: Arc::new(domain::StubWorklogRepository),
        members: Arc::new(domain::StubProjectMemberRepository),
        statuses: Arc::new(domain::StubStatusRepository),
        transitions: Arc::new(domain::StubWorkflowTransitionRepository),
        issue_types: Arc::new(domain::StubIssueTypeRepository),
        attachments: Arc::new(domain::StubAttachmentRepository),
        labels: Arc::new(domain::StubLabelRepository),
        issue_links: Arc::new(domain::StubIssueLinkRepository),
        notifications: Arc::new(domain::StubNotificationRepository),
        notification_settings: Arc::new(domain::StubUserNotificationSettingsRepository),
        issue_status_history: Arc::new(domain::StubIssueStatusHistoryRepository),
        watchers: Arc::new(domain::StubWatcherRepository),
        votes: Arc::new(domain::StubVoteRepository),
        components: Arc::new(domain::StubProjectComponentRepository),
        versions: Arc::new(domain::StubProjectVersionRepository),
        custom_fields: Arc::new(domain::StubCustomFieldRepository),
    });
    let ctx = Arc::new(app::context::AppContext::new(
        test_config(),
        repos,
        Arc::new(InMemoryStorage::default()),
    ));
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
        .access_token
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
async fn middleware_rejects_wrong_auth_scheme() {
    let ctx = ctx_with_user().await;
    let app = api::router(ctx.clone()).with_state(ctx);
    let req = Request::builder()
        .uri("/api/v1/dashboard")
        .header("authorization", "Basic invalid")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn middleware_accepts_lowercase_bearer_prefix() {
    let ctx = ctx_with_user().await;
    let token = login_token(&ctx).await;
    let app = api::router(ctx.clone()).with_state(ctx);
    let req = Request::builder()
        .uri("/api/v1/dashboard")
        .header("authorization", format!("bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn middleware_rejects_expired_token() {
    let ctx = ctx_with_user().await;
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &api::middleware::auth::UserClaims {
            sub: shared::UserId::from_uuid(uuid::Uuid::nil()).to_string(),
            exp: 1,
        },
        &jsonwebtoken::EncodingKey::from_secret("test-secret".as_bytes()),
    )
    .unwrap();
    let app = api::router(ctx.clone()).with_state(ctx);
    let req = Request::builder()
        .uri("/api/v1/dashboard")
        .header("authorization", format!("Bearer {}", token))
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
        .uri("/api/v1/projects/NONEXIST")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
