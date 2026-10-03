//! Isolated PostgreSQL + HTTP acceptance of Tracker only; Central Auth is a test issuer.
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::sdlc::*;
use p256::{
    SecretKey,
    elliptic_curve::sec1::ToEncodedPoint,
    pkcs8::{EncodePrivateKey, LineEnding},
};
use reqwest::Client;
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement};
use serde_json::{Value, json};
use shared::AppError;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::oneshot;
use uuid::Uuid;

#[path = "support/analysis_intent.rs"]
mod analysis_intent;
#[path = "support/analysis_reservation.rs"]
mod analysis_reservation;
#[path = "support/lifecycle_guard.rs"]
mod lifecycle_guard;
#[path = "support/metadata.rs"]
mod metadata;
#[path = "support/pm_credential_boundary.rs"]
mod pm_credential_boundary;
#[path = "support/routing_policy.rs"]
mod routing_policy;

#[derive(Clone)]
struct AuthStub {
    jwks: Value,
    tokens: Arc<HashMap<String, Value>>,
    unavailable: Arc<AtomicBool>,
}

async fn introspect(
    State(stub): State<AuthStub>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    if stub.unavailable.load(Ordering::SeqCst) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    Ok(Json(
        stub.tokens
            .get(token)
            .cloned()
            .ok_or(StatusCode::UNAUTHORIZED)?,
    ))
}

async fn session(State(stub): State<AuthStub>) -> StatusCode {
    if stub.unavailable.load(Ordering::SeqCst) {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    }
}

async fn sql(db: &DatabaseConnection, query: &str, values: Vec<sea_orm::Value>) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        query,
        values,
    ))
    .await
    .unwrap();
}

async fn count(db: &DatabaseConnection, table: &str) -> i64 {
    // Table names are test-owned constants, never request data.
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

async fn post(client: &Client, url: &str, token: &str, body: &Value, status: u16) -> Value {
    let response = client
        .post(url)
        .bearer_auth(token)
        .json(body)
        .send()
        .await
        .unwrap();
    let actual = response.status().as_u16();
    let text = response.text().await.unwrap();
    assert_eq!(actual, status, "{url}: {text}");
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

fn human_token(
    secret: &SecretKey,
    issuer: &str,
    subject: &str,
    audience: &str,
    expires: i64,
) -> String {
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256);
    header.kid = Some("tracker-test".into());
    let pem = secret.to_pkcs8_pem(LineEnding::LF).unwrap();
    jsonwebtoken::encode(&header, &json!({"sub":subject,"iss":issuer,"aud":audience,"exp":expires,"iat":chrono_now(),"sid":"test-session","email":format!("{subject}@example.test")}), &jsonwebtoken::EncodingKey::from_ec_pem(pem.as_bytes()).unwrap()).unwrap()
}

fn chrono_now() -> i64 {
    shared::now().timestamp()
}

async fn start_tracker(
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

#[tokio::test]
#[ignore = "requires isolated TT_SDLC_TEST_DATABASE_URL PostgreSQL database"]
async fn postgres_http_clarification_ownership_replay_gate_and_restart() {
    let database_url = std::env::var("TT_SDLC_TEST_DATABASE_URL")
        .expect("isolated PostgreSQL test database required");
    let db = Database::connect(&database_url).await.unwrap();
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
            jwt_secret: "tracker-test-local".into(),
            ..Default::default()
        },
        ..Default::default()
    });
    infra::run_migrations(config.database.clone())
        .await
        .unwrap();
    sql(
        &db,
        "TRUNCATE users,projects,issues,boards,sprints,sdlc_instance CASCADE",
        vec![],
    )
    .await;
    let owner_id = Uuid::new_v4();
    let project = Uuid::new_v4();
    let task = Uuid::new_v4();
    let foreign_task = Uuid::new_v4();
    let unbound = Uuid::new_v4();
    for (subject, id) in [
        ("owner", owner_id),
        ("operator", Uuid::new_v4()),
        ("foreign", Uuid::new_v4()),
        ("pm", Uuid::new_v4()),
        ("fleet", Uuid::new_v4()),
        ("verifier", Uuid::new_v4()),
        ("scheduler", Uuid::new_v4()),
    ] {
        sql(&db, "INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active,is_system_admin) VALUES($1,$2,$3,$3,'!',$3,true,$4)", vec![id.into(), format!("{subject}@example.test").into(), subject.into(), (subject == "operator" || subject == "foreign").into()]).await;
    }
    for (id, key) in [(project, "SDLC"), (Uuid::new_v4(), "OTHER")] {
        sql(
            &db,
            "INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,$2,$2,$3,$4)",
            vec![
                id.into(),
                key.into(),
                owner_id.into(),
                Uuid::new_v4().into(),
            ],
        )
        .await;
    }
    sql(&db, "INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'SDLC test','[]' FROM projects", vec![]).await;
    for (id, key, project_key) in [
        (task, "SDLC-1", "SDLC"),
        (unbound, "SDLC-2", "SDLC"),
        (foreign_task, "OTHER-1", "OTHER"),
    ] {
        sql(&db, "INSERT INTO issues(id,project_id,key,issue_type,status_id,summary,reporter_id,priority,labels,position,time_spent_seconds) VALUES($1,(SELECT id FROM projects WHERE key=$2),$3,'task',(SELECT id FROM statuses WHERE is_default LIMIT 1),'Clarify',$4,'medium','[]',0,0)", vec![id.into(), project_key.into(), key.into(), owner_id.into()]).await;
    }
    for subject in ["operator", "pm", "fleet", "verifier", "scheduler"] {
        sql(&db, "INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub=$2", vec![project.into(), subject.into()]).await;
    }
    let assignment = PmAssignment {
        assignment_id: Uuid::new_v4(),
        execution_id: Uuid::new_v4(),
        agent_id: Uuid::new_v4(),
        version: 1,
        machine_subject: "pm".into(),
    };
    let fence = json!({"assignment_id":assignment.assignment_id,"execution_id":assignment.execution_id,"agent_id":assignment.agent_id,"assignment_version":1});
    let replacement = PmAssignment {
        version: 2,
        assignment_id: Uuid::new_v4(),
        execution_id: Uuid::new_v4(),
        agent_id: Uuid::new_v4(),
        machine_subject: "pm".into(),
    };
    let mut tokens = HashMap::new();
    for (token, subject, scopes) in [
        (
            "sdlc_pat_reservation_scheduler",
            "scheduler",
            vec!["task-tracker:write"],
        ),
        (
            "sdlc_pat_reservation_reader",
            "scheduler",
            vec!["task-tracker:read"],
        ),
        (
            "sdlc_pat_reservation_compound",
            "scheduler",
            vec!["task-tracker:write", "task-tracker:read"],
        ),
        (
            "sdlc_pat_reservation_foreign",
            "foreign",
            vec!["task-tracker:read"],
        ),
    ] {
        tokens.insert(token.into(),json!({"sub":subject,"email":format!("{subject}@example.test"),"scopes":scopes,"expiresAt":chrono_now()+3600}));
    }
    for (token, subject, scopes) in [
        (
            "sdlc_pat_replacement",
            "pm",
            vec![
                "task-tracker:read".to_string(),
                "task-tracker:write".to_string(),
                replacement.scope(task),
            ],
        ),
        (
            "sdlc_pat_pm",
            "pm",
            vec![
                "task-tracker:read".to_string(),
                "task-tracker:write".to_string(),
                assignment.scope(task),
            ],
        ),
        (
            "sdlc_pat_fleet",
            "fleet",
            vec![
                "task-tracker:write".to_string(),
                "task-tracker:sdlc:assign".to_string(),
            ],
        ),
        (
            "sdlc_pat_verifier",
            "verifier",
            vec![
                "task-tracker:write".to_string(),
                format!("task-tracker:sdlc:evidence:{task}:1"),
            ],
        ),
        (
            "sdlc_pat_wrong_scope",
            "pm",
            vec!["task-tracker:read".to_string()],
        ),
        (
            "sdlc_pat_owner",
            "owner",
            vec![
                "task-tracker:read".to_string(),
                "task-tracker:write".to_string(),
            ],
        ),
    ] {
        tokens.insert(
            token.to_string(),
            json!({"sub":subject,"email":format!("{subject}@example.test"),"scopes":scopes}),
        );
    }
    pm_credential_boundary::tokens(&mut tokens, &assignment, &replacement, task);
    let secret = SecretKey::from_slice(&[7u8; 32]).unwrap();
    let point = secret.public_key().to_encoded_point(false);
    let unavailable = Arc::new(AtomicBool::new(false));
    let stub = AuthStub {
        jwks: json!({"keys":[{"kid":"tracker-test","kty":"EC","crv":"P-256","alg":"ES256","use":"sig","x":URL_SAFE_NO_PAD.encode(point.x().unwrap()),"y":URL_SAFE_NO_PAD.encode(point.y().unwrap())}]}),
        tokens: Arc::new(tokens),
        unavailable: unavailable.clone(),
    };
    let auth_router = Router::new()
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
        axum::serve(listener, auth_router).await.unwrap();
    });
    // This binary has one test. Configuration is set before starting Tracker or its auth bridge.
    unsafe {
        std::env::set_var("TT_AUTH__CENTRAL_JWKS_URI", format!("{issuer}/jwks"));
        std::env::set_var("TT_AUTH__CENTRAL_ISSUER", &issuer);
        std::env::set_var(
            "TASKTRACKER_SDLC__INSTANCE_ID",
            "tracker-integration-\u{416}\u{1f680}\"\\\r\n",
        );
        std::env::set_var("TASKTRACKER_SDLC__ORCHESTRATOR_SUBJECT", "fleet");
        std::env::set_var("TASKTRACKER_SDLC__VERIFIER_SUBJECT", "verifier");
        std::env::set_var(
            "TASKTRACKER_SDLC__RESERVATION_SCHEDULER_SUBJECT",
            "scheduler",
        );
    }
    let owner = human_token(&secret, &issuer, "owner", "sdlc", chrono_now() + 3600);
    let operator = human_token(&secret, &issuer, "operator", "sdlc", chrono_now() + 3600);
    let foreign = human_token(&secret, &issuer, "foreign", "sdlc", chrono_now() + 3600);
    let client = Client::new();
    let (base, stop, handle) = start_tracker(config.clone()).await;
    let url = format!("{base}/api/v1/issues/{task}/sdlc");
    for token in [
        "local-token".to_string(),
        human_token(&secret, &issuer, "owner", "wrong", chrono_now() + 3600),
        human_token(&secret, &issuer, "owner", "sdlc", chrono_now() - 3600),
    ] {
        assert_eq!(
            client
                .get(format!("{url}/context"))
                .bearer_auth(token)
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
    }
    post(
        &client,
        &format!("{url}/binding"),
        &owner,
        &json!({"root_task_id":foreign_task,"idempotency_key":"cross-project"}),
        422,
    )
    .await;
    lifecycle_guard::binding(&db, &client, &base, &owner, task, unbound).await;
    let bind = json!({"root_task_id":task,"idempotency_key":"binding"});
    let context = post(&client, &format!("{url}/binding"), &owner, &bind, 200).await;
    assert_eq!(context["contract_version"], 1);
    assert_eq!(context["owner_subject"], "owner");
    assert_eq!(
        context["tracker_instance_id"],
        "tracker-integration-\u{416}\u{1f680}\"\\\r\n"
    );
    assert_eq!(
        context["permissions"],
        json!({"can_answer":false,"can_confirm":false})
    );
    post(&client, &format!("{url}/binding"), &owner, &bind, 200).await;
    assert_eq!(count(&db, "sdlc_tasks").await, 1);
    lifecycle_guard::reject_child_row(&db, task, unbound).await;
    assert_eq!(
        client
            .get(format!("{base}/api/v1/issues/{unbound}/sdlc/context"))
            .bearer_auth(&owner)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    for path in [
        "context",
        "clarifications",
        "requirements/revisions",
        "events",
    ] {
        assert_eq!(
            client
                .get(format!("{url}/{path}"))
                .bearer_auth(&foreign)
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    }
    assert_eq!(
        client
            .get(format!("{url}/clarifications"))
            .bearer_auth(&owner)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap(),
        json!({"questions":[]})
    );
    assert_eq!(
        client
            .get(format!("{url}/requirements/revisions"))
            .bearer_auth(&owner)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap(),
        json!({"revisions":[]})
    );
    post(&client,&format!("{url}/assignment"),"sdlc_pat_fleet",&json!({"assignment":assignment,"expected_assignment_version":null,"idempotency_key":"assign"}),200).await;
    pm_credential_boundary::initial(&db, &client, &base, task, foreign_task, project, &owner).await;
    // A valid result larger than Fleet's 1 MiB cap must remain pollable as metadata.
    let document = json!({"goal":"Approved goal","scope":vec!["private-result-\u{416}\u{1f680}\"\\\n".repeat(2500);20],"exclusions":[],"scenarios":["Scenario"],"acceptance_criteria":["Acceptance"],"constraints":[],"dependencies":[],"assumptions":[],"checklist":["review"],"prerequisites":["contract"]});
    let publish = json!({"fence":fence,"expected_requirement_revision":null,"document":document,"idempotency_key":"rev-1"});
    post(
        &client,
        &format!("{url}/requirements"),
        &owner,
        &publish,
        403,
    )
    .await;
    post(
        &client,
        &format!("{url}/requirements"),
        "sdlc_pat_wrong_scope",
        &publish,
        403,
    )
    .await;
    let revision = post(
        &client,
        &format!("{url}/requirements"),
        "sdlc_pat_pm",
        &publish,
        200,
    )
    .await;
    assert_eq!(revision["goal"], "Approved goal");
    assert!(revision.get("document").is_none());
    let question_id = Uuid::new_v4();
    let option_id = Uuid::new_v4();
    let question = json!({"fence":fence,"request_id":Uuid::new_v4(),"question_id":question_id,"expected_question_version":null,"requirement_revision":1,"checkpoint_id":Uuid::new_v4(),"requirement_reference":"scope","text":"Choose scope","rationale":"Needed for final requirements","required":true,"mode":"single","options":[{"id":option_id,"label":"Standard","consequences":"Known path","is_custom":false}],"recommended_option_id":option_id,"idempotency_key":"question"});
    let q = post(
        &client,
        &format!("{url}/clarifications"),
        "sdlc_pat_pm",
        &question,
        200,
    )
    .await;
    assert!(q["answer"].is_null());
    assert_eq!(q["options"][0]["is_custom"], false);
    for suffix in [
        "requirements",
        "requirements/1",
        "requirements/1/diff?against=1",
    ] {
        let response = client
            .get(format!("{url}/{suffix}"))
            .bearer_auth(&owner)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let read = response.json::<Value>().await.unwrap();
        if suffix.contains("diff") {
            assert_eq!(read["before"], revision);
            assert_eq!(read["after"], revision);
        } else {
            assert_eq!(read, revision);
        }
    }
    let mut premature = publish.clone();
    premature["expected_requirement_revision"] = json!(1);
    premature["idempotency_key"] = json!("premature");
    post(
        &client,
        &format!("{url}/requirements"),
        "sdlc_pat_pm",
        &premature,
        409,
    )
    .await;
    let answer_url = format!("{url}/clarifications/{question_id}/answers");
    let answer = json!({"expected_question_version":1,"requirement_revision":1,"selected_option_ids":[option_id],"text":null,"comment":"Owner choice","idempotency_key":"answer"});
    post(&client, &answer_url, &operator, &answer, 403).await;
    post(&client, &answer_url, "sdlc_pat_owner", &answer, 403).await;
    let mut unsafe_version = answer.clone();
    unsafe_version["expected_question_version"] = json!(9007199254740992i64);
    post(&client, &answer_url, &owner, &unsafe_version, 422).await;
    let mut spoof = answer.clone();
    spoof["author_subject"] = json!("operator");
    post(&client, &answer_url, &owner, &spoof, 422).await;
    let mut stale = answer.clone();
    stale["expected_question_version"] = json!(2);
    post(&client, &answer_url, &owner, &stale, 409).await;
    // Inject an outbox failure after business inserts: the entire command must roll back.
    db.execute_unprepared("CREATE FUNCTION test_fail_outbox() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_type='clarification.answered' THEN RAISE EXCEPTION 'injected outbox failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER test_fail_outbox BEFORE INSERT ON sdlc_outbox FOR EACH ROW EXECUTE FUNCTION test_fail_outbox();").await.unwrap();
    post(&client, &answer_url, &owner, &answer, 500).await;
    assert_eq!(count(&db, "sdlc_answers").await, 0);
    db.execute_unprepared(
        "DROP TRIGGER test_fail_outbox ON sdlc_outbox; DROP FUNCTION test_fail_outbox();",
    )
    .await
    .unwrap();
    let mut pending = vec![];
    for _ in 0..8 {
        let client = client.clone();
        let answer_url = answer_url.clone();
        let owner = owner.clone();
        let answer = answer.clone();
        pending.push(tokio::spawn(async move {
            post(&client, &answer_url, &owner, &answer, 200).await
        }));
    }
    let first = pending.remove(0).await.unwrap();
    for request in pending {
        assert_eq!(first, request.await.unwrap());
    }
    assert_eq!(count(&db, "sdlc_answers").await, 1);
    let mut changed = answer.clone();
    changed["comment"] = json!("Changed choice");
    post(&client, &answer_url, &owner, &changed, 409).await;
    for mode in ["multiple", "text", "single"] {
        let id = Uuid::new_v4();
        let option = Uuid::new_v4();
        let mut input = question.clone();
        input["question_id"] = json!(id);
        input["request_id"] = json!(Uuid::new_v4());
        input["mode"] = json!(mode);
        input["recommended_option_id"] = Value::Null;
        input["idempotency_key"] = json!(format!("question-{mode}"));
        input["options"] = if mode == "text" {
            json!([])
        } else {
            json!([{ "id":option,"label":"Choice","consequences":"Outcome","is_custom":mode=="single" }])
        };
        post(
            &client,
            &format!("{url}/clarifications"),
            "sdlc_pat_pm",
            &input,
            200,
        )
        .await;
        input["expected_question_version"] = json!(1);
        input["text"] = json!("Updated question");
        input["idempotency_key"] = json!(format!("question-{mode}-v2"));
        post(
            &client,
            &format!("{url}/clarifications"),
            "sdlc_pat_pm",
            &input,
            200,
        )
        .await;
        let mode_answer_url = format!("{url}/clarifications/{id}/answers");
        let selected = if mode == "text" {
            json!([])
        } else {
            json!([option])
        };
        let mut mode_answer = json!({"expected_question_version":1,"requirement_revision":1,"selected_option_ids":selected,"text":"Owner explanation","comment":null,"idempotency_key":format!("answer-{mode}")});
        post(&client, &mode_answer_url, &owner, &mode_answer, 409).await;
        mode_answer["expected_question_version"] = json!(2);
        if mode != "multiple" {
            let mut blank = mode_answer.clone();
            blank["text"] = json!("  ");
            post(&client, &mode_answer_url, &owner, &blank, 422).await;
        }
        post(&client, &mode_answer_url, &owner, &mode_answer, 200).await;
    }
    let cancel_id = Uuid::new_v4();
    let mut cancel_input = question.clone();
    cancel_input["question_id"] = json!(cancel_id);
    cancel_input["request_id"] = json!(Uuid::new_v4());
    cancel_input["idempotency_key"] = json!("question-cancel");
    post(
        &client,
        &format!("{url}/clarifications"),
        "sdlc_pat_pm",
        &cancel_input,
        200,
    )
    .await;
    post(
        &client,
        &format!("{url}/clarifications/{cancel_id}/cancel"),
        "sdlc_pat_pm",
        &json!({"fence":fence,"expected_question_version":1,"idempotency_key":"cancel"}),
        200,
    )
    .await;
    let confirm = json!({"content_hash":revision["content_hash"],"idempotency_key":"confirm-old"});
    post(
        &client,
        &format!("{url}/requirements/1/confirm"),
        &owner,
        &confirm,
        409,
    )
    .await;
    assert!(db.execute_unprepared(&format!("UPDATE issues SET status_id=(SELECT id FROM statuses WHERE name='Backlog' LIMIT 1) WHERE id='{task}'")).await.is_err());
    let backlog_status = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT id FROM statuses WHERE name='Backlog' LIMIT 1".to_string(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Uuid>("", "id")
        .unwrap();
    sql(&db,"INSERT INTO workflow_transitions(id,from_status_id,to_status_id,name) SELECT $1,status_id,$2,'SDLC gate test' FROM issues WHERE id=$3",vec![Uuid::new_v4().into(),backlog_status.into(),task.into()]).await;
    sql(&db, "UPDATE boards SET columns=$1 WHERE project_id=$2", vec![json!([{"id":backlog_status,"name":"Backlog","category":"todo","position":0,"wip_limit":null}]).into(),project.into()]).await;
    let response = client
        .patch(format!("{base}/api/v1/issues/{task}"))
        .bearer_auth(&owner)
        .json(&json!({"status_id":backlog_status}))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.text().await.unwrap();
    assert_eq!(status, 409, "legacy status gate: {body}");
    let mut final_publish = publish.clone();
    final_publish["expected_requirement_revision"] = json!(1);
    final_publish["idempotency_key"] = json!("rev-2");
    let final_revision = post(
        &client,
        &format!("{url}/requirements"),
        "sdlc_pat_pm",
        &final_publish,
        200,
    )
    .await;
    let mut final_confirm =
        json!({"content_hash":final_revision["content_hash"],"idempotency_key":"confirm-final"});
    let confirm_url = format!("{url}/requirements/2/confirm");
    post(&client, &confirm_url, &owner, &final_confirm, 409).await;
    for check in ["review", "contract"] {
        let evidence = json!({"fence":fence,"requirement_revision":2,"content_hash":final_revision["content_hash"],"check_id":check,"evidence_reference":format!("workflow://verified/{check}"),"idempotency_key":format!("evidence-{check}")});
        post(
            &client,
            &format!("{url}/evidence"),
            "sdlc_pat_pm",
            &evidence,
            403,
        )
        .await;
        post(
            &client,
            &format!("{url}/evidence"),
            "sdlc_pat_verifier",
            &evidence,
            200,
        )
        .await;
    }
    let context = client
        .get(format!("{url}/context"))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(context["permissions"]["can_confirm"], true);
    post(&client, &confirm_url, &operator, &final_confirm, 403).await;
    let (routing, routing_ready) = routing_policy::before(
        &db, &client, &base, project, task, &owner, &operator, &foreign,
    )
    .await;
    let mut stale_routing = final_confirm.clone();
    stale_routing["expected_routing_policy_version"] = json!(1);
    post(&client, &confirm_url, &owner, &stale_routing, 409).await;
    assert_eq!(count(&db, "sdlc_confirmations").await, 0);
    assert_eq!(count(&db, "sdlc_task_routing_snapshots").await, 0);
    final_confirm["expected_routing_policy_version"] = routing["version"].clone();
    let (confirmation, intent) =
        analysis_intent::confirm(&db, &client, &url, &owner, &final_confirm, task).await;
    let routing_snapshot = routing_policy::snapshot(&client, &base, task, &owner).await;
    assert_eq!(routing_snapshot["policy"], routing);
    analysis_intent::read(&client, &url, &foreign, 403).await;
    assert_eq!(confirmation["stage"], "Backlog");
    assert_eq!(confirmation["revision"], 2);
    let events = client
        .get(format!("{url}/events"))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(
        events["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["event_type"] == "clarification.answered")
            .count(),
        4
    );
    assert_eq!(count(&db, "sdlc_confirmations").await, 1);
    let metadata_url = format!("{url}/events?projection=metadata_v1");
    let (metadata_bytes, metadata_page) = metadata::get(&client, &metadata_url, &owner, 200).await;
    metadata::verify(&metadata_page, &events);
    assert_eq!(
        metadata_page["events"].as_array().unwrap().len(),
        events["events"].as_array().unwrap().len()
    );
    assert!(serde_json::to_vec(&events).unwrap().len() > 1_048_576);
    assert!(metadata_bytes.len() < 262_144);
    assert!(!String::from_utf8_lossy(&metadata_bytes).contains("private-result"));
    let types: std::collections::HashSet<_> = metadata_page["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["event_type"].as_str().unwrap())
        .collect();
    assert_eq!(types.len(), 9);
    metadata::save("tracker-metadata-all9.http.json", &metadata_bytes);
    assert_eq!(
        metadata::get(
            &client,
            &format!("{url}/events?limit=bad&limit=bad&max_bytes=0"),
            &owner,
            200
        )
        .await
        .1,
        events
    );
    for suffix in [
        "after=01",
        "after=%2B1",
        "after=-1",
        "after=9223372036854775808",
        "limit=0",
        "limit=101",
        "max_bytes=1023",
        "max_bytes=1048577",
        "limit=bad",
    ] {
        metadata::get(&client, &format!("{metadata_url}&{suffix}"), &owner, 422).await;
    }
    metadata::get(
        &client,
        &format!("{url}/events?projection=metadata_v2"),
        &owner,
        422,
    )
    .await;
    metadata::get(&client, &metadata_url, &foreign, 403).await;
    metadata::get(&client, &metadata_url, "local-token", 401).await;
    metadata::get(&client, &metadata_url, "sdlc_pat_pm", 200).await;
    metadata::get(&client, &metadata_url, "sdlc_pat_fleet", 403).await;
    metadata::get(&client, &metadata_url, &operator, 200).await;
    let previous = metadata_page["events"][0]["sequence"].as_str().unwrap();
    let (one, _) = metadata::get(
        &client,
        &format!("{metadata_url}&after={previous}&limit=1"),
        &owner,
        200,
    )
    .await;
    // HTTP checks measure the actual emitted bytes, including the envelope.
    let one_bound = one.len().max(1024);
    let (same, _) = metadata::get(
        &client,
        &format!("{metadata_url}&after={previous}&limit=1&max_bytes={one_bound}"),
        &owner,
        200,
    )
    .await;
    assert_eq!(one, same);
    let (prefix_bytes, prefix) = metadata::get(
        &client,
        &format!("{metadata_url}&max_bytes=1024"),
        &owner,
        200,
    )
    .await;
    assert!(prefix_bytes.len() <= 1024);
    assert_eq!(prefix["events"].as_array().unwrap().len(), 1);
    assert_eq!(prefix["has_more"], true);
    let mut after = "0".to_string();
    for event in metadata_page["events"].as_array().unwrap() {
        let (bytes, _) = metadata::get(
            &client,
            &format!("{metadata_url}&after={after}&limit=1"),
            &owner,
            200,
        )
        .await;
        if bytes.len() > 1024 {
            let budget = bytes.len();
            for bound in [budget - 1, budget, budget + 1] {
                let (actual, value) = metadata::get(
                    &client,
                    &format!("{metadata_url}&after={after}&limit=1&max_bytes={bound}"),
                    &owner,
                    if bound < budget { 422 } else { 200 },
                )
                .await;
                assert!(actual.len() <= bound);
                if bound < budget {
                    assert_eq!(value["code"], "metadata_budget_too_small");
                    assert_eq!(value["after"], after);
                } else {
                    assert_eq!(actual, bytes);
                }
            }
            break;
        }
        after = event["sequence"].as_str().unwrap().to_string();
    }
    assert!(
        db.execute_unprepared("UPDATE sdlc_requirements SET content_hash=repeat('a',64)")
            .await
            .is_err()
    );
    assert!(
        db.execute_unprepared("DELETE FROM sdlc_answers")
            .await
            .is_err()
    );
    sql(&db, "DELETE FROM project_members WHERE project_id=$1 AND user_id=(SELECT id FROM users WHERE central_sub='operator')", vec![project.into()]).await;
    metadata::get(&client, &metadata_url, &operator, 403).await;
    sql(
        &db,
        "UPDATE users SET is_active=false WHERE central_sub='pm'",
        vec![],
    )
    .await;
    metadata::get(&client, &metadata_url, "sdlc_pat_pm", 403).await;
    sql(
        &db,
        "UPDATE users SET is_active=true WHERE central_sub='pm'",
        vec![],
    )
    .await;
    assert_eq!(
        client
            .get(format!("{url}/context"))
            .bearer_auth(&operator)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let mismatch = infra::sdlc::PostgresSdlcRepository::connect(
        &database_url,
        SdlcConfig {
            reservation_scheduler_subject: "scheduler".into(),
            instance_id: "changed-instance".into(),
            orchestrator_subject: "fleet".into(),
            verifier_subject: "verifier".into(),
        },
    )
    .await;
    assert!(matches!(mismatch, Err(AppError::Conflict(_))));
    stop.send(()).unwrap();
    handle.await.unwrap();
    let (restarted, stop, handle) = start_tracker(config.clone()).await;
    let restart_url = format!("{restarted}/api/v1/issues/{task}/sdlc");
    let restart_metadata_url = format!("{restart_url}/events?projection=metadata_v1");
    assert_eq!(
        intent,
        analysis_intent::read(&client, &restart_url, &owner, 200).await
    );
    assert_eq!(count(&db, "sdlc_analysis_intents").await, 1);
    pm_credential_boundary::current(&client, &restarted, task, false).await;
    assert_eq!(
        metadata::get(&client, &restart_metadata_url, &owner, 200)
            .await
            .0,
        metadata_bytes
    );
    assert_eq!(
        confirmation,
        post(
            &client,
            &format!("{restart_url}/requirements/2/confirm"),
            &owner,
            &final_confirm,
            200
        )
        .await
    );
    assert_eq!(
        first,
        post(
            &client,
            &format!("{restart_url}/clarifications/{question_id}/answers"),
            &owner,
            &answer,
            200
        )
        .await
    );
    assert_eq!(
        events,
        client
            .get(format!("{restart_url}/events"))
            .bearer_auth(&owner)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    );
    // A new PM assignment cannot reopen a revision already queued for Analysis.
    post(&client, &format!("{restart_url}/assignment"), "sdlc_pat_fleet",
        &json!({"assignment":replacement,"expected_assignment_version":1,"idempotency_key":"replacement"}), 409).await;
    pm_credential_boundary::current(&client, &restarted, task, false).await;
    let replacement_fence = json!({"assignment_id":replacement.assignment_id,
        "execution_id":replacement.execution_id,"agent_id":replacement.agent_id,"assignment_version":2});
    let mut new_revision = publish.clone();
    new_revision["fence"] = replacement_fence.clone();
    new_revision["expected_requirement_revision"] = json!(2);
    new_revision["document"]["scope"] = json!(["changed later scope"]);
    new_revision["idempotency_key"] = json!("revision-after-replacement");
    post(
        &client,
        &format!("{restart_url}/requirements"),
        "sdlc_pat_replacement",
        &new_revision,
        409,
    )
    .await;
    new_revision["fence"] = fence.clone();
    new_revision["idempotency_key"] = json!("late-original-pm-revision");
    post(
        &client,
        &format!("{restart_url}/requirements"),
        "sdlc_pat_pm",
        &new_revision,
        409,
    )
    .await;
    let mut changed_question = question.clone();
    changed_question["fence"] = replacement_fence;
    changed_question["requirement_revision"] = json!(3);
    changed_question["expected_question_version"] = json!(1);
    changed_question["text"] = json!("later changed question");
    changed_question["idempotency_key"] = json!("question-after-replacement");
    post(
        &client,
        &format!("{restart_url}/clarifications"),
        "sdlc_pat_replacement",
        &changed_question,
        409,
    )
    .await;
    let changed = metadata::get(&client, &restart_metadata_url, &owner, 200)
        .await
        .1;
    assert_eq!(changed, metadata_page);
    assert_eq!(
        &changed["events"].as_array().unwrap()[..metadata_page["events"].as_array().unwrap().len()],
        metadata_page["events"].as_array().unwrap()
    );
    metadata_pagination_and_blockers(&client, &restart_metadata_url, &owner, &db, task).await;
    routing_policy::after(
        &db,
        &client,
        &restarted,
        project,
        task,
        &owner,
        &foreign,
        &routing_snapshot,
        &routing_ready,
    )
    .await;
    let (reserved_task, reserved_head) = analysis_reservation::check(
        &db,
        &client,
        &restarted,
        project,
        task,
        &owner,
        &routing_ready,
    )
    .await;
    unavailable.store(true, Ordering::SeqCst);
    pm_credential_boundary::unavailable(&client, &restarted, task).await;
    metadata::get(&client, &restart_metadata_url, &owner, 503).await;
    assert_eq!(
        client
            .get(format!("{restart_url}/context"))
            .bearer_auth(&owner)
            .send()
            .await
            .unwrap()
            .status(),
        503
    );
    stop.send(()).unwrap();
    handle.await.unwrap();
    unavailable.store(false, Ordering::SeqCst);
    let (reservation_restart, stop, handle) = start_tracker(config).await;
    let persisted = client
        .get(format!(
            "{reservation_restart}/api/v1/issues/{reserved_task}/sdlc/analysis-reservation"
        ))
        .bearer_auth("sdlc_pat_reservation_reader")
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(persisted["current"], reserved_head);
    assert_eq!(persisted["lease_state"], "expired");
    assert_eq!(persisted["reconciliation_needed"], true);
    assert_eq!(persisted["capacity_held"], true);
    println!("ANALYSIS_RESERVATION restart durable readback passed");
    stop.send(()).unwrap();
    handle.await.unwrap();
    auth_handle.abort();
}

async fn append_event(db: &DatabaseConnection, task: Uuid, kind: &str, payload: Value) -> i64 {
    db.query_one(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,$3,$4) RETURNING sequence",
        vec![Uuid::new_v4().into(), task.into(), kind.into(), payload.into()]))
        .await.unwrap().unwrap().try_get("", "sequence").unwrap()
}

async fn metadata_pagination_and_blockers(
    client: &Client,
    url: &str,
    owner: &str,
    db: &DatabaseConnection,
    task: Uuid,
) {
    let row = db.query_one(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "SELECT payload FROM sdlc_outbox WHERE task_id=$1 AND event_type='pm.assigned' ORDER BY sequence LIMIT 1",
        vec![task.into()])).await.unwrap().unwrap();
    let valid_payload: Value = row.try_get("", "payload").unwrap();
    sql(db, "INSERT INTO sdlc_outbox(sequence,event_id,task_id,event_type,payload) OVERRIDING SYSTEM VALUE
        SELECT nextval(pg_get_serial_sequence('sdlc_outbox','sequence')) + 0*nextval(pg_get_serial_sequence('sdlc_outbox','sequence')),
        gen_random_uuid(),$1,'pm.assigned',$2 FROM generate_series(1,121)",
        vec![task.into(), valid_payload.clone().into()]).await;
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT sequence,event_id FROM sdlc_outbox WHERE task_id=$1 ORDER BY sequence",
            vec![task.into()],
        ))
        .await
        .unwrap();
    let expected: Vec<_> = rows
        .iter()
        .map(|r| {
            (
                r.try_get::<i64>("", "sequence").unwrap().to_string(),
                r.try_get::<Uuid>("", "event_id").unwrap().to_string(),
            )
        })
        .collect();
    assert!(expected.len() > 100);
    assert!(
        expected
            .windows(2)
            .any(|pair| pair[1].0.parse::<i64>().unwrap() > pair[0].0.parse::<i64>().unwrap() + 1)
    );
    let first = metadata::get(client, url, owner, 200).await.1;
    assert_eq!(first["events"].as_array().unwrap().len(), 100);
    assert_eq!(first["has_more"], true);
    let second = metadata::get(
        client,
        &format!("{url}&after={}", first["next_after"].as_str().unwrap()),
        owner,
        200,
    )
    .await
    .1;
    assert_eq!(
        second["events"].as_array().unwrap().len(),
        expected.len() - 100
    );
    assert_eq!(second["has_more"], false);
    let mut received = vec![];
    let mut after = "0".to_string();
    loop {
        let (bytes, page) = metadata::get(
            client,
            &format!("{url}&after={after}&limit=7&max_bytes=4096"),
            owner,
            200,
        )
        .await;
        assert!(bytes.len() <= 4096);
        for event in page["events"].as_array().unwrap() {
            received.push((
                event["sequence"].as_str().unwrap().to_string(),
                event["event_id"].as_str().unwrap().to_string(),
            ));
        }
        assert_ne!(after, page["next_after"].as_str().unwrap());
        after = page["next_after"].as_str().unwrap().to_string();
        if page["has_more"] == false {
            break;
        }
    }
    assert_eq!(received, expected);
    let empty = metadata::get(client, &format!("{url}&after={after}"), owner, 200)
        .await
        .1;
    assert_eq!(empty["next_after"], after);
    assert_eq!(empty["has_more"], false);
    assert_eq!(
        metadata::get(
            client,
            &format!("{url}&after=9223372036854775807"),
            owner,
            200
        )
        .await
        .1["next_after"],
        "9223372036854775807"
    );

    let good = append_event(db, task, "pm.assigned", valid_payload.clone()).await;
    let bad = append_event(
        db,
        task,
        "unsupported-private-result",
        valid_payload.clone(),
    )
    .await;
    append_event(db, task, "pm.assigned", valid_payload.clone()).await;
    let prefix = metadata::get(client, &format!("{url}&after={after}"), owner, 200)
        .await
        .1;
    assert_eq!(prefix["next_after"], good.to_string());
    assert_eq!(prefix["has_more"], true);
    assert_eq!(prefix["events"].as_array().unwrap().len(), 1);
    let (bytes, blocked) = metadata::get(client, &format!("{url}&after={good}"), owner, 409).await;
    assert!(bytes.len() < 1024);
    assert_eq!(blocked["blocked_sequence"], bad.to_string());
    assert_eq!(blocked["after"], good.to_string());
    assert_eq!(blocked["code"], "metadata_source_invalid");
    assert!(!String::from_utf8_lossy(&bytes).contains("private-result"));
    let mut cursor = bad;
    for mutation in 0..5 {
        let mut payload = valid_payload.clone();
        match mutation {
            0 => payload["result"]["execution_id"] = json!(Uuid::nil()),
            1 => {
                payload["result"]
                    .as_object_mut()
                    .unwrap()
                    .remove("assignment_id");
            }
            2 => payload["root_task_id"] = json!(Uuid::new_v4()),
            3 => payload["result"]["version"] = json!(9007199254740992i64),
            _ => payload["contract_version"] = json!(2),
        }
        // Each corrupt row is followed by a valid row, proving it is not silently filtered.
        let sequence = append_event(db, task, "pm.assigned", payload).await;
        let following = append_event(db, task, "pm.assigned", valid_payload.clone()).await;
        let start = sequence - 1;
        let blocked = metadata::get(client, &format!("{url}&after={start}"), owner, 409)
            .await
            .1;
        assert_eq!(blocked["after"], start.to_string());
        assert_eq!(blocked["blocked_sequence"], sequence.to_string());
        assert_eq!(blocked["code"], "metadata_source_invalid");
        cursor = following;
    }
    assert!(cursor > bad);
    let mut conflicting = db.query_one(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "SELECT payload FROM sdlc_outbox WHERE task_id=$1 AND event_type='requirements.evidence_recorded' ORDER BY sequence LIMIT 1",
        vec![task.into()])).await.unwrap().unwrap().try_get::<Value>("", "payload").unwrap();
    conflicting["result"]["content_hash"] = json!("0".repeat(64));
    let invalid_hash = append_event(db, task, "requirements.evidence_recorded", conflicting).await;
    assert_eq!(
        metadata::get(
            client,
            &format!("{url}&after={}", invalid_hash - 1),
            owner,
            409
        )
        .await
        .1["code"],
        "metadata_source_invalid"
    );

    // PostgreSQL accepts this compressed synthetic identity; JSON escaping exceeds the hard cap.
    let huge_owner = "\u{1}".repeat(190_000);
    let huge_user = Uuid::new_v4();
    let huge_task = Uuid::new_v4();
    let project = Uuid::parse_str(valid_payload["project_id"].as_str().unwrap()).unwrap();
    sql(
        db,
        "INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active)
        VALUES($1,'metadata-size@example.test','metadata-size','metadata-size','!',$2,true)",
        vec![huge_user.into(), huge_owner.clone().into()],
    )
    .await;
    sql(db, "INSERT INTO issues(id,project_id,key,issue_type,status_id,summary,reporter_id,priority,labels,position,time_spent_seconds)
        SELECT $1,project_id,'SDLC-999999','task',status_id,'metadata-size',$2,'medium','[]',0,0 FROM issues WHERE id=$3",
        vec![huge_task.into(), huge_user.into(), task.into()]).await;
    let state = TaskState {
        tracker_instance_id: valid_payload["tracker_instance_id"]
            .as_str()
            .unwrap()
            .into(),
        project_id: project,
        task_id: huge_task,
        root_task_id: huge_task,
        owner_subject: huge_owner,
        stage: Stage::Draft,
        confirmation_revision: None,
        assignment: None,
        questions: vec![],
        revisions: vec![],
        confirmations: vec![],
        evidence: vec![],
    };
    sql(db, "INSERT INTO sdlc_tasks(task_id,tracker_instance_id,project_id,root_task_id,owner_subject,state)
        VALUES($1,$2,$3,$1,$4,$5)", vec![huge_task.into(), state.tracker_instance_id.clone().into(),
        project.into(), state.owner_subject.clone().into(), serde_json::to_value(&state).unwrap().into()]).await;
    let payload = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
        "project_id":project,"task_id":huge_task,"root_task_id":huge_task,"owner_subject":state.owner_subject,
        "stage":"Draft","requirement_revision":null,"result":state});
    append_event(db, huge_task, "task.bound", payload).await;
    let huge_url = url.replace(&task.to_string(), &huge_task.to_string());
    let (bytes, impossible) =
        metadata::get(client, &format!("{huge_url}&max_bytes=1048576"), owner, 409).await;
    assert!(bytes.len() <= 1024);
    assert_eq!(impossible["code"], "metadata_event_unrepresentable");
    assert_eq!(impossible["after"], "0");
    assert!(impossible["required_bytes"].as_u64().unwrap() > 1_048_576);
}
