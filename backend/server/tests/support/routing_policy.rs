use super::*;

pub(super) struct Actors<'a> {
    pub owner: &'a str,
    pub operator: &'a str,
    pub foreign: &'a str,
}

async fn read(client: &Client, url: &str, token: &str, status: u16) -> Value {
    let response = client.get(url).bearer_auth(token).send().await.unwrap();
    let actual = response.status().as_u16();
    if actual == 200 {
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    let value = response.json().await.unwrap_or(Value::Null);
    assert_eq!(actual, status, "routing readback {url}: {value}");
    value
}

fn command() -> Value {
    let mut routes = json!({});
    for (i, role) in [
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
        let profile = match *role {
            "tester" => "hermes-sdlc-quality".into(),
            "devops" => "hermes-sdlc-operations".into(),
            _ => format!("hermes-sdlc-{suffix}"),
        };
        routes[*role] = json!({"agent_id":Uuid::new_v4(), "fleet_config_revision":1,
            "package_commit":domain::sdlc_routing::PACKAGE_COMMIT,"package_manifest_sha256":"a".repeat(64),
            "namespace_id":(i+1).to_string(),"namespace_name":format!("hermes-{suffix}"),
            "workflow_id":(i+11).to_string(),"workflow_key":format!("hermes-sdlc:{role}"),
            "profile":profile,"workflow_catalog_version":3,"workflow_catalog_sha256":"b".repeat(64)});
    }
    json!({"expected_version":null,"routes":routes,"idempotency_key":"routing-initial"})
}

pub(super) async fn before(
    db: &DatabaseConnection,
    client: &Client,
    base: &str,
    project: Uuid,
    task: Uuid,
    actors: &Actors<'_>,
) -> (Value, TaskState) {
    let owner = actors.owner;
    let operator = actors.operator;
    let foreign = actors.foreign;
    let url = format!("{base}/api/v1/projects/{project}/sdlc/routing-policy");
    let body = command();
    for token in [
        operator,
        foreign,
        "sdlc_pat_owner",
        "sdlc_pat_fleet",
        "local-token",
    ] {
        post(
            client,
            &url,
            token,
            &body,
            if token == "local-token" { 401 } else { 403 },
        )
        .await;
    }
    assert_eq!(count(db, "sdlc_project_routing_revisions").await, 0);
    let initial = post(client, &url, owner, &body, 201).await;
    assert_eq!(initial["version"], 1);
    assert_eq!(initial["verification"], "declared");
    assert_eq!(initial["native_ready"], false);
    assert_eq!(initial["dispatch_allowed"], false);
    assert_eq!(
        initial,
        read(client, &url, "sdlc_pat_wrong_scope", 200).await
    );
    assert_eq!(
        initial,
        read(
            client,
            &format!("{url}/operations/routing-initial"),
            owner,
            200
        )
        .await
    );
    let (a, b) = tokio::join!(
        post(client, &url, owner, &body, 200),
        post(client, &url, owner, &body, 200)
    );
    assert_eq!(a, b);
    assert_eq!(a, initial);
    let mut changed = body.clone();
    changed["routes"]["analyst"]["fleet_config_revision"] = json!(2);
    post(client, &url, owner, &changed, 409).await;
    changed["idempotency_key"] = json!("stale-create");
    post(client, &url, owner, &changed, 409).await;
    let mut invalid = body.clone();
    invalid["expected_version"] = json!(1);
    invalid["idempotency_key"] = json!("invalid-agent");
    invalid["routes"]["analyst"]["agent_id"] =
        invalid["routes"]["project_manager"]["agent_id"].clone();
    post(client, &url, owner, &invalid, 422).await;
    invalid["routes"]["analyst"]["agent_id"] = json!(Uuid::new_v4());
    invalid["routes"]["analyst"]["runtime_ready"] = json!(true);
    post(client, &url, owner, &invalid, 422).await;
    let mut left = body.clone();
    left["expected_version"] = json!(1);
    left["idempotency_key"] = json!("left-update");
    left["routes"]["analyst"]["fleet_config_revision"] = json!(2);
    let mut right = left.clone();
    right["idempotency_key"] = json!("right-update");
    right["routes"]["analyst"]["fleet_config_revision"] = json!(3);
    let (left, right) = tokio::join!(
        client.post(&url).bearer_auth(owner).json(&left).send(),
        client.post(&url).bearer_auth(owner).json(&right).send()
    );
    let left = left.unwrap();
    let right = right.unwrap();
    assert!(matches!(
        (left.status().as_u16(), right.status().as_u16()),
        (201, 409) | (409, 201)
    ));
    let current = read(client, &url, owner, 200).await;
    assert_eq!(current["version"], 2);
    assert_eq!(initial, post(client, &url, owner, &body, 200).await);
    assert_eq!(
        initial,
        read(client, &format!("{url}/versions/1"), owner, 200).await
    );
    assert_eq!(count(db, "sdlc_project_routing_revisions").await, 2);
    assert_eq!(count(db, "sdlc_task_routing_snapshots").await, 0);
    read(
        client,
        &format!("{base}/api/v1/issues/{task}/sdlc/routing-snapshot"),
        owner,
        404,
    )
    .await;
    read(client, &url, foreign, 403).await;
    read(client, &url, "sdlc_pat_pm", 403).await;
    sql(
        db,
        "UPDATE users SET is_active=false WHERE central_sub='owner'",
        vec![],
    )
    .await;
    post(client, &url, owner, &body, 403).await;
    read(
        client,
        &format!("{url}/operations/routing-initial"),
        owner,
        403,
    )
    .await;
    sql(
        db,
        "UPDATE users SET is_active=true WHERE central_sub='owner'",
        vec![],
    )
    .await;
    sql(db,"DELETE FROM project_members WHERE project_id=$1 AND user_id=(SELECT id FROM users WHERE central_sub='pm')",vec![project.into()]).await;
    read(client, &url, "sdlc_pat_wrong_scope", 403).await;
    sql(db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub='pm'",vec![project.into()]).await;
    let state: TaskState = serde_json::from_value(
        db.query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT state FROM sdlc_tasks WHERE task_id=$1",
            [task.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Value>("", "state")
        .unwrap(),
    )
    .unwrap();
    println!(
        "ROUTING_POLICY owner/central/session/ACL/replay/concurrent CAS/strict pins/no automatic enrollment passed"
    );
    (current, state)
}

pub async fn snapshot(client: &Client, base: &str, task: Uuid, owner: &str) -> Value {
    read(
        client,
        &format!("{base}/api/v1/issues/{task}/sdlc/routing-snapshot"),
        owner,
        200,
    )
    .await
}

pub(super) async fn ready_task(db: &DatabaseConnection, source: &TaskState, key: &str) -> Uuid {
    // A legacy ready aggregate fixture, not an invented assignment or runtime receipt.
    let task = Uuid::new_v4();
    let mut state = source.clone();
    state.task_id = task;
    state.root_task_id = task;
    state.assignment = None;
    state.questions.clear();
    state.confirmations.clear();
    sql(db,"INSERT INTO issues(id,project_id,key,issue_type,status_id,summary,reporter_id,priority,labels,position,time_spent_seconds) SELECT $1,$2,$3,'task',(SELECT id FROM statuses WHERE name='SDLC Clarification' LIMIT 1),'Routing fixture',id,'medium','[]',0,0 FROM users WHERE central_sub=$4",vec![task.into(),state.project_id.into(),key.into(),state.owner_subject.clone().into()]).await;
    sql(db,"INSERT INTO sdlc_tasks(task_id,tracker_instance_id,project_id,root_task_id,owner_subject,state) VALUES($1,$2,$3,$1,$4,$5)",vec![task.into(),state.tracker_instance_id.clone().into(),state.project_id.into(),state.owner_subject.clone().into(),serde_json::to_value(&state).unwrap().into()]).await;
    for revision in &state.revisions {
        sql(db,"INSERT INTO sdlc_requirements(task_id,revision,content_hash,payload) VALUES($1,$2,$3,$4)",vec![task.into(),revision.revision.into(),revision.content_hash.clone().into(),serde_json::to_value(revision).unwrap().into()]).await;
    }
    task
}

pub(super) async fn after(
    db: &DatabaseConnection,
    client: &Client,
    base: &str,
    task: Uuid,
    actors: &Actors<'_>,
    old: &Value,
    ready: &TaskState,
) {
    let project = ready.project_id;
    let owner = actors.owner;
    let foreign = actors.foreign;
    assert_eq!(old, &snapshot(client, base, task, owner).await);
    let url = format!("{base}/api/v1/projects/{project}/sdlc/routing-policy");
    let mut update = json!({"expected_version":2,"routes":old["policy"]["routes"],"idempotency_key":"next-policy"});
    update["routes"]["analyst"]["fleet_config_revision"] = json!(4);
    update["routes"]["analyst"]["agent_id"] = json!(Uuid::new_v4());
    let next = post(client, &url, owner, &update, 201).await;
    assert_eq!(next["version"], 3);
    assert_eq!(old, &snapshot(client, base, task, owner).await);
    let mut replay = json!({"content_hash":old["content_hash"],"idempotency_key":"confirm-final","expected_routing_policy_version":2});
    let consent_url = format!("{base}/api/v1/issues/{task}/sdlc/requirements/2/confirm");
    assert_eq!(
        post(client, &consent_url, owner, &replay, 200).await["id"],
        old["confirmation_id"]
    );
    replay["expected_routing_policy_version"] = json!(3);
    post(client, &consent_url, owner, &replay, 409).await;
    assert_eq!(
        old,
        &read(
            client,
            &format!("{base}/api/v1/issues/{task}/sdlc/routing-snapshot"),
            "sdlc_pat_wrong_scope",
            200
        )
        .await
    );
    for (field, value) in [
        (
            "agent_id",
            next["routes"]["project_manager"]["agent_id"].clone(),
        ),
        ("namespace_id", json!("9223372036854775808")),
        ("workflow_id", json!("012")),
        ("package_commit", json!("HEAD")),
        ("workflow_catalog_version", json!(2)),
    ] {
        let mut invalid = next.clone();
        invalid["version"] = json!(4);
        invalid["routes"]["analyst"][field] = value;
        assert!(db.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
            "INSERT INTO sdlc_project_routing_revisions(project_id,version,tracker_instance_id,routing_hash,payload) VALUES($1,4,$2,$3,$4)",
            [project.into(),ready.tracker_instance_id.clone().into(),next["routing_hash"].as_str().unwrap().into(),invalid.into()])).await.is_err(), "SQL must reject {field}");
    }
    for field in ["native_ready", "dispatch_allowed"] {
        let mut invalid = next.clone();
        invalid["version"] = json!(4);
        invalid[field] = json!(true);
        assert!(db.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
            "INSERT INTO sdlc_project_routing_revisions(project_id,version,tracker_instance_id,routing_hash,payload) VALUES($1,4,$2,$3,$4)",
            [project.into(),ready.tracker_instance_id.clone().into(),next["routing_hash"].as_str().unwrap().into(),invalid.into()])).await.is_err());
    }
    assert_eq!(count(db, "sdlc_project_routing_revisions").await, 3);
    let current_task = ready_task(db, ready, "SDLC-900").await;
    let legacy_task = ready_task(db, ready, "SDLC-901").await;
    let hash = &ready.revisions.last().unwrap().content_hash;
    for (id, version) in [(current_task, Some(3)), (legacy_task, None)] {
        let mut command = json!({"content_hash":hash,"idempotency_key":"publish"});
        if let Some(v) = version {
            command["expected_routing_policy_version"] = json!(v);
        }
        post(
            client,
            &format!("{base}/api/v1/issues/{id}/sdlc/requirements/2/confirm"),
            owner,
            &command,
            200,
        )
        .await;
    }
    let frozen = snapshot(client, base, current_task, owner).await;
    assert_eq!(frozen["policy"], next);
    assert_eq!(old, &snapshot(client, base, task, owner).await);
    read(
        client,
        &format!("{base}/api/v1/issues/{legacy_task}/sdlc/routing-snapshot"),
        owner,
        404,
    )
    .await;
    read(
        client,
        &format!("{base}/api/v1/issues/{task}/sdlc/routing-snapshot"),
        foreign,
        403,
    )
    .await;
    read(
        client,
        &format!("{base}/api/v1/issues/{task}/sdlc/routing-snapshot"),
        "sdlc_pat_pm",
        403,
    )
    .await;
    assert!(
        db.execute_unprepared(
            "UPDATE sdlc_project_routing_revisions SET routing_hash=repeat('f',64)"
        )
        .await
        .is_err()
    );
    assert!(
        db.execute_unprepared("DELETE FROM sdlc_task_routing_snapshots")
            .await
            .is_err()
    );
    assert!(
        db.execute_unprepared("UPDATE sdlc_project_routing_heads SET version=1")
            .await
            .is_err()
    );
    let task_ref = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT routing_snapshot_id FROM sdlc_analysis_intents WHERE task_id=$1",
            [task.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Uuid>("", "routing_snapshot_id")
        .unwrap();
    assert_eq!(old["snapshot_id"], task_ref.to_string());
    assert_eq!(count(db, "sdlc_task_routing_snapshots").await, 2);
    println!(
        "TASK_ROUTING publication/restart/immutable old snapshot/new-only policy/explicit legacy opt-out/ACL passed"
    );
}
