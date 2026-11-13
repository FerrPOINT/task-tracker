//! The real Auth bridge confirms both finite and persistent machine credentials.
use super::*;
use pdlc_auth_core::service_bridge::ServiceBridge;
use serde_json::json;

struct Environment(Vec<(String, Option<std::ffi::OsString>)>);
impl Drop for Environment {
    fn drop(&mut self) {
        for (key, previous) in &self.0 {
            // This fixture owns a unique bridge prefix, never runtime settings.
            unsafe {
                match previous {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}

struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[tokio::test]
async fn persistent_service_root_does_not_extend_or_replace_the_owner_window() {
    let now = chrono::Utc::now();
    let router = Router::new()
        .route("/oidc/jwks", get(|| async { Json(json!({"keys":[]})) }))
        .route("/auth/tokens/introspect", get(move |headers: HeaderMap| async move {
            let finite = match headers[header::AUTHORIZATION].to_str().unwrap() {
                "Bearer sdlc_pat_background_finite_fixture" => true,
                "Bearer sdlc_pat_background_persistent_fixture" => false,
                _ => panic!("unexpected fixture credential"),
            };
            Json(json!({
                "sub":Uuid::new_v4(), "email":"background@example.test", "subject_role":"service_account",
                "scopes":["task-tracker:background:read"], "observed_at":now,
                "expires_at":finite.then_some(now+chrono::Duration::minutes(5)),
            }))
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let _server = Server(tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap()
    }));
    let mut environment = Environment(vec![]);
    for (key, value) in [
        (
            "TT_BACKGROUND_CLOCK_FIXTURE_JWKS_URI",
            format!("{origin}/oidc/jwks"),
        ),
        ("TT_BACKGROUND_CLOCK_FIXTURE_ISSUER", origin),
    ] {
        environment.0.push((key.to_owned(), std::env::var_os(key)));
        unsafe { std::env::set_var(key, value) };
    }
    let bridge = ServiceBridge::for_application("TT_BACKGROUND_CLOCK_FIXTURE", "task-tracker");
    let persistent = bridge
        .try_verified_token("sdlc_pat_background_persistent_fixture")
        .await
        .unwrap();
    let finite = bridge
        .try_verified_token("sdlc_pat_background_finite_fixture")
        .await
        .unwrap();
    let owner_until = now + chrono::Duration::minutes(10);
    assert!(persistent.lifetime.local_valid_until().is_none());
    assert_eq!(
        source_deadline(&persistent, owner_until).unwrap(),
        owner_until
    );
    let identity_until =
        chrono::DateTime::from_timestamp(finite.lifetime.local_valid_until().unwrap(), 0).unwrap();
    assert_eq!(
        source_deadline(&finite, owner_until).unwrap(),
        identity_until
    );
    let shorter_owner = now + chrono::Duration::seconds(1);
    assert_eq!(
        source_deadline(&persistent, shorter_owner).unwrap(),
        shorter_owner
    );
    assert_eq!(
        source_deadline(&finite, shorter_owner).unwrap(),
        shorter_owner
    );
    assert!(matches!(
        source_deadline(&persistent, now - chrono::Duration::seconds(1)),
        Err(AppError::Forbidden)
    ));
}
