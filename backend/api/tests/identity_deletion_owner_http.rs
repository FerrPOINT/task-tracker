//! Real TCP private owner, real PostgreSQL, controlled loopback Auth introspection.
//! One test owns this process environment before creating any async runtime.
use axum::{
    Json, Router,
    http::{HeaderMap, StatusCode},
    routing::get,
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
    TransactionTrait,
};
use serde_json::json;
use shared::identity_deletion::*;
use shared::space_access::CentralIdentity;
use std::{
    ffi::OsString,
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;
const SUBJECT: &str = "00000000-0000-4000-8000-000000000900";
const FRESH: &str = "sdlc_pat_owner-http-fresh-fixture";
const SHORT: &str = "sdlc_pat_owner-http-short-fixture";
const SLOW: &str = "sdlc_pat_owner-http-slow-fixture";
struct Environment(Vec<(&'static str, Option<OsString>)>);
impl Environment {
    fn configure(origin: &str) -> Self {
        let keys = [
            "TT_AUTH__CENTRAL_JWKS_URI",
            "TT_AUTH__CENTRAL_ISSUER",
            "TT_AUTH__CENTRAL_TIMEOUT_SECS",
            "TT_IDENTITY_DELETION_COORDINATOR_SUBJECTS",
        ];
        let old = keys.into_iter().map(|k| (k, std::env::var_os(k))).collect();
        unsafe {
            std::env::set_var(keys[0], format!("{origin}/oidc/jwks"));
            std::env::set_var(keys[1], origin);
            std::env::set_var(keys[2], "20");
            std::env::set_var(keys[3], SUBJECT);
        }
        Self(old)
    }
}
impl Drop for Environment {
    fn drop(&mut self) {
        for (k, v) in &self.0 {
            unsafe {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
    }
}
struct Abort(tokio::task::AbortHandle);
impl Drop for Abort {
    fn drop(&mut self) {
        self.0.abort();
    }
}
fn request(issuer: &str) -> DeletionRequest {
    DeletionRequest {
        schema_version: 1,
        operation_id: Uuid::new_v4(),
        identity: CentralIdentity {
            issuer: issuer.into(),
            subject: Uuid::new_v4().to_string(),
        },
        inventory_revision: Uuid::new_v4(),
    }
}
// RFC8032 section7.1 public Ed25519 TEST VECTOR, never a runtime signing key.
fn machine_jwt(issuer: &str) -> String {
    let seed: [u8; 32] = [
        0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c,
        0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae,
        0x7f, 0x60,
    ];
    let mut der = vec![
        0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04,
        0x20,
    ];
    der.extend_from_slice(&seed);
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::EdDSA);
    header.kid = Some("rfc8032-public-test-vector".into());
    jsonwebtoken::encode(&header,&json!({"sub":SUBJECT,"aud":pdlc_auth_core::FLEET_AUDIENCE,"iss":issuer,"iat":chrono::Utc::now().timestamp(),"exp":chrono::Utc::now().timestamp()+300,"role":"service_account","scopes":["task-tracker:identity-lifecycle"]}),&jsonwebtoken::EncodingKey::from_ed_der(&der)).unwrap()
}
async fn post(
    client: &reqwest::Client,
    origin: &str,
    token: Option<&str>,
    r: &DeletionRequest,
    action: &str,
) -> reqwest::Response {
    let mut q = client
        .post(format!(
            "{origin}/api/internal/v1/user-deletions/{}/{action}",
            r.operation_id
        ))
        .json(r);
    if let Some(token) = token {
        q = q.bearer_auth(token);
    }
    let response = q.send().await.unwrap();
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    response
}
async fn wait_owner(db: &DatabaseConnection, owner_pid: i32) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: bool = db
                .query_one(sql(
                    "SELECT cardinality(pg_blocking_pids($1))>0 AS waiting",
                    [owner_pid.into()],
                ))
                .await
                .unwrap()
                .unwrap()
                .try_get("", "waiting")
                .unwrap();
            if waiting {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("actual owner must be blocked behind writer");
}
fn sql(query: &str, args: impl IntoIterator<Item = sea_orm::Value>) -> Statement {
    Statement::from_sql_and_values(DatabaseBackend::Postgres, query, args)
}

async fn connect(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("connect dedicated identity fixture");
    let database: String = db
        .query_one(sql("SELECT current_database() AS name", []))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "name")
        .unwrap();
    assert_eq!(
        database, "tasktracker_identity_test",
        "refuse application database"
    );
    db
}

async fn isolated<F, Fut>(issuer: String, work: F)
where
    F: FnOnce(DatabaseConnection, String) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let url = std::env::var("TT_IDENTITY_TEST_DATABASE_URL")
        .expect("dedicated tasktracker_identity_test URL required");
    let cleanup = connect(&url).await;
    let schema = format!("identity_contract_{}", Uuid::new_v4().simple());
    cleanup
        .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    let mut schema_url = reqwest::Url::parse(&url).expect("test connection URL");
    schema_url
        .query_pairs_mut()
        .append_pair("options", &format!("-csearch_path={schema}"));
    let schema_url = schema_url.to_string();
    let work_schema = schema.clone();
    // The task owns every work connection; a panic drops it before exact cleanup.
    let result = tokio::spawn(async move {
        let db = connect(&schema_url).await;
        let selected: String = db
            .query_one(sql("SELECT current_schema() AS name", []))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "name")
            .unwrap();
        assert_eq!(
            selected, work_schema,
            "all connections must use the isolated schema"
        );
        Migrator::up(&db, None).await.unwrap();
        db.execute(sql(
            "INSERT INTO tracker_identity_legacy_issuer(issuer) VALUES($1)",
            [issuer.into()],
        ))
        .await
        .unwrap();
        work(db, schema_url).await;
    })
    .await;
    cleanup
        .execute_unprepared("SET statement_timeout='15s'")
        .await
        .unwrap();
    let removed = cleanup
        .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await;
    cleanup.close().await.unwrap();
    removed.expect("clean only this test's exact schema after success or panic");
    if let Err(error) = result {
        if error.is_panic() {
            std::panic::resume_unwind(error.into_panic());
        }
        panic!("identity contract task was cancelled");
    }
}
#[test]
#[ignore = "requires dedicated tasktracker_identity_test PostgreSQL database"]
fn private_owner_fresh_machine_boundary_expiry_timeouts_and_no_jit() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let _environment = Environment::configure(&issuer);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async{
 let introspections=Arc::new(AtomicUsize::new(0));let counted=introspections.clone();
 let central=Router::new().route("/oidc/jwks",get(||async{Json(json!({"keys":[{"kty":"OKP","crv":"Ed25519","alg":"EdDSA","use":"sig","kid":"rfc8032-public-test-vector","x":"11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo"}]}))}))
 .route("/auth/me",get(||async{Json(json!({"id":SUBJECT,"role":"service_account","status":"active","display_name":"Synthetic"}))}))
 .route("/auth/tokens/introspect",get(move|headers:HeaderMap|{let count=counted.clone();async move{
 count.fetch_add(1,Ordering::SeqCst);let token=headers.get("authorization").and_then(|h|h.to_str().ok()).unwrap_or("").strip_prefix("Bearer ").unwrap_or("");
 if token==SLOW{tokio::time::sleep(Duration::from_secs(12)).await;}
 if token=="sdlc_pat_owner-http-revoked-fixture"{return (StatusCode::UNAUTHORIZED,Json(json!({})));}
 let role=if token=="sdlc_pat_owner-http-human-fixture"{"platform_admin"}else{"service_account"};
 let subject=if token=="sdlc_pat_owner-http-unregistered-fixture"{"00000000-0000-4000-8000-000000000901"}else{SUBJECT};
 let scopes=if token=="sdlc_pat_owner-http-wrong-scope-fixture"{vec!["task-tracker:write"]}else{vec!["task-tracker:identity-lifecycle"]};
 let observed=chrono::Utc::now();let lifetime=if token==SHORT{2}else{30};
 (StatusCode::OK,Json(json!({"sub":subject,"email":"synthetic@example.test","subject_role":role,"scopes":scopes,"observed_at":observed,"expires_at":observed+chrono::Duration::seconds(lifetime)})))
 }}));
 let central_listener=tokio::net::TcpListener::from_std(listener).unwrap();let central_server=tokio::spawn(async move{axum::serve(central_listener,central).await.unwrap()});let _central_guard=Abort(central_server.abort_handle());
 let origin_issuer=issuer.clone();isolated(issuer.clone(),move|db,url|async move{
 let observer=connect(&url).await;let blocker=connect(&url).await;
 let owner_pid:i32=db.query_one(sql("SELECT pg_backend_pid() AS pid",[])).await.unwrap().unwrap().try_get("","pid").unwrap();
 let repos=Arc::new(infra::SeaOrmRepositories::new(db));
 // Build the actual AppContext with the actual PostgreSQL UserRepository; all
 // other owner routes need no content/namespace services and never JIT.
 let repository_set=domain::Repositories{users:repos.users.clone(),..Default::default()};
 let ctx=Arc::new(app::AppContext::new(Arc::new(shared::AppConfig::default()),Arc::new(repository_set),Arc::new(domain::InMemoryStorage::default())));
 let owner_listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();let owner_origin=format!("http://{}",owner_listener.local_addr().unwrap());
 let owner_router=api::router(ctx.clone()).with_state(ctx);let owner_server=tokio::spawn(async move{axum::serve(owner_listener,owner_router).await.unwrap()});let _owner_guard=Abort(owner_server.abort_handle());
 let client=reqwest::Client::builder().timeout(Duration::from_secs(15)).build().unwrap();let r=request(&origin_issuer);
 assert_eq!(post(&client,&owner_origin,None,&r,"check").await.status(),StatusCode::UNAUTHORIZED);
 for token in ["sdlc_pat_owner-http-human-fixture","sdlc_pat_owner-http-wrong-scope-fixture","sdlc_pat_owner-http-unregistered-fixture"]{assert_eq!(post(&client,&owner_origin,Some(token),&r,"check").await.status(),StatusCode::FORBIDDEN);}
 assert_eq!(post(&client,&owner_origin,Some("sdlc_pat_owner-http-revoked-fixture"),&r,"check").await.status(),StatusCode::UNAUTHORIZED);
 let jwt=machine_jwt(&origin_issuer);assert_eq!(post(&client,&owner_origin,Some(&jwt),&r,"check").await.status(),StatusCode::FORBIDDEN,"even a valid freshly confirmed service-account JWT is not this PAT capability");
 let response=post(&client,&owner_origin,Some(FRESH),&r,"freeze").await;assert_eq!(response.status(),StatusCode::OK);let receipt:OwnerFreezeReceipt=response.json().await.unwrap();assert_eq!(receipt.reference_count,0);
 assert_eq!(post(&client,&owner_origin,Some(FRESH),&r,"release").await.status(),StatusCode::OK);
 let big=client.post(format!("{owner_origin}/api/internal/v1/user-deletions/{}/check",r.operation_id)).header("content-type","application/json").bearer_auth(FRESH).body(" ".repeat(20_000)).send().await.unwrap();assert_eq!(big.status(),StatusCode::PAYLOAD_TOO_LARGE);assert_eq!(big.headers()["cache-control"],"no-store");
 // Introspection may succeed initially; the original two-second proof cannot
 // be extended while the real PostgreSQL writer prevents the owner effect.
 let tx=blocker.begin().await.unwrap();tx.execute_unprepared("SELECT pg_advisory_xact_lock_shared(714402216002::bigint)").await.unwrap();let expiry=request(&origin_issuer);let pending_client=client.clone();let pending_origin=owner_origin.clone();let pending_request=expiry.clone();let pending=tokio::spawn(async move{post(&pending_client,&pending_origin,Some(SHORT),&pending_request,"freeze").await});let _pending_guard=Abort(pending.abort_handle());wait_owner(&observer,owner_pid).await;tokio::time::sleep(Duration::from_millis(2100)).await;tx.commit().await.unwrap();assert_eq!(pending.await.unwrap().status(),StatusCode::UNAUTHORIZED);
 let n:i64=observer.query_one(sql("SELECT count(*) AS n FROM tracker_identity_deletion_operations WHERE operation_id=$1",[expiry.operation_id.into()])).await.unwrap().unwrap().try_get("","n").unwrap();assert_eq!(n,0);
 // The actual storage timeout ends an otherwise-valid owner before the outer
 // nine-second request deadline, without persisting a fence.
 let tx=blocker.begin().await.unwrap();tx.execute_unprepared("SELECT pg_advisory_xact_lock_shared(714402216002::bigint)").await.unwrap();let timeout_request=request(&origin_issuer);let started=Instant::now();let response=post(&client,&owner_origin,Some(FRESH),&timeout_request,"freeze").await;assert_eq!(response.status(),StatusCode::SERVICE_UNAVAILABLE);assert!(started.elapsed()>=Duration::from_secs(7));assert!(started.elapsed()<Duration::from_secs(11));tx.rollback().await.unwrap();
 let n:i64=observer.query_one(sql("SELECT count(*) AS n FROM tracker_identity_deletion_operations WHERE operation_id=$1",[timeout_request.operation_id.into()])).await.unwrap().unwrap().try_get("","n").unwrap();assert_eq!(n,0);
 // Slow Auth exceeds the whole-request limit, independently of SQL8s.
 let started=Instant::now();assert_eq!(post(&client,&owner_origin,Some(SLOW),&request(&origin_issuer),"check").await.status(),StatusCode::SERVICE_UNAVAILABLE);assert!(started.elapsed()>=Duration::from_secs(8));assert!(started.elapsed()<Duration::from_secs(12));
 let n:i64=observer.query_one(sql("SELECT count(*) AS n FROM users",[])).await.unwrap().unwrap().try_get("","n").unwrap();assert_eq!(n,0,"private machine calls must not materialize/JIT a local user");
 assert!(introspections.load(Ordering::SeqCst)>=8);owner_server.abort();let _=owner_server.await;drop(repos);observer.close().await.unwrap();blocker.close().await.unwrap();
 }).await;
 });
}
