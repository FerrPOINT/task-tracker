//! Real bridge and restricted Auth owner reader; no human session is fabricated.
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use pdlc_auth_core::{human_request, service_bridge::VerifiedCentralToken};
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicUsize, Ordering},
};
use uuid::Uuid;

#[derive(Clone)]
struct Fixture {
    machine: Uuid,
    owner: Uuid,
    issuer: String,
    applications: Vec<&'static str>,
    expires_ms: i64,
    unavailable: bool,
    enforced: bool,
    calls: Arc<AtomicUsize>,
}
async fn machine(
    State(state): State<Arc<RwLock<Fixture>>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    assert_eq!(
        headers["authorization"],
        "Bearer sdlc_pat_native_owner_fixture"
    );
    let state = state.read().unwrap().clone();
    let now = chrono::Utc::now();
    Json(
        serde_json::json!({"sub":state.machine,"email":"native-owner-fixture@example.test","subject_role":"service_account",
        "scopes":["task-tracker:read","task-tracker:write"],
        "observed_at":now,"expires_at":now+chrono::Duration::seconds(300)}),
    )
}
async fn owner(
    State(state): State<Arc<RwLock<Fixture>>>,
    headers: HeaderMap,
    Json(request): Json<serde_json::Value>,
) -> axum::response::Response {
    assert_eq!(
        headers["authorization"],
        "Bearer fixture-application-access-reader"
    );
    let state = state.read().unwrap().clone();
    assert_eq!(
        request,
        serde_json::json!({"schema_version":1,"subjects":[state.owner]})
    );
    state.calls.fetch_add(1, Ordering::SeqCst);
    if state.unavailable {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let now = chrono::Utc::now();
    Json(serde_json::json!({"schema_version":1,"decisions":[{
        "schema_version":1,"issuer":state.issuer,"subject":state.owner,"role":"user",
        "applications":state.applications,"revision":1,"enforced":state.enforced,
        "observed_at":now,"expires_at":now+chrono::Duration::milliseconds(state.expires_ms)
    }]}))
    .into_response()
}
fn scope(actor: &VerifiedCentralToken) -> app::native_space_access::NativeRequest {
    app::native_space_access::NativeRequest::verified(actor, true, true).unwrap()
}
#[tokio::test]
async fn native_owner_admission_uses_current_apps_and_one_original_window_per_operation() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let state = Arc::new(RwLock::new(Fixture {
        machine: Uuid::new_v4(),
        owner: Uuid::new_v4(),
        issuer: issuer.clone(),
        applications: vec!["task-tracker", "fleet-control"],
        expires_ms: 30000,
        unavailable: false,
        enforced: true,
        calls: Arc::new(AtomicUsize::new(0)),
    }));
    let router = Router::new()
        .route(
            "/oidc/jwks",
            get(|| async { Json(serde_json::json!({"keys":[]})) }),
        )
        .route("/auth/tokens/introspect", get(machine))
        .route("/auth/access/decisions", post(owner))
        .with_state(state.clone());
    // This dedicated test executable owns its configuration before either client starts.
    unsafe {
        std::env::set_var("TT_AUTH__CENTRAL_JWKS_URI", format!("{issuer}/oidc/jwks"));
        std::env::set_var("TT_AUTH__CENTRAL_ISSUER", &issuer);
        std::env::set_var("TT_AUTH__CENTRAL_APPLICATION_ACCESS_REQUIRED", "true");
        std::env::set_var(
            "TT_AUTH__CENTRAL_APPLICATION_ACCESS_READER_TOKEN",
            "fixture-application-access-reader",
        );
    }
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let actor = app::space_access::CENTRAL_BRIDGE
        .try_verified_token("sdlc_pat_native_owner_fixture")
        .await
        .unwrap();
    assert_eq!(actor.subject_role.as_deref(), Some("service_account"));
    let target = state.read().unwrap().owner;
    let calls = state.read().unwrap().calls.clone();
    let first = scope(&actor);
    first
        .require_owner(target, &["task-tracker", "fleet-control"])
        .await
        .unwrap();
    let deadline = first.deadline();
    first
        .clone()
        .require_owner(target, &["task-tracker", "fleet-control"])
        .await
        .unwrap();
    assert_eq!(first.deadline(), deadline);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(first.identity().subject, actor.context.user_id);
    assert_eq!(human_request::current_role(), Ok(None));
    assert!(
        first
            .require_owner(Uuid::new_v4(), &["task-tracker"])
            .await
            .is_err()
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "another aggregate cannot replace the bound owner"
    );
    for required in [
        vec![],
        vec![""],
        vec!["task-tracker:write"],
        vec!["project-workflow"],
    ] {
        assert!(first.require_owner(target, &required).await.is_err());
    }
    assert!(
        first
            .require_owner(Uuid::nil(), &["task-tracker"])
            .await
            .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    state.write().unwrap().applications = vec!["task-tracker"];
    assert!(
        scope(&actor)
            .require_owner(target, &["task-tracker", "fleet-control"])
            .await
            .is_err()
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "a new stage rechecks the owner"
    );
    state.write().unwrap().applications = vec!["task-tracker", "fleet-control"];
    state.write().unwrap().expires_ms = 1000;
    let expiring = scope(&actor);
    expiring
        .require_owner(target, &["task-tracker", "fleet-control"])
        .await
        .unwrap();
    let deadline = expiring.deadline();
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert!(!expiring.is_current());
    assert!(!expiring.may_write());
    assert!(
        expiring
            .require_owner(target, &["task-tracker", "fleet-control"])
            .await
            .is_err()
    );
    assert_eq!(deadline, expiring.deadline());
    assert_eq!(
        calls.load(Ordering::SeqCst),
        3,
        "expiry cannot silently refresh one accepted operation"
    );
    state.write().unwrap().unavailable = true;
    assert!(matches!(
        scope(&actor).require_owner(target, &["task-tracker"]).await,
        Err(shared::AppError::Unavailable(_))
    ));
    let mut human = actor.clone();
    human.subject_role = Some("user".into());
    assert!(app::native_space_access::NativeRequest::verified(&human, true, true).is_err());
    assert_eq!(human_request::current_role(), Ok(None));
    server.abort();
}
