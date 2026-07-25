use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use std::sync::Arc;
use tokio::time::{Duration, timeout};
use tower::ServiceExt;

mod mocks;

fn valid_token(ctx: &app::AppContext) -> String {
    use app::auth::UserClaims;
    use jsonwebtoken::{EncodingKey, Header};
    use shared::UserId;

    let claims = UserClaims {
        sub: UserId::new().to_string(),
        exp: usize::MAX,
    };
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(ctx.config.auth.jwt_secret.as_bytes()),
    )
    .unwrap()
}

#[tokio::test]
async fn serve_forever_responds_to_request() {
    use shared::{AppConfig, AuthConfig, DatabaseConfig, ServerConfig};

    let config = Arc::new(AppConfig {
        database: DatabaseConfig::default(),
        server: ServerConfig {
            address: "127.0.0.1".to_string(),
            port: 0,
        },
        auth: AuthConfig {
            jwt_secret: "test-secret-32-chars-long!!!!!".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
        },
    });

    let ctx = mocks::failing_context_with_config(config.clone());
    let listener = api::bind(ctx.clone()).await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(api::serve_forever(listener, ctx));

    let client = reqwest::Client::new();
    let response = timeout(
        Duration::from_secs(5),
        client.get(format!("http://{}/api/v1/health", addr)).send(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    server.abort();
}

#[tokio::test]
async fn serve_handles_bind_error_gracefully() {
    use shared::{AppConfig, AuthConfig, DatabaseConfig, ServerConfig};

    let config = Arc::new(AppConfig {
        database: DatabaseConfig::default(),
        server: ServerConfig {
            address: "255.255.255.255".to_string(),
            port: 80,
        },
        auth: AuthConfig {
            jwt_secret: "test-secret-32-chars-long!!!!!".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
        },
    });

    let ctx = mocks::failing_context_with_config(config);
    let result = timeout(Duration::from_secs(2), api::serve(ctx)).await;
    assert!(
        result.is_err(),
        "serve should block forever on valid bind; this test uses unreachable addr"
    );
}

#[tokio::test]
async fn routes_return_500_on_internal_errors() {
    let ctx = mocks::failing_context();
    let token = valid_token(ctx.as_ref());
    let app = api::router(ctx.clone()).with_state(ctx);

    let cases: Vec<(&str, &str, Option<&str>)> = vec![
        (
            "POST",
            "/api/v1/auth/register",
            Some(r#"{"email":"t@e.com","username":"t","name":"T","password":"12345678"}"#),
        ),
        (
            "POST",
            "/api/v1/auth/login",
            Some(r#"{"email":"t@e.com","password":"12345678"}"#),
        ),
        (
            "POST",
            "/api/v1/projects",
            Some(r#"{"key":"KEY","name":"Test"}"#),
        ),
        ("GET", "/api/v1/projects/KEY", None),
        ("GET", "/api/v1/projects/KEY/board", None),
        ("GET", "/api/v1/projects/KEY/backlog", None),
        (
            "POST",
            "/api/v1/projects/KEY/board/move",
            Some(
                r#"{"issue_id":"00000000-0000-0000-0000-000000000000","status_id":"00000000-0000-0000-0000-000000000000"}"#,
            ),
        ),
        (
            "POST",
            "/api/v1/issues",
            Some(
                r#"{"project_key":"KEY","issue_type":"task","summary":"T","priority":"medium","status_id":"00000000-0000-0000-0000-000000000000","reporter_id":"00000000-0000-0000-0000-000000000000"}"#,
            ),
        ),
        ("GET", "/api/v1/issues?q=query", None),
        (
            "GET",
            "/api/v1/issues/00000000-0000-0000-0000-000000000000",
            None,
        ),
        (
            "PATCH",
            "/api/v1/issues/00000000-0000-0000-0000-000000000000",
            Some(r#"{}"#),
        ),
    ];

    for (method, path, body) in cases {
        let req = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .body(Body::from(body.unwrap_or("")))
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert!(
            response.status().is_server_error(),
            "{} {} should return 5xx, got {}",
            method,
            path,
            response.status()
        );
    }
}

#[tokio::test]
async fn project_create_invalid_key_returns_400() {
    let ctx = mocks::failing_context();
    let token = valid_token(ctx.as_ref());
    let app = api::router(ctx.clone()).with_state(ctx);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(
                    r#"{"key":"!!! invalid","name":"Bad","description":null}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn project_get_invalid_key_returns_400() {
    let ctx = mocks::failing_context();
    let token = valid_token(ctx.as_ref());
    let app = api::router(ctx.clone()).with_state(ctx);
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/projects/toolongkeyxx")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn bind_returns_listener_on_valid_addr() {
    use shared::{AppConfig, AuthConfig, DatabaseConfig, ServerConfig};

    let config = Arc::new(AppConfig {
        database: DatabaseConfig::default(),
        server: ServerConfig {
            address: "127.0.0.1".to_string(),
            port: 0,
        },
        auth: AuthConfig {
            jwt_secret: "test-secret-32-chars-long!!!!!".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
        },
    });

    let ctx = mocks::failing_context_with_config(config);
    let listener = api::bind(ctx).await.unwrap();
    let local = listener.local_addr().unwrap();
    assert!(local.port() > 0);
}

#[tokio::test]
async fn issue_create_invalid_project_key_returns_400() {
    let ctx = mocks::failing_context();
    let token = valid_token(ctx.as_ref());
    let req = api::dto::CreateIssueRequest {
        project_key: "!!!".to_string(),
        summary: "Test".to_string(),
        description: None,
        issue_type: "task".to_string(),
        priority: "medium".to_string(),
        status_id: "00000000-0000-0000-0000-000000000000".to_string(),
        reporter_id: "00000000-0000-0000-0000-000000000000".to_string(),
        assignee_id: None,
    };
    let app = api::router(ctx.clone()).with_state(ctx);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/issues")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(serde_json::to_string(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn issue_create_invalid_reporter_id_returns_400() {
    let ctx = mocks::failing_context();
    let token = valid_token(ctx.as_ref());
    let req = api::dto::CreateIssueRequest {
        project_key: "KEY".to_string(),
        summary: "Test".to_string(),
        description: None,
        issue_type: "task".to_string(),
        priority: "medium".to_string(),
        status_id: "00000000-0000-0000-0000-000000000000".to_string(),
        reporter_id: "not-a-uuid".to_string(),
        assignee_id: None,
    };
    let app = api::router(ctx.clone()).with_state(ctx);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/issues")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(serde_json::to_string(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn issue_get_invalid_id_returns_400() {
    let ctx = mocks::failing_context();
    let token = valid_token(ctx.as_ref());
    let app = api::router(ctx.clone()).with_state(ctx);
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/issues/not-a-uuid")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn board_move_invalid_issue_id_returns_400() {
    let ctx = mocks::failing_context();
    let token = valid_token(ctx.as_ref());
    let req = api::dto::MoveIssueRequest {
        issue_id: "not-a-uuid".to_string(),
        status_id: "00000000-0000-0000-0000-000000000000".to_string(),
    };
    let app = api::router(ctx.clone()).with_state(ctx);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects/KEY/board/move")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(serde_json::to_string(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn board_move_invalid_status_id_returns_400() {
    let ctx = mocks::failing_context();
    let token = valid_token(ctx.as_ref());
    let req = api::dto::MoveIssueRequest {
        issue_id: "00000000-0000-0000-0000-000000000000".to_string(),
        status_id: "not-a-uuid".to_string(),
    };
    let app = api::router(ctx.clone()).with_state(ctx);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/projects/KEY/board/move")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(serde_json::to_string(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn update_issue_invalid_assignee_id_returns_400() {
    let ctx = mocks::failing_context();
    let token = valid_token(ctx.as_ref());
    let req = api::dto::UpdateIssueRequest {
        summary: None,
        description: None,
        priority: None,
        status_id: None,
        assignee_id: Some("not-a-uuid".to_string()),
    };
    let app = api::router(ctx.clone()).with_state(ctx);
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/v1/issues/00000000-0000-0000-0000-000000000000")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(serde_json::to_string(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
