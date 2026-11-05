//! A failed second factor must not rotate the user's existing refresh session.
use app::{AppContext, commands::RegisterCommand};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    routing::post,
};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

#[tokio::test]
async fn password_probe_does_not_issue_or_replace_a_session_before_mfa() {
    let mut config = shared::AppConfig::default();
    config.auth.jwt_secret = "mfa-standalone-fixture-signing-key".into();
    config.auth.totp_key = "mfa-standalone-fixture-encryption-key".into();
    let repos = Arc::new(domain::Repositories {
        users: Arc::new(domain::MemoryUserRepository::default()),
        totp: Arc::new(domain::stubs::memory::MemoryTotpRepository::default()),
        ..Default::default()
    });
    let ctx = Arc::new(AppContext::new(
        Arc::new(config),
        repos,
        Arc::new(domain::InMemoryStorage::default()),
    ));
    let initial = ctx
        .services
        .auth
        .register(RegisterCommand {
            email: "login-mfa@example.test".into(),
            username: "mfa-login".into(),
            name: "MFA fixture".into(),
            password: "fixture-password-only".into(),
        })
        .await
        .unwrap();
    let user = initial.user.id.parse().unwrap();
    let enrollment = ctx.services.totp.setup(user).await.unwrap();
    let step = chrono::Utc::now().timestamp() / 30;
    let code = app::totp::TotpService::code_for_step(&enrollment.secret, step).unwrap();
    ctx.services.totp.enable(user, &code).await.unwrap();
    let previous = ctx
        .repos
        .users
        .get_by_id(user)
        .await
        .unwrap()
        .refresh_token_hash;
    let app = Router::new()
        .route("/login", post(api::routes::auth::login))
        .with_state(ctx.clone());
    let request = |code: Option<String>| {
        Request::post("/login").header("content-type","application/json")
        .body(Body::from(json!({"email":"login-mfa@example.test","password":"fixture-password-only","totp_code":code}).to_string())).unwrap()
    };
    let response = app.clone().oneshot(request(None)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("set-cookie").is_none());
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(value["totp_required"], true);
    assert_eq!(value["access_token"], "");
    assert_eq!(
        ctx.repos
            .users
            .get_by_id(user)
            .await
            .unwrap()
            .refresh_token_hash,
        previous,
        "a correct password with missing MFA must preserve the old session"
    );
    let current = chrono::Utc::now().timestamp() / 30;
    let code = app::totp::TotpService::code_for_step(&enrollment.secret, current + 1).unwrap();
    let response = app.oneshot(request(Some(code))).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("set-cookie").is_some());
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(value["totp_required"], false);
    assert!(!value["access_token"].as_str().unwrap().is_empty());
}
