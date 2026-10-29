use super::*;
use axum::{http::StatusCode, routing::get};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};

fn managed(id: uuid::Uuid, role: &str, status: &str) -> Value {
    json!({"id":id,"email":"service@example.test","display_name":"PM service",
        "role":role,"status":status,"username":"service","setup_delivery_status":"not_required"})
}

fn headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        "Bearer fixture-admin-session".parse().unwrap(),
    );
    headers
}

#[test]
fn active_admin_status_does_not_replace_identity_active_confirmation() {
    use shared::space_access::{AdmissionValidity, CentralIdentity, IdentityConfirmation};
    let target = UserId::new();
    let listed: CentralManagedUser =
        serde_json::from_value(managed(target.as_uuid(), "service_account", "active")).unwrap();
    assert_eq!(listed.status, "active");
    assert_eq!(listed.role, "service_account");
    let now = chrono::Utc::now().timestamp();
    let confirmed = IdentityConfirmation {
        identity: CentralIdentity {
            issuer: "https://auth.example.test".into(),
            subject: listed.id,
        },
        active: true,
        validity: AdmissionValidity::confirmed(now, now + 60, now + 60).unwrap(),
    };
    assert!(require_active_service_identity(Some(confirmed.clone())).is_ok());
    let inactive = IdentityConfirmation {
        active: false,
        ..confirmed
    };
    assert!(require_active_service_identity(Some(inactive)).is_err());
    assert!(require_active_service_identity(None).is_err());
}

#[tokio::test]
async fn explicit_service_lookup_uses_admin_active_pages_and_exact_subject() {
    let target = UserId::new();
    let seen = Arc::new(AtomicUsize::new(0));
    let calls = seen.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = axum::Router::new().route(
        "/auth/users",
        get(
            move |axum::extract::Query(query): axum::extract::Query<HashMap<String, String>>,
                  received: HeaderMap| {
                let calls = calls.clone();
                async move {
                    assert_eq!(received["authorization"], "Bearer fixture-admin-session");
                    assert_eq!(query.len(), 3);
                    assert_eq!(query["status"], "active");
                    assert_eq!(query["limit"], "100");
                    let page = calls.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(query["offset"], (page * 100).to_string());
                    if page == 0 {
                        Json(
                            (1..=100)
                                .map(|n| managed(uuid::Uuid::from_u128(n), "user", "active"))
                                .collect::<Vec<_>>(),
                        )
                    } else {
                        Json(vec![managed(target.as_uuid(), "service_account", "active")])
                    }
                }
            },
        ),
    );
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let outcome =
        fetch_explicit_service_account(&headers(), &format!("http://{address}/oidc/jwks"), target)
            .await;
    server.abort();
    let _ = server.await;
    let result = outcome.unwrap();
    assert_eq!(result.id, target.to_string());
    assert_eq!(result.status, "active");
    assert_eq!(seen.load(Ordering::SeqCst), 2);
    // The explicit result is never appended to the human-selector collection.
    assert!(project_directory(&[], &HashMap::new()).unwrap().is_empty());
}

#[tokio::test]
async fn explicit_service_lookup_refuses_human_inactive_foreign_and_malformed_identity() {
    let target = UserId::new();
    let good = managed(target.as_uuid(), "service_account", "active");
    let mut no_email = good.clone();
    no_email["email"] = json!("");
    for body in [
        json!([]),
        json!([managed(target.as_uuid(), "user", "active")]),
        json!([managed(target.as_uuid(), "admin", "active")]),
        json!([managed(target.as_uuid(), "service_account", "disabled")]),
        json!([managed(target.as_uuid(), "service_account", "pending")]),
        json!([managed(uuid::Uuid::new_v4(), "service_account", "active")]),
        json!([no_email]),
        json!([good.clone(), good]),
        json!({"users":[]}),
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = axum::Router::new().route(
            "/auth/users",
            get(move || {
                let body = body.clone();
                async move { Json(body) }
            }),
        );
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let outcome = fetch_explicit_service_account(
            &headers(),
            &format!("http://{address}/oidc/jwks"),
            target,
        )
        .await;
        server.abort();
        let _ = server.await;
        assert!(outcome.is_err());
    }
}

#[tokio::test]
async fn explicit_service_lookup_preserves_admin_denial_and_never_follows_redirects() {
    for status in [
        StatusCode::FORBIDDEN,
        StatusCode::UNAUTHORIZED,
        StatusCode::TEMPORARY_REDIRECT,
    ] {
        let followed = Arc::new(AtomicUsize::new(0));
        let calls = followed.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let router = axum::Router::new()
            .route(
                "/auth/users",
                get(move || async move { (status, [("location", "/unexpected")]) }),
            )
            .route(
                "/unexpected",
                get(move || {
                    let calls = calls.clone();
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        StatusCode::OK
                    }
                }),
            );
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let outcome = fetch_explicit_service_account(
            &headers(),
            &format!("http://{address}/oidc/jwks"),
            UserId::new(),
        )
        .await;
        server.abort();
        let _ = server.await;
        if status == StatusCode::FORBIDDEN {
            assert!(matches!(outcome, Err(shared::AppError::Forbidden)));
        } else {
            assert!(outcome.is_err());
        }
        assert_eq!(followed.load(Ordering::SeqCst), 0);
    }
}
