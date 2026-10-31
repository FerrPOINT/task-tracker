//! Real Tracker HTTP + PostgreSQL + SDK Central Auth bridge. Auth/Fleet owners
//! and source facts are isolated fixtures, never live execution acceptance.
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    routing::{get, post},
};
use domain::sdlc::{Principal, SdlcConfig, SdlcRepository};
use domain::sdlc_source::{
    SetSourcePolicy, SourcePolicySpec, TaskSourceRepository, VerifiedSource,
};
use migration::MigratorTrait;
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tower::ServiceExt;
use uuid::Uuid;
const OWNER: &str = "10000000-0000-4000-8000-000000000001";
const TASK: &str = "50000000-0000-4000-8000-000000000004";
const JWT: &str = "isolated-handoff-fixture-only";
const FULL: &str = "pdlc_pat_fixture_full_owner_AAAAAAAAAAAAAAAAAA";
const TRACKER: &str = "pdlc_pat_fixture_tracker_owner_AAAAAAAAAAAAAAA";
const MACHINE: &str = "pdlc_pat_fixture_machine_owner_AAAAAAAAAAAAAAA";
const SCHEDULER: &str = "pdlc_pat_fixture_scheduler_read_AAAAAAAAAAAAAAA";
const SCHEDULER_WRITE: &str = "pdlc_pat_fixture_scheduler_write_AAAAAAAAAAAAAA";
#[derive(Clone)]
struct Owners {
    issuer: String,
    routes: Value,
    available: Arc<AtomicBool>,
    fleet_reads: Arc<AtomicUsize>,
    owner_apps: Arc<AtomicBool>,
    space_access: Arc<AtomicBool>,
    space_revision: Arc<AtomicUsize>,
    allowed_spaces: Vec<shared::space_access::SpaceRef>,
    owner_reads: Arc<AtomicUsize>,
    change_during_lookup: Arc<AtomicBool>,
    db: Arc<sea_orm::DatabaseConnection>,
}
async fn introspect(State(s): State<Owners>, headers: HeaderMap) -> Json<Value> {
    let bearer = headers.get("authorization").unwrap().to_str().unwrap();
    let full = bearer.contains("_full_");
    let scheduler = bearer.contains("_scheduler_");
    let subject = if scheduler { "scheduler-qa" } else { OWNER };
    let machine = bearer.contains("_machine_") || scheduler;
    let now = chrono::Utc::now();
    let apps = if full {
        vec!["task-tracker", "fleet-control", "project-workflow", "ci-cd"]
    } else {
        vec!["task-tracker"]
    };
    let mut scopes: Vec<String> = apps
        .iter()
        .flat_map(|app| [format!("{app}:read"), format!("{app}:write")])
        .filter(|scope| !bearer.contains("readonly") || scope.ends_with(":read"))
        .collect();
    if scheduler {
        scopes = if bearer.contains("_write_") {
            vec!["task-tracker:write".into()]
        } else {
            vec!["task-tracker:read".into()]
        }
    }
    Json(
        json!({"sub":subject,"email":"operator@example.test","scopes":scopes,"display_name":"Fixture owner",
        "subject_role":if machine{"service_account"}else{"user"},"observed_at":now,"expires_at":now+chrono::Duration::seconds(60),
        "application_access":if machine{Value::Null}else{json!({"schema_version":1,"issuer":s.issuer,"subject":OWNER,"role":"user","applications":apps,"revision":1,"enforced":true,"observed_at":now,"expires_at":now+chrono::Duration::seconds(60)})}}),
    )
}
async fn owner_decisions(
    State(s): State<Owners>,
    headers: HeaderMap,
    Json(query): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers.get("authorization").unwrap(),
        "Bearer isolated-owner-reader-fixture"
    );
    assert_eq!(query["subjects"], json!([OWNER]));
    let open=s.db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT count(*) AS n FROM pg_stat_activity WHERE datname=current_database() AND state='idle in transaction'".to_owned())).await.unwrap().unwrap().try_get::<i64>("","n").unwrap();
    assert_eq!(
        open, 0,
        "owner HTTP must not hold a product transaction open"
    );
    s.owner_reads.fetch_add(1, Ordering::SeqCst);
    if s.change_during_lookup.swap(false, Ordering::SeqCst) {
        s.db.execute_unprepared("UPDATE issues SET description='Changed while owner HTTP was in flight' WHERE key='CORE-4'").await.unwrap();
    }
    let apps = if s.owner_apps.load(Ordering::SeqCst) {
        vec!["task-tracker", "fleet-control", "project-workflow", "ci-cd"]
    } else {
        vec!["task-tracker"]
    };
    let now = chrono::Utc::now();
    Json(
        json!({"schema_version":1,"decisions":[{"schema_version":1,"issuer":s.issuer,"subject":OWNER,"role":"user","applications":apps,"revision":1,"enforced":true,"observed_at":now,"expires_at":now+chrono::Duration::seconds(60)}]}),
    )
}
async fn space_decision(
    State(s): State<Owners>,
    headers: HeaderMap,
    Json(query): Json<Value>,
) -> Json<Value> {
    assert_eq!(
        headers.get("authorization").unwrap(),
        "Bearer isolated-space-reader-fixture"
    );
    assert_eq!(
        query["identity"],
        json!({"issuer":s.issuer,"subject":OWNER})
    );
    let open=s.db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT count(*) AS n FROM pg_stat_activity WHERE datname=current_database() AND state='idle in transaction'".to_owned())).await.unwrap().unwrap().try_get::<i64>("","n").unwrap();
    assert_eq!(
        open, 0,
        "space owner IO must not hold a product transaction"
    );
    let now = chrono::Utc::now().timestamp();
    let spaces = query["spaces"].as_array().unwrap();
    let selected = if s.space_access.load(Ordering::SeqCst) {
        json!(s.allowed_spaces)
    } else {
        json!([])
    };
    Json(
        json!({"schema_version":1,"identity":query["identity"],"identity_active":true,"policy_revision":s.space_revision.load(Ordering::SeqCst),"policy":{"mode":"selected","spaces":selected},"validity":{"observed_at":now,"valid_until":now+2},"spaces":spaces.iter().map(|space|json!({"space":space,"revision":1,"state":"active"})).collect::<Vec<_>>()}),
    )
}
async fn fleet(
    State(s): State<Owners>,
    axum::extract::Path(agent): axum::extract::Path<String>,
) -> (StatusCode, Json<Value>) {
    s.fleet_reads.fetch_add(1, Ordering::SeqCst);
    if !s.available.load(Ordering::SeqCst) {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(Value::Null));
    }
    let (role, r) = s
        .routes
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, r)| r["agent_id"] == agent)
        .unwrap();
    let key = if role == "developer" {
        "initial"
    } else if role == "project_manager" {
        "draft"
    } else if role == "analyst" {
        "analysis"
    } else if role == "architect" {
        "decomposition"
    } else {
        "delivery"
    };
    let modes = if role == "developer" {
        vec![key, "rework"]
    } else {
        vec![key]
    };
    let scope = if matches!(role.as_str(), "project_manager" | "analyst" | "architect") {
        "business"
    } else {
        "delivery"
    };
    let modes:Vec<Value>=modes.into_iter().enumerate().map(|(i,k)|json!({"mode_id":(i+1).to_string(),"key":k,"execution_scopes":[scope],"phase_ids":[(i*3+1).to_string(),(i*3+2).to_string(),(i*3+3).to_string()]})).collect();
    (
        StatusCode::OK,
        Json(
            json!({"contract_version":2,"observation_ref":Uuid::new_v4(),"agent_id":agent,"sdlc_role":role,
      "effective_revision":r["fleet_config_revision"],"managed_files_verified":true,"observed_at":chrono::Utc::now(),
      "workflow_binding":{"schema":"single-task-sdlc/workflow-binding/v1","instruction_profile":"single-task-v1","instruction_manifest_sha256":r["instruction_manifest_sha256"],
        "workflow_catalog_version":1,"workflow_catalog_sha256":r["workflow_catalog_sha256"],"namespace_id":r["namespace_id"],"namespace_name":r["namespace_name"],
        "namespace_cli_command":format!("workflow-single-task-{}",role.replace('_',"-")),"workflow_id":r["workflow_id"],"workflow_key":r["workflow_key"],"role_key":role,
        "profile":r["profile"],"role_instructions_sha256":"a".repeat(64),"modes":modes,"runtime_ready":false,"live_activation_allowed":false}}),
        ),
    )
}
struct Source(VerifiedSource);
#[async_trait::async_trait]
impl domain::task_repositories::TaskRepositoriesReader for Source {
    async fn verify(
        &self,
        _: &shared::resource_context::NamespaceRef,
        _: &domain::task_repositories::RepositoryRef,
    ) -> Result<Value, shared::AppError> {
        Err(shared::AppError::Forbidden)
    }
    async fn verify_source(
        &self,
        namespace: &shared::resource_context::NamespaceRef,
        source: &SourcePolicySpec,
    ) -> Result<VerifiedSource, shared::AppError> {
        assert_eq!(*namespace, self.0.namespace);
        assert_eq!(source.repository_id, self.0.repository_id);
        let mut v = self.0.clone();
        v.observed_at = chrono::Utc::now();
        Ok(v)
    }
}
async fn call(
    router: &Router,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = bearer {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(if body.is_null() {
                    Body::empty()
                } else {
                    Body::from(body.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
#[ignore = "owned fresh *_task_lifecycle_test PostgreSQL required"]
async fn handoff_http_requires_current_human_access_and_replays_without_reobserving_peers() {
    let url = std::env::var("TT_TASK_LIFECYCLE_TEST_DATABASE_URL").unwrap();
    let db = Arc::new(Database::connect(url.as_str()).await.unwrap());
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name,to_regclass('public.issues') IS NULL AS fresh"
                .to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_task_lifecycle_test")
    );
    assert!(row.try_get::<bool>("", "fresh").unwrap());
    migration::Migrator::up(db.as_ref(), None).await.unwrap();
    db.execute_unprepared(include_str!(
        "../../migration/src/task_lifecycle_fixture.sql"
    ))
    .await
    .unwrap();
    db.execute_unprepared("SELECT task_lifecycle_activate('10000000-0000-4000-8000-000000000001',task_lifecycle_inventory_snapshot())").await.unwrap();
    let project = Uuid::parse_str("20000000-0000-4000-8000-000000000001").unwrap();
    let repository = infra::sdlc::PostgresSdlcRepository::connect(
        &url,
        SdlcConfig {
            instance_id: "40000000-0000-4000-8000-000000000001".into(),
            single_task_project_ids: [project].into_iter().collect(),
            reservation_scheduler_subject: "scheduler-qa".into(),
            orchestrator_subject: "orchestrator-qa".into(),
            verifier_subject: "verifier-qa".into(),
        },
    )
    .await
    .unwrap();
    let actor = Principal {
        subject: "10000000-0000-4000-8000-000000000001".into(),
        trusted_human: true,
        human_session: true,
        scopes: Default::default(),
    };
    let mut routes = json!({});
    for (index, role) in [
        "project_manager",
        "analyst",
        "architect",
        "developer",
        "reviewer",
        "tester",
        "devops",
    ]
    .iter()
    .enumerate()
    {
        let suffix = role.replace('_', "-");
        routes[*role] = json!({"agent_id":Uuid::new_v4(),"fleet_config_revision":1,"instruction_profile":"single-task-v1",
            "instruction_manifest_sha256":"b".repeat(64),"namespace_id":(index+1).to_string(),"namespace_name":format!("single-task-{suffix}"),
            "workflow_id":(index+11).to_string(),"workflow_key":format!("single-task-v1:{role}"),"profile":format!("single-task-v1-{suffix}"),
            "workflow_catalog_version":1,"workflow_catalog_sha256":"c".repeat(64)});
    }
    repository
        .set_routing_policy(
            project,
            &actor,
            domain::sdlc_routing::SetRoutingPolicy {
                expected_version: None,
                routes: serde_json::from_value(routes).unwrap(),
                idempotency_key: "fixture-routing".into(),
            },
        )
        .await
        .unwrap();
    let namespace = shared::resource_context::NamespaceRef {
        registry_instance_id: Uuid::new_v4(),
        namespace_id: Uuid::new_v4(),
    };
    let project_a = Uuid::new_v4();
    let project_b = Uuid::new_v4();
    let binding = json!({"schema_version":1,"namespace":namespace,"resource":{"kind":"tracker_project","instance_id":Uuid::new_v4(),"resource_id":project},
        "operation_id":Uuid::new_v4(),"generation":1,"state":"active"});
    db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO tracker_namespace_bindings(resource_id,registry_instance_id,namespace_id,generation,state,command) VALUES($1,$2,$3,1,'active',$4)",
        [project.into(),namespace.registry_instance_id.into(),namespace.namespace_id.into(),binding.into()])).await.unwrap();
    let source = SourcePolicySpec {
        forge_instance_id: Uuid::new_v4(),
        repository_id: Uuid::new_v4(),
        base_ref: "refs/heads/main".into(),
        base_commit: "d".repeat(40),
    };
    let verified = VerifiedSource {
        namespace: namespace.clone(),
        forge_instance_id: source.forge_instance_id,
        repository_id: source.repository_id,
        public_name: "fixture/source".into(),
        base_ref: source.base_ref.clone(),
        base_commit: source.base_commit.clone(),
        default_ref: source.base_ref.clone(),
        protected_refs: vec![],
        observed_at: chrono::Utc::now(),
    };
    repository
        .set_source_policy(
            project,
            &actor,
            SetSourcePolicy {
                expected_source_revision: None,
                source,
                idempotency_key: "fixture-source".into(),
            },
            verified,
        )
        .await
        .unwrap();

    let policy = repository
        .prepare_current_stage_handoff(TASK.parse().unwrap(), &actor)
        .await
        .unwrap();
    let sources = Source(policy.source.as_ref().unwrap().verification.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let available = Arc::new(AtomicBool::new(true));
    let reads = Arc::new(AtomicUsize::new(0));
    db.execute_unprepared("INSERT INTO users(id,email,username,display_name,password_hash,central_sub) VALUES('10000000-0000-4000-8000-000000000003','scheduler@example.test','scheduler','Fixture scheduler','fixture','scheduler-qa'); INSERT INTO project_members(project_id,user_id,role) VALUES('20000000-0000-4000-8000-000000000001','10000000-0000-4000-8000-000000000003','member')").await.unwrap();
    let owner_apps = Arc::new(AtomicBool::new(true));
    let owner_reads = Arc::new(AtomicUsize::new(0));
    let space_access = Arc::new(AtomicBool::new(false));
    let space_revision = Arc::new(AtomicUsize::new(1));
    let change_during_lookup = Arc::new(AtomicBool::new(false));
    let owners = Owners {
        issuer: issuer.clone(),
        routes: serde_json::to_value(&policy.routing.as_ref().unwrap().routes).unwrap(),
        available: available.clone(),
        fleet_reads: reads.clone(),
        owner_apps: owner_apps.clone(),
        space_access: space_access.clone(),
        space_revision: space_revision.clone(),
        allowed_spaces: vec![
            shared::space_access::SpaceRef {
                registry_instance_id: namespace.registry_instance_id,
                space_id: namespace.namespace_id,
            },
            shared::space_access::SpaceRef {
                registry_instance_id: namespace.registry_instance_id,
                space_id: project_a,
            },
        ],
        owner_reads: owner_reads.clone(),
        change_during_lookup: change_during_lookup.clone(),
        db: db.clone(),
    };
    let stub = Router::new()
        .route("/jwks", get(|| async { Json(json!({"keys":[]})) }))
        .route("/auth/tokens/introspect", get(introspect))
        .route("/auth/access/decisions", post(owner_decisions))
        .route("/spaces/admission", post(space_decision))
        .route(
            "/internal/runtime/v1/agents/{agent}/configuration",
            get(fleet),
        )
        .with_state(owners);
    let stub_task = tokio::spawn(async move { axum::serve(listener, stub).await.unwrap() });
    // One external test binary owns this process/environment and bridge OnceCell.
    unsafe {
        std::env::set_var("TT_AUTH__CENTRAL_JWKS_URI", format!("{issuer}/jwks"));
        std::env::set_var("TT_AUTH__CENTRAL_ISSUER", &issuer);
        std::env::set_var("TT_AUTH__CENTRAL_APPLICATION_ACCESS_REQUIRED", "true");
        std::env::set_var(
            "TT_SPACES__ADMISSION_URL",
            format!("{issuer}/spaces/admission"),
        );
        std::env::set_var(
            "TT_SPACES__ADMISSION_TOKEN",
            "isolated-space-reader-fixture",
        );
        std::env::set_var(
            "TASKTRACKER_SDLC__RESERVATION_SCHEDULER_SUBJECT",
            "scheduler-qa",
        );
        std::env::set_var(
            "TT_AUTH__CENTRAL_APPLICATION_ACCESS_READER_TOKEN",
            "isolated-owner-reader-fixture",
        );
    }
    let config = Arc::new(shared::AppConfig {
        database: shared::DatabaseConfig {
            url: url.clone(),
            max_connections: 4,
            min_connections: 0,
            ..Default::default()
        },
        server: shared::ServerConfig::default(),
        auth: shared::AuthConfig {
            jwt_secret: JWT.into(),
            ..Default::default()
        },
        storage: shared::StorageConfig::default(),
        email: shared::EmailConfig::default(),
        metrics: shared::MetricsConfig::default(),
    });
    let repos = Arc::new(
        infra::build_repositories(config.database.clone())
            .await
            .unwrap(),
    );

    let mut context =
        app::AppContext::new(config, repos, Arc::new(domain::InMemoryStorage::default()));
    let empty = Arc::new(context.clone());
    let empty_router = api::router(empty.clone()).with_state(empty);
    let path = format!("/api/v1/issues/{TASK}/handoff");
    let (status, missing) = call(&empty_router, "GET", &path, Some(FULL), Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{missing}");
    assert!(
        missing["blockers"]
            .as_array()
            .unwrap()
            .contains(&json!("execution_not_configured"))
    );
    let repository = Arc::new(repository);
    context.current_stage_handoff = Some(repository.clone());
    context.sdlc = Some(app::sdlc::SdlcService {
        repository: repository.clone(),
        configured_workflow: None,
        fleet_configuration: Some(
            app::sdlc_configuration::FleetConfigurationReader::new(&issuer, FULL).unwrap(),
        ),
    });
    context.task_repositories = Arc::new(sources);
    let context = Arc::new(context);
    let router = api::router(context.clone()).with_state(context);
    available.store(false, Ordering::SeqCst);
    let (status, unavailable) = call(&router, "GET", &path, Some(FULL), Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{unavailable}");
    assert_eq!(unavailable["blockers"], json!(["fleet_unavailable"]));
    available.store(true, Ordering::SeqCst);
    let (status, preview) = call(&router, "GET", &path, Some(FULL), Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["blockers"], json!([]));
    assert_eq!(preview["state"]["stage"], "Review");
    let command = json!({"expected_revision":preview["state"]["revision"],"expected_routing_policy_version":preview["routing_policy_version"],"confirmed_issue_sha256":preview["issue_sha256"],"idempotency_key":"original-http-request","requirements":{
        "goal":"Real human material","scope":["Scope"],"exclusions":[],"scenarios":["Scenario"],"acceptance_criteria":["Acceptance"],"constraints":[],"dependencies":[],"assumptions":[],"checklist":["Check"],"prerequisites":["Source"]}});
    assert_eq!(
        call(&router, "GET", &path, None, Value::Null).await.0,
        StatusCode::UNAUTHORIZED
    );
    let local = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &app::auth::UserClaims {
            sub: OWNER.into(),
            exp: (shared::now().timestamp() + 60) as usize,
            typ: Some("access".into()),
            jti: None,
        },
        &jsonwebtoken::EncodingKey::from_secret(JWT.as_bytes()),
    )
    .unwrap();
    assert_eq!(
        call(&router, "GET", &path, Some(&local), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&router, "POST", &path, Some(MACHINE), command.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &path,
            Some("pdlc_pat_fixture_full_readonly_AAAAAAAAAAAAAAAA"),
            command.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let before_validation = reads.load(Ordering::SeqCst);
    let mut stale = command.clone();
    stale["expected_revision"] = json!(999);
    assert_eq!(
        call(&router, "POST", &path, Some(FULL), stale).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(reads.load(Ordering::SeqCst), before_validation);
    let (status, denied) = call(&router, "POST", &path, Some(TRACKER), command.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    let mut injected = command.clone();
    injected["stage"] = json!("Deployment");
    assert_eq!(
        call(&router, "POST", &path, Some(FULL), injected).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, accepted) = call(&router, "POST", &path, Some(FULL), command.clone()).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{accepted}");
    assert_eq!(accepted["receipt"]["entry"]["state"]["stage"], "Review");
    assert_eq!(accepted["receipt"]["actor_subject"], OWNER);
    assert_eq!(accepted["execution_requested"], true);
    assert!(accepted["cancellation"].is_null());
    let no_peer_reads = reads.load(Ordering::SeqCst);
    let (status, owned) = call(&router, "GET", &path, Some(TRACKER), Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{owned}");
    assert_eq!(
        owned["current_request"]["operation"]["receipt"]["handoff_id"],
        accepted["receipt"]["handoff_id"]
    );
    assert_eq!(owned["current_request"]["can_cancel"], true);
    let (_, readonly) = call(
        &router,
        "GET",
        &path,
        Some("pdlc_pat_fixture_tracker_readonly_AAAAAAAAAAAAA"),
        Value::Null,
    )
    .await;
    assert_eq!(readonly["current_request"]["can_cancel"], false);
    let (_, pending_view) = call(&router, "GET", &path, Some(FULL), Value::Null).await;
    assert_eq!(pending_view["blockers"], json!(["execution_busy"]));
    assert_eq!(reads.load(Ordering::SeqCst), no_peer_reads);
    let discovery = "/api/v1/sdlc/current-stage-candidates?project_id=20000000-0000-4000-8000-000000000001&limit=1";
    assert_eq!(
        call(&router, "GET", discovery, Some(FULL), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, page) = call(&router, "GET", discovery, Some(SCHEDULER), Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    let execution = format!("{path}/execution");
    let foreign_reader = "pdlc_pat_fixture_machine_readonly_AAAAAAAAAAAAA";
    assert_eq!(
        call(&router, "GET", discovery, Some(foreign_reader), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &router,
            "GET",
            &execution,
            Some(foreign_reader),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );

    let (status, pending) = call(&router, "GET", &execution, Some(SCHEDULER), Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{pending}");
    assert!(pending["assignment"].is_null() && pending["lease"].is_null());
    assert_eq!(
        pending["consent"]["handoff_id"],
        accepted["receipt"]["handoff_id"]
    );
    assert_eq!(owner_reads.load(Ordering::SeqCst), 1);
    change_during_lookup.store(true, Ordering::SeqCst);
    assert_eq!(
        call(&router, "GET", &execution, Some(SCHEDULER), Value::Null)
            .await
            .0,
        StatusCode::CONFLICT
    );
    db.execute_unprepared("UPDATE issues SET description='Unicode: кириллица' WHERE key='CORE-4'")
        .await
        .unwrap();

    owner_apps.store(false, Ordering::SeqCst);
    assert_eq!(
        call(&router, "GET", &execution, Some(SCHEDULER), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    owner_apps.store(true, Ordering::SeqCst);
    let before = reads.load(Ordering::SeqCst);
    available.store(false, Ordering::SeqCst);
    let (status, replayed) = call(&router, "POST", &path, Some(FULL), command.clone()).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed, accepted);
    assert_eq!(reads.load(Ordering::SeqCst), before);
    let mut changed = command.clone();
    changed["requirements"]["goal"] = json!("Different material");
    assert_eq!(
        call(&router, "POST", &path, Some(FULL), changed).await.0,
        StatusCode::CONFLICT
    );
    let operation = format!("{path}/operations/original-http-request");
    assert_eq!(
        call(&router, "GET", &operation, Some(TRACKER), Value::Null)
            .await
            .1,
        accepted
    );
    let cancel_path = format!(
        "{path}/{}/cancel",
        accepted["receipt"]["handoff_id"].as_str().unwrap()
    );
    let cancel = json!({"expected_revision":accepted["receipt"]["resulting_revision"],"idempotency_key":"cancel-original-http"});
    assert_eq!(
        call(&router, "POST", &cancel_path, Some(MACHINE), cancel.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, cancelled) =
        call(&router, "POST", &cancel_path, Some(TRACKER), cancel.clone()).await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_eq!(
        call(&router, "POST", &cancel_path, Some(TRACKER), cancel)
            .await
            .1,
        cancelled
    );
    assert!(
        call(&router, "GET", &path, Some(TRACKER), Value::Null)
            .await
            .1["current_request"]
            .is_null()
    );
    let (status, history) = call(&router, "GET", &operation, Some(TRACKER), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(history["receipt"], accepted["receipt"]);
    assert_eq!(history["cancellation"], cancelled);
    assert_eq!(
        call(&router, "POST", &path, Some(FULL), command.clone())
            .await
            .1,
        history
    );
    assert_eq!(reads.load(Ordering::SeqCst), before);
    let counts=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT (SELECT count(*) FROM task_handoff_materials) AS requests,(SELECT count(*) FROM sdlc_pm_executions) AS pm_runs,(SELECT count(*) FROM sdlc_delivery_assignments) AS assignments".to_owned())).await.unwrap().unwrap();
    assert_eq!(counts.try_get::<i64>("", "requests").unwrap(), 1);
    assert_eq!(counts.try_get::<i64>("", "pm_runs").unwrap(), 0);
    assert_eq!(counts.try_get::<i64>("", "assignments").unwrap(), 0);
    // A foreign active catalog entry cannot stand in for the exact current
    // human source namespace. This read never installs a writable task scope.
    available.store(true, Ordering::SeqCst);
    let (_, fresh) = call(&router, "GET", &path, Some(FULL), Value::Null).await;
    let mut next = command.clone();
    next["expected_revision"] = fresh["state"]["revision"].clone();
    next["confirmed_issue_sha256"] = fresh["issue_sha256"].clone();
    next["idempotency_key"] = json!("catalog-qualified-request");
    let (status, queued_again) = call(&router, "POST", &path, Some(FULL), next).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{queued_again}");
    let producer = Uuid::new_v4();
    db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub) VALUES($1,'producer@example.test','producer','Fixture producer','fixture',$2)",vec![producer.into(),producer.to_string().into()])).await.unwrap();
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
        vec![project.into(), producer.into()],
    ))
    .await
    .unwrap();
    // Both real human consents precede catalog/admission activation. Admission
    // itself must recheck the now-active policy instead of borrowing that consent.
    available.store(true, Ordering::SeqCst);
    let second_path = "/api/v1/issues/50000000-0000-4000-8000-000000000002/handoff";
    let (status, preview) = call(&router, "GET", second_path, Some(FULL), Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["blockers"], json!([]));
    let mut second_command = command.clone();
    second_command["expected_revision"] = preview["state"]["revision"].clone();
    second_command["confirmed_issue_sha256"] = preview["issue_sha256"].clone();
    second_command["idempotency_key"] = json!("second-human-request");
    let (status, second_queue) =
        call(&router, "POST", second_path, Some(FULL), second_command).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{second_queue}");
    let second_reservations = format!("{second_path}/reservations");
    let second_reservation = json!({"handoff_id":second_queue["receipt"]["handoff_id"],"expected_revision":second_queue["receipt"]["resulting_revision"],"machine_subject":producer,"idempotency_key":"second-technical-admission"});
    db.execute_unprepared("INSERT INTO tracker_space_catalog(registry_instance_id,space_id,admin_revision,state,metadata) VALUES('90000000-0000-4000-8000-000000000001','90000000-0000-4000-8000-000000000002',1,'active','{}')").await.unwrap();
    assert_eq!(
        call(&router, "GET", &execution, Some(SCHEDULER), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO tracker_space_catalog(registry_instance_id,space_id,admin_revision,state,metadata) VALUES($1,$2,1,'active','{}')",[namespace.registry_instance_id.into(),namespace.namespace_id.into()])).await.unwrap();
    let (status, exact) = call(&router, "GET", &execution, Some(SCHEDULER), Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{exact}");
    assert_eq!(exact["source_binding"]["namespace"], json!(namespace));
    db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"UPDATE tracker_space_catalog SET state='provisioning' WHERE registry_instance_id=$1 AND space_id=$2",[namespace.registry_instance_id.into(),namespace.namespace_id.into()])).await.unwrap();
    assert_eq!(
        call(&router, "GET", &execution, Some(SCHEDULER), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"UPDATE tracker_space_catalog SET state='active' WHERE registry_instance_id=$1 AND space_id=$2",[namespace.registry_instance_id.into(),namespace.namespace_id.into()])).await.unwrap();
    let reservations = format!("{path}/reservations");
    let reservation = json!({"handoff_id":queued_again["receipt"]["handoff_id"],"expected_revision":queued_again["receipt"]["resulting_revision"],"machine_subject":producer,"idempotency_key":"first-technical-admission"});
    for token in [FULL, TRACKER, MACHINE, SCHEDULER] {
        assert_eq!(
            call(
                &router,
                "POST",
                &reservations,
                Some(token),
                reservation.clone()
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    let (status, reserved) = call(
        &router,
        "POST",
        &reservations,
        Some(SCHEDULER_WRITE),
        reservation.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{reserved}");
    assert!(reserved["assignment"].is_object() && reserved["lease"].is_object());
    assert!(reserved["binding"].is_null() && reserved["result"].is_null());
    assert_eq!(reserved["task"]["stage"], "Review");
    assert_eq!(reserved["task"]["runtime"], "reserved");
    let frozen=db.query_one(Statement::from_sql_and_values(DbBackend::Postgres,"SELECT origin,spaces,original_actor FROM tracker_execution_space_grants WHERE assignment_id=$1",vec![Uuid::parse_str(reserved["assignment"]["assignment_id"].as_str().unwrap()).unwrap().into()])).await.unwrap().expect("real technical assignment must freeze its archive dependencies");
    assert_eq!(frozen.try_get::<String>("", "origin").unwrap(), "delivery");
    assert_eq!(
        frozen.try_get::<Value>("", "spaces").unwrap(),
        json!([{"registry_instance_id":namespace.registry_instance_id,"space_id":namespace.namespace_id}])
    );
    assert_eq!(
        frozen.try_get::<Value>("", "original_actor").unwrap(),
        json!({"issuer":issuer,"subject":OWNER})
    );

    let original_dependency_expiry = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT initial_valid_until FROM tracker_execution_space_grants WHERE assignment_id=$1",
            vec![
                Uuid::parse_str(reserved["assignment"]["assignment_id"].as_str().unwrap())
                    .unwrap()
                    .into(),
            ],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "initial_valid_until")
        .unwrap();
    for statement in [
        "UPDATE tracker_execution_space_grants SET original_actor='{}' WHERE assignment_id=$1",
        "DELETE FROM tracker_execution_space_grants WHERE assignment_id=$1",
    ] {
        let error = db
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                statement,
                vec![
                    Uuid::parse_str(reserved["assignment"]["assignment_id"].as_str().unwrap())
                        .unwrap()
                        .into(),
                ],
            ))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("dependencies are immutable"));
    }
    let operation = format!("{reservations}/first-technical-admission");
    let (status, recovered) = call(&router, "GET", &operation, Some(SCHEDULER), Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{recovered}");
    assert_eq!(recovered, reserved);
    let (status, current) = call(
        &router,
        "GET",
        &format!("/api/v1/issues/{TASK}/sdlc/lifecycle"),
        Some(SCHEDULER),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{current}");
    assert_eq!(current["runtime_state"], "reserved");
    let (status, source) = call(
        &router,
        "GET",
        &format!("/api/v1/issues/{TASK}/sdlc/source-binding"),
        Some(SCHEDULER),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{source}");
    assert_eq!(source["binding"]["namespace"], json!(namespace));
    assert_eq!(source["reason"], "execution_not_running");
    let before_native_replay = reads.load(Ordering::SeqCst);
    available.store(false, Ordering::SeqCst);
    let (status, repeated) = call(
        &router,
        "POST",
        &reservations,
        Some(SCHEDULER_WRITE),
        reservation.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{repeated}");
    assert_eq!(repeated, reserved);
    assert_eq!(reads.load(Ordering::SeqCst), before_native_replay);
    let mut conflict = reservation.clone();
    conflict["machine_subject"] = json!(Uuid::new_v4());
    assert_eq!(
        call(
            &router,
            "POST",
            &reservations,
            Some(SCHEDULER_WRITE),
            conflict
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(&router, "GET", &operation, Some(FULL), Value::Null)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &router,
            "GET",
            &format!("{reservations}/absent-original-key"),
            Some(SCHEDULER),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    // The original human space grant is separate from a valid native credential
    // and from current application access. Recheck it for native reads/recovery.
    for space in [project_a, project_b] {
        db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO tracker_space_catalog(registry_instance_id,space_id,admin_revision,state,metadata) VALUES($1,$2,1,'active','{}')",vec![namespace.registry_instance_id.into(),space.into()])).await.unwrap();
        db.execute(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id) VALUES($1,$2,$3)",vec![project.into(),namespace.registry_instance_id.into(),space.into()])).await.unwrap();
    }
    db.execute_unprepared("UPDATE tracker_space_admission_state SET active=true WHERE singleton")
        .await
        .unwrap();
    let protected = [
        format!("/api/v1/issues/{TASK}/sdlc/lifecycle"),
        format!("/api/v1/issues/{TASK}/sdlc/source-binding"),
        operation.clone(),
    ];
    for endpoint in &protected {
        assert_eq!(
            call(&router, "GET", endpoint, Some(SCHEDULER), Value::Null)
                .await
                .0,
            StatusCode::FORBIDDEN,
            "revoked owner data access must restrict {endpoint}"
        );
    }
    assert_eq!(
        call(
            &router,
            "POST",
            &second_reservations,
            Some(SCHEDULER_WRITE),
            second_reservation.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN,
        "valid scheduler cannot start for a revoked owner"
    );
    let second_execution_read = format!("{second_path}/execution");
    assert_eq!(
        call(
            &router,
            "GET",
            &second_execution_read,
            Some(SCHEDULER),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN,
        "unclaimed materials are also private owner data"
    );
    let absent_original = format!("{reservations}/absent-original-key");
    assert_eq!(
        call(
            &router,
            "GET",
            &absent_original,
            Some(SCHEDULER),
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN,
        "missing operations cannot bypass the owner data boundary"
    );
    // This explicitly ignored integration fixture uses a 2s original owner TTL;
    // no production timeout/assertion is shortened to make an automatic test pass.
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    space_revision.store(2, Ordering::SeqCst);
    space_access.store(true, Ordering::SeqCst);
    let probe_actor = app::space_access::CENTRAL_BRIDGE
        .try_verified_token(TRACKER)
        .await
        .unwrap();
    let probe_spaces = [
        shared::space_access::SpaceRef {
            registry_instance_id: namespace.registry_instance_id,
            space_id: project_a,
        },
        shared::space_access::SpaceRef {
            registry_instance_id: namespace.registry_instance_id,
            space_id: project_b,
        },
        shared::space_access::SpaceRef {
            registry_instance_id: namespace.registry_instance_id,
            space_id: namespace.namespace_id,
        },
    ];
    let probe = app::space_access::admission_client()
        .await
        .unwrap()
        .confirm_many_read(
            &app::space_access::CENTRAL_BRIDGE,
            &probe_actor,
            &probe_spaces,
        )
        .await
        .unwrap();
    let partial = probe.monotonic_deadline() - std::time::Duration::from_millis(900);
    tokio::time::sleep_until(partial.into()).await;
    assert!(
        probe.is_current(),
        "the original local decision is still current"
    );
    assert!(
        probe.bounded_admission().is_err(),
        "a partial second cannot be exported as new whole-second wire metadata"
    );
    for endpoint in &protected {
        let (status, body) = call(&router, "GET", endpoint, Some(SCHEDULER), Value::Null).await;
        assert_eq!(status, StatusCode::OK, "{endpoint}: {body}");
    }
    tokio::time::sleep_until(
        (probe.monotonic_deadline() + std::time::Duration::from_millis(25)).into(),
    )
    .await;
    let (status, pending_context) = call(
        &router,
        "GET",
        &second_execution_read,
        Some(SCHEDULER),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{pending_context}");
    assert!(pending_context["assignment"].is_null());
    assert_eq!(
        call(
            &router,
            "GET",
            &absent_original,
            Some(SCHEDULER),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    // A genuine human Tracker PAT keeps its own read context. Native owner's
    // machine restriction must not replace this independent human authorization.
    use domain::sdlc_lifecycle::LifecycleRepository;
    unsafe {
        std::env::set_var(
            "TT_NAMESPACE__INSTANCE_ID",
            "40000000-0000-4000-8000-000000000001",
        );
    }
    let verified_human = app::space_access::CENTRAL_BRIDGE
        .try_verified_token(TRACKER)
        .await
        .unwrap();
    assert_eq!(verified_human.subject_role.as_deref(), Some("user"));
    let human = Principal {
        subject: OWNER.into(),
        trusted_human: true,
        human_session: false,
        scopes: ["task-tracker:read".into()].into(),
    };
    let user = shared::UserId::from_uuid(OWNER.parse().unwrap());
    let human_spaces = [
        shared::space_access::SpaceRef {
            registry_instance_id: namespace.registry_instance_id,
            space_id: project_a,
        },
        shared::space_access::SpaceRef {
            registry_instance_id: namespace.registry_instance_id,
            space_id: namespace.namespace_id,
        },
    ];
    let proof = app::space_access::admission_client()
        .await
        .unwrap()
        .confirm_many_read(
            &app::space_access::CENTRAL_BRIDGE,
            &verified_human,
            &human_spaces,
        )
        .await
        .unwrap();
    let spaces =
        app::space_access::SpaceActor::from_read_proof(user, &verified_human, &proof).unwrap();
    let frame = Arc::new(
        app::space_access::CentralRequestContext::verified(
            user,
            &verified_human,
            Some(spaces),
            app::space_access::RequestOperation::Read,
        )
        .unwrap(),
    );
    let current = app::space_access::with_request_context(
        frame.clone(),
        repository.lifecycle_read(TASK.parse().unwrap(), &human),
    )
    .await
    .expect("a human Tracker reader can inspect project runtime without a native credential");
    assert_eq!(
        current.runtime_state,
        domain::sdlc_lifecycle::RuntimeState::Reserved
    );
    let source = app::space_access::with_request_context(
        frame,
        repository.task_source(TASK.parse().unwrap(), &human),
    )
    .await
    .expect("the human source reader has explicit source C confirmation");
    assert!(matches!(
        source,
        domain::sdlc_source::TaskSourceReadback::Blocked {
            binding: Some(_),
            ..
        }
    ));
    let narrow = app::space_access::admission_client()
        .await
        .unwrap()
        .confirm_many_read(
            &app::space_access::CENTRAL_BRIDGE,
            &verified_human,
            &human_spaces[..1],
        )
        .await
        .unwrap();
    let narrow =
        app::space_access::SpaceActor::from_read_proof(user, &verified_human, &narrow).unwrap();
    let frame = Arc::new(
        app::space_access::CentralRequestContext::verified(
            user,
            &verified_human,
            Some(narrow),
            app::space_access::RequestOperation::Read,
        )
        .unwrap(),
    );
    assert!(
        app::space_access::with_request_context(
            frame.clone(),
            repository.lifecycle_read(TASK.parse().unwrap(), &human)
        )
        .await
        .is_ok(),
        "project runtime remains readable through A alone"
    );
    assert!(
        app::space_access::with_request_context(
            frame,
            repository.task_source(TASK.parse().unwrap(), &human)
        )
        .await
        .is_err(),
        "project A visibility must not borrow source C metadata missing from this confirmed context"
    );
    available.store(true, Ordering::SeqCst);
    // The caller authority below is synthetic; actual public owner commands
    // and real technical assignments/holds exercise the namespace drain query.
    use shared::{
        space_access::{AdmissionValidity, CentralIdentity, SpaceRef},
        space_lifecycle::{
            DesiredSpaceState, SpaceMetadata, SpaceOwnerAuthority, SpaceOwnerCommandV2,
        },
    };
    let owner_instance = Uuid::parse_str("40000000-0000-4000-8000-000000000001").unwrap();
    unsafe {
        std::env::set_var(
            "TT_NAMESPACE__REGISTRY_INSTANCE_ID",
            namespace.registry_instance_id.to_string(),
        );
        std::env::set_var("TT_NAMESPACE__INSTANCE_ID", owner_instance.to_string());
    }
    let now = chrono::Utc::now().timestamp();
    let authority = SpaceOwnerAuthority::confirmed(
        CentralIdentity {
            issuer: issuer.clone(),
            subject: Uuid::new_v4().to_string(),
        },
        AdmissionValidity::confirmed(now, now + 60, i64::MAX).unwrap(),
    )
    .unwrap();
    let mut ca = SpaceOwnerCommandV2 {
        schema_version: 2,
        space: SpaceRef {
            registry_instance_id: namespace.registry_instance_id,
            space_id: project_a,
        },
        participant_instance_id: owner_instance,
        operation_id: Uuid::new_v4(),
        revision: 1,
        desired_state: DesiredSpaceState::Active,
        metadata: SpaceMetadata {
            slug: "project-alpha".into(),
            name: "Project Alpha".into(),
            description: "Owned integration fixture".into(),
            responsible_subject: OWNER.into(),
        },
    };
    let mut cb = ca.clone();
    cb.space.space_id = project_b;
    cb.operation_id = Uuid::new_v4();
    cb.metadata.slug = "project-beta".into();
    for command in [&ca, &cb] {
        assert!(
            infra::space_owner::apply(&db, command, &authority)
                .await
                .unwrap()
                .matches(command)
        );
    }
    // The SDK fixture retains its earlier Active observation; the local archive
    // barrier must win inside that bounded decision window.
    ca.operation_id = Uuid::new_v4();
    ca.revision = 2;
    ca.desired_state = DesiredSpaceState::Archived;
    assert!(
        infra::space_owner::apply(&db, &ca, &authority)
            .await
            .unwrap()
            .drained,
        "the first execution only retained C, before M:N activation"
    );
    assert_eq!(
        call(
            &router,
            "POST",
            &second_reservations,
            Some(SCHEDULER_WRITE),
            second_reservation.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN,
        "a stale Active decision cannot start through local archived A or ungranted B"
    );
    ca.operation_id = Uuid::new_v4();
    ca.revision = 3;
    ca.desired_state = DesiredSpaceState::Active;
    assert!(
        infra::space_owner::apply(&db, &ca, &authority)
            .await
            .unwrap()
            .matches(&ca)
    );
    // Delay the short original space clock only in a transaction that actually
    // inserted this task's adoption. A sequence proves the COMMIT path ran even
    // though all task/assignment/hold/operation writes must roll back.
    db.execute_unprepared("CREATE SEQUENCE injected_space_clock_seen; CREATE FUNCTION injected_space_clock_delay() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.valid_until<clock_timestamp()+interval '3 seconds' AND EXISTS(SELECT 1 FROM task_handoff_delivery_adoptions WHERE task_id='50000000-0000-4000-8000-000000000002' AND transaction_id=pg_current_xact_id()) THEN PERFORM nextval('injected_space_clock_seen');PERFORM pg_sleep(2.1); END IF; RETURN NEW; END $$;CREATE CONSTRAINT TRIGGER a_injected_space_clock_delay AFTER INSERT ON tracker_request_commit_fences DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION injected_space_clock_delay()").await.unwrap();
    let (status, expired) = call(
        &router,
        "POST",
        &second_reservations,
        Some(SCHEDULER_WRITE),
        second_reservation.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{expired}");
    let rollback=db.query_one(Statement::from_string(DbBackend::Postgres,"SELECT (SELECT is_called FROM injected_space_clock_seen) AS delayed,(SELECT count(*) FROM sdlc_delivery_assignments WHERE task_id='50000000-0000-4000-8000-000000000002') AS assignments,(SELECT count(*) FROM task_handoff_delivery_adoptions WHERE task_id='50000000-0000-4000-8000-000000000002') AS adoptions,(SELECT manual_control AND state ? 'human_handoff_id' FROM sdlc_tasks WHERE task_id='50000000-0000-4000-8000-000000000002') AS queued".to_owned())).await.unwrap().unwrap();
    assert!(rollback.try_get::<bool>("", "delayed").unwrap());
    assert!(rollback.try_get::<bool>("", "queued").unwrap());
    assert_eq!(rollback.try_get::<i64>("", "assignments").unwrap(), 0);
    assert_eq!(rollback.try_get::<i64>("", "adoptions").unwrap(), 0);
    db.execute_unprepared("DROP TRIGGER a_injected_space_clock_delay ON tracker_request_commit_fences;DROP FUNCTION injected_space_clock_delay();DROP SEQUENCE injected_space_clock_seen").await.unwrap();
    let (status, second_execution) = call(
        &router,
        "POST",
        &second_reservations,
        Some(SCHEDULER_WRITE),
        second_reservation,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{second_execution}");
    assert_eq!(second_execution["task"]["stage"], "Development");
    assert_eq!(second_execution["task"]["status"], "InProgress");
    let scope=db.query_one(Statement::from_sql_and_values(DbBackend::Postgres,"SELECT spaces,initial_valid_until FROM tracker_execution_space_grants WHERE assignment_id=$1",vec![Uuid::parse_str(second_execution["assignment"]["assignment_id"].as_str().unwrap()).unwrap().into()])).await.unwrap().unwrap();
    let scopes = scope.try_get::<Value>("", "spaces").unwrap();
    let mut expected = vec![
        json!({"registry_instance_id":namespace.registry_instance_id,"space_id":project_a}),
        json!({"registry_instance_id":namespace.registry_instance_id,"space_id":namespace.namespace_id}),
    ];
    expected.sort_by_key(|space| space["space_id"].as_str().unwrap().to_owned());
    assert_eq!(
        scopes,
        json!(expected),
        "freeze admitted project A and source C; B is not a dependency"
    );

    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    space_revision.store(3, Ordering::SeqCst);
    space_access.store(false, Ordering::SeqCst);
    for endpoint in &protected {
        assert_eq!(
            call(&router, "GET", endpoint, Some(SCHEDULER), Value::Null)
                .await
                .0,
            StatusCode::FORBIDDEN,
            "cached decision must expire for {endpoint}"
        );
    }
    assert_eq!(
        call(
            &router,
            "POST",
            &reservations,
            Some(SCHEDULER_WRITE),
            reservation
        )
        .await
        .0,
        StatusCode::FORBIDDEN,
        "original operation replay is also subject to current data admission"
    );
    assert!(
        scope.try_get::<i64>("", "initial_valid_until").unwrap() <= chrono::Utc::now().timestamp()
    );
    cb.operation_id = Uuid::new_v4();
    cb.revision = 2;
    cb.desired_state = DesiredSpaceState::Archived;
    assert!(
        infra::space_owner::apply(&db, &cb, &authority)
            .await
            .unwrap()
            .drained,
        "ungranted B must not wait for A/C work"
    );
    ca.operation_id = Uuid::new_v4();
    ca.revision = 4;
    ca.desired_state = DesiredSpaceState::Archived;
    assert!(
        !infra::space_owner::apply(&db, &ca, &authority)
            .await
            .unwrap()
            .drained,
        "expired admission and revoked access do not release A's actual hold"
    );
    let old = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT initial_valid_until FROM tracker_execution_space_grants WHERE assignment_id=$1",
            vec![
                Uuid::parse_str(reserved["assignment"]["assignment_id"].as_str().unwrap())
                    .unwrap()
                    .into(),
            ],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "initial_valid_until")
        .unwrap();
    assert_eq!(
        old, original_dependency_expiry,
        "replay never renews frozen dependency admission"
    );
    stub_task.abort();
    let _ = stub_task.await;
}

#[test]
fn handoff_openapi_operation_ids_do_not_shadow_existing_consumers() {
    use utoipa::OpenApi;
    let document = serde_json::to_value(api::ApiDoc::openapi()).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for (path, item) in document["paths"].as_object().unwrap() {
        for (method, operation) in item.as_object().unwrap() {
            if let Some(id) = operation.get("operationId").and_then(Value::as_str) {
                assert!(
                    seen.insert(id.to_owned()),
                    "duplicate operationId {id} at {method} {path}"
                );
            }
        }
    }
    for id in [
        "preview_task_handoff",
        "request_task_handoff",
        "get_task_handoff_operation",
        "cancel_task_handoff",
    ] {
        assert!(seen.contains(id));
    }
}
