use super::*;

pub async fn verify(
    config: Arc<shared::AppConfig>,
    f: &Fixture<'_>,
    subject: &str,
    verifier: &str,
) {
    let (base, stop, handle) = start(config.clone()).await;
    let task = create(f, &base, "execution-lease-source").await;
    let assignment = expect(
        f.client,
        &endpoint(&base, task),
        f.owner,
        &reserve("lease-source-reservation", Uuid::new_v4()),
        201,
    )
    .await;
    let a = &assignment["assignment"];
    let fence = json!({"assignment_id":a["assignment_id"],"execution_id":a["execution_id"],
        "agent_id":a["agent_id"],"assignment_version":a["version"]});
    let grant = format!(
        "task-tracker:sdlc:pm:{task}:{}:{}:{}:1",
        a["assignment_id"].as_str().unwrap(),
        a["execution_id"].as_str().unwrap(),
        a["agent_id"].as_str().unwrap()
    );
    let token = machine(
        f.secret,
        f.issuer,
        subject,
        vec![
            "task-tracker:read".into(),
            "task-tracker:write".into(),
            grant.clone(),
        ],
    );
    let no_grant = machine(
        f.secret,
        f.issuer,
        subject,
        vec!["task-tracker:read".into(), "task-tracker:write".into()],
    );
    let foreign = machine(
        f.secret,
        f.issuer,
        verifier,
        vec![
            "task-tracker:read".into(),
            "task-tracker:write".into(),
            grant,
        ],
    );
    let url = format!("{base}/api/v1/issues/{task}/sdlc/pm-draft-execution-lease");
    let claim = json!({"expected_owner_version":1,"fence":fence,"idempotency_key":"lease-claim"});
    let empty = get_json(f.client, &url, &token, 200).await;
    assert_eq!(empty["state"], "unclaimed");
    assert!(empty["current"].is_null() && empty["operation"].is_null());
    assert_eq!(empty["dispatch_allowed"], false);
    let unknown = json!({"expected_owner_version":1,"fence":fence,"lease_id":Uuid::new_v4(),"expected_lease_version":1,"idempotency_key":"unknown"});
    expect(f.client, &format!("{url}/heartbeat"), &token, &unknown, 409).await;
    for denied in [f.owner, f.operator, &no_grant, &foreign] {
        get_json(f.client, &url, denied, 403).await;
        expect(f.client, &url, denied, &claim, 403).await;
        expect(f.client, &format!("{url}/heartbeat"), denied, &unknown, 403).await;
    }
    assert_eq!(f.client.get(&url).send().await.unwrap().status(), 401);
    let mut stale = claim.clone();
    stale["expected_owner_version"] = json!(2);
    expect(f.client, &url, &token, &stale, 409).await;
    for field in ["assignment_id", "execution_id", "agent_id"] {
        let mut c = claim.clone();
        c["fence"][field] = json!(Uuid::new_v4());
        expect(f.client, &url, &token, &c, 409).await;
    }
    for bad in [
        json!({}),
        {
            let mut c = claim.clone();
            c["holder_subject"] = json!(subject);
            c
        },
        {
            let mut c = claim.clone();
            c["expected_owner_version"] = json!(0);
            c
        },
        {
            let mut c = claim.clone();
            c["ttl_seconds"] = json!(31);
            c
        },
        {
            let mut c = claim.clone();
            c["fence"]["execution_id"] = json!(Uuid::nil());
            c
        },
    ] {
        expect(f.client, &url, &token, &bad, 422).await;
    }
    // Actual duplicate HTTP claims serialize at the Tracker aggregate, not in a test mock.
    let mut joins = vec![];
    for _ in 0..8 {
        let (client, url, token, claim) =
            (f.client.clone(), url.clone(), token.clone(), claim.clone());
        joins.push(tokio::spawn(async move {
            post(&client, &url, &token, &claim).await
        }));
    }
    let mut first = None;
    let mut created = 0;
    for j in joins {
        let (status, result) = j.await.unwrap();
        assert!([200, 201].contains(&status), "{result}");
        created += usize::from(status == 201);
        if let Some(ref first) = first {
            assert_eq!(first, &result);
        } else {
            first = Some(result);
        }
    }
    assert_eq!(created, 1);
    let first = first.unwrap();
    assert_eq!(first.as_object().unwrap().len(), 8);
    assert_eq!(first["lease"].as_object().unwrap().len(), 6);
    assert_eq!(first["lease"]["version"], 1);
    assert_eq!(first["lease"]["holder_subject"], subject);
    assert_eq!(first["owner_version"], 1);
    assert_eq!(first["ttl_seconds"], 30);
    assert_eq!(first["heartbeat_seconds"], 10);
    assert_eq!(first["dispatch_allowed"], false);
    for name in ["claimed_at", "heartbeat_at", "expires_at"] {
        let timestamp = first["lease"][name].as_str().unwrap();
        assert!(timestamp.ends_with('Z') && timestamp.split('.').nth(1).unwrap().len() == 10);
    }
    let parse = |value: &Value| {
        value
            .as_str()
            .unwrap()
            .parse::<shared::Timestamp>()
            .unwrap()
    };
    assert_eq!(
        (parse(&first["lease"]["expires_at"]) - parse(&first["lease"]["heartbeat_at"]))
            .num_seconds(),
        30
    );
    assert_eq!(
        curl_json(&format!("{url}?idempotency_key=lease-claim"), &token, None).await["operation"]["result"],
        first
    );
    let mut another = claim.clone();
    another["idempotency_key"] = json!("different-claim-key");
    expect(f.client, &url, &token, &another, 409).await;
    let heartbeat = json!({"expected_owner_version":1,"fence":fence,"lease_id":first["lease"]["lease_id"],"expected_lease_version":1,"idempotency_key":"heartbeat-1"});
    let hb_url = format!("{url}/heartbeat");
    let response = f
        .client
        .post(&hb_url)
        .bearer_auth(&token)
        .json(&heartbeat)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    drop(response); // Unknown accepted renewal, recovered via GET before replay.
    let live = get_json(
        f.client,
        &format!("{url}?idempotency_key=heartbeat-1"),
        &token,
        200,
    )
    .await;
    let second = live["operation"]["result"].clone();
    assert_eq!(second["lease"]["version"], 2);
    assert_eq!(live["current"], second["lease"]);
    assert_eq!(live["state"], "active");
    assert_eq!(
        expect(f.client, &hb_url, &token, &heartbeat, 200).await,
        second
    );
    let mut changed = heartbeat.clone();
    changed["expected_lease_version"] = json!(2);
    expect(f.client, &hb_url, &token, &changed, 409).await;
    let mut wrong_id = heartbeat.clone();
    wrong_id["lease_id"] = json!(Uuid::new_v4());
    wrong_id["idempotency_key"] = json!("wrong-id");
    expect(f.client, &hb_url, &token, &wrong_id, 409).await;
    let mut stale_owner = heartbeat.clone();
    stale_owner["expected_owner_version"] = json!(2);
    expect(f.client, &hb_url, &token, &stale_owner, 409).await;
    let mut cross_operation = heartbeat.clone();
    cross_operation["idempotency_key"] = json!("lease-claim");
    expect(f.client, &hb_url, &token, &cross_operation, 409).await;
    let mut same = heartbeat.clone();
    same["idempotency_key"] = json!("concurrent-heartbeat");
    same["expected_lease_version"] = json!(2);
    let mut joins = vec![];
    for _ in 0..8 {
        let (client, url, token, command) = (
            f.client.clone(),
            hb_url.clone(),
            token.clone(),
            same.clone(),
        );
        joins.push(tokio::spawn(async move {
            expect(&client, &url, &token, &command, 200).await
        }));
    }
    let mut third = None;
    for j in joins {
        let result = j.await.unwrap();
        if let Some(ref third) = third {
            assert_eq!(third, &result);
        } else {
            third = Some(result);
        }
    }
    let third = third.unwrap();
    assert_eq!(third["lease"]["version"], 3);
    let mut joins = vec![];
    for n in 0..6 {
        let mut command = heartbeat.clone();
        command["expected_lease_version"] = json!(3);
        command["idempotency_key"] = json!(format!("race-{n}"));
        let (client, url, token) = (f.client.clone(), hb_url.clone(), token.clone());
        joins.push(tokio::spawn(async move {
            post(&client, &url, &token, &command).await
        }));
    }
    let mut won = 0;
    for j in joins {
        let (status, result) = j.await.unwrap();
        assert!([200, 409].contains(&status), "{result}");
        won += usize::from(status == 200);
    }
    assert_eq!(won, 1);
    let live = get_json(f.client, &url, &token, 200).await;
    assert_eq!(live["current"]["version"], 4);
    assert_eq!(count(f.db, "sdlc_pm_execution_leases").await, 1);
    assert_eq!(count(f.db, "sdlc_pm_lease_operations").await, 4);
    assert!(
        migration::MigrationTrait::down(
            &migration::m20261001_0000034_sdlc_clarification::Migration,
            &migration::SchemaManager::new(f.db),
        )
        .await
        .is_err()
    );
    assert_eq!(count(f.db, "sdlc_pm_execution_leases").await, 1);
    let saved = get_json(
        f.client,
        &format!("{url}?idempotency_key=lease-claim"),
        &token,
        200,
    )
    .await;
    assert_eq!(saved["operation"]["result"], first);
    assert_eq!(saved["current"], live["current"]);
    assert_eq!(
        get_json(f.client, &endpoint(&base, task), f.owner, 200).await["current"],
        assignment
    );
    let pm_write = json!({"fence":fence,"document":{"goal":"goal","scope":["scope"],"exclusions":[],"scenarios":["scenario"],"acceptance_criteria":["criterion"],"constraints":[],"dependencies":[],"assumptions":[],"checklist":["check"],"prerequisites":["prerequisite"]},"expected_requirement_revision":null,"idempotency_key":"lease-is-not-admission"});
    expect(
        f.client,
        &format!("{base}/api/v1/issues/{task}/sdlc/requirements"),
        &token,
        &pm_write,
        409,
    )
    .await;
    let execution_id = Uuid::parse_str(a["execution_id"].as_str().unwrap()).unwrap();
    assert!(
        f.db.execute(statement(
            "DELETE FROM sdlc_pm_execution_leases WHERE execution_id=$1",
            vec![execution_id.into()]
        ))
        .await
        .is_err()
    );
    assert!(
        f.db.execute(statement(
            "UPDATE sdlc_pm_execution_leases SET version=version+1 WHERE execution_id=$1",
            vec![execution_id.into()]
        ))
        .await
        .is_err()
    );
    assert!(
        f.db.execute(statement(
            "DELETE FROM sdlc_pm_lease_operations WHERE execution_id=$1",
            vec![execution_id.into()]
        ))
        .await
        .is_err()
    );
    // Corrupt retained history on this disposable DB, then prove read/replay/renew fail closed.
    let original_history =
        f.db.query_one(statement(
            "SELECT result FROM sdlc_pm_lease_operations WHERE execution_id=$1 AND lease_version=4",
            vec![execution_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Value>("", "result")
        .unwrap();
    f.db.execute_unprepared(
        "ALTER TABLE sdlc_pm_lease_operations DISABLE TRIGGER sdlc_history_immutable",
    )
    .await
    .unwrap();
    sql(f.db,"UPDATE sdlc_pm_lease_operations SET result=result || '{\"unexpected\":true}'::jsonb WHERE execution_id=$1 AND lease_version=4",vec![execution_id.into()]).await;
    get_json(f.client, &url, &token, 409).await;
    expect(f.client, &url, &token, &claim, 409).await;
    expect(f.client, &hb_url, &token, &heartbeat, 409).await;
    sql(
        f.db,
        "UPDATE sdlc_pm_lease_operations SET result=$2 WHERE execution_id=$1 AND lease_version=4",
        vec![execution_id.into(), original_history.into()],
    )
    .await;
    f.db.execute_unprepared(
        "ALTER TABLE sdlc_pm_lease_operations ENABLE TRIGGER sdlc_history_immutable",
    )
    .await
    .unwrap();
    let machine_id: Uuid =
        f.db.query_one(statement(
            "SELECT id FROM users WHERE central_sub=$1",
            vec![subject.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap();
    sql(
        f.db,
        "DELETE FROM project_members WHERE project_id=$1 AND user_id=$2",
        vec![f.project.into(), machine_id.into()],
    )
    .await;
    for route in [&url, &format!("{url}?idempotency_key=lease-claim")] {
        get_json(f.client, route, &token, 403).await;
    }
    expect(f.client, &url, &token, &claim, 403).await;
    expect(f.client, &hb_url, &token, &heartbeat, 403).await;
    sql(
        f.db,
        "INSERT INTO project_members(project_id,user_id,role) VALUES($1,$2,'member')",
        vec![f.project.into(), machine_id.into()],
    )
    .await;
    sql(
        f.db,
        "UPDATE users SET is_active=false WHERE id=$1",
        vec![machine_id.into()],
    )
    .await;
    get_json(f.client, &url, &token, 403).await;
    expect(f.client, &url, &token, &claim, 403).await;
    expect(f.client, &hb_url, &token, &heartbeat, 403).await;
    sql(
        f.db,
        "UPDATE users SET is_active=true WHERE id=$1",
        vec![machine_id.into()],
    )
    .await;
    f.unavailable.store(true, Ordering::SeqCst);
    get_json(f.client, &url, &token, 503).await;
    expect(f.client, &url, &token, &claim, 503).await;
    expect(f.client, &hb_url, &token, &heartbeat, 503).await;
    f.unavailable.store(false, Ordering::SeqCst);
    stop.send(()).unwrap();
    handle.await.unwrap();
    let (base, stop, handle) = start(config).await;
    let url = format!("{base}/api/v1/issues/{task}/sdlc/pm-draft-execution-lease");
    let after_restart = get_json(
        f.client,
        &format!("{url}?idempotency_key=lease-claim"),
        &token,
        200,
    )
    .await;
    assert_eq!(after_restart["current"], live["current"]);
    assert_eq!(after_restart["operation"]["result"], first);
    // A command admitted before expiry must recheck the database clock after locking.
    let lock = f.db.begin().await.unwrap();
    lock.query_one(statement(
        "SELECT id FROM issues WHERE id=$1 FOR UPDATE",
        vec![task.into()],
    ))
    .await
    .unwrap()
    .unwrap();
    let blocker: i32 = lock
        .query_one(statement("SELECT pg_backend_pid() AS pid", vec![]))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "pid")
        .unwrap();
    assert!(lock
        .query_one(statement(
            "SELECT expires_at > clock_timestamp() AS active FROM sdlc_pm_execution_leases WHERE execution_id=$1",
            vec![execution_id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<bool>("", "active")
        .unwrap());
    let mut waiting_command = heartbeat.clone();
    waiting_command["idempotency_key"] = json!("lock-wait-crossing-expiry");
    waiting_command["expected_lease_version"] = json!(4);
    let (client, waiting_url, waiting_token) = (
        Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap(),
        format!("{url}/heartbeat"),
        token.clone(),
    );
    let waiting =
        tokio::spawn(
            async move { post(&client, &waiting_url, &waiting_token, &waiting_command).await },
        );
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let blocked: bool = f.db
                .query_one(statement(
                    "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event_type='Lock' AND $1=ANY(pg_blocking_pids(pid))) AS blocked",
                    vec![blocker.into()],
                ))
                .await
                .unwrap()
                .unwrap()
                .try_get("", "blocked")
                .unwrap();
            if blocked {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("heartbeat must actually wait on the held issue lock");
    tokio::time::sleep(std::time::Duration::from_secs(31)).await;
    lock.commit().await.unwrap();
    let (status, result) = waiting.await.unwrap();
    assert_eq!(status, 409, "{result}");
    assert_eq!(count(f.db, "sdlc_pm_lease_operations").await, 4);
    let expired = get_json(
        f.client,
        &format!("{url}?idempotency_key=heartbeat-1"),
        &token,
        200,
    )
    .await;
    assert_eq!(expired["state"], "expired");
    assert_eq!(expired["current"], live["current"]);
    assert_eq!(expired["operation"]["result"], second);
    assert_eq!(expect(f.client, &url, &token, &claim, 200).await, first);
    assert_eq!(
        expect(
            f.client,
            &format!("{url}/heartbeat"),
            &token,
            &heartbeat,
            200
        )
        .await,
        second
    );
    let mut nonce = heartbeat;
    nonce["idempotency_key"] = json!("expired-nonce");
    nonce["expected_lease_version"] = json!(4);
    expect(f.client, &format!("{url}/heartbeat"), &token, &nonce, 409).await;
    expect(f.client, &url, &token, &another, 409).await;
    assert_eq!(
        get_json(f.client, &url, &token, 200).await["current"],
        live["current"]
    );
    assert_eq!(
        get_json(f.client, &endpoint(&base, task), f.owner, 200).await["current"],
        assignment
    );
    stop.send(()).unwrap();
    handle.await.unwrap();
}

fn statement(sql: &str, values: Vec<sea_orm::Value>) -> Statement {
    Statement::from_sql_and_values(DatabaseBackend::Postgres, sql, values)
}
