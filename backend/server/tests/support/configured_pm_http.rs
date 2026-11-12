//! Current configured PM over real Tracker HTTP/PostgreSQL; Auth/Workflow are explicit peers.
use super::*;
use domain::sdlc::{Principal, SdlcConfig, SdlcRepository};
use domain::sdlc_routing::SetRoutingPolicy;
use domain::sdlc_source::{
    SetSourcePolicy, SourcePolicySpec, TaskSourceRepository, VerifiedSource,
};
use shared::resource_context::NamespaceRef;

async fn subject(f: &Fixture<'_>, id: Uuid) -> String {
    f.db.query_one(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT central_sub FROM users WHERE id=$1",
        [id.into()],
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "central_sub")
    .unwrap()
}

pub(super) async fn prepare(db: &DatabaseConnection, project: Uuid, url: &str) -> Uuid {
    // Bootstrap synthetic project settings before the HTTP fixture enables Central
    // Auth. Never remove or impersonate a verified request to configure them.
    assert!(std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_none());
    let name: String = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT current_database() AS name".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "name")
        .unwrap();
    assert!(name.ends_with("_draft_test"));
    let row =
        db.query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT u.central_sub AS owner_subject FROM projects p JOIN users u ON u.id=p.owner_id WHERE p.id=$1",
            [project.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    let operator = Principal {
        subject: row.try_get("", "owner_subject").unwrap(),
        trusted_human: true,
        human_session: true,
        scopes: Default::default(),
    };
    let namespace = NamespaceRef {
        registry_instance_id: Uuid::new_v4(),
        namespace_id: Uuid::new_v4(),
    };
    let binding = json!({"schema_version":1,"namespace":namespace,
        "resource":{"kind":"tracker_project","instance_id":Uuid::new_v4(),"resource_id":project},
        "operation_id":Uuid::new_v4(),"generation":1,"state":"active"});
    sql(db, "INSERT INTO tracker_namespace_bindings(resource_id,registry_instance_id,namespace_id,generation,state,command) VALUES($1,$2,$3,1,'active',$4)",
        vec![project.into(),namespace.registry_instance_id.into(),namespace.namespace_id.into(),binding.into()]).await;
    let repo = infra::sdlc::PostgresSdlcRepository::connect(
        url,
        SdlcConfig {
            instance_id: "tracker-draft-test".into(),
            orchestrator_subject: String::new(),
            verifier_subject: String::new(),
            reservation_scheduler_subject: String::new(),
        },
    )
    .await
    .unwrap();
    let source = SourcePolicySpec {
        development_workspace_id: None,
        forge_instance_id: Uuid::new_v4(),
        repository_id: Uuid::new_v4(),
        base_ref: "refs/heads/main".into(),
        base_commit: "a".repeat(40),
    };
    // This is declared disposable fixture data, not acceptance of a real Forge.
    repo.set_source_policy(
        project,
        &operator,
        SetSourcePolicy {
            expected_source_revision: None,
            source: source.clone(),
            idempotency_key: "configured-http:source".into(),
        },
        VerifiedSource {
            development_workspace_id: source.development_workspace_id,
            namespace,
            forge_instance_id: source.forge_instance_id,
            repository_id: source.repository_id,
            public_name: "fixture/source".into(),
            base_ref: source.base_ref,
            base_commit: source.base_commit,
            default_ref: "refs/heads/main".into(),
            protected_refs: vec![],
            observed_at: shared::now().to_utc(),
        },
    )
    .await
    .unwrap();
    let mut routes = json!({});
    let agent = Uuid::new_v4();
    for (n, role) in [
        "project_manager",
        "analyst",
        "architect",
        "developer",
        "reviewer",
        "tester",
        "devops",
    ]
    .into_iter()
    .enumerate()
    {
        let suffix = role.replace('_', "-");
        routes[role] = json!({"agent_id":if n==0 {agent}else{Uuid::new_v4()},"fleet_config_revision":1,
            "instruction_profile":"single-task-v1","instruction_manifest_sha256":"a".repeat(64),
            "namespace_id":(n+1).to_string(),"namespace_name":format!("single-task-{suffix}"),
            "workflow_id":(n+11).to_string(),"workflow_key":format!("single-task-v1:{role}"),
            "profile":format!("single-task-v1-{suffix}"),"workflow_catalog_version":1,"workflow_catalog_sha256":"b".repeat(64)});
    }
    repo.set_routing_policy(
        project,
        &operator,
        SetRoutingPolicy {
            expected_version: None,
            routes: serde_json::from_value(routes).unwrap(),
            idempotency_key: "configured-http:routing".into(),
        },
    )
    .await
    .unwrap();
    agent
}

pub async fn verify(
    config: Arc<shared::AppConfig>,
    f: &Fixture<'_>,
    pm_subject: &str,
    agent: Uuid,
) {
    let response = Arc::new(tokio::sync::Mutex::new(Value::Null));
    let peer = response.clone();
    let workflow = Router::new().route(
        "/internal/runtime/terminal-receipt/readback",
        axum::routing::post(move |headers: HeaderMap, Json(selector): Json<Value>| {
            let peer = peer.clone();
            async move {
                assert_eq!(headers["authorization"], "Bearer configured-http-reader");
                assert_eq!(selector["interaction_protocol"], "configured-cli-v1");
                let value = peer.lock().await.clone();
                if value.is_null() {
                    Err(StatusCode::SERVICE_UNAVAILABLE)
                } else {
                    Ok(Json(value))
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let workflow_handle = tokio::spawn(async move {
        axum::serve(listener, workflow).await.unwrap();
    });
    unsafe {
        std::env::set_var("TASKTRACKER_SDLC__WORKFLOW_URL", origin);
        std::env::set_var(
            "TASKTRACKER_SDLC__WORKFLOW_READ_TOKEN",
            "configured-http-reader",
        );
    }
    let (base, stop, handle) = start(config).await;
    let pm = machine(
        f.secret,
        f.issuer,
        pm_subject,
        vec!["task-tracker:read".into(), "task-tracker:write".into()],
    );
    let mut create = command("configured-http:create");
    create["interaction_protocol"] = json!("configured-cli-v1");
    create["flow"] = json!("single-task-v1");
    let created = expect(
        f.client,
        &format!("{base}/api/v1/projects/{}/sdlc/drafts", f.project),
        f.owner,
        &create,
        201,
    )
    .await;
    let task = Uuid::parse_str(created["task_id"].as_str().unwrap()).unwrap();
    let key = created["task_key"]
        .as_str()
        .unwrap()
        .rsplit_once('-')
        .unwrap()
        .0;
    let board = get_json(
        f.client,
        &format!("{base}/api/v1/projects/{key}/board"),
        f.owner,
        200,
    )
    .await;
    assert_eq!(
        board["sdlc"]["stages"],
        json!([
            "Draft",
            "Backlog",
            "Analysis",
            "Architecture",
            "Development",
            "Review",
            "Testing",
            "Rework",
            "Deployment",
        ])
    );
    assert!(
        board["sdlc"]["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["issue_id"] == json!(task)
                && row["stage"] == "Draft"
                && row["status"] == "Ready")
    );
    let backlog = get_json(
        f.client,
        &format!("{base}/api/v1/projects/{key}/backlog?limit=1"),
        f.owner,
        200,
    )
    .await;
    assert_eq!(backlog["backlog_limit"], 1);
    assert!(
        backlog["backlog_issues"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["id"] != json!(task))
    );
    let root = format!("{base}/api/v1/issues/{task}/sdlc");
    let mut reservation = reserve("configured-http:reserve", agent);
    reservation["interaction_protocol"] = json!("configured-cli-v1");
    reservation["machine_subject"] = json!(pm_subject);
    let saved = expect(
        f.client,
        &format!("{root}/pm-draft-assignment"),
        f.owner,
        &reservation,
        201,
    )
    .await;
    assert_eq!(
        expect(
            f.client,
            &format!("{root}/pm-draft-assignment"),
            f.owner,
            &reservation,
            200
        )
        .await,
        saved
    );
    let assignment = &saved["assignment"];
    let fence = json!({"assignment_id":assignment["assignment_id"],"execution_id":assignment["execution_id"],
        "agent_id":agent,"assignment_version":assignment["version"]});
    let initial = get_json(f.client, &format!("{root}/context"), &pm, 200).await;
    assert_eq!(initial["stage"], "Draft");
    assert_eq!(initial["status"], "Ready");
    let stage_url = format!("{root}/stage-starts");
    let stage_operation_url = format!("{stage_url}/operations/configured-http:stage-start");
    let mut stage_fence = fence.clone();
    stage_fence["kind"] = json!("pm");
    let stage_start = json!({"fence":stage_fence,"idempotency_key":"configured-http:stage-start"});
    get_json(f.client, &stage_operation_url, &pm, 404).await;
    let foreign_subject = std::env::var("TASKTRACKER_SDLC__ORCHESTRATOR_SUBJECT").unwrap();
    assert!(Uuid::parse_str(&foreign_subject).is_ok());
    assert_ne!(foreign_subject, pm_subject);
    let foreign = machine(
        f.secret,
        f.issuer,
        &foreign_subject,
        vec!["task-tracker:read".into(), "task-tracker:write".into()],
    );
    let before_start = [
        count(f.db, "sdlc_idempotency").await,
        count(f.db, "issue_status_history").await,
        count(f.db, "sdlc_outbox").await,
    ];
    for token in [f.owner, f.operator, foreign.as_str()] {
        expect(f.client, &stage_url, token, &stage_start, 403).await;
    }
    for field in [
        "assignment_id",
        "execution_id",
        "agent_id",
        "assignment_version",
    ] {
        let mut stale = stage_start.clone();
        stale["fence"][field] = if field == "assignment_version" {
            json!(assignment["version"].as_i64().unwrap() + 1)
        } else {
            json!(Uuid::new_v4())
        };
        expect(f.client, &stage_url, &pm, &stale, 409).await;
    }
    let mut arbitrary_status = stage_start.clone();
    arbitrary_status["status"] = json!("InProgress");
    expect(f.client, &stage_url, &pm, &arbitrary_status, 422).await;
    assert_eq!(
        [
            count(f.db, "sdlc_idempotency").await,
            count(f.db, "issue_status_history").await,
            count(f.db, "sdlc_outbox").await,
        ],
        before_start
    );
    let unstarted = get_json(f.client, &format!("{root}/context"), &pm, 200).await;
    assert_eq!(unstarted["status"], "Ready");
    assert_eq!(unstarted["assignment"], initial["assignment"]);
    get_json(f.client, &stage_operation_url, &pm, 404).await;
    let started = expect(f.client, &stage_url, &pm, &stage_start, 201).await;
    assert_eq!(started["task_id"], json!(task));
    assert_eq!(started["stage"], "Draft");
    assert_eq!(started["status"], "InProgress");
    assert_eq!(started["execution_id"], fence["execution_id"]);
    assert_eq!(started["idempotency_key"], stage_start["idempotency_key"]);
    assert_eq!(
        get_json(f.client, &stage_operation_url, &pm, 200).await,
        started
    );
    assert_eq!(
        expect(f.client, &stage_url, &pm, &stage_start, 200).await,
        started
    );
    let mut changed_start = stage_start.clone();
    changed_start["fence"]["assignment_version"] =
        json!(assignment["version"].as_i64().unwrap() + 1);
    expect(f.client, &stage_url, &pm, &changed_start, 409).await;
    let mut another_start = stage_start.clone();
    another_start["idempotency_key"] = json!("configured-http:another-stage-start");
    expect(f.client, &stage_url, &pm, &another_start, 409).await;
    assert_eq!(
        [
            count(f.db, "sdlc_idempotency").await,
            count(f.db, "issue_status_history").await,
            count(f.db, "sdlc_outbox").await,
        ],
        [before_start[0] + 1, before_start[1] + 1, before_start[2]]
    );
    let working = get_json(f.client, &format!("{root}/context"), &pm, 200).await;
    assert_eq!(working["stage"], "Draft");
    assert_eq!(working["status"], "InProgress");
    assert_eq!(working["assignment"], initial["assignment"]);
    let publish = json!({"fence":fence,"expected_requirement_revision":null,"idempotency_key":"configured-http:rev1",
        "document":{"goal":"HTTP configured PM","scope":["Saved scope"],"exclusions":[],"scenarios":["Owner approves"],
        "acceptance_criteria":["Draft Completed receipt and application-owned Analysis Ready queue"],"constraints":[],"dependencies":[],"assumptions":[],
        "checklist":["Review"],"prerequisites":["Source"]}});
    expect(
        f.client,
        &format!("{root}/requirements"),
        f.owner,
        &publish,
        403,
    )
    .await;
    let retired_pm = machine(
        f.secret,
        f.issuer,
        pm_subject,
        vec![
            "task-tracker:read".into(),
            "task-tracker:write".into(),
            format!(
                "task-tracker:sdlc:pm:{task}:{}:{}:{agent}:1",
                assignment["assignment_id"].as_str().unwrap(),
                assignment["execution_id"].as_str().unwrap()
            ),
        ],
    );
    expect(
        f.client,
        &format!("{root}/requirements"),
        &retired_pm,
        &publish,
        403,
    )
    .await;
    expect(
        f.client,
        &format!("{root}/requirements"),
        &pm,
        &publish,
        200,
    )
    .await;
    let question = Uuid::new_v4();
    let option = Uuid::new_v4();
    let question_body = json!({"fence":fence,"request_id":Uuid::new_v4(),"question_id":question,"expected_question_version":null,
        "requirement_revision":1,"requirement_reference":"scope","text":"Approve scope?","rationale":"Exact owner choice",
        "required":true,"mode":"single","options":[{"id":option,"label":"Standard","consequences":"Saved scope","is_custom":false}],
        "recommended_option_id":option,"idempotency_key":"configured-http:question"});
    let mut retired = question_body.clone();
    retired["checkpoint_id"] = json!(Uuid::new_v4());
    expect(
        f.client,
        &format!("{root}/clarifications"),
        &pm,
        &retired,
        422,
    )
    .await;
    expect(
        f.client,
        &format!("{root}/clarifications"),
        &pm,
        &question_body,
        200,
    )
    .await;
    let answer = json!({"expected_question_version":1,"requirement_revision":1,"selected_option_ids":[option],
        "text":null,"comment":"Approved scope","idempotency_key":"configured-http:answer"});
    expect(
        f.client,
        &format!("{root}/clarifications/{question}/answers"),
        &pm,
        &answer,
        403,
    )
    .await;
    let answered = expect(
        f.client,
        &format!("{root}/clarifications/{question}/answers"),
        f.owner,
        &answer,
        200,
    )
    .await;
    assert_eq!(
        expect(
            f.client,
            &format!("{root}/clarifications/{question}/answers"),
            f.owner,
            &answer,
            200
        )
        .await,
        answered
    );
    let optional_id = Uuid::new_v4();
    let mut optional = question_body.clone();
    optional["question_id"] = json!(optional_id);
    optional["request_id"] = json!(Uuid::new_v4());
    optional["required"] = json!(false);
    optional["idempotency_key"] = json!("configured-http:optional-question");
    expect(
        f.client,
        &format!("{root}/clarifications"),
        &pm,
        &optional,
        200,
    )
    .await;
    let cancel = json!({"fence":fence,"expected_question_version":1,"idempotency_key":"configured-http:cancel"});
    expect(
        f.client,
        &format!("{root}/clarifications/{optional_id}/cancel"),
        f.owner,
        &cancel,
        403,
    )
    .await;
    expect(
        f.client,
        &format!("{root}/clarifications/{optional_id}/cancel"),
        &pm,
        &cancel,
        200,
    )
    .await;
    let mut final_publish = publish;
    final_publish["expected_requirement_revision"] = json!(1);
    final_publish["idempotency_key"] = json!("configured-http:rev2");
    let revision = expect(
        f.client,
        &format!("{root}/requirements"),
        &pm,
        &final_publish,
        200,
    )
    .await;
    let consent = json!({"content_hash":revision["content_hash"],"expected_routing_policy_version":1,"idempotency_key":"configured-http:consent"});
    expect(
        f.client,
        &format!("{root}/requirements/2/confirm"),
        &pm,
        &consent,
        403,
    )
    .await;
    let confirmed = expect(
        f.client,
        &format!("{root}/requirements/2/confirm"),
        f.owner,
        &consent,
        200,
    )
    .await;
    assert_eq!(confirmed["stage"], "Draft");
    assert_eq!(
        expect(
            f.client,
            &format!("{root}/requirements/2/confirm"),
            f.owner,
            &consent,
            200
        )
        .await,
        confirmed
    );
    let context = get_json(f.client, &format!("{root}/context"), f.owner, 200).await;
    assert_eq!(context["stage"], "Draft");
    assert_eq!(context["status"], "InProgress");
    let complete = json!({"interaction_protocol":"configured-cli-v1","fence":fence,"requirement_revision":2,
        "content_hash":revision["content_hash"],"namespace_id":1,"workflow_execution_ref":Uuid::new_v4(),
        "workflow_operation_key":"configured-http:terminal","workflow_result_version":4,"idempotency_key":"configured-http:complete"});
    expect(
        f.client,
        &format!("{root}/pm-draft-completion"),
        f.owner,
        &complete,
        403,
    )
    .await;
    expect(
        f.client,
        &format!("{root}/pm-draft-completion"),
        &pm,
        &complete,
        503,
    )
    .await;
    let receipt = json!({"interaction_protocol":"configured-cli-v1","execution_ref":complete["workflow_execution_ref"],
        "task":saved["execution"]["key"],"tracker_task_id":task,"namespace_id":1,"concrete_agent_ref":agent,
        "assignment_ref":fence["assignment_id"],"tracker_execution_id":fence["execution_id"],"assignment_version":1,
        "machine_subject":pm_subject,"operation_key":complete["workflow_operation_key"],"request_sha256":"a".repeat(64),
        "result_version":4,"requirements_revision":2,"requirements_sha256":revision["content_hash"],
        "owner_subject":subject(f,f.owner_id).await,"confirmation_id":confirmed["id"],"history_id":17,"status":"done"});
    *response.lock().await = json!({"ok":true,"complete":true,"receipt":receipt,"workflow_receipt":{
        "reference":format!("workflow-configured-cli:{}:17",complete["workflow_execution_ref"].as_str().unwrap()),
        "sha256":app::sdlc::canonical_hash(&receipt).unwrap()}});
    expect(
        f.client,
        &format!("{root}/pm-draft-completion"),
        &pm,
        &complete,
        409,
    )
    .await;
    let comment = json!({"fence":fence,"body":"Final report: owner-approved requirements verified; all checklist items addressed.",
        "idempotency_key":"configured-http:comment"});
    expect(f.client, &format!("{root}/discussion"), &pm, &comment, 201).await;
    let completed = expect(
        f.client,
        &format!("{root}/pm-draft-completion"),
        &pm,
        &complete,
        201,
    )
    .await;
    assert_eq!(completed["stage"], "Draft");
    assert_eq!(completed["status"], "Completed");
    *response.lock().await = Value::Null;
    assert_eq!(
        expect(
            f.client,
            &format!("{root}/pm-draft-completion"),
            &pm,
            &complete,
            200
        )
        .await,
        completed
    );
    for token in [f.owner, pm.as_str()] {
        let read = get_json(f.client, &format!("{root}/pm-draft-completion"), token, 200).await;
        assert_eq!(read["current"], completed);
        let operation = get_json(
            f.client,
            &format!("{root}/pm-draft-completion/operations/configured-http:complete"),
            token,
            200,
        )
        .await;
        assert_eq!(operation["result"], completed);
    }
    let mut changed = complete;
    changed["workflow_result_version"] = json!(5);
    expect(
        f.client,
        &format!("{root}/pm-draft-completion"),
        &pm,
        &changed,
        409,
    )
    .await;
    let row =
        f.db.query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT (SELECT count(*) FROM sdlc_analysis_intents WHERE task_id=$1) AS intents,
         (SELECT count(*) FROM sdlc_delivery_assignments WHERE task_id=$1) AS assignments,
         (SELECT count(*) FROM sdlc_pm_execution_leases l JOIN sdlc_pm_executions e USING(execution_id) WHERE e.task_id=$1) AS native_leases,
         (SELECT count(*) FROM sdlc_configured_pm_completions WHERE task_id=$1) AS completions",
            [task.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    for key in ["assignments", "native_leases"] {
        assert_eq!(row.try_get::<i64>("", key).unwrap(), 0, "{key}");
    }
    assert_eq!(row.try_get::<i64>("", "intents").unwrap(), 1);
    assert_eq!(row.try_get::<i64>("", "completions").unwrap(), 1);
    let final_context = get_json(f.client, &format!("{root}/context"), f.owner, 200).await;
    assert_eq!(final_context["stage"], "Analysis");
    assert_eq!(final_context["status"], "Ready");
    assert!(final_context["assignment"].is_null());
    assert!(final_context["expected_assignment_version"].is_null());
    let lifecycle = get_json(f.client, &format!("{root}/lifecycle"), f.owner, 200).await;
    assert_eq!(lifecycle["runtime_state"], "queued");
    let queued = &lifecycle["receipt"];
    assert_eq!(queued["task"]["stage"], "Analysis");
    assert_eq!(queued["task"]["status"], "Ready");
    assert_eq!(queued["task"]["runtime"], "queued");
    assert_eq!(
        queued["task"]["routing"]["confirmation_id"],
        confirmed["id"]
    );
    assert_eq!(queued["task"]["routing"]["requirement_revision"], 2);
    assert_eq!(
        queued["task"]["routing"]["content_hash"],
        revision["content_hash"]
    );
    assert!(
        queued["assignment"].is_null() && queued["lease"].is_null() && queued["binding"].is_null()
    );
    assert!(queued["result"].is_null());
    assert_eq!(
        get_json(f.client, &stage_operation_url, &pm, 200).await,
        started
    );
    assert_eq!(
        expect(f.client, &stage_url, &pm, &stage_start, 200).await,
        started
    );
    let start_url = format!("{root}/lifecycle");
    let operation_url = format!("{root}/lifecycle/operations/configured-http:start");
    let start_command = json!({"operation":"start_analysis", "pm_completion_id":completed["completion_id"],
        "idempotency_key":"configured-http:start"});
    get_json(f.client, &operation_url, f.owner, 404).await;
    for token in [pm.as_str(), f.operator] {
        expect(f.client, &start_url, token, &start_command, 403).await;
    }
    let mut wrong_start = start_command.clone();
    wrong_start["pm_completion_id"] = json!(Uuid::new_v4());
    expect(f.client, &start_url, f.owner, &wrong_start, 409).await;
    // This task is already queued by PM completion, not an old unqueued Backlog.
    expect(f.client, &start_url, f.owner, &start_command, 409).await;
    get_json(f.client, &operation_url, f.owner, 404).await;
    expect(f.client, &start_url, f.owner, &wrong_start, 409).await;
    let counts = f.db.query_one(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT (SELECT count(*) FROM sdlc_analysis_intents WHERE task_id=$1) AS intents,
         (SELECT count(*) FROM sdlc_delivery_operations WHERE task_id=$1 AND command IS NOT NULL) AS starts,
         (SELECT count(*) FROM sdlc_delivery_assignments WHERE task_id=$1) AS assignments",
        [task.into()],
    )).await.unwrap().unwrap();
    assert_eq!(counts.try_get::<i64>("", "intents").unwrap(), 1);
    assert_eq!(counts.try_get::<i64>("", "starts").unwrap(), 1);
    assert_eq!(counts.try_get::<i64>("", "assignments").unwrap(), 0);
    let after_replay = get_json(f.client, &format!("{root}/lifecycle"), f.owner, 200).await;
    assert_eq!(after_replay["receipt"], *queued);
    for suffix in [
        "",
        "/heartbeat",
        "/resume",
        "/resume/operations/original",
        "/retry-bootstrap",
        "/retry-bootstrap/operations/original",
        "/recover-unsubmitted",
        "/recover-unsubmitted/operations/original",
    ] {
        let endpoint = format!("{root}/pm-draft-execution-lease{suffix}");
        for method in [reqwest::Method::GET, reqwest::Method::POST] {
            assert_eq!(
                f.client
                    .request(method, &endpoint)
                    .bearer_auth(&pm)
                    .json(&json!({}))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                404
            );
        }
    }
    stop.send(()).unwrap();
    handle.await.unwrap();
    workflow_handle.abort();
    unsafe {
        for key in [
            "TASKTRACKER_SDLC__WORKFLOW_URL",
            "TASKTRACKER_SDLC__WORKFLOW_READ_TOKEN",
        ] {
            std::env::remove_var(key);
        }
    }
}
