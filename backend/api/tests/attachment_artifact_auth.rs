use axum::{
    Extension, Json, Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode},
    middleware::from_fn_with_state,
    routing::get,
};
use domain::sdlc::Principal;
use serde_json::json;
use std::{
    ffi::OsString,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tower::ServiceExt;
use uuid::Uuid;

const DELIVERY: &str = "sdlc_pat_attachment-delivery-fixture";
const READ_ONLY: &str = "sdlc_pat_attachment-readonly-fixture";
const PM: &str = "sdlc_pat_attachment-pm-fixture";
const PM_NO_READ: &str = "sdlc_pat_attachment-pm-no-read-fixture";
const PM_MIXED: &str = "sdlc_pat_attachment-pm-mixed-fixture";
const SCHEDULER: &str = "sdlc_pat_attachment-scheduler-fixture";
const REVOKED: &str = "sdlc_pat_attachment-revoked-fixture";
const UNAVAILABLE: &str = "sdlc_pat_attachment-unavailable-fixture";
const MISSING_LIFETIME: &str = "sdlc_pat_attachment-missing-lifetime-fixture";

struct Environment(Vec<(&'static str, Option<OsString>)>);

impl Environment {
    fn isolated() -> Self {
        let keys = [
            "TT_AUTH__CENTRAL_JWKS_URI",
            "TT_AUTH__CENTRAL_ISSUER",
            "TT_AUTH__CENTRAL_LOGIN_URL",
            "TT_AUTH__CENTRAL_TIMEOUT_SECS",
            "TT_NAMESPACE__OWNER_SUBJECTS",
            "TT_NAMESPACE__READER_SUBJECTS",
            "TASKTRACKER_SDLC__ORCHESTRATOR_SUBJECT",
            "TASKTRACKER_SDLC__VERIFIER_SUBJECT",
            "TASKTRACKER_SDLC__RESERVATION_SCHEDULER_SUBJECT",
        ];
        let saved = keys
            .into_iter()
            .map(|key| (key, std::env::var_os(key)))
            .collect();
        // This integration target has one test. Environment changes precede Tokio runtimes.
        for key in keys {
            unsafe { std::env::remove_var(key) };
        }
        Self(saved)
    }
}

impl Drop for Environment {
    fn drop(&mut self) {
        // Both owned runtimes have been dropped before restoring the test process environment.
        for (key, value) in &self.0 {
            unsafe {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}

fn context() -> Arc<app::AppContext> {
    let mut config = shared::AppConfig::default();
    config.auth.jwt_secret = "attachment-human-fallback-fixture-secret".into();
    let repos = Arc::new(domain::Repositories {
        users: Arc::new(domain::MemoryUserRepository::default()),
        ..Default::default()
    });
    let storage = Arc::new(domain::InMemoryStorage::default());
    let mut ctx = app::AppContext::new(Arc::new(config), repos.clone(), storage.clone());
    ctx.services.attachment = Arc::new(
        app::services::AttachmentServiceImpl::new(
            repos.attachments.clone(),
            repos.issues.clone(),
            storage,
            repos.watchers.clone(),
            ctx.events.clone(),
            repos.notifications.clone(),
            repos.notification_settings.clone(),
            ctx.authz.clone(),
        )
        .with_artifact_config(domain::sdlc::SdlcConfig {
            instance_id: "attachment-test".into(),
            orchestrator_subject: "pm-fixture".into(),
            verifier_subject: "verifier-fixture".into(),
            reservation_scheduler_subject: "scheduler-fixture".into(),
            single_task_project_ids: Default::default(),
        }),
    );
    Arc::new(ctx)
}

async fn machine_probe(
    actor: Option<Extension<Principal>>,
    claims: Option<Extension<api::middleware::auth::UserClaims>>,
) -> StatusCode {
    let Extension(actor) = actor.expect("Central-validated service principal is required");
    assert!(
        claims.is_none(),
        "service accounts must not acquire UserClaims"
    );
    assert!(!actor.human_session && !actor.trusted_human);
    assert!(matches!(
        actor.subject.as_str(),
        "delivery-fixture" | "scheduler-fixture"
    ));
    StatusCode::NO_CONTENT
}

async fn human_probe(
    actor: Option<Extension<Principal>>,
    claims: Option<Extension<api::middleware::auth::UserClaims>>,
) -> StatusCode {
    assert!(actor.is_none());
    assert!(
        claims.is_some(),
        "legacy human authentication must still supply its own claims"
    );
    StatusCode::NO_CONTENT
}

async fn download_probe(
    actor: Option<Extension<Principal>>,
    claims: Option<Extension<api::middleware::auth::UserClaims>>,
) -> StatusCode {
    match actor {
        Some(Extension(actor)) if app::sdlc::has_pm_grant(&actor.scopes) => {
            assert!(claims.is_none(), "PM must not acquire human UserClaims");
            assert!(!actor.human_session && !actor.trusted_human);
            assert_eq!(actor.subject, "pm-fixture");
            assert_eq!(actor.scopes.len(), 3);
            assert!(actor.scopes.contains("task-tracker:read"));
            assert!(actor.scopes.contains("task-tracker:write"));
            assert!(app::sdlc::pm_grant_task(&actor).unwrap().is_some());
            assert!(!app::sdlc_lifecycle::has_delivery_grant(&actor.scopes));
            StatusCode::NO_CONTENT
        }
        actor => machine_probe(actor, claims).await,
    }
}

fn machine_routes(ctx: Arc<app::AppContext>) -> Router {
    Router::new().nest(
        "/api/v1",
        Router::new()
            .route(
                "/issues/{id}/attachments",
                get(machine_probe).post(machine_probe),
            )
            .route("/attachments/{id}/download", get(download_probe))
            .route_layer(from_fn_with_state(
                ctx,
                api::middleware::attachment_auth::attachment_auth,
            )),
    )
}

async fn request(router: Router, method: &str, path: &str, token: Option<&str>) -> StatusCode {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    if method == "POST" {
        request = request.header(
            "content-type",
            "multipart/form-data; boundary=attachment-test",
        );
    }
    let body = if method == "POST" {
        Body::from(
            "--attachment-test\r\nContent-Disposition: form-data; name=\"file\"; filename=\"report.txt\"\r\nContent-Type: text/plain\r\n\r\nfixture\r\n--attachment-test--\r\n",
        )
    } else {
        Body::empty()
    };
    router
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap()
        .status()
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn attachment_routes_use_real_central_bridge_and_service_policy_without_machine_user_claims() {
    let _environment = Environment::isolated();
    let task = Uuid::new_v4();
    let foreign_task = Uuid::new_v4();
    let attachment = Uuid::new_v4();
    let task_path = format!("/api/v1/issues/{task}/attachments");
    let download = format!("/api/v1/attachments/{attachment}/download");
    {
        let runtime = runtime();
        runtime.block_on(async {
            let ctx = context();
            ctx.services
                .auth
                .register(app::commands::RegisterCommand {
                    email: "attachment-human@example.test".into(),
                    username: "attachment-human".into(),
                    name: "Attachment human".into(),
                    password: "fixture-password-123".into(),
                })
                .await
                .unwrap();
            let token = ctx
                .services
                .auth
                .login(app::commands::LoginCommand {
                    email: "attachment-human@example.test".into(),
                    password: "fixture-password-123".into(),
                })
                .await
                .unwrap()
                .access_token;
            let probe =
                Router::new()
                    .route("/human", get(human_probe))
                    .route_layer(from_fn_with_state(
                        ctx.clone(),
                        api::middleware::attachment_auth::attachment_auth,
                    ));
            assert_eq!(
                request(probe, "GET", "/human", Some(&token)).await,
                StatusCode::NO_CONTENT
            );
            let app = api::router(ctx.clone()).with_state(ctx);
            assert_eq!(
                request(app.clone(), "GET", &task_path, Some(&token)).await,
                StatusCode::NOT_FOUND
            );
            assert_eq!(
                request(app.clone(), "POST", &task_path, Some(&token)).await,
                StatusCode::NOT_FOUND
            );
            assert_eq!(
                request(app.clone(), "GET", &download, Some(&token)).await,
                StatusCode::NOT_FOUND
            );
            for (method, path) in [
                ("GET", task_path.as_str()),
                ("POST", task_path.as_str()),
                ("GET", download.as_str()),
            ] {
                assert_eq!(
                    request(app.clone(), method, path, None).await,
                    StatusCode::UNAUTHORIZED
                );
            }
        });
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    // No Tokio runtime exists while configuring this isolated test process.
    unsafe {
        std::env::set_var("TT_AUTH__CENTRAL_JWKS_URI", format!("{origin}/oidc/jwks"));
        std::env::set_var("TT_AUTH__CENTRAL_ISSUER", &origin);
        std::env::set_var("TT_AUTH__CENTRAL_TIMEOUT_SECS", "2");
        std::env::set_var(
            "TASKTRACKER_SDLC__RESERVATION_SCHEDULER_SUBJECT",
            "scheduler-fixture",
        );
        std::env::set_var("TASKTRACKER_SDLC__ORCHESTRATOR_SUBJECT", "pm-fixture");
        std::env::set_var("TASKTRACKER_SDLC__VERIFIER_SUBJECT", "verifier-fixture");
    }
    let runtime = runtime();
    runtime.block_on(async {
        let calls=Arc::new(AtomicUsize::new(0));
        let count=calls.clone();
        let grant=format!("task-tracker:sdlc:delivery:{task}:{}:{}:{}:1",Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4());
        let pm_grant=grant.replace(":delivery:",":pm:");
        let central=Router::new()
            .route("/oidc/jwks",get(||async {Json(json!({"keys":[]}))}))
            .route("/auth/tokens/introspect",get(move |headers:HeaderMap| {
                count.fetch_add(1,Ordering::SeqCst);
                let token=headers["authorization"].to_str().unwrap().strip_prefix("Bearer ").unwrap();
                let (status,subject,scopes)=match token {
                    DELIVERY | MISSING_LIFETIME | "sdlc_pat_missing_role_fixture" => (StatusCode::OK,"delivery-fixture",vec!["task-tracker:read".into(),"task-tracker:write".into(),grant.clone()]),
                    READ_ONLY => (StatusCode::OK,"delivery-fixture",vec!["task-tracker:read".into(),grant.clone()]),
                    PM => (StatusCode::OK,"pm-fixture",vec!["task-tracker:read".into(),"task-tracker:write".into(),pm_grant.clone()]),
                    PM_NO_READ => (StatusCode::OK,"pm-fixture",vec!["task-tracker:write".into(),pm_grant.clone()]),
                    PM_MIXED => (StatusCode::OK,"pm-fixture",vec!["task-tracker:read".into(),"task-tracker:write".into(),pm_grant.clone(),grant.clone()]),
                    SCHEDULER => (StatusCode::OK,"scheduler-fixture",vec!["task-tracker:read".into()]),
                    UNAVAILABLE => (StatusCode::SERVICE_UNAVAILABLE,"",vec![]),
                    _ => (StatusCode::UNAUTHORIZED,"",vec![]),
                };
                let include_lifetime=token != MISSING_LIFETIME;
                let include_role=token != "sdlc_pat_missing_role_fixture";
                async move {
                    let mut identity=json!({"sub":subject,"email":"central-fixture@example.test","scopes":scopes});
                    if include_role {
                        identity["subject_role"]=json!("service_account");
                    }
                    if include_lifetime {
                        let observed=chrono::Utc::now();
                        identity["observed_at"]=json!(observed);
                        identity["expires_at"]=json!(observed+chrono::Duration::seconds(60));
                    }
                    (status,Json(identity))
                }
            }));
        let listener=tokio::net::TcpListener::from_std(listener).unwrap();
        let server=tokio::spawn(async move {axum::serve(listener,central).await.unwrap()});
        let ctx=context();
        let probe=machine_routes(ctx.clone());
        for (token,method,path,expected) in [
            ("sdlc_pat_missing_role_fixture","POST",task_path.as_str(),StatusCode::FORBIDDEN),
            (DELIVERY,"POST",task_path.as_str(),StatusCode::NO_CONTENT),
            (DELIVERY,"GET",task_path.as_str(),StatusCode::NO_CONTENT),
            (DELIVERY,"GET",download.as_str(),StatusCode::NO_CONTENT),
            (READ_ONLY,"GET",task_path.as_str(),StatusCode::NO_CONTENT),
            (READ_ONLY,"POST",task_path.as_str(),StatusCode::FORBIDDEN),
            (SCHEDULER,"GET",download.as_str(),StatusCode::NO_CONTENT),
            (SCHEDULER,"GET",task_path.as_str(),StatusCode::NO_CONTENT),
            (SCHEDULER,"POST",task_path.as_str(),StatusCode::FORBIDDEN),
            (PM,"GET",task_path.as_str(),StatusCode::FORBIDDEN),
            (PM,"POST",task_path.as_str(),StatusCode::FORBIDDEN),
            (PM,"GET",download.as_str(),StatusCode::NO_CONTENT),
            (PM,"HEAD",download.as_str(),StatusCode::FORBIDDEN),
            (PM_NO_READ,"GET",download.as_str(),StatusCode::FORBIDDEN),
            (PM_MIXED,"GET",download.as_str(),StatusCode::FORBIDDEN),
            (REVOKED,"GET",download.as_str(),StatusCode::UNAUTHORIZED),
            (UNAVAILABLE,"GET",download.as_str(),StatusCode::SERVICE_UNAVAILABLE),
        ] {
            let before=calls.load(Ordering::SeqCst);
            assert_eq!(request(probe.clone(),method,path,Some(token)).await,expected,"{method} {path} fixture {token}");
            assert_eq!(calls.load(Ordering::SeqCst),before+1,"machine request must not fall back to human auth");
        }
        let foreign=format!("/api/v1/issues/{foreign_task}/attachments");
        for method in ["GET","POST"] {
            assert_eq!(request(probe.clone(),method,&foreign,Some(DELIVERY)).await,StatusCode::FORBIDDEN);
        }
        let app=api::router(ctx.clone()).with_state(ctx);
        // Route admission is not evidence authority: the real service has no saved artifact custody.
        for (method,path,token) in [("POST",task_path.as_str(),DELIVERY),("GET",task_path.as_str(),DELIVERY),("GET",download.as_str(),SCHEDULER),("GET",download.as_str(),PM)] {
            assert_eq!(request(app.clone(),method,path,Some(token)).await,StatusCode::SERVICE_UNAVAILABLE);
        }
        // Unbounded legacy introspection must never authorize a generic human route.
        assert_eq!(request(app.clone(),"GET","/api/v1/dashboard",Some(MISSING_LIFETIME)).await,StatusCode::SERVICE_UNAVAILABLE);
        for token in [DELIVERY,PM] {
            assert_eq!(request(app.clone(),"GET","/api/v1/dashboard",Some(token)).await,StatusCode::FORBIDDEN);
        }
        for (method,path,token) in [("POST",task_path.as_str(),PM),("GET",task_path.as_str(),PM),
            ("HEAD",download.as_str(),PM),
            ("GET",download.as_str(),PM_NO_READ),("GET",download.as_str(),PM_MIXED),
            ("POST",task_path.as_str(),SCHEDULER),("GET",task_path.as_str(),SCHEDULER),
            ("GET",foreign.as_str(),DELIVERY),("POST",foreign.as_str(),DELIVERY)] {
            assert_eq!(request(app.clone(),method,path,Some(token)).await,StatusCode::FORBIDDEN);
        }
        server.abort();
    });
}
