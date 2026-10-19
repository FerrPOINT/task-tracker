//! Synthetic loopback Auth boundary used only by the owned native fixture.
//! The production constructor still requires the actual SDK verification path.
use app::space_access::{CentralRequestContext, RequestOperation, SpaceActor};
use shared::{UserId, space_access::*};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;
pub struct Fixture {
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
impl Fixture {
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        unsafe {
            std::env::set_var("TT_AUTH__CENTRAL_JWKS_URI", format!("{origin}/oidc/jwks"));
            std::env::set_var("TT_AUTH__CENTRAL_ISSUER", &origin);
        }
        let server = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    loop {
                        if bytes.windows(4).any(|s| s == b"\r\n\r\n") {
                            break;
                        }
                        let mut chunk = [0; 4096];
                        let n = stream.read(&mut chunk).await.unwrap();
                        assert!(n > 0 && bytes.len() + n <= 16384);
                        bytes.extend_from_slice(&chunk[..n]);
                    }
                    let head = String::from_utf8(bytes).unwrap();
                    let body = if head.starts_with("GET /oidc/jwks ") {
                        serde_json::json!({"keys":[]})
                    } else {
                        assert_eq!(
                            head.lines().next().unwrap(),
                            "GET /auth/tokens/introspect?include_lifetime=true HTTP/1.1"
                        );
                        let bearer = head
                            .lines()
                            .filter_map(|s| s.split_once(':'))
                            .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
                            .unwrap()
                            .1
                            .trim();
                        let (subject, scope) = bearer
                            .strip_prefix("Bearer sdlc_pat_fixture_")
                            .unwrap()
                            .split_once('_')
                            .unwrap();
                        Uuid::parse_str(subject).unwrap();
                        assert!(matches!(scope, "read" | "write"));
                        serde_json::json!({"sub":subject,"email":"fixture@example.test","subject_role":"member","scopes":[format!("task-tracker:{scope}")],"observed_at":shared::now(),"expires_at":shared::now()+std::time::Duration::from_secs(300)})
                    };
                    let body = serde_json::to_vec(&body).unwrap();
                    stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await.unwrap();
                    stream.write_all(&body).await.unwrap();
                });
            }
        });
        Self { server }
    }
    pub async fn frame(
        &self,
        user: UserId,
        subject: Uuid,
        spaces: &[(SpaceRef, SpaceState)],
        seconds: i64,
        write: bool,
    ) -> Arc<CentralRequestContext> {
        let token = format!(
            "sdlc_pat_fixture_{subject}_{}",
            if write { "write" } else { "read" }
        );
        let actor = app::space_access::CENTRAL_BRIDGE
            .try_verified_token(&token)
            .await
            .unwrap();
        let now = shared::now().timestamp();
        let identity = CentralIdentity {
            issuer: actor.issuer.clone(),
            subject: subject.to_string(),
        };
        let admission = SpaceAdmission {
            schema_version: 1,
            identity,
            identity_active: true,
            policy_revision: 1,
            policy: SpaceAccessPolicy::Selected {
                spaces: spaces.iter().map(|(s, _)| s.clone()).collect(),
            },
            validity: AdmissionValidity::confirmed(now, now + seconds, i64::MAX).unwrap(),
            spaces: spaces
                .iter()
                .map(|(s, state)| ConfirmedSpace {
                    space: s.clone(),
                    revision: 1,
                    state: *state,
                })
                .collect(),
        };
        let spaces = SpaceActor::verified(user, &actor, admission).unwrap();
        Arc::new(
            CentralRequestContext::verified(user, &actor, Some(spaces), RequestOperation::Write)
                .unwrap(),
        )
    }
}
