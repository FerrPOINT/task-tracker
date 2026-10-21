//! Ordinary signed identities + real Router/PostgreSQL. Only external Auth/Admin HTTP are fixtures.
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use p256::{
    SecretKey,
    elliptic_curve::sec1::ToEncodedPoint,
    pkcs8::{EncodePrivateKey, LineEnding},
};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(Clone)]
struct Authority {
    issuer: String,
    key: Arc<jsonwebtoken::DecodingKey>,
    jwks: Value,
    users: Vec<Uuid>,
    registry: Uuid,
    spaces: Vec<Uuid>,
    archived: Arc<AtomicBool>,
    direct: Arc<AtomicBool>,
    linked: Arc<AtomicBool>,
    revoked: Arc<AtomicBool>,
    unavailable: Arc<AtomicBool>,
}
fn reference(a: &Authority, id: Uuid) -> Value {
    json!({"registry_instance_id":a.registry,"space_id":id})
}
async fn profile(
    State(a): State<Authority>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::ES256);
    validation.set_issuer(&[&a.issuer]);
    validation.set_audience(&["sdlc"]);
    let claims = jsonwebtoken::decode::<Value>(token, &a.key, &validation)
        .map_err(|_| StatusCode::UNAUTHORIZED)?
        .claims;
    let subject = claims["sub"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !a.users.contains(&subject) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(Json(
        json!({"id":subject,"role":if subject==a.users[5]{"service_account"}else{"member"},"status":"active","email":format!("{subject}@example.test"),"display_name":"Same member"}),
    ))
}
async fn admission(
    State(a): State<Authority>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    assert_eq!(
        headers.get("authorization").unwrap(),
        "Bearer sdlc_pat_synthetic-space-reader"
    );
    if a.unavailable.load(Ordering::SeqCst) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    assert_eq!(request["schema_version"], 1);
    assert_eq!(request["identity"]["issuer"], a.issuer);
    let index = request["identity"]["subject"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .and_then(|id| a.users.iter().position(|u| *u == id))
        .unwrap();
    let mut selected = match index {
        0 => vec![a.spaces[0]],
        1 => {
            if a.revoked.load(Ordering::SeqCst) {
                vec![]
            } else {
                vec![a.spaces[1]]
            }
        }
        2 => vec![a.spaces[2]],
        3 => vec![a.spaces[1], a.spaces[2]],
        _ => vec![],
    };
    if index == 0 && a.direct.load(Ordering::SeqCst) {
        selected.push(a.spaces[1]);
    }
    let archived = a.archived.load(Ordering::SeqCst);
    let mut inherited = Vec::new();
    if index == 0 {
        for child in &a.spaces[1..] {
            if *child == a.spaces[1] && !a.linked.load(Ordering::SeqCst) {
                continue;
            }
            inherited.push(json!({"space":reference(&a,*child),"source":{"space":reference(&a,a.spaces[0]),"revision":if archived{2}else{1},"state":if archived{"archived"}else{"active"}}}));
        }
    }
    let confirmed=request["spaces"].as_array().unwrap().iter().filter_map(|space|{
        let id=space["space_id"].as_str().and_then(|id|Uuid::parse_str(id).ok())?;
        if !a.spaces.contains(&id) || space["registry_instance_id"]!=a.registry.to_string(){return None;}
        Some(json!({"space":space,"revision":if id==a.spaces[0] && archived{2}else{1},"state":if id==a.spaces[0] && archived{"archived"}else{"active"}}))
    }).collect::<Vec<_>>();
    let now = shared::now().timestamp();
    Ok(Json(
        json!({"schema_version":2,"identity":request["identity"],"identity_active":true,
        "policy_revision":if a.direct.load(Ordering::SeqCst)||a.revoked.load(Ordering::SeqCst){2}else{1},
        "policy":{"mode":"selected","spaces":selected.into_iter().map(|id|reference(&a,id)).collect::<Vec<_>>()},
        "inheritance":{"revision":if a.linked.load(Ordering::SeqCst){1}else{2},"spaces":inherited},
        "validity":{"observed_at":now,"valid_until":now+3},"spaces":confirmed}),
    ))
}
fn token(secret: &SecretKey, issuer: &str, subject: Uuid) -> String {
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256);
    header.kid = Some("space-test".into());
    jsonwebtoken::encode(&header,&json!({"sub":subject,"iss":issuer,"aud":"sdlc","exp":shared::now().timestamp()+3600,
        "iat":shared::now().timestamp(),"sid":"ordinary-session","email":format!("{subject}@example.test")}),
        &jsonwebtoken::EncodingKey::from_ec_pem(secret.to_pkcs8_pem(LineEnding::LF).unwrap().as_bytes()).unwrap()).unwrap()
}
async fn sql(db: &DatabaseConnection, text: &str, values: Vec<sea_orm::Value>) {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        text,
        values,
    ))
    .await
    .unwrap();
}
async fn read(client: &reqwest::Client, base: &str, path: &str, token: &str, status: u16) -> Value {
    let response = client
        .get(format!("{base}/api/v1/{path}"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    let actual = response.status().as_u16();
    let body = response.text().await.unwrap();
    assert_eq!(actual, status, "{path}: {body}");
    serde_json::from_str(&body).unwrap_or(Value::Null)
}
async fn directory(client: &reqwest::Client, base: &str, token: &str, mut expected: Vec<Uuid>) {
    expected.sort();
    let result = read(client, base, "sdlc/project-access", token, 200).await;
    assert_eq!(result["project_ids"], json!(expected));
    let response = client
        .get(format!("{base}/api/v2/sdlc/project-access"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let text = response.text().await.unwrap();
    assert_eq!(status.as_u16(), 200, "bounded v2 scope: {text}");
    let proof: shared::project_access::ProjectAccessV2 = serde_json::from_str(&text).unwrap();
    assert!(proof.valid());
    assert_eq!(proof.project_ids, expected);
    assert!(proof.validity.valid_until - proof.validity.observed_at <= 3);
    let mut seen = Vec::<Uuid>::new();
    let mut after = None;
    for _ in 0..=expected.len() {
        let query = after.map(|id| format!("&after={id}")).unwrap_or_default();
        let result = read(
            client,
            base,
            &format!("sdlc/project-directory?limit=1{query}"),
            token,
            200,
        )
        .await;
        let rows = result["projects"].as_array().unwrap();
        assert!(rows.len() <= 1);
        for row in rows {
            let id = serde_json::from_value(row["id"].clone()).unwrap();
            assert!(expected.contains(&id));
            seen.push(id);
        }
        after = serde_json::from_value::<Option<Uuid>>(result["next_cursor"].clone()).unwrap();
        if after.is_none() {
            assert_eq!(seen, expected);
            return;
        }
    }
    panic!("directory did not terminate");
}
async fn space_catalog(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    authority: &Authority,
    space: usize,
    mut expected: Vec<Uuid>,
    status: u16,
) {
    let query = format!(
        "registry_instance_id={}&space_id={}",
        authority.registry, authority.spaces[space]
    );
    let response = client
        .get(format!("{base}/api/v2/projects?{query}&limit=1"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    let actual = response.status().as_u16();
    let body = response.text().await.unwrap();
    assert_eq!(actual, status, "namespace catalog: {body}");
    if status != 200 {
        return;
    }
    let first: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(first["total"], expected.len());
    let mut seen = first["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| serde_json::from_value::<Uuid>(row["id"].clone()).unwrap())
        .collect::<Vec<_>>();
    for offset in 1..expected.len() {
        let response = client
            .get(format!(
                "{base}/api/v2/projects?{query}&limit=1&offset={offset}"
            ))
            .bearer_auth(token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 200);
        let page: Value = response.json().await.unwrap();
        assert_eq!(page["total"], expected.len());
        seen.extend(
            page["projects"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| serde_json::from_value::<Uuid>(row["id"].clone()).unwrap()),
        );
    }
    seen.sort();
    expected.sort();
    assert_eq!(
        seen, expected,
        "namespace filtering must precede count and pagination"
    );
}

async fn draft(
    client: &reqwest::Client,
    base: &str,
    project: Uuid,
    token: &str,
    status: u16,
    key: &str,
) {
    let response=client.post(format!("{base}/api/v1/projects/{project}/sdlc/drafts")).bearer_auth(token)
        .json(&json!({"title":"Namespace guarded draft","description":"Native role remains additional","idempotency_key":key}))
        .send().await.unwrap();
    let actual = response.status().as_u16();
    let text = response.text().await.unwrap();
    assert_eq!(actual, status, "draft: {text}");
}
#[tokio::test]
#[ignore = "dedicated unified_pdlc4_sdlc_space_test database only"]
async fn ordinary_spaces_guard_sdlc_catalog_resources_roles_and_original_deadlines() {
    let url =
        std::env::var("TT_SDLC_SPACES_TEST_DATABASE_URL").expect("dedicated disposable PostgreSQL");
    let db = Database::connect(&url).await.unwrap();
    let name = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT current_database() AS name",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<String>("", "name")
        .unwrap();
    assert_eq!(
        name, "unified_pdlc4_sdlc_space_test",
        "refuse working databases"
    );
    let instance = Uuid::new_v4();
    let secret = SecretKey::from_slice(&[9; 32]).unwrap();
    let point = secret.public_key().to_encoded_point(false);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let a = Authority {
        issuer: issuer.clone(),
        key: Arc::new(
            jsonwebtoken::DecodingKey::from_ec_components(
                &URL_SAFE_NO_PAD.encode(point.x().unwrap()),
                &URL_SAFE_NO_PAD.encode(point.y().unwrap()),
            )
            .unwrap(),
        ),
        jwks: json!({"keys":[{"kid":"space-test","kty":"EC","crv":"P-256","alg":"ES256","use":"sig",
            "x":URL_SAFE_NO_PAD.encode(point.x().unwrap()),"y":URL_SAFE_NO_PAD.encode(point.y().unwrap())}]}),
        users: (0..6).map(|_| Uuid::new_v4()).collect(),
        registry: Uuid::new_v4(),
        spaces: (0..3).map(|_| Uuid::new_v4()).collect(),
        archived: Arc::new(AtomicBool::new(false)),
        direct: Arc::new(AtomicBool::new(false)),
        linked: Arc::new(AtomicBool::new(true)),
        revoked: Arc::new(AtomicBool::new(false)),
        unavailable: Arc::new(AtomicBool::new(false)),
    };
    let auth = Router::new()
        .route(
            "/jwks",
            get(|State(a): State<Authority>| async move { Json(a.jwks) }),
        )
        .route("/auth/me", get(profile))
        .route("/admission", post(admission))
        .with_state(a.clone());
    let authority = tokio::spawn(async move { axum::serve(listener, auth).await.unwrap() });
    unsafe {
        std::env::set_var("TT_AUTH__CENTRAL_JWKS_URI", format!("{issuer}/jwks"));
        std::env::set_var("TT_AUTH__CENTRAL_ISSUER", &issuer);
        std::env::set_var("TT_SPACES__ADMISSION_URL", format!("{issuer}/admission"));
        std::env::set_var(
            "TT_SPACES__ADMISSION_TOKEN",
            "sdlc_pat_synthetic-space-reader",
        );
        std::env::set_var("TT_NAMESPACE__INSTANCE_ID", instance.to_string());
        std::env::set_var("TASKTRACKER_SDLC__INSTANCE_ID", instance.to_string());
        std::env::set_var(
            "TASKTRACKER_SDLC__ORCHESTRATOR_SUBJECT",
            a.users[5].to_string(),
        );
    }
    let config = Arc::new(shared::AppConfig {
        database: shared::DatabaseConfig {
            url: url.clone(),
            ..Default::default()
        },
        server: shared::ServerConfig {
            address: "127.0.0.1".into(),
            port: 0,
            general_rate_burst: 1000,
            ..Default::default()
        },
        ..Default::default()
    });
    infra::run_migrations(config.database.clone())
        .await
        .unwrap();
    sql(
        &db,
        "INSERT INTO sdlc_instance(singleton,instance_id) VALUES(true,$1)",
        vec![instance.to_string().into()],
    )
    .await;
    let locals = (0..6).map(|_| Uuid::new_v4()).collect::<Vec<_>>();
    for (index, (subject, id)) in a.users.iter().zip(&locals).enumerate() {
        sql(&db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active,is_system_admin) VALUES($1,$2,$3,'Same member','!',$4,true,false)",
            vec![(*id).into(),format!("{subject}@example.test").into(),format!("member-{index}").into(),subject.to_string().into()]).await;
    }
    for (index, space) in a.spaces.iter().enumerate() {
        sql(&db,"INSERT INTO tracker_space_catalog(registry_instance_id,space_id,admin_revision,state) VALUES($1,$2,1,'active')",
            vec![a.registry.into(),(*space).into()]).await;
        assert!(index < 3);
    }
    let projects = [Uuid::new_v4(), Uuid::new_v4()];
    let tasks = [Uuid::new_v4(), Uuid::new_v4()];
    for index in 0..2 {
        let board = Uuid::new_v4();
        let key = if index == 0 { "ALPHA" } else { "BETA" };
        sql(&db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,$2,'Same project',$3,$4)",
            vec![projects[index].into(),key.into(),locals[4].into(),board.into()]).await;
        sql(
            &db,
            "INSERT INTO boards(id,project_id,name,columns) VALUES($1,$2,'Board','[]')",
            vec![board.into(), projects[index].into()],
        )
        .await;
        sql(&db,"INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id) VALUES($1,$2,$3)",
            vec![projects[index].into(),a.registry.into(),a.spaces[index+1].into()]).await;
        sql(&db,"INSERT INTO issues(id,project_id,key,issue_type,status_id,summary,reporter_id,priority,labels,position,time_spent_seconds) SELECT $1,$2,$3,'task',id,'Protected content',$4,'medium','[]',0,0 FROM statuses WHERE is_default LIMIT 1",
            vec![tasks[index].into(),projects[index].into(),format!("{key}-1").into(),locals[4].into()]).await;
        let state = domain::sdlc::TaskState {
            status: None,
            retained_fields: Default::default(),
            draft_submode: None,
            tracker_instance_id: instance.to_string(),
            project_id: projects[index],
            task_id: tasks[index],
            root_task_id: tasks[index],
            owner_subject: a.users[4].to_string(),
            stage: domain::sdlc::Stage::Draft,
            confirmation_revision: None,
            assignment: None,
            questions: vec![],
            revisions: vec![],
            confirmations: vec![],
            evidence: vec![],
        };
        sql(&db,"INSERT INTO sdlc_tasks(task_id,tracker_instance_id,project_id,root_task_id,owner_subject,state) VALUES($1,$2,$3,$1,$4,$5)",
            vec![tasks[index].into(),instance.to_string().into(),projects[index].into(),a.users[4].to_string().into(),serde_json::to_value(state).unwrap().into()]).await;
    }
    for index in 0..2 {
        let command = json!({"schema_version":1,"namespace":{"registry_instance_id":a.registry,"namespace_id":a.spaces[index+1]},
            "resource":{"kind":"tracker_project","instance_id":instance,"resource_id":projects[index]},
            "operation_id":Uuid::new_v4(),"generation":1,"state":"active"});
        sql(&db,"INSERT INTO tracker_namespace_bindings(resource_id,registry_instance_id,namespace_id,generation,state,command) VALUES($1,$2,$3,1,'active',$4)",
            vec![projects[index].into(),a.registry.into(),a.spaces[index+1].into(),command.into()]).await;
    }
    // Keep native memberships deliberately pointing at the wrong namespaces.
    for (user, project) in [
        (locals[1], projects[1]),
        (locals[2], projects[0]),
        (locals[0], projects[0]),
        (locals[5], projects[0]),
    ] {
        sql(
            &db,
            "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
            vec![project.into(), user.into()],
        )
        .await;
    }
    // Historical repository B remains owned by B even when its project is visible through A.
    let now = shared::now().to_utc();
    let source = domain::sdlc_source::SourcePolicySpec {
        forge_instance_id: Uuid::new_v4(),
        repository_id: Uuid::new_v4(),
        base_ref: "refs/heads/main".into(),
        base_commit: "a".repeat(40),
    };
    let verification = domain::sdlc_source::VerifiedSource {
        namespace: shared::resource_context::NamespaceRef {
            registry_instance_id: a.registry,
            namespace_id: a.spaces[2],
        },
        forge_instance_id: source.forge_instance_id,
        repository_id: source.repository_id,
        public_name: "protected/source".into(),
        base_ref: source.base_ref.clone(),
        base_commit: source.base_commit.clone(),
        default_ref: "refs/heads/main".into(),
        protected_refs: Vec::new(),
        observed_at: now,
    };
    let mut source_policy = domain::sdlc_source::ProjectSourcePolicy {
        contract_version: 1,
        tracker_instance_id: instance.to_string(),
        project_id: projects[0],
        source_revision: 1,
        source_hash: String::new(),
        source,
        verification,
        author_subject: a.users[4].to_string(),
        created_at: now,
    };
    source_policy.source_hash = app::sdlc_source::sealed_hash(&source_policy).unwrap();
    sql(&db,"INSERT INTO sdlc_source_policy_revisions(project_id,source_revision,source_hash,payload) VALUES($1,1,$2,$3)",
        vec![projects[0].into(),source_policy.source_hash.clone().into(),serde_json::to_value(&source_policy).unwrap().into()]).await;
    sql(
        &db,
        "INSERT INTO sdlc_source_policy_heads(project_id,source_revision) VALUES($1,1)",
        vec![projects[0].into()],
    )
    .await;
    let mut current_source_policy = source_policy.clone();
    current_source_policy.source_revision = 2;
    current_source_policy.verification.namespace.namespace_id = a.spaces[1];
    current_source_policy.source_hash =
        app::sdlc_source::sealed_hash(&current_source_policy).unwrap();
    sql(&db,"INSERT INTO sdlc_source_policy_revisions(project_id,source_revision,source_hash,payload) VALUES($1,2,$2,$3)",
        vec![projects[0].into(),current_source_policy.source_hash.clone().into(),serde_json::to_value(&current_source_policy).unwrap().into()]).await;
    sql(
        &db,
        "UPDATE sdlc_source_policy_heads SET source_revision=2 WHERE project_id=$1",
        vec![projects[0].into()],
    )
    .await;
    sql(
        &db,
        "UPDATE tracker_space_admission_state SET active=true",
        vec![],
    )
    .await;
    // The SQL execution-start port freezes actual resource owners, never all
    // memberships, and does not relabel the Forge source as a Wiki owner.
    for foreign_source in [false, true] {
        use sea_orm::TransactionTrait;
        let tx = db.begin().await.unwrap();
        let now = shared::now().timestamp();
        let active = if foreign_source {
            vec![reference(&a, a.spaces[1])]
        } else {
            a.spaces
                .iter()
                .map(|id| reference(&a, *id))
                .collect::<Vec<_>>()
        };
        let human = json!({"schema_version":1,"kind":"human","user_id":locals[0],
            "identity":{"issuer":issuer,"subject":a.users[0].to_string()},"active_spaces":active,
            "readable_spaces":active,"policy_revision":1,"writable":true,"valid_until":now+30});
        tx.execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT set_config('pdlc.tracker.write_actor',$1,true)",
            vec![human.to_string().into()],
        ))
        .await
        .unwrap();
        let selected = if foreign_source {
            &source_policy
        } else {
            &current_source_policy
        };
        tx.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,"INSERT INTO sdlc_task_source_selections(task_id,project_id,source_revision,policy_hash) VALUES($1,$2,$3,$4)",
            vec![tasks[0].into(),projects[0].into(),selected.source_revision.into(),selected.source_hash.clone().into()])).await.unwrap();
        let assignment = domain::sdlc::PmAssignment {
            assignment_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            agent_id: Uuid::new_v4(),
            version: 1,
            machine_subject: a.users[5].to_string(),
        };
        let start = tx
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "INSERT INTO sdlc_assignments(task_id,version,payload) VALUES($1,1,$2)",
                vec![
                    tasks[0].into(),
                    serde_json::to_value(&assignment).unwrap().into(),
                ],
            ))
            .await;
        if foreign_source {
            assert!(
                start.is_err(),
                "project membership cannot start an execution in a foreign repository owner"
            );
        } else {
            start.unwrap();
            let row=tx.query_one(Statement::from_sql_and_values(DatabaseBackend::Postgres,"SELECT spaces,required_wiki_space FROM tracker_execution_space_grants WHERE assignment_id=$1",vec![assignment.assignment_id.into()])).await.unwrap().unwrap();
            assert_eq!(
                row.try_get::<Value>("", "spaces").unwrap(),
                json!([reference(&a, a.spaces[1])]),
                "unrelated account memberships are not frozen"
            );
            assert!(
                row.try_get::<Option<Value>>("", "required_wiki_space")
                    .unwrap()
                    .is_none()
            );
        }
        tx.rollback().await.unwrap();
    }
    {
        use sea_orm::TransactionTrait;
        let tx = db.begin().await.unwrap();
        let now = shared::now().timestamp();
        let identity = json!({"issuer":issuer,"subject":a.users[5].to_string()});
        let validity = json!({"observed_at":now,"valid_until":now+3});
        let caller = json!({"schema_version":1,"identity":identity,"validity":validity,"writable":true,"scopes":["task-tracker:read","task-tracker:write"]});
        let scope = json!({"schema_version":1,"task_id":tasks[0],"project_id":projects[0],"identity":identity,"validity":validity,"writable":true});
        tx.execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT set_config('pdlc.tracker.native_caller',$1,true)",
            vec![caller.to_string().into()],
        ))
        .await
        .unwrap();
        tx.execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT tracker_bind_native_task($1)",
            vec![scope.into()],
        ))
        .await
        .unwrap();
        tx.execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE sdlc_tasks SET state=state WHERE task_id=$1",
            vec![tasks[0].into()],
        ))
        .await
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(3200)).await;
        assert!(
            tx.commit().await.is_err(),
            "deferred native write rejects an expired original clock"
        );
    }
    {
        use sea_orm::TransactionTrait;
        let tx = db.begin().await.unwrap();
        let now = shared::now().timestamp();
        let identity = json!({"issuer":issuer,"subject":a.users[5].to_string()});
        let validity = json!({"observed_at":now,"valid_until":now+30});
        let caller = json!({"schema_version":1,"identity":identity,"validity":validity,"writable":true,"scopes":["task-tracker:write"]});
        let scope = json!({"schema_version":1,"task_id":tasks[0],"project_id":projects[0],"identity":identity,"validity":validity,"writable":true});
        tx.execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT set_config('pdlc.tracker.native_caller',$1,true)",
            vec![caller.to_string().into()],
        ))
        .await
        .unwrap();
        tx.execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT tracker_bind_native_task($1)",
            vec![scope.into()],
        ))
        .await
        .unwrap();
        assert!(
            tx.execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "UPDATE sdlc_tasks SET state=state WHERE task_id=$1",
                vec![tasks[1].into()]
            ))
            .await
            .is_err(),
            "task capability cannot write a neighboring aggregate"
        );
        tx.rollback().await.unwrap();
    }
    let (ready_tx, ready_rx) = oneshot::channel();
    let (stop_tx, stop_rx) = oneshot::channel();
    let server = tokio::spawn(server::run(config, ready_tx, stop_rx));
    let address = tokio::time::timeout(std::time::Duration::from_secs(30), ready_rx)
        .await
        .unwrap()
        .unwrap();
    let base = format!("http://{address}");
    let client = reqwest::Client::new();
    let tokens = a
        .users
        .iter()
        .map(|user| token(&secret, &issuer, *user))
        .collect::<Vec<_>>();
    let mut machine_header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256);
    machine_header.kid = Some("space-test".into());
    let machine_token=jsonwebtoken::encode(&machine_header,&json!({"sub":a.users[5],"iss":issuer,"aud":"sdlc",
        "exp":shared::now().timestamp()+3600,"iat":shared::now().timestamp(),"scopes":["task-tracker:read","task-tracker:write"],
        "email":format!("{}@example.test",a.users[5])}),
        &jsonwebtoken::EncodingKey::from_ec_pem(secret.to_pkcs8_pem(LineEnding::LF).unwrap().as_bytes()).unwrap()).unwrap();
    read(
        &client,
        &base,
        &format!("issues/{}/sdlc/context", tasks[0]),
        &machine_token,
        200,
    )
    .await;
    read(
        &client,
        &base,
        &format!("issues/{}/sdlc/context", tasks[1]),
        &machine_token,
        403,
    )
    .await;
    read(&client, &base, "sdlc/project-access", &machine_token, 403).await;
    // The portfolio has no project links of its own; only explicit hierarchy groups children.
    space_catalog(&client, &base, &tokens[0], &a, 0, projects.to_vec(), 200).await;
    space_catalog(&client, &base, &tokens[1], &a, 0, vec![], 403).await;
    space_catalog(&client, &base, &tokens[1], &a, 1, vec![projects[0]], 200).await;
    let expected = [
        vec![projects[0], projects[1]],
        vec![projects[0]],
        vec![projects[1]],
        vec![projects[0], projects[1]],
        vec![],
    ];
    for (index, expected) in expected.iter().enumerate() {
        directory(&client, &base, &tokens[index], expected.clone()).await;
        for n in 0..2 {
            read(
                &client,
                &base,
                &format!("issues/{}/sdlc/context", tasks[n]),
                &tokens[index],
                if expected.contains(&projects[n]) {
                    200
                } else {
                    403
                },
            )
            .await;
        }
    }
    read(
        &client,
        &base,
        &format!("projects/{}/sdlc/source-policy/versions/1", projects[0]),
        &tokens[0],
        200,
    )
    .await;
    read(
        &client,
        &base,
        &format!("projects/{}/sdlc/source-policy/versions/1", projects[0]),
        &tokens[1],
        403,
    )
    .await;
    // Retained native project ownership does not replace space membership on settings ports.
    read(
        &client,
        &base,
        &format!("projects/{}/sdlc/routing-policy", projects[0]),
        &tokens[4],
        403,
    )
    .await;
    draft(
        &client,
        &base,
        projects[0],
        &tokens[3],
        403,
        "reader-cannot-edit",
    )
    .await;
    draft(
        &client,
        &base,
        projects[1],
        &tokens[1],
        403,
        "old-member-cannot-bypass-space",
    )
    .await;
    draft(
        &client,
        &base,
        projects[0],
        &tokens[0],
        201,
        "active-parent-and-native-role",
    )
    .await;
    a.archived.store(true, Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(3200)).await;
    draft(
        &client,
        &base,
        projects[0],
        &tokens[0],
        403,
        "archived-parent-readonly",
    )
    .await;
    let response = client
        .get(format!("{base}/api/v2/projects?working_only=true"))
        .bearer_auth(&tokens[0])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let working: Value = response.json().await.unwrap();
    assert_eq!(working["total"], 0);
    assert_eq!(working["projects"], json!([]));
    a.direct.store(true, Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(3200)).await;
    draft(
        &client,
        &base,
        projects[0],
        &tokens[0],
        201,
        "independent-direct-grant",
    )
    .await;
    let response = client
        .get(format!("{base}/api/v2/projects?working_only=true"))
        .bearer_auth(&tokens[0])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let working: Value = response.json().await.unwrap();
    assert_eq!(working["total"], 1);
    assert_eq!(working["projects"][0]["id"], projects[0].to_string());
    a.linked.store(false, Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(3200)).await;
    directory(&client, &base, &tokens[0], projects.to_vec()).await;
    space_catalog(&client, &base, &tokens[0], &a, 0, vec![projects[1]], 200).await;
    space_catalog(&client, &base, &tokens[0], &a, 1, vec![projects[0]], 200).await;
    a.direct.store(false, Ordering::SeqCst);
    a.revoked.store(true, Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(3200)).await;
    directory(&client, &base, &tokens[0], vec![projects[1]]).await;
    directory(&client, &base, &tokens[1], vec![]).await;
    directory(&client, &base, &tokens[3], projects.to_vec()).await;
    a.unavailable.store(true, Ordering::SeqCst);
    tokio::time::sleep(std::time::Duration::from_millis(3200)).await;
    read(&client, &base, "sdlc/project-access", &tokens[0], 503).await;
    let admins = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT count(*) AS n FROM users WHERE is_system_admin",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<i64>("", "n")
        .unwrap();
    assert_eq!(
        admins, 0,
        "SSO and namespace membership cannot promote any ordinary account"
    );
    stop_tx.send(()).unwrap();
    server.await.unwrap();
    authority.abort();
}
