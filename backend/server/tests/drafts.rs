//! Real Tracker TCP HTTP and clean disposable PostgreSQL creation-saga checks.
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::sdlc::{CreatedDraft, PmDraftInputResponse};
use p256::{
    SecretKey,
    elliptic_curve::sec1::ToEncodedPoint,
    pkcs8::{EncodePrivateKey, LineEnding},
};
use reqwest::Client;
use sea_orm::{
    ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement, TransactionTrait,
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::oneshot;
use uuid::Uuid;

#[path = "support/metadata.rs"]
mod metadata;

#[derive(Clone)]
struct AuthStub {
    jwks: Value,
    owner: String,
    unavailable: Arc<AtomicBool>,
}

async fn session(State(stub): State<AuthStub>) -> StatusCode {
    if stub.unavailable.load(Ordering::SeqCst) {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    }
}

async fn introspect(
    State(stub): State<AuthStub>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    if stub.unavailable.load(Ordering::SeqCst) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let scopes = match headers.get("authorization").and_then(|h| h.to_str().ok()) {
        Some("Bearer sdlc_pat_owner") => vec!["task-tracker:read", "task-tracker:write"],
        Some("Bearer sdlc_pat_read") => vec!["task-tracker:read"],
        Some("Bearer sdlc_pat_other") => vec!["fleet-control:read"],
        _ => return Err(StatusCode::UNAUTHORIZED),
    };
    Ok(Json(
        json!({"sub":stub.owner,"email":"owner@example.test","scopes":scopes}),
    ))
}

fn bearer(secret: &SecretKey, issuer: &str, subject: &str, human: bool) -> String {
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256);
    header.kid = Some("draft-test".into());
    let mut claims = json!({"sub":subject,"iss":issuer,"aud":"sdlc","exp":shared::now().timestamp()+3600,
        "iat":shared::now().timestamp(),"email":"owner@example.test"});
    if human {
        claims["sid"] = json!("draft-session");
    }
    let pem = secret.to_pkcs8_pem(LineEnding::LF).unwrap();
    jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_ec_pem(pem.as_bytes()).unwrap(),
    )
    .unwrap()
}

async fn sql(db: &impl ConnectionTrait, query: &str, values: Vec<sea_orm::Value>) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        query,
        values,
    ))
    .await
    .unwrap();
}

async fn count(db: &DatabaseConnection, table: &str) -> i64 {
    db.query_one(Statement::from_string(
        DatabaseBackend::Postgres,
        format!("SELECT count(*) AS n FROM {table}"),
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "n")
    .unwrap()
}

async fn post(client: &Client, url: &str, token: &str, body: &Value) -> (u16, Value) {
    let response = client
        .post(url)
        .bearer_auth(token)
        .json(body)
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let text = response.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

async fn expect(client: &Client, url: &str, token: &str, body: &Value, status: u16) -> Value {
    let (actual, result) = post(client, url, token, body).await;
    assert_eq!(actual, status, "{result}");
    result
}

async fn get_json(client: &Client, url: &str, token: &str, status: u16) -> Value {
    let response = client.get(url).bearer_auth(token).send().await.unwrap();
    let actual = response.status().as_u16();
    let result: Value = response.json().await.unwrap();
    assert_eq!(actual, status, "{result}");
    result
}

async fn start(
    config: Arc<shared::AppConfig>,
) -> (String, oneshot::Sender<()>, tokio::task::JoinHandle<()>) {
    let (ready_tx, ready_rx) = oneshot::channel();
    let (stop_tx, stop_rx) = oneshot::channel();
    let handle = tokio::spawn(server::run(config, ready_tx, stop_rx));
    let address = tokio::time::timeout(std::time::Duration::from_secs(30), ready_rx)
        .await
        .unwrap()
        .unwrap();
    (format!("http://{address}"), stop_tx, handle)
}

fn command(key: &str) -> Value {
    json!({"title":"Restart-safe Draft", "description":"Exact human request\nSecond line", "idempotency_key":key})
}

#[tokio::test]
#[ignore = "requires clean isolated TT_SDLC_DRAFT_TEST_DATABASE_URL PostgreSQL database"]
async fn clean_migration_http_creation_ownership_concurrency_rollback_and_restart() {
    let database_url = std::env::var("TT_SDLC_DRAFT_TEST_DATABASE_URL")
        .expect("own clean PostgreSQL database required");
    let db = Database::connect(&database_url).await.unwrap();
    let schema = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT to_regclass('public.seaql_migrations') IS NULL AS clean",
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(
        schema.try_get::<bool>("", "clean").unwrap(),
        "test requires a freshly created database"
    );
    let config = Arc::new(shared::AppConfig {
        database: shared::DatabaseConfig {
            url: database_url.clone(),
            ..Default::default()
        },
        server: shared::ServerConfig {
            address: "127.0.0.1".into(),
            port: 0,
            general_rate_burst: 1000,
            ..Default::default()
        },
        auth: shared::AuthConfig {
            jwt_secret: "draft-test-local".into(),
            ..Default::default()
        },
        ..Default::default()
    });
    infra::run_migrations(config.database.clone())
        .await
        .unwrap();
    assert_eq!(count(&db, "sdlc_draft_creations").await, 0);
    assert_eq!(count(&db, "sdlc_tasks").await, 0);
    let owner_id = Uuid::new_v4();
    let operator_id = Uuid::new_v4();
    let owner_sub = Uuid::new_v4().to_string();
    let operator_sub = Uuid::new_v4().to_string();
    let foreign_sub = Uuid::new_v4().to_string();
    let disabled_sub = Uuid::new_v4().to_string();
    assert_ne!(owner_sub, owner_id.to_string());
    for (id, subject, name, active, admin) in [
        (owner_id, &owner_sub, "owner", true, false),
        (operator_id, &operator_sub, "operator", true, true),
        (Uuid::new_v4(), &foreign_sub, "foreign", true, true),
        (Uuid::new_v4(), &disabled_sub, "disabled", false, false),
    ] {
        sql(&db, "INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active,is_system_admin)
            VALUES($1,$2,$3,$3,'!',$4,$5,$6)",
            vec![id.into(),format!("{name}@example.test").into(),name.into(),subject.clone().into(),active.into(),admin.into()]).await;
    }
    let project = Uuid::new_v4();
    let other_project = Uuid::new_v4();
    for (id, key) in [(project, "DRAFT"), (other_project, "OTHER")] {
        sql(
            &db,
            "INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,$2,$2,$3,$4)",
            vec![
                id.into(),
                key.into(),
                operator_id.into(),
                Uuid::new_v4().into(),
            ],
        )
        .await;
        sql(&db, "INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Draft test','[]' FROM projects WHERE id=$1", vec![id.into()]).await;
        sql(
            &db,
            "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
            vec![id.into(), owner_id.into()],
        )
        .await;
    }
    let unavailable = Arc::new(AtomicBool::new(false));
    let secret = SecretKey::from_slice(&[9u8; 32]).unwrap();
    let point = secret.public_key().to_encoded_point(false);
    let stub = AuthStub {
        jwks: json!({"keys":[{"kid":"draft-test","kty":"EC","crv":"P-256","alg":"ES256","use":"sig",
            "x":URL_SAFE_NO_PAD.encode(point.x().unwrap()),"y":URL_SAFE_NO_PAD.encode(point.y().unwrap())}]}),
        owner: owner_sub.clone(),
        unavailable: unavailable.clone(),
    };
    let auth = Router::new()
        .route(
            "/jwks",
            get(|State(stub): State<AuthStub>| async move { Json(stub.jwks) }),
        )
        .route("/auth/me", get(session))
        .route("/auth/tokens/introspect", get(introspect))
        .with_state(stub);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let auth_handle = tokio::spawn(async move {
        axum::serve(listener, auth).await.unwrap();
    });
    // One test per binary; initialize process-global auth configuration before Tracker starts.
    unsafe {
        std::env::set_var("TT_AUTH__CENTRAL_JWKS_URI", format!("{issuer}/jwks"));
        std::env::set_var("TT_AUTH__CENTRAL_ISSUER", &issuer);
        std::env::set_var("TASKTRACKER_SDLC__INSTANCE_ID", "tracker-draft-test");
        std::env::remove_var("TASKTRACKER_SDLC__ORCHESTRATOR_SUBJECT");
        std::env::remove_var("TASKTRACKER_SDLC__VERIFIER_SUBJECT");
    }
    let owner = bearer(&secret, &issuer, &owner_sub, true);
    let operator = bearer(&secret, &issuer, &operator_sub, true);
    let foreign = bearer(&secret, &issuer, &foreign_sub, true);
    let local = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &app::auth::UserClaims {
            sub: owner_id.to_string(),
            exp: (shared::now().timestamp() + 3600) as usize,
            typ: Some("access".into()),
            jti: None,
        },
        &jsonwebtoken::EncodingKey::from_secret(config.auth.jwt_secret.as_bytes()),
    )
    .unwrap();
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap();
    let (base, stop, handle) = start(config.clone()).await;
    let url = format!("{base}/api/v1/projects/{project}/sdlc/drafts");
    let access_url = format!("{base}/api/v1/sdlc/project-access");
    let mut projects = vec![project, other_project];
    projects.sort_unstable();
    let expected_scope = json!({"contract_version":1,"tracker_instance_id":"tracker-draft-test","project_ids":projects});
    assert_eq!(
        get_json(&client, &access_url, &owner, 200).await,
        expected_scope
    );
    assert_eq!(
        get_json(&client, &access_url, "sdlc_pat_read", 200).await,
        expected_scope
    );
    sql(
        &db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
        vec![project.into(), operator_id.into()],
    )
    .await;
    assert_eq!(
        get_json(&client, &access_url, &operator, 200).await,
        expected_scope
    );
    assert_eq!(
        get_json(&client, &access_url, &foreign, 200).await["project_ids"],
        json!([])
    );
    for (token, status) in [
        ("local-token".to_string(), 401),
        ("sdlc_pat_other".to_string(), 403),
        (bearer(&secret, &issuer, &owner_sub, false), 403),
        (bearer(&secret, &issuer, &owner_id.to_string(), true), 403),
        (
            bearer(&secret, &issuer, &Uuid::new_v4().to_string(), true),
            403,
        ),
        (bearer(&secret, &issuer, &disabled_sub, true), 403),
        (
            bearer(&secret, &format!("{issuer}/wrong"), &owner_sub, true),
            401,
        ),
    ] {
        get_json(&client, &access_url, &token, status).await;
    }
    assert_eq!(client.get(&access_url).send().await.unwrap().status(), 401);
    assert_eq!(
        client
            .get(&access_url)
            .header("Cookie", format!("access_token={owner}"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let membership_index = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT indexdef FROM pg_indexes WHERE indexname='sdlc_project_members_user_idx'",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<String>("", "indexdef")
        .unwrap();
    assert!(membership_index.contains("(user_id, project_id)"));
    let before = count(&db, "issues").await;
    for (token, status) in [
        ("local-token".to_string(), 401),
        (
            bearer(&secret, &format!("{issuer}/wrong"), &owner_sub, true),
            401,
        ),
        (bearer(&secret, &issuer, &owner_id.to_string(), true), 403),
        (
            bearer(&secret, &issuer, &Uuid::new_v4().to_string(), true),
            403,
        ),
        (bearer(&secret, &issuer, &disabled_sub, true), 403),
        (bearer(&secret, &issuer, &owner_sub, false), 403),
        ("sdlc_pat_owner".into(), 403),
        ("sdlc_pat_read".into(), 403),
        (foreign.clone(), 403),
    ] {
        expect(&client, &url, &token, &command("denied"), status).await;
    }
    assert_eq!(
        client
            .post(&url)
            .json(&command("anonymous"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .post(&url)
            .header("Cookie", format!("access_token={owner}"))
            .json(&command("cookie"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    for field in [
        "user",
        "owner_subject",
        "owner_id",
        "reporter_id",
        "agent_id",
        "status_id",
        "status",
        "root_task_id",
        "project_id",
    ] {
        let mut body = command("spoof");
        body[field] = json!(operator_id);
        expect(&client, &url, &owner, &body, 422).await;
    }
    for field in ["title", "description", "idempotency_key"] {
        let mut body = command("missing");
        body.as_object_mut().unwrap().remove(field);
        expect(&client, &url, &owner, &body, 422).await;
    }
    for (field, value) in [
        ("title", json!("  ")),
        ("title", json!("x".repeat(501))),
        ("title", json!("control\n")),
        ("description", json!("x".repeat(100001))),
        ("idempotency_key", json!("")),
        ("idempotency_key", json!("key with space")),
    ] {
        let mut body = command("invalid");
        body[field] = value;
        expect(&client, &url, &owner, &body, 422).await;
    }
    assert_eq!(count(&db, "issues").await, before);
    // Fail the last business write. Issue, binding, history and ledger must all roll back.
    db.execute_unprepared("CREATE FUNCTION draft_test_outbox_failure() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_type='task.created' THEN RAISE EXCEPTION 'injected outbox failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER draft_test_outbox_failure BEFORE INSERT ON sdlc_outbox FOR EACH ROW EXECUTE FUNCTION draft_test_outbox_failure();").await.unwrap();
    expect(&client, &url, &owner, &command("creation"), 500).await;
    for table in [
        "sdlc_tasks",
        "sdlc_draft_creations",
        "sdlc_outbox",
        "issue_status_history",
    ] {
        assert_eq!(count(&db, table).await, 0);
    }
    assert_eq!(count(&db, "issues").await, before);
    db.execute_unprepared("DROP TRIGGER draft_test_outbox_failure ON sdlc_outbox; DROP FUNCTION draft_test_outbox_failure();").await.unwrap();
    let mut pending = vec![];
    for _ in 0..12 {
        let client = client.clone();
        let url = url.clone();
        let owner = owner.clone();
        pending.push(tokio::spawn(async move {
            post(&client, &url, &owner, &command("creation")).await
        }));
    }
    let mut created = 0;
    let mut result = Value::Null;
    for task in pending {
        let (status, body) = task.await.unwrap();
        assert!(status == 200 || status == 201, "{status}: {body}");
        created += usize::from(status == 201);
        if result.is_null() {
            result = body;
        } else {
            assert_eq!(result, body);
        }
    }
    assert_eq!(created, 1);
    let draft: CreatedDraft = serde_json::from_value(result.clone()).unwrap();
    assert_eq!(result.as_object().unwrap().len(), 7);
    assert_eq!(draft.project_id, project);
    assert_eq!(draft.task_id, draft.root_task_id);
    assert_eq!(draft.owner_subject, owner_sub);
    assert_eq!(draft.task_key, "DRAFT-1");
    let mut unknown = result.clone();
    unknown["unexpected"] = json!(true);
    assert!(serde_json::from_value::<CreatedDraft>(unknown).is_err());
    for table in [
        "sdlc_tasks",
        "sdlc_draft_creations",
        "sdlc_outbox",
        "issue_status_history",
    ] {
        assert_eq!(count(&db, table).await, 1);
    }
    assert_eq!(count(&db, "issues").await, before + 1);
    let event = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT event_type,payload FROM sdlc_outbox WHERE task_id=$1",
            [draft.task_id.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        event.try_get::<String>("", "event_type").unwrap(),
        "task.created"
    );
    let payload = event.try_get::<Value>("", "payload").unwrap();
    assert_eq!(payload["result"], result);
    assert_eq!(payload["contract_version"], 1);
    assert_eq!(payload["owner_subject"], owner_sub);
    assert_eq!(payload["root_task_id"], draft.task_id.to_string());
    assert_eq!(payload.get("requirement_revision"), Some(&Value::Null));
    for table in [
        "sdlc_assignments",
        "sdlc_agent_bindings",
        "sdlc_requirements",
        "sdlc_requests",
    ] {
        assert_eq!(count(&db, table).await, 0);
    }
    let row=db.query_one(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "SELECT i.reporter_id,i.assignee_id,i.summary,i.description,s.name FROM issues i JOIN statuses s ON s.id=i.status_id WHERE i.id=$1",
        [draft.task_id.into()])).await.unwrap().unwrap();
    assert_eq!(row.try_get::<Uuid>("", "reporter_id").unwrap(), owner_id);
    assert_eq!(
        row.try_get::<Option<Uuid>>("", "assignee_id").unwrap(),
        None
    );
    assert_eq!(row.try_get::<String>("", "name").unwrap(), "SDLC Draft");
    assert_eq!(
        row.try_get::<String>("", "summary").unwrap(),
        command("creation")["title"]
    );
    assert_eq!(
        row.try_get::<String>("", "description").unwrap(),
        command("creation")["description"]
    );
    let context_url = format!("{base}/api/v1/issues/{}/sdlc/context", draft.task_id);
    let context = client
        .get(&context_url)
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(context["owner_subject"], owner_sub);
    assert_eq!(context["stage"], "Draft");
    assert!(context["assignment"].is_null());
    let input_url = format!("{base}/api/v1/issues/{}/sdlc/pm-draft-input", draft.task_id);
    let snapshot = get_json(&client, &input_url, &owner, 200).await;
    let curl = tokio::process::Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--write-out",
            "\n%{http_code}",
            &input_url,
        ])
        .output()
        .await
        .unwrap();
    assert!(curl.status.success());
    assert_eq!(
        String::from_utf8(curl.stdout).unwrap().lines().last(),
        Some("401")
    );
    let typed: PmDraftInputResponse = serde_json::from_value(snapshot.clone()).unwrap();
    assert_eq!(snapshot.as_object().unwrap().len(), 7);
    assert_eq!(snapshot["input"].as_object().unwrap().len(), 4);
    assert_eq!(typed.contract_version, 1);
    assert_eq!(typed.tracker_instance_id, draft.tracker_instance_id);
    assert_eq!(typed.project_id, draft.project_id);
    assert_eq!(typed.task_id, draft.task_id);
    assert_eq!(typed.root_task_id, draft.root_task_id);
    assert_eq!(typed.owner_subject, draft.owner_subject);
    assert!(!typed.input.snapshot_ref.is_nil());
    assert_eq!(typed.input.title, command("creation")["title"]);
    assert_eq!(typed.input.description, command("creation")["description"]);
    let metadata_url = format!(
        "{base}/api/v1/issues/{}/sdlc/events?projection=metadata_v1",
        draft.task_id
    );
    let (created_metadata, metadata_page) =
        metadata::get(&client, &metadata_url, &owner, 200).await;
    let legacy_events = get_json(
        &client,
        &format!("{base}/api/v1/issues/{}/sdlc/events", draft.task_id),
        &owner,
        200,
    )
    .await;
    metadata::verify(&metadata_page, &legacy_events);
    assert_eq!(metadata_page["events"][0]["event_type"], "task.created");
    assert_eq!(
        metadata_page["events"][0]["payload"]["resource"]["input"],
        json!({"snapshot_ref":typed.input.snapshot_ref,"sha256":typed.input.sha256})
    );
    assert!(!String::from_utf8_lossy(&created_metadata).contains("description"));
    metadata::save("tracker-metadata-created.http.json", &created_metadata);
    assert_eq!(
        typed.input.sha256,
        app::sdlc::pm_draft_input_hash(&typed.input.title, &typed.input.description).unwrap()
    );
    for field in ["unexpected", "input"] {
        let mut unknown = snapshot.clone();
        if field == "input" {
            unknown[field]["unexpected"] = json!(true);
        } else {
            unknown[field] = json!(true);
        }
        assert!(serde_json::from_value::<PmDraftInputResponse>(unknown).is_err());
    }
    let mut reads = vec![];
    for _ in 0..12 {
        let client = client.clone();
        let url = input_url.clone();
        let owner = owner.clone();
        reads.push(tokio::spawn(async move {
            get_json(&client, &url, &owner, 200).await
        }));
    }
    for read in reads {
        assert_eq!(read.await.unwrap(), snapshot);
    }
    // Read ACL is exactly context ACL, not owner-only and not a new operator consent path.
    assert_eq!(
        get_json(&client, &input_url, &operator, 200).await,
        snapshot
    );
    assert_eq!(
        get_json(&client, &input_url, "sdlc_pat_read", 200).await,
        snapshot
    );
    expect(
        &client,
        &format!(
            "{base}/api/v1/issues/{}/sdlc/requirements/1/confirm",
            draft.task_id
        ),
        &operator,
        &json!({"content_hash":"not-consent","idempotency_key":"operator-not-owner"}),
        403,
    )
    .await;
    for (token, status) in [
        (foreign.clone(), 403),
        (bearer(&secret, &issuer, &disabled_sub, true), 403),
        (bearer(&secret, &issuer, &owner_id.to_string(), true), 403),
        ("local-token".into(), 401),
        (local, 401),
        ("sdlc_pat_other".into(), 403),
        (bearer(&secret, &issuer, &owner_sub, false), 403),
    ] {
        get_json(&client, &input_url, &token, status).await;
        get_json(&client, &context_url, &token, status).await;
    }
    assert_eq!(
        client
            .get(&input_url)
            .header("Cookie", format!("access_token={owner}"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let edit = client
        .patch(format!("{base}/api/v1/issues/{}", draft.task_id))
        .bearer_auth(&owner)
        .json(&json!({"summary":"Edited display title","description":"Edited mutable issue"}))
        .send()
        .await
        .unwrap();
    assert_eq!(edit.status(), 200, "{}", edit.text().await.unwrap());
    assert_eq!(
        metadata::get(&client, &metadata_url, &owner, 200).await.0,
        created_metadata
    );
    assert_eq!(get_json(&client, &input_url, &owner, 200).await, snapshot);
    assert_eq!(
        expect(&client, &url, &owner, &command("creation"), 200).await,
        result
    );
    let unicode = json!({"title":"  \u{0417}\u{0430}\u{0434}\u{0430}\u{0447}\u{0430} \u{1f680}  ",
        "description":"first\r\nsecond\ne\u{0301} \u{00e9}\t\"\\","idempotency_key":"unicode-snapshot"});
    let unicode_draft = expect(&client, &url, &owner, &unicode, 201).await;
    let unicode_input = get_json(
        &client,
        &format!(
            "{base}/api/v1/issues/{}/sdlc/pm-draft-input",
            unicode_draft["task_id"].as_str().unwrap()
        ),
        &owner,
        200,
    )
    .await;
    assert_eq!(unicode_input["input"]["title"], unicode["title"]);
    assert_eq!(
        unicode_input["input"]["description"],
        unicode["description"]
    );
    assert_eq!(
        unicode_input["input"]["sha256"],
        "32b3c95cffc2114b62b969de058f4e3839c3e6b82d7ab09c061e35b0ed0d0b35"
    );
    // JSON member order is irrelevant; string whitespace/content remains exact.
    let reordered = json!({"idempotency_key":"creation", "description":command("creation")["description"], "title":command("creation")["title"]});
    assert_eq!(expect(&client, &url, &owner, &reordered, 200).await, result);
    for field in ["title", "description"] {
        let mut changed = command("creation");
        changed[field] = json!("different exact content");
        expect(&client, &url, &owner, &changed, 409).await;
    }
    let mut changed = command("concurrent-conflict");
    changed["title"] = json!("competing request");
    let a = command("concurrent-conflict");
    let (a, b) = tokio::join!(
        post(&client, &url, &owner, &a),
        post(&client, &url, &owner, &changed)
    );
    assert!((a.0 == 201 && b.0 == 409) || (a.0 == 409 && b.0 == 201));
    // Same key is independent for a different authenticated human and project.
    let operator_draft = expect(&client, &url, &operator, &command("creation"), 201).await;
    assert_eq!(operator_draft["owner_subject"], operator_sub);
    assert_ne!(operator_draft["task_id"], result["task_id"]);
    let other_url = format!("{base}/api/v1/projects/{other_project}/sdlc/drafts");
    let other = expect(&client, &other_url, &owner, &command("creation"), 201).await;
    assert_ne!(other["task_id"], result["task_id"]);
    assert_eq!(other["task_key"], "OTHER-1");
    let other_input_url = format!(
        "{base}/api/v1/issues/{}/sdlc/pm-draft-input",
        other["task_id"].as_str().unwrap()
    );
    get_json(&client, &other_input_url, &owner, 200).await;
    sql(
        &db,
        "DELETE FROM project_members WHERE project_id=$1 AND user_id=$2",
        vec![other_project.into(), owner_id.into()],
    )
    .await;
    get_json(&client, &other_input_url, &owner, 403).await;
    get_json(
        &client,
        &format!(
            "{base}/api/v1/issues/{}/sdlc/context",
            other["task_id"].as_str().unwrap()
        ),
        &owner,
        403,
    )
    .await;
    assert_eq!(get_json(&client, &input_url, &owner, 200).await, snapshot);
    sql(
        &db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
        vec![other_project.into(), owner_id.into()],
    )
    .await;
    // Ordinary issue creators share MAX(suffix) and the unique key, even while drafts are created.
    let ordinary = json!({"project_key":"DRAFT","issue_type":"task","summary":"Ordinary task","description":"ordinary","priority":"medium"});
    let ordinary_url = format!("{base}/api/v1/issues");
    let concurrent_draft = command("with-ordinary");
    let (ordinary, another) = tokio::join!(
        post(&client, &ordinary_url, &owner, &ordinary),
        post(&client, &url, &owner, &concurrent_draft)
    );
    assert_eq!(ordinary.0, 201, "{}", ordinary.1);
    assert_eq!(another.0, 201, "{}", another.1);
    assert_ne!(ordinary.1["key"], another.1["task_key"]);
    // Legacy binding has no original creation snapshot; mutable issue text is not provenance.
    let legacy_id = Uuid::parse_str(ordinary.1["id"].as_str().unwrap()).unwrap();
    expect(
        &client,
        &format!("{base}/api/v1/issues/{legacy_id}/sdlc/binding"),
        &owner,
        &json!({"root_task_id":legacy_id,"idempotency_key":"legacy-binding"}),
        200,
    )
    .await;
    let legacy_input_url = format!("{base}/api/v1/issues/{legacy_id}/sdlc/pm-draft-input");
    get_json(&client, &legacy_input_url, &owner, 409).await;
    let legacy_result = json!({"tracker_instance_id":"tracker-draft-test","project_id":project,
        "task_id":legacy_id,"root_task_id":legacy_id,"owner_subject":owner_sub,
        "task_key":ordinary.1["key"],"stage":"Draft"});
    assert!(db.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "INSERT INTO sdlc_draft_creations(project_id,actor_subject,idempotency_key,payload_hash,task_id,result,input_snapshot_ref)
         VALUES($1,$2,'partial-snapshot',$3,$4,$5,$6)",
        [project.into(),owner_sub.clone().into(),"a".repeat(64).into(),legacy_id.into(),legacy_result.clone().into(),Uuid::now_v7().into()])).await.is_err());
    sql(&db, "INSERT INTO sdlc_draft_creations(project_id,actor_subject,idempotency_key,payload_hash,task_id,result)
        VALUES($1,$2,'historical-no-snapshot',$3,$4,$5)",
        vec![project.into(),owner_sub.clone().into(),"a".repeat(64).into(),legacy_id.into(),legacy_result.into()]).await;
    get_json(&client, &legacy_input_url, &owner, 409).await;
    // Reserve an unseen ordinary issue number. The draft insert must wait for its
    // unique key, then retry allocation after the competing transaction commits.
    let tx = db.begin().await.unwrap();
    let max = tx.query_one(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "SELECT MAX((substring(key FROM '-([0-9]+)$'))::bigint) AS n FROM issues WHERE project_id=$1",
        [project.into()])).await.unwrap().unwrap().try_get::<i64>("", "n").unwrap();
    sql(&tx, "INSERT INTO issues(id,project_id,key,issue_type,status_id,summary,reporter_id,priority,labels,position,time_spent_seconds)
        SELECT $1,project_id,$2,issue_type,status_id,'Concurrent ordinary insertion',reporter_id,priority,labels,position,time_spent_seconds FROM issues WHERE id=$3",
        vec![Uuid::new_v4().into(),format!("DRAFT-{}", max + 1).into(),draft.task_id.into()]).await;
    let collision_client = client.clone();
    let collision_url = url.clone();
    let collision_owner = owner.clone();
    let mut collision = tokio::spawn(async move {
        post(
            &collision_client,
            &collision_url,
            &collision_owner,
            &command("forced-number-conflict"),
        )
        .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut collision)
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    let collision = collision.await.unwrap();
    assert_eq!(collision.0, 201, "{}", collision.1);
    assert_eq!(collision.1["task_key"], format!("DRAFT-{}", max + 2));
    let ordinary = post(&client, &ordinary_url, &owner,
        &json!({"project_key":"DRAFT","issue_type":"task","summary":"Highest ordinary task","priority":"medium"})).await;
    assert_eq!(ordinary.0, 201, "{}", ordinary.1);
    let ordinary_id = Uuid::parse_str(ordinary.1["id"].as_str().unwrap()).unwrap();
    sql(
        &db,
        "UPDATE issues SET deleted_at=now() WHERE id=$1",
        vec![ordinary_id.into()],
    )
    .await;
    let max=db.query_one(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "SELECT MAX((substring(key FROM '-([0-9]+)$'))::bigint) AS n FROM issues WHERE project_id=$1",
        [project.into()])).await.unwrap().unwrap().try_get::<i64>("","n").unwrap();
    let after_delete = expect(&client, &url, &owner, &command("after-deleted-number"), 201).await;
    assert_eq!(after_delete["task_key"], format!("DRAFT-{}", max + 1));
    // Authorization is locked and rechecked after a concurrent committed membership removal.
    let tx = db.begin().await.unwrap();
    sql(
        &tx,
        "DELETE FROM project_members WHERE project_id=$1 AND user_id=$2",
        vec![project.into(), owner_id.into()],
    )
    .await;
    let wait_client = client.clone();
    let wait_url = url.clone();
    let wait_owner = owner.clone();
    let mut waiting = tokio::spawn(async move {
        post(&wait_client, &wait_url, &wait_owner, &command("creation")).await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut waiting)
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    assert_eq!(waiting.await.unwrap().0, 403);
    assert_eq!(
        get_json(&client, &access_url, &owner, 200).await["project_ids"],
        json!([other_project])
    );
    expect(&client, &url, &owner, &command("creation"), 403).await;
    get_json(&client, &input_url, &owner, 403).await;
    expect(
        &client,
        &url,
        &owner,
        &command("after-membership-revoke"),
        403,
    )
    .await;
    assert_eq!(
        client
            .get(&context_url)
            .bearer_auth(&owner)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    sql(
        &db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
        vec![project.into(), owner_id.into()],
    )
    .await;
    sql(
        &db,
        "UPDATE users SET is_active=false WHERE id=$1",
        vec![owner_id.into()],
    )
    .await;
    get_json(&client, &access_url, &owner, 403).await;
    get_json(&client, &input_url, &owner, 403).await;
    expect(&client, &url, &owner, &command("creation"), 403).await;
    sql(
        &db,
        "UPDATE users SET is_active=true WHERE id=$1",
        vec![owner_id.into()],
    )
    .await;
    sql(
        &db,
        "UPDATE issues SET reporter_id=$2 WHERE id=$1",
        vec![draft.task_id.into(), operator_id.into()],
    )
    .await;
    expect(&client, &url, &owner, &command("creation"), 409).await;
    sql(
        &db,
        "UPDATE issues SET reporter_id=$2 WHERE id=$1",
        vec![draft.task_id.into(), owner_id.into()],
    )
    .await;
    sql(
        &db,
        "UPDATE issues SET deleted_at=now() WHERE id=$1",
        vec![draft.task_id.into()],
    )
    .await;
    expect(&client, &url, &owner, &command("creation"), 404).await;
    get_json(&client, &input_url, &owner, 404).await;
    sql(
        &db,
        "UPDATE issues SET deleted_at=NULL WHERE id=$1",
        vec![draft.task_id.into()],
    )
    .await;
    assert!(
        db.execute_unprepared("UPDATE sdlc_draft_creations SET result='{}'")
            .await
            .is_err()
    );
    assert!(
        db.execute_unprepared("UPDATE sdlc_draft_creations SET input_title='rewritten'")
            .await
            .is_err()
    );
    assert!(
        db.execute_unprepared("DELETE FROM sdlc_draft_creations")
            .await
            .is_err()
    );
    for malformed in [
        json!({}),
        json!(null),
        {
            let mut value = result.clone();
            value["stage"] = Value::Null;
            value
        },
        {
            let mut value = result.clone();
            value["unexpected"] = json!(true);
            value
        },
    ] {
        assert!(db.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
            "INSERT INTO sdlc_draft_creations(project_id,actor_subject,idempotency_key,payload_hash,task_id,result)
             VALUES($1,$2,'malformed-result',$3,$4,$5)",
            [project.into(),owner_sub.clone().into(),"a".repeat(64).into(),draft.task_id.into(),malformed.into()])).await.is_err());
    }
    // Lose the response after it is fully committed, then restart the actual server process task.
    let lost = client
        .post(&url)
        .bearer_auth(&owner)
        .json(&command("lost-response"))
        .send()
        .await
        .unwrap();
    assert_eq!(lost.status(), 201);
    drop(lost);
    let ledger_before = count(&db, "sdlc_draft_creations").await;
    let issues_before = count(&db, "issues").await;
    let events_before = count(&db, "sdlc_outbox").await;
    stop.send(()).unwrap();
    handle.await.unwrap();
    let (base, stop, handle) = start(config).await;
    assert_eq!(
        metadata::get(
            &client,
            &format!(
                "{base}/api/v1/issues/{}/sdlc/events?projection=metadata_v1",
                draft.task_id
            ),
            &owner,
            200
        )
        .await
        .0,
        created_metadata
    );
    let url = format!("{base}/api/v1/projects/{project}/sdlc/drafts");
    assert_eq!(
        get_json(
            &client,
            &format!("{base}/api/v1/issues/{}/sdlc/pm-draft-input", draft.task_id),
            &owner,
            200
        )
        .await,
        snapshot
    );
    assert_eq!(
        expect(&client, &url, &owner, &command("creation"), 200).await,
        result
    );
    let recovered = expect(&client, &url, &owner, &command("lost-response"), 200).await;
    assert_eq!(
        expect(&client, &url, &owner, &command("lost-response"), 200).await,
        recovered
    );
    assert_eq!(count(&db, "sdlc_draft_creations").await, ledger_before);
    assert_eq!(count(&db, "issues").await, issues_before);
    assert_eq!(count(&db, "sdlc_outbox").await, events_before);
    unavailable.store(true, Ordering::SeqCst);
    get_json(
        &client,
        &format!("{base}/api/v1/issues/{}/sdlc/pm-draft-input", draft.task_id),
        &owner,
        503,
    )
    .await;
    get_json(
        &client,
        &format!("{base}/api/v1/sdlc/project-access"),
        &owner,
        503,
    )
    .await;
    expect(&client, &url, &owner, &command("creation"), 503).await;
    stop.send(()).unwrap();
    handle.await.unwrap();
    auth_handle.abort();
}
