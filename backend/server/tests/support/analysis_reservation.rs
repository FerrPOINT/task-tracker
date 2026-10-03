use super::*;

async fn read(client: &Client, url: &str, token: &str, status: u16) -> Value {
    let r = client.get(url).bearer_auth(token).send().await.unwrap();
    let actual = r.status().as_u16();
    if actual == 200 && url.contains("/analysis-reservation") {
        assert_eq!(r.headers()["cache-control"], "no-store");
    }
    let v = r.json().await.unwrap_or(Value::Null);
    assert_eq!(actual, status, "{url}: {v}");
    v
}
async fn publication(
    db: &DatabaseConnection,
    client: &Client,
    base: &str,
    owner: &str,
    ready: &TaskState,
    key: &str,
    version: Option<i64>,
) -> (Uuid, Value) {
    // Disposable PG aggregate fixture without PM execution/history; never live state or a stop proof.
    let task = routing_policy::ready_task(db, ready, key).await;
    let rev = ready.revisions.last().unwrap();
    let mut c = json!({"content_hash":rev.content_hash,"idempotency_key":"confirm"});
    if let Some(v) = version {
        c["expected_routing_policy_version"] = json!(v);
    }
    post(
        client,
        &format!(
            "{base}/api/v1/issues/{task}/sdlc/requirements/{}/confirm",
            rev.revision
        ),
        owner,
        &c,
        200,
    )
    .await;
    let intent = read(
        client,
        &format!("{base}/api/v1/issues/{task}/sdlc/analysis-intent"),
        owner,
        200,
    )
    .await;
    let snapshot = if version.is_some() {
        routing_policy::snapshot(client, base, task, owner).await["snapshot_id"].clone()
    } else {
        json!(Uuid::new_v4())
    };
    (
        task,
        json!({"intent_id":intent["intent_id"],"routing_snapshot_id":snapshot,"requirement_revision":intent["requirement_revision"],"content_hash":intent["content_hash"],"expected_reservation_version":0,"idempotency_key":"reserve"}),
    )
}
async fn new_policy(
    client: &Client,
    base: &str,
    project: Uuid,
    owner: &str,
    version: i64,
) -> Value {
    let url = format!("{base}/api/v1/projects/{project}/sdlc/routing-policy");
    let current = read(client, &url, owner, 200).await;
    let mut c = json!({"expected_version":version,"routes":current["routes"],"idempotency_key":format!("reservation-policy-{version}")});
    c["routes"]["analyst"]["agent_id"] = json!(Uuid::new_v4());
    c["routes"]["analyst"]["fleet_config_revision"] = json!(version + 1);
    post(client, &url, owner, &c, 201).await
}
fn heartbeat(r: &Value, key: &str) -> Value {
    json!({"assignment_id":r["assignment"]["assignment_id"],"execution_id":r["assignment"]["execution_id"],"fencing_token":r["assignment"]["fencing_token"],"lease_id":r["lease"]["lease_id"],"expected_lease_version":r["lease"]["version"],"idempotency_key":key})
}

pub async fn check(
    db: &DatabaseConnection,
    client: &Client,
    base: &str,
    project: Uuid,
    pm_task: Uuid,
    owner: &str,
    ready: &TaskState,
) -> (Uuid, Value) {
    let path = |task| format!("{base}/api/v1/issues/{task}/sdlc/analysis-reservation");
    let scheduler = "sdlc_pat_reservation_scheduler";
    let reader = "sdlc_pat_reservation_reader";
    let pm_intent = read(
        client,
        &format!("{base}/api/v1/issues/{pm_task}/sdlc/analysis-intent"),
        owner,
        200,
    )
    .await;
    let pm_snapshot = routing_policy::snapshot(client, base, pm_task, owner).await;
    let pm = json!({"intent_id":pm_intent["intent_id"],"routing_snapshot_id":pm_snapshot["snapshot_id"],"requirement_revision":pm_intent["requirement_revision"],"content_hash":pm_intent["content_hash"],"expected_reservation_version":0,"idempotency_key":"pm-not-stopped"});
    assert!(
        post(client, &path(pm_task), scheduler, &pm, 409)
            .await
            .to_string()
            .contains("pm_quiescence_unverified")
    );
    let pm_readback = read(client, &path(pm_task), reader, 200).await;
    assert_eq!(pm_readback["reason"], "pm_quiescence_unverified");
    assert_eq!(pm_readback["reconciliation_needed"], true);
    let (history_task, history_command) =
        publication(db, client, base, owner, ready, "SDLC-1005", Some(3)).await;
    let history = PmAssignment {
        assignment_id: Uuid::new_v4(),
        execution_id: Uuid::new_v4(),
        agent_id: Uuid::new_v4(),
        version: 1,
        machine_subject: "pm".into(),
    };
    // Retained PM history with no current assignment is still unknown, not stopped.
    sql(
        db,
        "INSERT INTO sdlc_assignments(task_id,version,payload) VALUES($1,1,$2)",
        vec![
            history_task.into(),
            serde_json::to_value(history).unwrap().into(),
        ],
    )
    .await;
    assert!(
        post(
            client,
            &path(history_task),
            scheduler,
            &history_command,
            409
        )
        .await
        .to_string()
        .contains("pm_quiescence_unverified")
    );
    assert_eq!(count(db, "sdlc_analysis_reservations").await, 0);
    let (legacy, legacy_command) =
        publication(db, client, base, owner, ready, "SDLC-1000", None).await;
    post(client, &path(legacy), scheduler, &legacy_command, 409).await;
    let (a, ca) = publication(db, client, base, owner, ready, "SDLC-1001", Some(3)).await;
    let (b, cb) = publication(db, client, base, owner, ready, "SDLC-1002", Some(3)).await;
    for token in [
        owner,
        "sdlc_pat_fleet",
        "sdlc_pat_reservation_compound",
        reader,
        "sdlc_pat_pm",
    ] {
        post(client, &path(a), token, &ca, 403).await;
    }
    post(client, &path(a), "local-token", &ca, 401).await;
    for field in ["stopped", "native_ready", "agent_id", "ttl_seconds"] {
        let mut c = ca.clone();
        c[field] = json!(true);
        post(client, &path(a), scheduler, &c, 422).await;
    }
    sql(db,"DELETE FROM project_members WHERE project_id=$1 AND user_id=(SELECT id FROM users WHERE central_sub='scheduler')",vec![project.into()]).await;
    post(client, &path(a), scheduler, &ca, 403).await;
    read(client, &path(a), reader, 403).await;
    sql(db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub='scheduler'",vec![project.into()]).await;
    read(client, &path(a), "sdlc_pat_reservation_foreign", 403).await;
    read(client, &path(a), "sdlc_pat_reservation_compound", 403).await;
    sql(
        db,
        "UPDATE users SET is_active=false WHERE central_sub='scheduler'",
        vec![],
    )
    .await;
    post(client, &path(a), scheduler, &ca, 403).await;
    sql(
        db,
        "UPDATE users SET is_active=true WHERE central_sub='scheduler'",
        vec![],
    )
    .await;
    let unreserved = read(client, &path(a), reader, 200).await;
    assert_eq!(unreserved["lease_state"], "unreserved");
    assert_eq!(unreserved["capacity_held"], false);
    // Inject only a disposable DB outbox failure to prove reservation/capacity/receipt atomicity.
    db.execute_unprepared("CREATE FUNCTION reservation_qa_outbox_fail() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_type='analysis.assignment_reserved' THEN RAISE EXCEPTION 'SDLC QA injected outbox failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reservation_qa_outbox_fail BEFORE INSERT ON sdlc_outbox FOR EACH ROW EXECUTE FUNCTION reservation_qa_outbox_fail()").await.unwrap();
    post(client, &path(a), scheduler, &ca, 409).await;
    assert_eq!(count(db, "sdlc_analysis_reservations").await, 0);
    assert_eq!(count(db, "sdlc_analysis_reservation_leases").await, 0);
    assert_eq!(count(db, "sdlc_analysis_reservation_operations").await, 0);
    db.execute_unprepared("DROP TRIGGER reservation_qa_outbox_fail ON sdlc_outbox; DROP FUNCTION reservation_qa_outbox_fail()").await.unwrap();
    sql(
        db,
        "SELECT nextval('sdlc_analysis_workflow_task_ordinal')",
        vec![],
    )
    .await;
    let (ra, rb) = tokio::join!(
        client.post(path(a)).bearer_auth(scheduler).json(&ca).send(),
        client.post(path(b)).bearer_auth(scheduler).json(&cb).send()
    );
    let ra = ra.unwrap();
    let rb = rb.unwrap();
    assert!(matches!(
        (ra.status().as_u16(), rb.status().as_u16()),
        (201, 409) | (409, 201)
    ));
    let (winner, command, result, loser, loser_command) = if ra.status() == 201 {
        (a, ca, ra.json::<Value>().await.unwrap(), b, cb)
    } else {
        (b, cb, rb.json::<Value>().await.unwrap(), a, ca)
    };
    assert_eq!(result["admission_state"], "awaiting_admission");
    assert_eq!(result["dispatch_allowed"], false);
    assert_eq!(result["capacity_held"], true);
    assert_eq!(result["assignment"]["attempt_number"], 1);
    let workflow_ref = result["assignment"]["workflow_task_ref"].as_str().unwrap();
    assert!(
        workflow_ref
            .strip_prefix("SDLC-")
            .unwrap()
            .parse::<i64>()
            .unwrap()
            > 0
    );
    assert_ne!(
        workflow_ref,
        format!("SDLC-{}", result["assignment"]["fencing_token"])
    );
    let mut altered: domain::sdlc_reservation::PreparedAnalysisAssignment =
        serde_json::from_value(result["assignment"].clone()).unwrap();
    altered.workflow_task_ref = "SDLC-999999".into();
    assert_ne!(
        app::sdlc_reservation::assignment_hash(&altered).unwrap(),
        result["assignment"]["assignment_hash"]
    );
    assert!(result["assignment"].get("run_id").is_none());
    let winner_path = path(winner);
    let (x, y) = tokio::join!(
        post(client, &winner_path, scheduler, &command, 200),
        post(client, &winner_path, scheduler, &command, 200)
    );
    assert_eq!(x, result);
    assert_eq!(x, y);
    sql(db,"DELETE FROM project_members WHERE project_id=$1 AND user_id=(SELECT id FROM users WHERE central_sub='scheduler')",vec![project.into()]).await;
    post(client, &path(winner), scheduler, &command, 403).await;
    read(
        client,
        &format!("{}/operations/reserve", path(winner)),
        reader,
        403,
    )
    .await;
    sql(db,"INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub='scheduler'",vec![project.into()]).await;
    let mut c = command.clone();
    c["content_hash"] = json!("f".repeat(64));
    post(client, &path(winner), scheduler, &c, 409).await;
    c = command.clone();
    c["idempotency_key"] = json!("another-root-claim");
    post(client, &path(winner), scheduler, &c, 409).await;
    c = command.clone();
    c["intent_id"] = json!(Uuid::new_v4());
    post(client, &path(winner), scheduler, &c, 409).await;
    assert_eq!(
        read(
            client,
            &format!("{}/operations/reserve", path(winner)),
            reader,
            200
        )
        .await["result"],
        result
    );
    let metadata = read(
        client,
        &format!("{base}/api/v1/issues/{winner}/sdlc/events?projection=metadata_v1"),
        reader,
        200,
    )
    .await;
    if let Ok(dir) = std::env::var("TT_SDLC_TEST_EVIDENCE_DIR") {
        std::fs::write(
            std::path::Path::new(&dir).join("analysis-reservation-metadata-v1.json"),
            serde_json::to_vec_pretty(&metadata).unwrap(),
        )
        .unwrap();
    }
    assert!(
        metadata["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["event_type"] == "analysis.assignment_reserved"
                && e["payload"]["resource"]["assignment_hash"]
                    == result["assignment"]["assignment_hash"])
    );
    new_policy(client, base, project, owner, 3).await;
    let (c, cc) = publication(db, client, base, owner, ready, "SDLC-1003", Some(4)).await;
    new_policy(client, base, project, owner, 4).await;
    let (d, cd) = publication(db, client, base, owner, ready, "SDLC-1004", Some(5)).await;
    let (rc, rd) = tokio::join!(
        client.post(path(c)).bearer_auth(scheduler).json(&cc).send(),
        client.post(path(d)).bearer_auth(scheduler).json(&cd).send()
    );
    let rc = rc.unwrap();
    let rd = rd.unwrap();
    assert!(matches!(
        (rc.status().as_u16(), rd.status().as_u16()),
        (201, 409) | (409, 201)
    ));
    assert_eq!(count(db, "sdlc_analysis_reservations").await, 2);
    assert_eq!(
        read(client, &path(winner), reader, 200).await["current"]["assignment"],
        result["assignment"]
    );
    let hb = heartbeat(&result, "heartbeat-1");
    let mut other = hb.clone();
    other["idempotency_key"] = json!("heartbeat-competing");
    let (hx, hy) = tokio::join!(
        client
            .post(format!("{}/heartbeat", path(winner)))
            .bearer_auth(scheduler)
            .json(&hb)
            .send(),
        client
            .post(format!("{}/heartbeat", path(winner)))
            .bearer_auth(scheduler)
            .json(&other)
            .send()
    );
    let hx = hx.unwrap();
    let hy = hy.unwrap();
    assert!(matches!(
        (hx.status().as_u16(), hy.status().as_u16()),
        (201, 409) | (409, 201)
    ));
    let (hb, second) = if hx.status() == 201 {
        (hb, hx.json::<Value>().await.unwrap())
    } else {
        (other, hy.json::<Value>().await.unwrap())
    };
    let third = post(
        client,
        &format!("{}/heartbeat", path(winner)),
        scheduler,
        &heartbeat(&second, "heartbeat-2"),
        201,
    )
    .await;
    assert_eq!(third["lease"]["version"], 3);
    assert_eq!(third["assignment"], result["assignment"]);
    assert_eq!(
        post(
            client,
            &format!("{}/heartbeat", path(winner)),
            scheduler,
            &hb,
            200
        )
        .await,
        second
    );
    assert_eq!(
        read(client, &path(winner), reader, 200).await["current"],
        third
    );
    for field in ["assignment_id", "execution_id", "lease_id"] {
        let mut bad = heartbeat(&third, "bad-fence");
        bad[field] = json!(Uuid::new_v4());
        post(
            client,
            &format!("{}/heartbeat", path(winner)),
            scheduler,
            &bad,
            409,
        )
        .await;
    }
    let mut bad = heartbeat(&third, "bad-fence");
    bad["fencing_token"] = json!(0);
    post(
        client,
        &format!("{}/heartbeat", path(winner)),
        scheduler,
        &bad,
        422,
    )
    .await;
    bad["fencing_token"] = json!(third["assignment"]["fencing_token"].as_i64().unwrap() + 1);
    post(
        client,
        &format!("{}/heartbeat", path(winner)),
        scheduler,
        &bad,
        409,
    )
    .await;
    let mut changed = hb.clone();
    changed["expected_lease_version"] = json!(3);
    post(
        client,
        &format!("{}/heartbeat", path(winner)),
        scheduler,
        &changed,
        409,
    )
    .await;
    {
        use sea_orm::TransactionTrait;
        // Use one captured clock so the TTL CHECK is exact, then require a matching operation at commit.
        let tx = db.begin().await.unwrap();
        tx.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,"WITH t AS MATERIALIZED (SELECT clock_timestamp() AS now) UPDATE sdlc_analysis_reservation_leases SET version=version+1,heartbeat_at=t.now,expires_at=t.now+interval '30 seconds' FROM t WHERE assignment_id=$1",[Uuid::parse_str(result["assignment"]["assignment_id"].as_str().unwrap()).unwrap().into()])).await.unwrap();
        assert!(
            tx.commit().await.is_err(),
            "lease head without immutable operation receipt must fail"
        );
        assert_eq!(
            read(client, &path(winner), reader, 200).await["current"],
            third
        );
    }
    for query in [
        "UPDATE sdlc_analysis_reservations SET payload=jsonb_set(payload,'{assignment_hash}',to_jsonb(repeat('f',64)))",
        "DELETE FROM sdlc_analysis_reservations",
        "DELETE FROM sdlc_analysis_reservation_leases",
        "DELETE FROM sdlc_analysis_reservation_operations",
        "UPDATE sdlc_analysis_reservation_leases SET version=version+2",
    ] {
        assert!(db.execute_unprepared(query).await.is_err(), "{query}");
    }
    let pm = PmAssignment {
        assignment_id: Uuid::new_v4(),
        execution_id: Uuid::new_v4(),
        agent_id: Uuid::parse_str(result["assignment"]["agent_id"].as_str().unwrap()).unwrap(),
        version: 1,
        machine_subject: "pm".into(),
    };
    assert!(
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "INSERT INTO sdlc_assignments(task_id,version,payload) VALUES($1,1,$2)",
            [legacy.into(), serde_json::to_value(pm).unwrap().into()]
        ))
        .await
        .is_err()
    );
    println!(
        "ANALYSIS_RESERVATION concurrency/root1-agent1-pool2/ACL/CAS/immutable hash/outbox rollback/old replay passed; waiting real TTL"
    );
    tokio::time::sleep(std::time::Duration::from_secs(31)).await;
    let expired = read(client, &path(winner), reader, 200).await;
    assert_eq!(expired["lease_state"], "expired");
    assert_eq!(expired["reconciliation_needed"], true);
    assert_eq!(expired["reason"], "lease_expired_stop_unverified");
    assert_eq!(expired["capacity_held"], true);
    assert_eq!(expired["current"], third);
    assert_eq!(
        read(client, &path(winner), reader, 200).await["current"],
        third
    );
    assert_eq!(
        post(client, &path(winner), scheduler, &command, 200).await,
        result
    );
    assert_eq!(
        post(
            client,
            &format!("{}/heartbeat", path(winner)),
            scheduler,
            &hb,
            200
        )
        .await,
        second
    );
    post(
        client,
        &format!("{}/heartbeat", path(winner)),
        scheduler,
        &heartbeat(&third, "after-expiry"),
        409,
    )
    .await;
    post(client, &path(loser), scheduler, &loser_command, 409).await;
    let mut replacement = cc.clone();
    replacement["idempotency_key"] = json!("replacement");
    post(client, &path(c), scheduler, &replacement, 409).await;
    assert_eq!(count(db, "sdlc_analysis_reservations").await, 2);
    println!(
        "ANALYSIS_RESERVATION real expiry/read-only GET/unknown capacity hold/no release/no replay rewind passed"
    );
    (winner, third)
}
