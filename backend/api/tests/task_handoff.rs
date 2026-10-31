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
#[derive(Clone)]
struct Owners {
    issuer: String,
    routes: Value,
    available: Arc<AtomicBool>,
    fleet_reads: Arc<AtomicUsize>,
    owner_apps: Arc<AtomicBool>,
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
        scopes = vec!["task-tracker:read".into()]
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
        namespace:namespace.clone(),
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
    let change_during_lookup = Arc::new(AtomicBool::new(false));
    let owners = Owners {
        issuer: issuer.clone(),
        routes: serde_json::to_value(&policy.routing.as_ref().unwrap().routes).unwrap(),
        available: available.clone(),
        fleet_reads: reads.clone(),
        owner_apps: owner_apps.clone(),
        owner_reads: owner_reads.clone(),
        change_during_lookup: change_during_lookup.clone(),
        db: db.clone(),
    };
    let stub = Router::new()
        .route("/jwks", get(|| async { Json(json!({"keys":[]})) }))
        .route("/auth/tokens/introspect", get(introspect))
        .route("/auth/access/decisions", post(owner_decisions))
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
    let mut next = command;
    next["expected_revision"] = fresh["state"]["revision"].clone();
    next["confirmed_issue_sha256"] = fresh["issue_sha256"].clone();
    next["idempotency_key"] = json!("catalog-qualified-request");
    let (status, queued_again) = call(&router, "POST", &path, Some(FULL), next).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{queued_again}");
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
