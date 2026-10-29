use super::*;

#[path = "configured_pm_http.rs"]
mod configured_pm_http;

async fn reject_retired_grant(client: &Client, url: &str, token: &str, body: &Value) {
    let error = expect(client, url, token, body, 403).await;
    assert_eq!(error["error"]["code"], "FORBIDDEN");
}

pub struct Fixture<'a> {
    pub db: &'a DatabaseConnection,
    pub client: &'a Client,
    pub secret: &'a SecretKey,
    pub issuer: &'a str,
    pub owner: &'a str,
    pub operator: &'a str,
    pub owner_id: Uuid,
    pub project: Uuid,
    pub unavailable: Arc<AtomicBool>,
}

fn machine(secret: &SecretKey, issuer: &str, subject: &str, scopes: Vec<String>) -> String {
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256);
    header.kid = Some("draft-test".into());
    let claims = json!({"sub":subject,"iss":issuer,"aud":"sdlc","iat":shared::now().timestamp(),"exp":shared::now().timestamp()+3600,"scopes":scopes});
    jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_ec_pem(
            secret.to_pkcs8_pem(LineEnding::LF).unwrap().as_bytes(),
        )
        .unwrap(),
    )
    .unwrap()
}
pub(super) const PM_SUBJECT: &str = "00000000-0000-4000-8000-000000000019";

fn reserve(key: &str, agent: Uuid) -> Value {
    json!({"interaction_protocol":"configured-cli-v1","machine_subject":PM_SUBJECT,"expected_owner_version":0,"expected_assignment_version":null,"requested_agent_id":agent,"idempotency_key":key})
}
async fn create(f: &Fixture<'_>, base: &str, key: &str) -> Uuid {
    let result = expect(
        f.client,
        &format!("{base}/api/v1/projects/{}/sdlc/drafts", f.project),
        f.owner,
        &command(key),
        201,
    )
    .await;
    Uuid::parse_str(result["task_id"].as_str().unwrap()).unwrap()
}
fn endpoint(base: &str, task: Uuid) -> String {
    format!("{base}/api/v1/issues/{task}/sdlc/pm-draft-assignment")
}
fn creation_endpoint(base: &str, project: Uuid, key: &str) -> String {
    let mut url = reqwest::Url::parse(&format!(
        "{base}/api/v1/projects/{project}/sdlc/drafts/operations/"
    ))
    .unwrap();
    url.path_segments_mut().unwrap().pop_if_empty().push(key);
    url.to_string()
}
async fn curl_json(url: &str, token: &str, body: Option<&Value>) -> Value {
    let (url, token, body) = (
        url.to_string(),
        token.to_string(),
        body.map(Value::to_string),
    );
    let output = tokio::task::spawn_blocking(move || {
        let mut command = std::process::Command::new("curl");
        command
            .args([
                "--silent",
                "--show-error",
                "--fail-with-body",
                "--max-time",
                "10",
            ])
            .arg("--header")
            .arg(format!("Authorization: Bearer {token}"))
            .arg("--url")
            .arg(url);
        if let Some(body) = body {
            command
                .args([
                    "--header",
                    "Content-Type: application/json",
                    "--data-binary",
                ])
                .arg(body);
        }
        command.output().unwrap()
    })
    .await
    .unwrap();
    assert!(output.status.success(), "curl HTTP smoke failed");
    serde_json::from_slice(&output.stdout).unwrap()
}

pub async fn prepare(db: &DatabaseConnection, url: &str) -> (Uuid, Uuid) {
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
    // Keep this synthetic configured project out of the ordinary creation/edit
    // fixture. Its settings owner is distinct and has no HTTP session.
    let settings_owner = Uuid::new_v4();
    sql(db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active)
        VALUES($1,'configured-settings-owner@example.test','configured-settings-owner','Configured settings owner','!',$2,true)",
        vec![settings_owner.into(),Uuid::new_v4().to_string().into()]).await;
    let project = Uuid::new_v4();
    sql(db,"INSERT INTO projects(id,key,name,owner_id,default_board_id) VALUES($1,'CPM','Configured PM HTTP',$2,$3)",
        vec![project.into(),settings_owner.into(),Uuid::new_v4().into()]).await;
    sql(db,"INSERT INTO boards(id,project_id,name,columns) SELECT default_board_id,id,'Configured PM','[]' FROM projects WHERE id=$1",
        vec![project.into()]).await;
    (project, configured_pm_http::prepare(db, project, url).await)
}

pub async fn verify(config: Arc<shared::AppConfig>, f: Fixture<'_>, selector: Uuid) {
    let subject = Uuid::new_v4().to_string();
    let verifier = Uuid::new_v4().to_string();
    for (sub, name) in [
        (subject.as_str(), "reservation-machine"),
        (verifier.as_str(), "reservation-verifier"),
        ("scheduler", "reservation-scheduler"),
        (PM_SUBJECT, "configured-pm-service"),
    ] {
        let id = Uuid::new_v4();
        sql(f.db,"INSERT INTO users(id,email,username,display_name,password_hash,central_sub,is_active) VALUES($1,$2,$3,$3,'!',$4,true)",
            vec![id.into(),format!("{name}@example.test").into(),name.into(),sub.into()]).await;
        sql(
            f.db,
            "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
            vec![f.project.into(), id.into()],
        )
        .await;
    }
    unsafe {
        std::env::set_var("TASKTRACKER_SDLC__ORCHESTRATOR_SUBJECT", &subject);
        std::env::set_var("TASKTRACKER_SDLC__VERIFIER_SUBJECT", &verifier);
        std::env::set_var(
            "TASKTRACKER_SDLC__RESERVATION_SCHEDULER_SUBJECT",
            "scheduler",
        );
    }
    let orch = machine(
        f.secret,
        f.issuer,
        &subject,
        vec![
            "task-tracker:read".into(),
            "task-tracker:write".into(),
            "task-tracker:sdlc:assign".into(),
        ],
    );

    unsafe {
        std::env::set_var(
            "TASKTRACKER_SDLC__SINGLE_TASK_PROJECT_IDS",
            f.project.to_string(),
        );
    }
    let (base, stop, handle) = start(config.clone()).await;
    let creation_key = "creation/readback-\u{043a}\u{043b}\u{044e}\u{0447}";
    let creation_url = creation_endpoint(&base, f.project, creation_key);
    get_json(f.client, &creation_url, f.owner, 404).await;
    let counts = [
        count(f.db, "issues").await,
        count(f.db, "sdlc_draft_creations").await,
    ];
    let response = f
        .client
        .post(format!("{base}/api/v1/projects/{}/sdlc/drafts", f.project))
        .bearer_auth(f.owner)
        .json(&command(creation_key))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    drop(response); // Fleet does not know whether creation committed; GET precedes any POST replay.
    let created = get_json(f.client, &creation_url, f.owner, 200).await;
    assert_eq!(curl_json(&creation_url, f.owner, None).await, created);
    assert_eq!(created.as_object().unwrap().len(), 7);
    let _: CreatedDraft = serde_json::from_value(created.clone()).unwrap();
    assert!(created.get("title").is_none() && created.get("description").is_none());
    get_json(f.client, &creation_url, f.operator, 404).await;
    for token in ["sdlc_pat_owner", &orch] {
        get_json(f.client, &creation_url, token, 403).await;
    }
    assert_eq!(
        f.client.get(&creation_url).send().await.unwrap().status(),
        401
    );
    let mut changed = command(creation_key);
    changed["title"] = json!("different creation intent");
    expect(
        f.client,
        &format!("{base}/api/v1/projects/{}/sdlc/drafts", f.project),
        f.owner,
        &changed,
        409,
    )
    .await;
    assert_eq!(
        get_json(f.client, &creation_url, f.owner, 200).await,
        created
    );
    assert_eq!(count(f.db, "issues").await, counts[0] + 1);
    assert_eq!(count(f.db, "sdlc_draft_creations").await, counts[1] + 1);
    let task = create(&f, &base, "reservation-main").await;
    let url = endpoint(&base, task);
    let cmd = reserve("reservation", selector);
    let empty = get_json(f.client, &url, f.owner, 200).await;
    assert_eq!(empty["owner_version"], 0);
    assert!(empty["current"].is_null());
    assert!(empty["operation"].is_null());
    for token in [f.operator, "sdlc_pat_owner", &orch] {
        expect(f.client, &url, token, &cmd, 403).await;
    }
    assert_eq!(
        f.client
            .post(&url)
            .json(&cmd)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    for bad in [
        json!({}),
        {
            let mut c = cmd.clone();
            c["requested_agent_id"] = json!(Uuid::nil());
            c
        },
        {
            let mut c = cmd.clone();
            c["requested_agent_id"] = json!(selector.to_string().to_uppercase());
            c
        },
        {
            let mut c = cmd.clone();
            c["machine_subject"] = json!("claim");
            c
        },
        {
            let mut c = cmd.clone();
            c["expected_owner_version"] = json!(9007199254740992i64);
            c
        },
    ] {
        expect(f.client, &url, f.owner, &bad, 422).await;
    }
    let mut joins = vec![];
    for _ in 0..12 {
        let client = f.client.clone();
        let url = url.clone();
        let owner = f.owner.to_string();
        let cmd = cmd.clone();
        joins.push(tokio::spawn(async move {
            post(&client, &url, &owner, &cmd).await
        }));
    }
    let mut result = None;
    let mut fresh = 0;
    for join in joins {
        let (status, value) = join.await.unwrap();
        assert!([200, 201].contains(&status), "{value}");
        fresh += usize::from(status == 201);
        if let Some(ref expected) = result {
            assert_eq!(expected, &value);
        } else {
            result = Some(value);
        }
    }
    assert_eq!(fresh, 1);
    let result = result.unwrap();
    let _: domain::sdlc_pm_draft::PmDraftReservation =
        serde_json::from_value(result.clone()).unwrap();
    assert_eq!(result["dispatch_allowed"], false);
    assert_eq!(result["variant"], "pm_draft_reserved");
    assert_eq!(result["admission_state"], "reserved");
    assert_eq!(result["assignment"]["agent_id"], selector.to_string());
    assert_eq!(result["assignment"]["machine_subject"], PM_SUBJECT);
    assert_eq!(result["assignment"]["version"], 1);
    assert_eq!(
        result["owner_cas"],
        json!({"expected_version":0,"version":1})
    );
    let ordinal = result["execution"]["ordinal"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    assert!(ordinal > 0);
    assert_eq!(result["execution"]["key"], format!("SDLC-{ordinal}"));
    let input = get_json(
        f.client,
        &format!("{base}/api/v1/issues/{task}/sdlc/pm-draft-input"),
        f.owner,
        200,
    )
    .await;
    assert_eq!(
        result["input"],
        json!({"snapshot_ref":input["input"]["snapshot_ref"],"sha256":input["input"]["sha256"]})
    );
    let read = get_json(
        f.client,
        &format!("{url}?idempotency_key=reservation"),
        f.owner,
        200,
    )
    .await;
    assert_eq!(read["current"], result);
    assert_eq!(read["operation"]["result"], result);
    assert_eq!(read["owner_version"], 1);
    assert_eq!(curl_json(&url, f.owner, Some(&cmd)).await, result);
    assert_eq!(
        curl_json(&format!("{url}?idempotency_key=reservation"), f.owner, None).await,
        read
    );
    let member = get_json(
        f.client,
        &format!("{url}?idempotency_key=reservation"),
        f.operator,
        200,
    )
    .await;
    assert_eq!(member["current"], result);
    assert!(member["operation"].is_null());
    assert!(
        get_json(
            f.client,
            &format!("{url}?idempotency_key=unknown"),
            f.owner,
            200
        )
        .await["operation"]
            .is_null()
    );
    let before = count(f.db, "sdlc_pm_executions").await;
    for bad in [
        reserve("different-key", selector),
        reserve("reservation", Uuid::new_v4()),
        {
            let mut c = reserve("stale", selector);
            c["expected_owner_version"] = json!(1);
            c
        },
        {
            let mut c = reserve("assignment-cas", selector);
            c["expected_assignment_version"] = json!(1);
            c
        },
    ] {
        expect(f.client, &url, f.owner, &bad, 409).await;
    }
    assert_eq!(count(f.db, "sdlc_pm_executions").await, before);
    let fence = json!({"assignment_id":result["assignment"]["assignment_id"],"execution_id":result["assignment"]["execution_id"],
        "agent_id":selector,"assignment_version":1});
    let pm = machine(
        f.secret,
        f.issuer,
        PM_SUBJECT,
        vec![
            "task-tracker:read".into(),
            "task-tracker:write".into(),
            format!(
                "task-tracker:sdlc:pm:{task}:{}:{}:{selector}:1",
                result["assignment"]["assignment_id"].as_str().unwrap(),
                result["assignment"]["execution_id"].as_str().unwrap()
            ),
        ],
    );
    // Retired own-task grants cannot impersonate the current configured service.
    // Ordinary configured PM writes are exercised below without a native lease.
    let native_count_before = count(f.db, "sdlc_pm_native_admissions").await;
    let mut next = result["assignment"].clone();
    next["version"] = json!(2);
    next["execution_id"] = json!(Uuid::new_v4());
    expect(f.client,&format!("{base}/api/v1/issues/{task}/sdlc/assignment"),&orch,
        &json!({"assignment":next,"expected_assignment_version":1,"idempotency_key":"legacy-bypass"}),403).await;
    reject_retired_grant(f.client,&format!("{base}/api/v1/issues/{task}/sdlc/requirements"),&pm,
        &json!({"fence":fence,"expected_requirement_revision":null,"document":{"goal":"goal","scope":[],"exclusions":[],"scenarios":[],"acceptance_criteria":[],"constraints":[],"dependencies":[],"assumptions":[],"checklist":[],"prerequisites":[]},"idempotency_key":"denied-revision"})).await;
    reject_retired_grant(f.client,&format!("{base}/api/v1/issues/{task}/sdlc/clarifications"),&pm,
        &json!({"fence":fence,"request_id":Uuid::new_v4(),"question_id":Uuid::new_v4(),"expected_question_version":null,"requirement_revision":1,"requirement_reference":null,"text":"text","rationale":"rationale","required":true,"mode":"text","options":[],"recommended_option_id":null,"idempotency_key":"denied-question"})).await;
    // Native lease acquisition is retired; retained legacy reservations cannot
    // manufacture a new execution or become configured by posting old commands.
    expect(
        f.client,
        &format!("{base}/api/v1/issues/{task}/sdlc/pm-draft-execution-lease"),
        &pm,
        &json!({"expected_owner_version":1,"fence":fence,"idempotency_key":"retired-claim"}),
        404,
    )
    .await;
    assert_eq!(
        count(f.db, "sdlc_pm_native_admissions").await,
        native_count_before
    );
    assert!(f.db.execute_unprepared(&format!("UPDATE sdlc_tasks SET state=jsonb_set(state,'{{assignment,version}}','2') WHERE task_id='{task}'")).await.is_err());
    assert!(f.db.execute_unprepared(&format!("UPDATE sdlc_tasks SET pm_owner_version=0,pm_execution_id=NULL,pm_admission_state=NULL WHERE task_id='{task}'")).await.is_err());
    assert!(
        f.db.execute_unprepared("DELETE FROM sdlc_pm_executions")
            .await
            .is_err()
    );
    let legacy = get_json(
        f.client,
        &format!("{base}/api/v1/issues/{task}/sdlc/events"),
        f.owner,
        200,
    )
    .await;
    let legacy_events = legacy["events"].as_array().unwrap();
    assert!(
        legacy_events
            .iter()
            .any(|event| event["event_type"] == "pm.assigned"
                && event["payload"]["result"] == result["assignment"])
    );
    let (_, page) = metadata::get(
        f.client,
        &format!("{base}/api/v1/issues/{task}/sdlc/events?projection=metadata_v1"),
        f.owner,
        200,
    )
    .await;
    metadata::verify(&page, &legacy);
    let completion_url = format!("{base}/api/v1/issues/{task}/sdlc/pm-draft-completion");
    let retired_completion = json!({"contract_version":1,"fence":fence,"lease_id":Uuid::new_v4(),
        "lease_version":1,"requirement_revision":2,"content_hash":"a".repeat(64),
        "terminal_receipt_ref":"retired/native-terminal.json","terminal_receipt_sha256":"c".repeat(64),
        "workflow_receipt_ref":"retired/workflow-terminal.json","workflow_receipt_sha256":"d".repeat(64),
        "idempotency_key":"retired-completion"});
    let configured_pm = machine(
        f.secret,
        f.issuer,
        PM_SUBJECT,
        vec!["task-tracker:read".into(), "task-tracker:write".into()],
    );
    expect(
        f.client,
        &completion_url,
        &configured_pm,
        &retired_completion,
        422,
    )
    .await;
    // A forced failure after UUID/ordinal allocation rolls every business row back.
    let rollback = create(&f, &base, "reservation-rollback").await;
    sql(f.db,"CREATE FUNCTION reservation_test_failure() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_type='pm.assigned' THEN RAISE EXCEPTION 'forced reservation rollback'; END IF; RETURN NEW; END $$",vec![]).await;
    sql(f.db,"CREATE TRIGGER reservation_test_failure BEFORE INSERT ON sdlc_outbox FOR EACH ROW EXECUTE FUNCTION reservation_test_failure()",vec![]).await;
    let tables = [
        "sdlc_assignments",
        "sdlc_pm_executions",
        "sdlc_idempotency",
        "sdlc_outbox",
    ];
    let mut counts = vec![];
    for table in tables {
        counts.push(count(f.db, table).await);
    }
    expect(
        f.client,
        &endpoint(&base, rollback),
        f.owner,
        &reserve("rollback", selector),
        500,
    )
    .await;
    for (i, table) in tables.into_iter().enumerate() {
        assert_eq!(count(f.db, table).await, counts[i]);
    }
    assert_eq!(
        get_json(f.client, &endpoint(&base, rollback), f.owner, 200).await["owner_version"],
        0
    );
    sql(
        f.db,
        "DROP TRIGGER reservation_test_failure ON sdlc_outbox",
        vec![],
    )
    .await;
    sql(f.db, "DROP FUNCTION reservation_test_failure()", vec![]).await;
    let later = expect(
        f.client,
        &endpoint(&base, rollback),
        f.owner,
        &reserve("rollback", selector),
        201,
    )
    .await;
    assert!(
        later["execution"]["ordinal"]
            .as_str()
            .unwrap()
            .parse::<i64>()
            .unwrap()
            > ordinal + 1
    );
    // Different keys race for one initial CAS; only one result becomes current.
    let race = create(&f, &base, "reservation-race").await;
    let mut joins = vec![];
    for i in 0..8 {
        let client = f.client.clone();
        let url = endpoint(&base, race);
        let owner = f.owner.to_string();
        joins.push(tokio::spawn(async move {
            post(
                &client,
                &url,
                &owner,
                &reserve(&format!("race-{i}"), selector),
            )
            .await
        }));
    }
    let mut successes = 0;
    for join in joins {
        let (status, value) = join.await.unwrap();
        assert!([201, 409].contains(&status), "{value}");
        successes += usize::from(status == 201);
    }
    assert_eq!(successes, 1);
    let legacy_task = create(&f, &base, "reservation-legacy-first").await;
    let legacy_assignment = json!({"assignment_id":Uuid::new_v4(),"execution_id":Uuid::new_v4(),"agent_id":selector,"version":1,"machine_subject":subject});
    expect(f.client,&format!("{base}/api/v1/issues/{legacy_task}/sdlc/assignment"),&orch,
        &json!({"assignment":legacy_assignment,"expected_assignment_version":null,"idempotency_key":"legacy-first"}),200).await;
    let executions_before = count(f.db, "sdlc_pm_executions").await;
    expect(
        f.client,
        &endpoint(&base, legacy_task),
        f.owner,
        &reserve("reserve-after-legacy", selector),
        409,
    )
    .await;
    assert_eq!(count(f.db, "sdlc_pm_executions").await, executions_before);
    // Corrupt or absent immutable input must never allocate authority.
    let bad = create(&f, &base, "reservation-input-invalid").await;
    sql(
        f.db,
        "ALTER TABLE sdlc_draft_creations DISABLE TRIGGER sdlc_history_immutable",
        vec![],
    )
    .await;
    assert!(
        f.db.execute_unprepared(&format!(
            "UPDATE sdlc_draft_creations SET input_description=NULL WHERE task_id='{bad}'"
        ))
        .await
        .is_err()
    );
    assert!(f.db.execute_unprepared(&format!("UPDATE sdlc_draft_creations SET input_snapshot_ref='00000000-0000-0000-0000-000000000000' WHERE task_id='{bad}'")).await.is_err());
    sql(
        f.db,
        "UPDATE sdlc_draft_creations SET input_sha256=$2 WHERE task_id=$1",
        vec![bad.into(), "b".repeat(64).into()],
    )
    .await;
    expect(
        f.client,
        &endpoint(&base, bad),
        f.owner,
        &reserve("bad-input", selector),
        409,
    )
    .await;
    get_json(
        f.client,
        &creation_endpoint(&base, f.project, "reservation-input-invalid"),
        f.owner,
        409,
    )
    .await;
    sql(f.db,"UPDATE sdlc_draft_creations SET input_snapshot_ref=NULL,input_title=NULL,input_description=NULL,input_sha256=NULL WHERE task_id=$1",vec![bad.into()]).await;
    expect(
        f.client,
        &endpoint(&base, bad),
        f.owner,
        &reserve("missing-input", selector),
        409,
    )
    .await;
    get_json(
        f.client,
        &creation_endpoint(&base, f.project, "reservation-input-invalid"),
        f.owner,
        409,
    )
    .await;
    sql(
        f.db,
        "ALTER TABLE sdlc_draft_creations ENABLE TRIGGER sdlc_history_immutable",
        vec![],
    )
    .await;
    // Lost body and server restart preserve exactly the same historical/current result.
    let lost_task = create(&f, &base, "reservation-lost-response").await;
    let lost_cmd = reserve("lost-reservation", selector);
    let response = f
        .client
        .post(endpoint(&base, lost_task))
        .bearer_auth(f.owner)
        .json(&lost_cmd)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    drop(response);
    let lost_result =
        get_json(f.client, &endpoint(&base, lost_task), f.owner, 200).await["current"].clone();
    let creation_count = count(f.db, "sdlc_draft_creations").await;
    stop.send(()).unwrap();
    handle.await.unwrap();
    let (base, stop, handle) = start(config.clone()).await;
    let creation_url = creation_endpoint(&base, f.project, creation_key);
    assert_eq!(
        get_json(f.client, &creation_url, f.owner, 200).await,
        created
    );
    assert_eq!(
        expect(
            f.client,
            &format!("{base}/api/v1/projects/{}/sdlc/drafts", f.project),
            f.owner,
            &command(creation_key),
            200
        )
        .await,
        created
    );
    assert_eq!(count(f.db, "sdlc_draft_creations").await, creation_count);
    let url = endpoint(&base, task);
    assert_eq!(
        expect(
            f.client,
            &endpoint(&base, lost_task),
            f.owner,
            &lost_cmd,
            200
        )
        .await,
        lost_result
    );
    assert_eq!(expect(f.client, &url, f.owner, &cmd, 200).await, result);
    assert_eq!(
        get_json(
            f.client,
            &format!("{url}?idempotency_key=reservation"),
            f.owner,
            200
        )
        .await,
        read
    );
    // Configured issue rows require their canonical mutation permit even for a
    // title-only raw update. Ordinary HTTP editing/input isolation is exercised
    // by drafts.rs before this independently bound project is introduced.
    assert!(
        f.db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "UPDATE issues SET summary='mutable changed' WHERE id=$1",
            vec![task.into()]
        ))
        .await
        .is_err()
    );
    assert_eq!(expect(f.client, &url, f.owner, &cmd, 200).await, result);
    // A configured control pointer cannot be detached by a raw legacy update.
    assert!(f.db.execute_unprepared(&format!("UPDATE sdlc_tasks SET pm_execution_id=NULL,pm_owner_version=0,pm_admission_state=NULL WHERE task_id='{task}'")).await.is_err());
    let historical = get_json(
        f.client,
        &format!("{url}?idempotency_key=reservation"),
        f.owner,
        200,
    )
    .await;
    assert_eq!(historical["current"], result);
    assert_eq!(historical["operation"]["result"], result);
    assert_eq!(expect(f.client, &url, f.owner, &cmd, 200).await, result);
    sql(
        f.db,
        "DELETE FROM project_members WHERE project_id=$1 AND user_id=$2",
        vec![f.project.into(), f.owner_id.into()],
    )
    .await;
    // Retained owner operations still require fresh project access, including replay.
    get_json(f.client, &url, f.owner, 403).await;
    get_json(f.client, &creation_url, f.owner, 403).await;
    expect(f.client, &url, f.owner, &cmd, 403).await;
    sql(
        f.db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
        vec![f.project.into(), f.owner_id.into()],
    )
    .await;
    assert_eq!(
        get_json(
            f.client,
            &format!("{url}?idempotency_key=reservation"),
            f.owner,
            200
        )
        .await,
        historical
    );
    assert_eq!(
        get_json(f.client, &creation_url, f.owner, 200).await,
        created
    );
    assert_eq!(expect(f.client, &url, f.owner, &cmd, 200).await, result);
    sql(
        f.db,
        "UPDATE users SET is_active=false WHERE id=$1",
        vec![f.owner_id.into()],
    )
    .await;
    get_json(f.client, &url, f.owner, 403).await;
    get_json(f.client, &creation_url, f.owner, 403).await;
    expect(f.client, &url, f.owner, &cmd, 403).await;
    sql(
        f.db,
        "UPDATE users SET is_active=true WHERE id=$1",
        vec![f.owner_id.into()],
    )
    .await;
    f.unavailable.store(true, Ordering::SeqCst);
    get_json(f.client, &url, f.owner, 503).await;
    get_json(f.client, &creation_url, f.owner, 503).await;
    expect(f.client, &url, f.owner, &cmd, 503).await;
    f.unavailable.store(false, Ordering::SeqCst);
    stop.send(()).unwrap();
    handle.await.unwrap();
    configured_pm_http::verify(config, &f, PM_SUBJECT, selector).await;
}
