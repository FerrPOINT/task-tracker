//! Phase 9: Production hardening tests — rate limiting and Prometheus metrics.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use std::sync::Arc;
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

async fn test_ctx() -> Arc<app::context::AppContext> {
    let users = Arc::new(domain::MemoryUserRepository::default());
    let repos = Arc::new(domain::Repositories {
        users: users.clone(),
        audit_logs: Arc::new(domain::StubAuditLogRepository),
        system_settings: Arc::new(domain::StubSystemSettingRepository),
        projects: Arc::new(domain::StubProjectRepository),
        issues: Arc::new(domain::StubIssueRepository),
        boards: Arc::new(domain::StubBoardRepository),
        sprints: Arc::new(domain::StubSprintRepository),
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
    Arc::new(app::context::AppContext::new(
        test_config(),
        repos,
        Arc::new(domain::InMemoryStorage::default()),
    ))
}

/// Helper: send a request to the given path and return the status code.
#[allow(dead_code)]
async fn send_request(app: &axum::Router, method: &str, path: &str) -> StatusCode {
    let req = Request::builder()
        .method(method)
        .uri(path)
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn metrics_endpoint_returns_prometheus_format() {
    let ctx = test_ctx().await;
    let app = api::router(ctx.clone()).with_state(ctx);

    // First, make a request to generate some metrics.
    let peer_addr: std::net::SocketAddr = "127.0.0.1:7777".parse().unwrap();
    let req = Request::builder()
        .uri("/api/v1/health")
        .extension(axum::extract::ConnectInfo(peer_addr))
        .body(Body::empty())
        .unwrap();
    let _ = app.clone().oneshot(req).await.unwrap().status();

    // Now hit the /metrics endpoint.
    let req = Request::builder()
        .uri("/metrics")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Verify the content type is text/plain.
    let content_type = res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/plain"),
        "expected text/plain content-type, got: {content_type}"
    );

    // Read the body and check for Prometheus metric names.
    let body_bytes = axum::body::to_bytes(res.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(
        body.contains("axum_http_requests_total") || body.contains("http_requests_total"),
        "expected http_requests_total metric in body, got: {body}"
    );
    assert!(
        body.contains("axum_http_requests_duration_seconds")
            || body.contains("http_request_duration_seconds"),
        "expected http_request_duration_seconds metric in body, got: {body}"
    );
}

#[tokio::test]
async fn security_headers_present_on_responses() {
    let ctx = test_ctx().await;
    let app = api::router(ctx.clone()).with_state(ctx);

    let peer_addr: std::net::SocketAddr = "127.0.0.1:8888".parse().unwrap();
    let req = Request::builder()
        .uri("/api/v1/health")
        .extension(axum::extract::ConnectInfo(peer_addr))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    // Verify security headers.
    assert_eq!(
        res.headers()
            .get("x-content-type-options")
            .and_then(|v| v.to_str().ok()),
        Some("nosniff")
    );
    assert_eq!(
        res.headers()
            .get("x-frame-options")
            .and_then(|v| v.to_str().ok()),
        Some("DENY")
    );
    assert_eq!(
        res.headers()
            .get("x-xss-protection")
            .and_then(|v| v.to_str().ok()),
        Some("1; mode=block")
    );
    assert_eq!(
        res.headers()
            .get("referrer-policy")
            .and_then(|v| v.to_str().ok()),
        Some("no-referrer")
    );
    assert!(
        res.headers()
            .get("content-security-policy")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .contains("default-src 'self'"),
        "expected CSP header with default-src 'self'"
    );
    assert_eq!(
        res.headers()
            .get("strict-transport-security")
            .and_then(|v| v.to_str().ok()),
        Some("max-age=31536000; includeSubDomains")
    );
}

#[tokio::test]
async fn auth_rate_limit_returns_429_after_exceeding_limit() {
    let ctx = test_ctx().await;
    let app = api::router(ctx.clone()).with_state(ctx);

    // The auth rate limiter is configured for 5 requests per 15 seconds.
    // We send 6 POST requests to /api/v1/auth/login; the 6th should get 429.
    // The PeerIpKeyExtractor reads ConnectInfo from request extensions,
    // so we inject a fixed socket addr to simulate a real connection.
    let peer_addr: std::net::SocketAddr = "127.0.0.1:9999".parse().unwrap();
    let connect_info = axum::extract::ConnectInfo(peer_addr);

    let mut statuses = Vec::new();
    for i in 0..7 {
        let req = Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header("content-type", "application/json")
            .extension(connect_info)
            .body(Body::from(
                serde_json::json!({
                    "email": "nobody@example.com",
                    "password": "wrong"
                })
                .to_string(),
            ))
            .unwrap();
        let status = app.clone().oneshot(req).await.unwrap().status();
        statuses.push(status);
        // Once we see a 429, we've confirmed the rate limiter works.
        if status == StatusCode::TOO_MANY_REQUESTS {
            break;
        }
        // Small yield to let the rate limiter clock tick.
        if i < 6 {
            tokio::task::yield_now().await;
        }
    }

    // At least one of the later requests should be 429.
    assert!(
        statuses.contains(&StatusCode::TOO_MANY_REQUESTS),
        "expected at least one 429 response, got statuses: {statuses:?}"
    );
}
