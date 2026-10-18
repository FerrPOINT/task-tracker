//! Actual HTTP/auth/repository/PG boundary, with a synthetic Central Auth issuer.
//! Explicit manual lane: no real Fleet runs or production changes.
use super::*;
use domain::sdlc_discussion::{CommentFence, PublishStageComment};
use domain::sdlc_execution_lease::ClaimExecutionLease;

async fn read(client: &Client, url: &str, token: &str, status: u16) -> Value {
    let response = client.get(url).bearer_auth(token).send().await.unwrap();
    let actual = response.status().as_u16();
    if actual == 200 {
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    let body = response.text().await.unwrap();
    assert_eq!(actual, status, "{url}: {body}");
    serde_json::from_str(&body).unwrap()
}

#[tokio::test]
#[ignore = "requires fresh disposable TT_DISCUSSION_TEST_DATABASE_URL ending in _discussion_test"]
async fn stage_discussion_http_pagination_comment_replay_and_revocation() {
    let url = std::env::var("TT_DISCUSSION_TEST_DATABASE_URL").unwrap();
    let db = Database::connect(&url).await.unwrap();
    let row = db.query_one(Statement::from_string(DatabaseBackend::Postgres,
        "SELECT current_database() AS name,to_regclass('public.seaql_migrations') IS NULL AS fresh"))
        .await.unwrap().unwrap();
    assert!(
        row.try_get::<String>("", "name")
            .unwrap()
            .ends_with("_discussion_test")
    );
    assert!(row.try_get::<bool>("", "fresh").unwrap());
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
        auth: shared::AuthConfig {
            jwt_secret: "discussion-test-only".into(),
            ..Default::default()
        },
        ..Default::default()
    });
    infra::run_migrations(config.database.clone())
        .await
        .unwrap();
    let owner_id = Uuid::new_v4();
    let pm_id = Uuid::new_v4();
    let participant_id = Uuid::new_v4();
    let project = Uuid::new_v4();
    for (id, name) in [
        (owner_id, "discussion-owner"),
        (pm_id, "discussion-pm"),
        (participant_id, "discussion-participant"),
    ] {
        sql(&db, "INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,$2,$3,$3,'!',$3,true)",
            vec![id.into(),format!("{name}@example.test").into(),name.into()]).await;
    }
    sql(&db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,'DISC','Discussion fixture',$2,$3)",
        vec![project.into(),owner_id.into(),Uuid::new_v4().into()]).await;
    sql(&db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Discussion','[]' FROM projects WHERE id=$1",vec![project.into()]).await;
    sql(
        &db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'developer')",
        vec![project.into(), pm_id.into()],
    )
    .await;
    let repo = infra::sdlc::PostgresSdlcRepository::connect(
        &url,
        SdlcConfig {
            instance_id: "discussion-http-test".into(),
            orchestrator_subject: "discussion-pm".into(),
            verifier_subject: "discussion-verifier".into(),
            reservation_scheduler_subject: "discussion-scheduler".into(),
            single_task_project_ids: [project].into(),
        },
    )
    .await
    .unwrap();
    let owner = Principal {
        subject: "discussion-owner".into(),
        human_session: true,
        trusted_human: true,
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
        routes[*role] = json!({"agent_id":Uuid::new_v4(),"fleet_config_revision":1,
            "instruction_profile":"single-task-v1","instruction_manifest_sha256":"c".repeat(64),
            "namespace_id":(index+1).to_string(),"namespace_name":format!("single-task-{suffix}"),
            "workflow_id":(index+11).to_string(),"workflow_key":format!("single-task-v1:{role}"),
            "profile":format!("single-task-v1-{suffix}"),"workflow_catalog_version":1,"workflow_catalog_sha256":"b".repeat(64)});
    }
    let agent_id =
        Uuid::parse_str(routes["project_manager"]["agent_id"].as_str().unwrap()).unwrap();
    repo.set_routing_policy(
        project,
        &owner,
        serde_json::from_value(json!({
            "expected_version":null,"routes":routes,"idempotency_key":"discussion:policy"
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    let (draft, _) = repo
        .create_draft(
            project,
            &owner,
            CreateDraftCommand {
                flow: Some(DraftFlow::SingleTaskV1),
                title: "Полная история задачи".into(),
                description: "Сохранённое описание 🚀".into(),
                idempotency_key: "discussion:create".into(),
            },
        )
        .await
        .unwrap();
    let task = draft.task_id;
    let (reservation, _) = repo.reserve_pm_draft(task, &owner, serde_json::from_value(json!({
        "requested_agent_id":agent_id,"expected_owner_version":0,"expected_assignment_version":null,"idempotency_key":"discussion:reserve"
    })).unwrap()).await.unwrap();
    let pm = Principal {
        subject: "discussion-pm".into(),
        human_session: false,
        trusted_human: false,
        scopes: [
            "task-tracker:read".into(),
            "task-tracker:write".into(),
            reservation.assignment.scope(task),
        ]
        .into(),
    };
    let fence = MachineFence {
        assignment_id: reservation.assignment.assignment_id,
        execution_id: reservation.assignment.execution_id,
        agent_id: reservation.assignment.agent_id,
        assignment_version: reservation.assignment.version,
    };
    let (lease, _) = repo
        .claim_execution_lease(
            task,
            &pm,
            ClaimExecutionLease {
                expected_owner_version: reservation.owner_cas.version,
                fence: fence.clone(),
                idempotency_key: "discussion:claim".into(),
            },
        )
        .await
        .unwrap();
    let mut tokens = HashMap::new();
    for (token, subject, scopes) in [
        ("sdlc_pat_discussion_pm", "discussion-pm", pm.scopes.clone()),
        (
            "sdlc_pat_discussion_read",
            "discussion-pm",
            [
                "task-tracker:read".into(),
                reservation.assignment.scope(task),
            ]
            .into(),
        ),
        (
            "sdlc_pat_discussion_foreign",
            "discussion-owner",
            pm.scopes.clone(),
        ),
        (
            "sdlc_pat_discussion_generic",
            "discussion-pm",
            ["task-tracker:read".into(), "task-tracker:write".into()].into(),
        ),
    ] {
        tokens.insert(token.into(),json!({"sub":subject,"email":format!("{subject}@example.test"),"display_name":subject,"role":"service_account","scopes":scopes}));
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let secret = SecretKey::from_slice(&[7u8; 32]).unwrap();
    let point = secret.public_key().to_encoded_point(false);
    let stub = AuthStub {
        jwks: json!({"keys":[{"kid":"tracker-test","kty":"EC","crv":"P-256","alg":"ES256","use":"sig",
        "x":URL_SAFE_NO_PAD.encode(point.x().unwrap()),"y":URL_SAFE_NO_PAD.encode(point.y().unwrap())}]}),
        tokens: Arc::new(tokens),
        issuer: issuer.clone(),
        key: Arc::new(
            jsonwebtoken::DecodingKey::from_ec_components(
                &URL_SAFE_NO_PAD.encode(point.x().unwrap()),
                &URL_SAFE_NO_PAD.encode(point.y().unwrap()),
            )
            .unwrap(),
        ),
        unavailable: Arc::new(AtomicBool::new(false)),
        profile_invalid: Arc::new(AtomicBool::new(false)),
    };
    let auth = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route(
                    "/jwks",
                    get(|State(stub): State<AuthStub>| async move { Json(stub.jwks) }),
                )
                .route("/auth/tokens/introspect", get(introspect))
                .with_state(stub),
        )
        .await
        .unwrap();
    });
    // This ignored test is run alone with --test-threads=1; never alongside the other global-env fixture.
    unsafe {
        std::env::set_var("TT_AUTH__CENTRAL_ISSUER", &issuer);
        std::env::set_var("TT_AUTH__CENTRAL_JWKS_URI", format!("{issuer}/jwks"));
        std::env::set_var("TASKTRACKER_SDLC__INSTANCE_ID", "discussion-http-test");
        std::env::set_var(
            "TASKTRACKER_SDLC__SINGLE_TASK_PROJECT_IDS",
            project.to_string(),
        );
        std::env::set_var("TASKTRACKER_SDLC__ORCHESTRATOR_SUBJECT", "discussion-pm");
        std::env::set_var("TASKTRACKER_SDLC__VERIFIER_SUBJECT", "discussion-verifier");
        std::env::set_var(
            "TASKTRACKER_SDLC__RESERVATION_SCHEDULER_SUBJECT",
            "discussion-scheduler",
        );
    }
    let (base, stop, handle) = start_tracker(config).await;
    let endpoint = format!("{base}/api/v1/issues/{task}/sdlc/discussion");
    let client = Client::new();
    let token = "sdlc_pat_discussion_pm";
    let first = Uuid::now_v7();
    sql(
        &db,
        "INSERT INTO comments(id,issue_id,author_id,body) VALUES($1,$2,$3,'Human owner input')",
        vec![first.into(), task.into(), owner_id.into()],
    )
    .await;
    sql(&db,"INSERT INTO comments(id,issue_id,author_id,body) VALUES($1,$2,$3,'Other participant input')",vec![Uuid::now_v7().into(),task.into(),participant_id.into()]).await;
    let command = serde_json::to_value(PublishStageComment {
        body: "Выполнено: проверены требования.\nПередача: Analyst 🚀".into(),
        fence: CommentFence::Pm {
            fence,
            lease_id: lease.lease.lease_id,
            expected_lease_version: lease.lease.version,
        },
        idempotency_key: "discussion:comment".into(),
    })
    .unwrap();
    post(
        &client,
        &endpoint,
        "sdlc_pat_discussion_read",
        &command,
        403,
    )
    .await;
    for denied in ["sdlc_pat_discussion_generic", "sdlc_pat_discussion_foreign"] {
        read(&client, &endpoint, denied, 403).await;
        post(&client, &endpoint, denied, &command, 403).await;
    }
    let saved = post(&client, &endpoint, token, &command, 201).await;
    assert_eq!(saved["author_id"], json!(pm_id));
    assert_eq!(saved["author_subject"], "discussion-pm");
    assert_eq!(post(&client, &endpoint, token, &command, 200).await, saved);
    let operation = format!("{endpoint}/operations/discussion:comment");
    assert_eq!(read(&client, &operation, token, 200).await, saved);
    let page = read(&client, &format!("{endpoint}?limit=1"), token, 200).await;
    assert_eq!(page["title"], "Полная история задачи");
    assert_eq!(page["description"], "Сохранённое описание 🚀");
    assert_eq!(page["comments"][0]["author_id"], json!(owner_id));
    let late = Uuid::now_v7();
    sql(&db,"INSERT INTO comments(id,issue_id,author_id,body) VALUES($1,$2,$3,'Late participant input')",vec![late.into(),task.into(),participant_id.into()]).await;
    let rest = read(
        &client,
        &format!(
            "{endpoint}?limit=500&after={}&snapshot={}",
            page["next_after"].as_str().unwrap(),
            page["snapshot"].as_str().unwrap()
        ),
        token,
        200,
    )
    .await;
    assert_eq!(rest["comments"].as_array().unwrap().len(), 2);
    assert_eq!(rest["comments"][0]["author_id"], json!(participant_id));
    assert_eq!(rest["comments"][1]["body"], command["body"]);
    assert!(rest["next_after"].is_null());
    let fresh = read(&client, &endpoint, token, 200).await;
    assert_eq!(fresh["comments"].as_array().unwrap().len(), 4);
    assert_eq!(fresh["comments"][3]["id"], json!(late));
    sql(&db, "DELETE FROM comments WHERE id=$1", vec![first.into()]).await;
    read(
        &client,
        &format!(
            "{endpoint}?after={first}&snapshot={}",
            page["snapshot"].as_str().unwrap()
        ),
        token,
        409,
    )
    .await;
    let mut changed = command.clone();
    changed["body"] = json!("Changed payload");
    post(&client, &endpoint, token, &changed, 409).await;
    // Use the real database clock; never disable guards or edit protected lease rows.
    db.execute_unprepared("SELECT pg_sleep(31)").await.unwrap();
    read(&client, &endpoint, token, 409).await;
    let mut new_operation = command.clone();
    new_operation["idempotency_key"] = json!("discussion:after-expiry");
    post(&client, &endpoint, token, &new_operation, 409).await;
    assert_eq!(read(&client, &operation, token, 200).await, saved);
    assert_eq!(post(&client, &endpoint, token, &command, 200).await, saved);
    sql(
        &db,
        "UPDATE comments SET body='Edited by another participant' WHERE id=$1",
        vec![
            Uuid::parse_str(saved["comment_id"].as_str().unwrap())
                .unwrap()
                .into(),
        ],
    )
    .await;
    read(&client, &operation, token, 409).await;
    post(&client, &endpoint, token, &command, 409).await;
    sql(
        &db,
        "DELETE FROM project_members WHERE project_id=$1 AND user_id=$2",
        vec![project.into(), pm_id.into()],
    )
    .await;
    read(&client, &endpoint, token, 403).await;
    read(&client, &operation, token, 403).await;
    stop.send(()).unwrap();
    handle.await.unwrap();
    auth.abort();
}
