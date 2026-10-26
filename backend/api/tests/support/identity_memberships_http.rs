//! Shared fixture for two separately executed integration-test processes.
//! Real router/repositories/PostgreSQL; only central Auth transport is controlled.
use axum::{
    Json, Router,
    http::{HeaderMap, StatusCode},
    routing::get,
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    ffi::OsString,
    future::Future,
    sync::{
        Arc, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

pub const HUMAN: &str = "00000000-0000-4000-8000-000000000910";
const REGISTERED_NAMESPACE: &str = "00000000-0000-4000-8000-000000000911";
const REGISTERED_READER: &str = "00000000-0000-4000-8000-000000000912";
const REGISTERED_MANAGEMENT: &str = "00000000-0000-4000-8000-000000000913";

struct Environment(Vec<(&'static str, Option<OsString>)>);
impl Environment {
    fn configure(origin: &str, enabled: bool) -> Self {
        let values = [
            (
                "TT_AUTH__CENTRAL_JWKS_URI",
                Some(format!("{origin}/oidc/jwks")),
            ),
            ("TT_AUTH__CENTRAL_ISSUER", Some(origin.to_owned())),
            ("TT_AUTH__CENTRAL_TIMEOUT_SECS", Some("5".into())),
            (
                "TT_AUTH__CENTRAL_APPLICATION_ACCESS_REQUIRED",
                Some("true".into()),
            ),
            (
                "TT_USER_MANAGEMENT_READ_ENABLED",
                enabled.then(|| "true".into()),
            ),
            (
                "TT_NAMESPACE__MACHINE_SUBJECTS",
                Some(REGISTERED_NAMESPACE.into()),
            ),
            (
                "TT_SPACES__ADMISSION_READER_SUBJECTS",
                Some(REGISTERED_READER.into()),
            ),
            (
                "TT_USER_MANAGEMENT_MACHINE_SUBJECTS",
                Some(REGISTERED_MANAGEMENT.into()),
            ),
        ];
        let old = values
            .iter()
            .map(|(key, _)| (*key, std::env::var_os(key)))
            .collect();
        // Each target has exactly one synchronous #[test]. No runtime or helper
        // thread exists until configuration is complete; no later env mutation.
        for (key, value) in values {
            unsafe {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
        Self(old)
    }
}
impl Drop for Environment {
    fn drop(&mut self) {
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
struct Abort(tokio::task::AbortHandle);
impl Drop for Abort {
    fn drop(&mut self) {
        self.0.abort();
    }
}

// RFC8032 section 7.1 public Ed25519 TEST VECTOR, never a runtime key.
pub fn jwt(issuer: &str, subject: &str, role_hint: &str, session: bool, expires: i64) -> String {
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
    jsonwebtoken::encode(
        &header,
        &json!({"sub":subject,"aud":pdlc_auth_core::FLEET_AUDIENCE,"jti":Uuid::new_v4(),
        "iss":issuer,"iat":chrono::Utc::now().timestamp(),"exp":expires,"role":role_hint,
        "sid":session.then_some("synthetic-membership-session"),"scopes":[]}),
        &jsonwebtoken::EncodingKey::from_ed_der(&der),
    )
    .unwrap()
}

pub fn sql(query: &str, values: impl IntoIterator<Item = sea_orm::Value>) -> Statement {
    Statement::from_sql_and_values(DatabaseBackend::Postgres, query, values)
}
async fn connect(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("dedicated identity fixture connection");
    let name: String = db
        .query_one(sql("SELECT current_database() AS name", []))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "name")
        .unwrap();
    assert_eq!(
        name, "tasktracker_identity_test",
        "refuse application database"
    );
    db
}
#[derive(Clone)]
struct Profile {
    subject: String,
    role: &'static str,
    applications: Option<Vec<&'static str>>,
    active: bool,
}
#[derive(Clone)]
struct Central {
    profiles: Arc<RwLock<HashMap<String, Profile>>>,
    calls: Arc<AtomicUsize>,
    issuer: String,
}
fn application(issuer: &str, profile: &Profile) -> Value {
    let now = chrono::Utc::now();
    profile.applications.as_ref().map_or(Value::Null, |apps| {
        json!({
            "schema_version":1,"issuer":issuer,"subject":profile.subject,"role":profile.role,
            "applications":apps,"revision":1,"enforced":true,"observed_at":now,
            "expires_at":now+chrono::Duration::seconds(60)
        })
    })
}
impl Central {
    fn router(self) -> Router {
        let personal = self.clone();
        Router::new().route("/oidc/jwks",get(||async{Json(json!({"keys":[{
            "kty":"OKP","crv":"Ed25519","alg":"EdDSA","use":"sig",
            "kid":"rfc8032-public-test-vector","x":"11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo"
        }]}))}))
        .route("/auth/me",get(move|headers:HeaderMap|{let state=self.clone();async move{
            state.calls.fetch_add(1,Ordering::SeqCst);
            let token=headers.get("authorization").and_then(|v|v.to_str().ok()).and_then(|v|v.strip_prefix("Bearer "));
            let Some(profile)=token.and_then(|token|state.profiles.read().unwrap().get(token).cloned()) else{return (StatusCode::UNAUTHORIZED,Json(json!({})));};
            (StatusCode::OK,Json(json!({"id":profile.subject,"role":profile.role,
                "status":if profile.active{"active"}else{"disabled"},"display_name":"Synthetic",
                "application_access":application(&state.issuer,&profile)})))
        }}))
        .route("/auth/tokens/introspect",get(move|headers:HeaderMap|{let state=personal.clone();async move{
            state.calls.fetch_add(1,Ordering::SeqCst);
            let token=headers.get("authorization").and_then(|v|v.to_str().ok()).and_then(|v|v.strip_prefix("Bearer "));
            let Some(profile)=token.and_then(|token|state.profiles.read().unwrap().get(token).cloned()) else{return (StatusCode::UNAUTHORIZED,Json(json!({})));};
            let now=chrono::Utc::now();
            (StatusCode::OK,Json(json!({"sub":profile.subject,"email":"synthetic@example.test",
                "subject_role":profile.role,"role":profile.role,"scopes":["task-tracker:read"],
                "observed_at":now,"expires_at":now+chrono::Duration::seconds(60),
                "application_access":application(&state.issuer,&profile)})))
        }}))
    }
}

// The two independent test binaries intentionally use different fixture fields.
#[allow(dead_code)]
pub struct Harness {
    pub client: reqwest::Client,
    pub origin: String,
    pub tokens: HashMap<&'static str, String>,
    pub auth_calls: Arc<AtomicUsize>,
    pub observer: DatabaseConnection,
    pub blocker: DatabaseConnection,
    pub repository_pid: i32,
    pub target: Uuid,
    pub owned: Uuid,
    pub member: Uuid,
    pub registry: Uuid,
    pub space: Uuid,
    issuer: String,
    profiles: Arc<RwLock<HashMap<String, Profile>>>,
}
impl Harness {
    // Only the enabled binary exercises expiry; both binaries share this helper.
    #[allow(dead_code)]
    pub fn short_admin(&self, expires: i64) -> String {
        let token = jwt(&self.issuer, HUMAN, "user", true, expires);
        self.profiles.write().unwrap().insert(
            token.clone(),
            Profile {
                subject: HUMAN.into(),
                role: "admin",
                applications: Some(vec!["task-tracker"]),
                active: true,
            },
        );
        token
    }
    pub async fn get(&self, token: Option<&str>, query: &str) -> reqwest::Response {
        let mut request = self.client.get(format!(
            "{}/api/v2/identity-users/{}/memberships{query}",
            self.origin, self.target
        ));
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let response = request.send().await.expect("synthetic memberships request");
        assert_eq!(response.headers()["cache-control"], "no-store");
        response
    }
    pub async fn denied(&self, token: Option<&str>, query: &str, status: StatusCode) {
        let response = tokio::time::timeout(Duration::from_secs(5), self.get(token, query))
            .await
            .expect("denied request must finish while actual users relation remains blocked");
        assert_eq!(response.status(), status);
    }
    pub async fn fingerprints(&self) -> Vec<String> {
        // Compare every seeded column without printing rows/password material.
        let row=self.observer.query_one(sql("SELECT md5(COALESCE((SELECT jsonb_agg(to_jsonb(u) ORDER BY u.id)::text FROM users u),'[]')) AS users,md5(COALESCE((SELECT jsonb_agg(to_jsonb(p) ORDER BY p.id)::text FROM projects p),'[]')) AS projects,md5(COALESCE((SELECT jsonb_agg(to_jsonb(m) ORDER BY m.project_id,m.user_id)::text FROM project_members m),'[]')) AS members,md5(COALESCE((SELECT jsonb_agg(to_jsonb(s) ORDER BY s.project_id,s.registry_instance_id,s.space_id)::text FROM tracker_project_spaces s),'[]')) AS spaces",[])).await.unwrap().unwrap();
        ["users", "projects", "members", "spaces"]
            .into_iter()
            .map(|key| row.try_get("", key).unwrap())
            .collect()
    }
}

pub fn run<F, Fut>(enabled: bool, work: F)
where
    F: FnOnce(Harness) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let _environment = Environment::configure(&issuer, enabled);
    assert_eq!(
        std::env::var("TT_USER_MANAGEMENT_READ_ENABLED").ok(),
        enabled.then(|| "true".into())
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async move {
        let url=std::env::var("TT_IDENTITY_TEST_DATABASE_URL").expect("dedicated tasktracker_identity_test URL required");
        let cleanup=connect(&url).await;
        let schema=format!("identity_memberships_http_{}",Uuid::new_v4().simple());
        let mut scoped=reqwest::Url::parse(&url).unwrap();
        assert!(!scoped.query_pairs().any(|(key,_)|key=="options"));
        scoped.query_pairs_mut().append_pair("options",&format!("-csearch_path={schema}"));
        let scoped=scoped.to_string();let selected_schema=schema.clone();
        cleanup.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let result=tokio::spawn(async move {
            let db=connect(&scoped).await;
            let actual:String=db.query_one(sql("SELECT current_schema() AS name",[])).await.unwrap().unwrap().try_get("","name").unwrap();
            assert_eq!(actual,selected_schema);
            Migrator::up(&db,None).await.unwrap();
            db.execute(sql("INSERT INTO tracker_identity_legacy_issuer(issuer) VALUES($1)",[issuer.clone().into()])).await.unwrap();
            let target=Uuid::new_v4();let local=Uuid::new_v4();let other=Uuid::new_v4();
            for (id,subject) in [(local,target),(other,Uuid::new_v4())] {
                db.execute(sql("INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,$2,$3,'Synthetic','!',$4,true)",[
                    id.into(),format!("{id}@example.test").into(),id.to_string().into(),subject.to_string().into()
                ])).await.unwrap();
            }
            let owned=Uuid::new_v4();let member=Uuid::new_v4();
            for (id,key,owner) in [(owned,"ZZ",local),(member,"AA",other)] {
                db.execute(sql("INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,$2,'Synthetic project',$3,$4)",[
                    id.into(),key.into(),owner.into(),Uuid::new_v4().into()
                ])).await.unwrap();
            }
            db.execute(sql("INSERT INTO project_members(project_id,user_id,role,joined_at) VALUES($1,$2,'viewer','2026-01-02T03:04:05Z')",[member.into(),local.into()])).await.unwrap();
            let registry=Uuid::new_v4();let space=Uuid::new_v4();
            db.execute(sql("INSERT INTO tracker_space_catalog(registry_instance_id,space_id,state,admin_revision) VALUES($1,$2,'active',1)",[registry.into(),space.into()])).await.unwrap();
            db.execute(sql("INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id) VALUES($1,$2,$3)",[member.into(),registry.into(),space.into()])).await.unwrap();
            let repository_pid:i32=db.query_one(sql("SELECT pg_backend_pid() AS pid",[])).await.unwrap().unwrap().try_get("","pid").unwrap();
            let mut tokens=HashMap::new();let mut profiles=HashMap::new();
            for (name,subject,role,session,apps,active) in [
                ("admin",HUMAN,"admin",true,Some(vec!["task-tracker"]),true),
                ("user",HUMAN,"user",true,Some(vec!["task-tracker"]),true),
                ("machine",HUMAN,"service_account",false,None,true),
                ("sessionless",HUMAN,"admin",false,Some(vec!["task-tracker"]),true),
                ("foreign-app",HUMAN,"user",true,Some(vec!["wiki"]),true),
                ("global-admin",HUMAN,"admin",true,Some(vec!["wiki"]),true),
                ("missing-proof",HUMAN,"admin",true,None,true),
                ("disabled",HUMAN,"admin",true,Some(vec!["task-tracker"]),false),
                ("namespace-machine",REGISTERED_NAMESPACE,"admin",true,Some(vec!["task-tracker"]),true),
                ("reader-machine",REGISTERED_READER,"admin",true,Some(vec!["task-tracker"]),true),
                ("management-machine",REGISTERED_MANAGEMENT,"admin",true,Some(vec!["task-tracker"]),true),
            ] {
                // Canonical signed hints are stale in both directions; only
                // the fresh profile/application proof may authorize the read.
                // A test-only jti keeps each exact recognized token distinct.
                let hint=match role {"admin"=>"user","user"=>"admin",_=>"service_account"};
                let token=jwt(&issuer,subject,hint,session,chrono::Utc::now().timestamp()+300);
                profiles.insert(token.clone(),Profile{subject:subject.into(),role,applications:apps,active});
                tokens.insert(name,token);
            }
            for (name,role,apps) in [("human-pat","admin",Some(vec!["task-tracker"])),("machine-pat","service_account",None)] {
                let token=format!("sdlc_pat_memberships-{name}-synthetic");
                profiles.insert(token.clone(),Profile{subject:HUMAN.into(),role,applications:apps,active:true});tokens.insert(name,token);
            }
            let auth_calls=Arc::new(AtomicUsize::new(0));
            let profiles=Arc::new(RwLock::new(profiles));
            let central=Central{profiles:profiles.clone(),calls:auth_calls.clone(),issuer:issuer.clone()}.router();
            let central_listener=tokio::net::TcpListener::from_std(listener).unwrap();
            let central_server=tokio::spawn(async move{axum::serve(central_listener,central).await.unwrap()});
            let _central_guard=Abort(central_server.abort_handle());
            let observer=connect(&scoped).await;let blocker=connect(&scoped).await;
            let repos=Arc::new(infra::SeaOrmRepositories::new(db));
            let repository_set=domain::Repositories{users:repos.users.clone(),members:repos.members.clone(),..Default::default()};
            let ctx=Arc::new(app::AppContext::new(Arc::new(shared::AppConfig::default()),Arc::new(repository_set),Arc::new(domain::InMemoryStorage::default())));
            let owner_listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let origin=format!("http://{}",owner_listener.local_addr().unwrap());
            let router=api::router(ctx.clone()).with_state(ctx);
            let owner_server=tokio::spawn(async move{axum::serve(owner_listener,router).await.unwrap()});
            let _owner_guard=Abort(owner_server.abort_handle());
            let client=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(15)).build().unwrap();
            work(Harness{client,origin,tokens,auth_calls,observer,blocker,repository_pid,target,owned,member,registry,space,issuer,profiles}).await;
            owner_server.abort();let _=owner_server.await;central_server.abort();let _=central_server.await;drop(repos);
        }).await;
        cleanup.execute_unprepared("SET statement_timeout='15s'").await.unwrap();
        let removed=cleanup.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await;
        cleanup.close().await.unwrap();
        removed.expect("clean only own exact schema after success or panic");
        if let Err(error)=result {
            if error.is_panic(){std::panic::resume_unwind(error.into_panic());}
            panic!("membership fixture task cancelled");
        }
    });
}
