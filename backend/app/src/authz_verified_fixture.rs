//! Local Auth fixture using the real public bridge, never a synthetic verified lifetime.
use pdlc_auth_core::service_bridge::{ServiceBridge, VerifiedCentralToken};
use serde_json::json;
use std::ffi::OsString;

pub(crate) struct IdentityFixture {
    pub(crate) verified: VerifiedCentralToken,
    server: tokio::task::JoinHandle<()>,
    environment: Vec<(String, Option<OsString>)>,
}

impl Drop for IdentityFixture {
    fn drop(&mut self) {
        self.server.abort();
        for (key, previous) in &self.environment {
            // Each test owns a distinct bridge prefix, never production Auth settings.
            unsafe {
                match previous {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}

pub(crate) async fn identity(
    prefix: &'static str,
    role: &str,
) -> IdentityFixture {
    let now = chrono::Utc::now();
    let issuer = "http://tracker-role.example.test";
    let mut body = json!({
        "sub":"central-identity", "email":"fixture@example.test",
        "subject_role":role, "role":role,
        "scopes":["task-tracker:read","task-tracker:write"],
        "observed_at":now, "expires_at":now + chrono::Duration::minutes(5),
    });
    body["application_access"] = json!({
            "schema_version":1, "issuer":issuer, "subject":"central-identity",
            "role":if role=="admin" {"admin"} else {"user"}, "applications":["task-tracker"], "revision":1,
            "enforced":true, "observed_at":now,
            "expires_at":now + chrono::Duration::seconds(30),
        });
    let router = axum::Router::new()
        .route(
            "/oidc/jwks",
            axum::routing::get(|| async { axum::Json(json!({"keys":[]})) }),
        )
        .route(
            "/auth/tokens/introspect",
            axum::routing::get(move |headers: axum::http::HeaderMap| {
                let body = body.clone();
                async move {
                    assert_eq!(
                        headers[axum::http::header::AUTHORIZATION],
                        "Bearer sdlc_pat_deadline-fixture"
                    );
                    axum::Json(body)
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let environment = [
        (
            format!("{prefix}_JWKS_URI"),
            format!("http://{address}/oidc/jwks"),
        ),
        (format!("{prefix}_ISSUER"), issuer.to_owned()),
    ]
    .into_iter()
    .map(|(key, value)| {
        let previous = std::env::var_os(&key);
        unsafe { std::env::set_var(&key, value) };
        (key, previous)
    })
    .collect();
    let bridge = ServiceBridge::for_application(prefix, "task-tracker");
    let verified = bridge
        .try_verified_token("sdlc_pat_deadline-fixture")
        .await
        .unwrap();
    IdentityFixture {
        verified,
        server,
        environment,
    }
}
