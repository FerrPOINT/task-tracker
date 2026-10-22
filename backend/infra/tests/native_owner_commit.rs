//! Current native owner admission composed with the actual SQL transaction adapter.
#[path = "../src/db_error.rs"]
mod db_error;
#[allow(dead_code)]
#[path = "../src/guarded_db.rs"]
mod guarded_db;
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicUsize, Ordering},
};
use uuid::Uuid;

#[derive(Clone)]
struct Owner {
    subject: Uuid,
    issuer: String,
    applications: Vec<&'static str>,
    expiry_ms: i64,
    calls: Arc<AtomicUsize>,
}

#[tokio::test]
#[ignore = "requires isolated TT_CLOCK_TEST_DATABASE_URL"]
async fn native_owner_proof_bounds_real_commit_and_next_stage() {
    let url = std::env::var("TT_CLOCK_TEST_DATABASE_URL").unwrap();
    let raw = sea_orm::Database::connect(url.clone()).await.unwrap();
    let name = raw
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_database() AS name",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        name.try_get::<String>("", "name").unwrap(),
        "tracker_clock_test"
    );
    raw.execute_unprepared(include_str!("../../migration/src/request_commit_clock.sql"))
        .await
        .unwrap();
    raw.execute_unprepared("CREATE TABLE tracker_space_admission_state(singleton boolean PRIMARY KEY,active boolean NOT NULL); INSERT INTO tracker_space_admission_state VALUES(true,false);
      CREATE TABLE qa_native_owner_effects(id integer PRIMARY KEY); CREATE SEQUENCE qa_native_commit_seen;
      CREATE FUNCTION qa_native_commit_delay() RETURNS trigger LANGUAGE plpgsql AS $$
      BEGIN PERFORM nextval('qa_native_commit_seen'); PERFORM pg_sleep(2.1); RETURN NEW; END $$;
      CREATE CONSTRAINT TRIGGER a_qa_native_commit_delay AFTER INSERT ON tracker_request_commit_fences
      DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION qa_native_commit_delay();").await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let machine = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let state = Arc::new(RwLock::new(Owner {
        subject: owner,
        issuer: issuer.clone(),
        applications: vec!["task-tracker", "fleet-control"],
        expiry_ms: 1500,
        calls: Arc::new(AtomicUsize::new(0)),
    }));
    let router=Router::new()
        .route("/oidc/jwks",get(||async{Json(serde_json::json!({"keys":[]}))}))
        .route("/auth/tokens/introspect",get(move ||async move {
            let now=chrono::Utc::now();
            Json(serde_json::json!({"sub":machine,"email":"native-commit@example.test","subject_role":"service_account",
            "scopes":["task-tracker:read","task-tracker:write"],"observed_at":now,"expires_at":now+chrono::Duration::seconds(300)}))
        }))
        .route("/auth/access/decisions",post(|State(state):State<Arc<RwLock<Owner>>>, headers:axum::http::HeaderMap,Json(request):Json<serde_json::Value>|async move{
            assert_eq!(headers["authorization"],"Bearer native-owner-commit-reader");
            let owner=state.read().unwrap().clone();
            assert_eq!(request,serde_json::json!({"schema_version":1,"subjects":[owner.subject]}));
            owner.calls.fetch_add(1,Ordering::SeqCst);
            let now=chrono::Utc::now();
            Json(serde_json::json!({"schema_version":1,"decisions":[{
             "schema_version":1,"issuer":owner.issuer,"subject":owner.subject,"role":"user","applications":owner.applications,
             "revision":1,"enforced":true,"observed_at":now,"expires_at":now+chrono::Duration::milliseconds(owner.expiry_ms)}]}))
        })).with_state(state.clone());
    unsafe {
        std::env::set_var("TT_AUTH__CENTRAL_JWKS_URI", format!("{issuer}/oidc/jwks"));
        std::env::set_var("TT_AUTH__CENTRAL_ISSUER", &issuer);
        std::env::set_var("TT_AUTH__CENTRAL_APPLICATION_ACCESS_REQUIRED", "true");
        std::env::set_var(
            "TT_AUTH__CENTRAL_APPLICATION_ACCESS_READER_TOKEN",
            "native-owner-commit-reader",
        );
    }
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let actor = app::space_access::CENTRAL_BRIDGE
        .try_verified_token("sdlc_pat_native_commit_fixture")
        .await
        .unwrap();
    let scope = app::native_space_access::NativeRequest::verified(&actor, true, true).unwrap();
    scope
        .require_owner(owner, &["task-tracker", "fleet-control"])
        .await
        .unwrap();
    let clock = scope.deadline();
    let db = guarded_db::GuardedDb::new(sea_orm::Database::connect(url).await.unwrap());
    let error = app::native_space_access::with_request(
        scope.clone(),
        db.execute_unprepared("INSERT INTO qa_native_owner_effects VALUES(1)"),
    )
    .await
    .unwrap_err();
    assert!(matches!(db_error::map(error), shared::AppError::Forbidden));
    assert_eq!(scope.deadline(), clock);
    assert!(!scope.is_current());
    assert_eq!(
        state.read().unwrap().calls.load(Ordering::SeqCst),
        1,
        "one accepted operation does not refresh"
    );
    let seen = raw
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT is_called AS seen FROM qa_native_commit_seen",
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(
        seen.try_get::<bool>("", "seen").unwrap(),
        "expired inside actual deferred COMMIT"
    );
    raw.execute_unprepared("DROP TRIGGER a_qa_native_commit_delay ON tracker_request_commit_fences; DROP FUNCTION qa_native_commit_delay();").await.unwrap();
    state.write().unwrap().expiry_ms = 30000;
    let fresh = app::native_space_access::NativeRequest::verified(&actor, true, true).unwrap();
    fresh
        .require_owner(owner, &["task-tracker", "fleet-control"])
        .await
        .unwrap();
    app::native_space_access::with_request(
        fresh,
        db.execute_unprepared("INSERT INTO qa_native_owner_effects VALUES(2)"),
    )
    .await
    .unwrap();
    state.write().unwrap().applications = vec!["task-tracker"];
    let revoked = app::native_space_access::NativeRequest::verified(&actor, true, true).unwrap();
    assert!(matches!(
        revoked
            .require_owner(owner, &["task-tracker", "fleet-control"])
            .await,
        Err(shared::AppError::Forbidden)
    ));
    let row=raw.query_one(Statement::from_string(DbBackend::Postgres,
        "SELECT (SELECT array_agg(id ORDER BY id) FROM qa_native_owner_effects) AS effects,(SELECT count(*) FROM tracker_request_commit_fences) AS fences")).await.unwrap().unwrap();
    assert_eq!(row.try_get::<Vec<i32>>("", "effects").unwrap(), vec![2]);
    assert_eq!(row.try_get::<i64>("", "fences").unwrap(), 0);
    server.abort();
    raw.close().await.unwrap();
}
