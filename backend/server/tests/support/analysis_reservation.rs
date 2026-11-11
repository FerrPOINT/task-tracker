use super::*;
use domain::sdlc_reservation::*;
use sea_orm::TransactionTrait;

pub(super) async fn read(client: &Client, url: &str, token: &str, status: u16) -> Value {
    let r = client.get(url).bearer_auth(token).send().await.unwrap();
    let actual = r.status().as_u16();
    if actual == 200 && url.contains("/analysis-reservation") {
        assert_eq!(r.headers()["cache-control"], "no-store");
    }
    let v = r.json().await.unwrap_or(Value::Null);
    assert_eq!(actual, status, "{url}: {v}");
    v
}

async fn business_snapshot(db: &DatabaseConnection) -> Value {
    let mut snapshot = json!({});
    for table in [
        "users",
        "projects",
        "project_members",
        "issues",
        "issue_status_history",
        "sdlc_tasks",
        "sdlc_assignments",
        "sdlc_pm_executions",
        "sdlc_pm_execution_leases",
        "sdlc_pm_completions",
        "sdlc_confirmations",
        "sdlc_task_routing_snapshots",
        "sdlc_analysis_intents",
        "sdlc_analysis_reservations",
        "sdlc_analysis_reservation_leases",
        "sdlc_analysis_reservation_operations",
        "sdlc_analysis_results",
        "sdlc_analysis_result_operations",
        "sdlc_analysis_capacity_holds",
        "sdlc_delivery_heads",
        "sdlc_delivery_holds",
        "sdlc_idempotency",
        "sdlc_outbox",
    ] {
        let row = db
            .query_one(Statement::from_string(
                DatabaseBackend::Postgres,
                format!(
                    "SELECT coalesce(jsonb_agg(to_jsonb(r) ORDER BY to_jsonb(r)::text),'[]'::jsonb) AS value FROM {table} r"
                ),
            ))
            .await
            .unwrap()
            .unwrap();
        snapshot[table] = row.try_get::<Value>("", "value").unwrap();
    }
    snapshot
}

async fn seed_retained_history(
    db: &DatabaseConnection,
    client: &Client,
    base: &str,
    owner: &str,
    ready: &TaskState,
) -> [(String, Value); 2] {
    let name: String = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT current_database() AS name",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "name")
        .unwrap();
    assert!(name.ends_with("_sdlc_test"));
    // Reuse the accepted historical aggregate helpers, never a retired HTTP writer.
    let task = routing_policy::ready_task(db, ready, "SDLC-902").await;
    let root = format!("{base}/api/v1/issues/{task}/sdlc");
    let policy = read(
        client,
        &format!(
            "{base}/api/v1/projects/{}/sdlc/routing-policy",
            ready.project_id
        ),
        owner,
        200,
    )
    .await;
    let revision = ready.revisions.last().unwrap();
    post(
        client,
        &format!("{root}/requirements/{}/confirm", revision.revision),
        owner,
        &json!({"content_hash":revision.content_hash,"idempotency_key":"historical-confirm",
            "expected_routing_policy_version":policy["version"]}),
        200,
    )
    .await;
    analysis_intent::seed_historical_analysis(db, task).await;
    let intent: AnalysisIntent =
        serde_json::from_value(analysis_intent::read(client, &root, owner, 200).await).unwrap();
    let snapshot: domain::sdlc_routing::TaskRoutingSnapshot =
        serde_json::from_value(routing_policy::snapshot(client, base, task, owner).await).unwrap();
    let tx = db.begin().await.unwrap();
    let row = tx
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT state FROM sdlc_tasks WHERE task_id=$1 AND pm_execution_id IS NULL FOR UPDATE",
            [task.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    let mut state: TaskState = serde_json::from_value(row.try_get("", "state").unwrap()).unwrap();
    assert!(state.assignment.is_none() && !state.configured_cli());
    assert_eq!(state.stage, Stage::Analysis);
    let sequence = tx.query_one(Statement::from_string(DatabaseBackend::Postgres,
        "SELECT nextval('sdlc_analysis_workflow_task_ordinal') AS ordinal,nextval('sdlc_analysis_reservation_fence') AS fence,clock_timestamp() AS recorded_at"))
        .await.unwrap().unwrap();
    let ordinal: i64 = sequence.try_get("", "ordinal").unwrap();
    // Use PostgreSQL's timestamp precision for both the ledger and its JSON receipt.
    let now = sequence.try_get("", "recorded_at").unwrap();
    let assignment_id = Uuid::new_v4();
    let mut assignment = PreparedAnalysisAssignment {
        contract_version: 1,
        tracker_instance_id: state.tracker_instance_id.clone(),
        project_id: state.project_id,
        task_id: task,
        root_task_id: task,
        owner_subject: state.owner_subject.clone(),
        intent_id: intent.intent_id,
        confirmation_id: intent.confirmation_id,
        requirement_revision: intent.requirement_revision,
        content_hash: intent.content_hash.clone(),
        routing_snapshot_id: snapshot.snapshot_id,
        routing_policy_version: snapshot.policy.version,
        routing_hash: snapshot.policy.routing_hash.clone(),
        assignment_id,
        execution_id: Uuid::new_v4(),
        reservation_version: 1,
        workflow_task_ref: format!("SDLC-{ordinal}"),
        agent_id: snapshot.policy.routes.analyst.agent_id,
        route: snapshot.policy.routes.analyst,
        stage: AnalysisStage::Analysis,
        role_key: "analyst".into(),
        workflow_key: "hermes-sdlc:analyst".into(),
        mode_key: "analysis".into(),
        scope: "business".into(),
        cycle_number: 0,
        attempt_number: 1,
        fencing_token: sequence.try_get("", "fence").unwrap(),
        lease_id: Uuid::new_v4(),
        assignment_operation_key: format!("analysis-reserve:{assignment_id}"),
        assignment_hash: String::new(),
        created_at: now,
    };
    assignment.assignment_hash = app::sdlc_reservation::assignment_hash(&assignment).unwrap();
    let lease = domain::sdlc_execution_lease::ExecutionLease {
        lease_id: assignment.lease_id,
        version: 1,
        holder_subject: "scheduler".into(),
        claimed_at: now,
        heartbeat_at: now,
        expires_at: now + std::time::Duration::from_secs(30),
    };
    let exact_timestamps = tx
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT $1::timestamptz=($3::jsonb->>'claimed_at')::timestamptz
            AND $1::timestamptz=($3::jsonb->>'heartbeat_at')::timestamptz
            AND $2::timestamptz=($3::jsonb->>'expires_at')::timestamptz AS exact",
            [now.into(), lease.expires_at.into(), json!(lease).into()],
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(
        exact_timestamps.try_get::<bool>("", "exact").unwrap(),
        "historical lease JSON timestamps must exactly match the PostgreSQL ledger"
    );
    let reservation = AnalysisReservationReceipt {
        assignment: assignment.clone(),
        lease: lease.clone(),
        technical_pool_slot: 1,
        ttl_seconds: 30,
        heartbeat_interval_seconds: 10,
        admission_state: AnalysisAdmissionState::AwaitingAdmission,
        dispatch_allowed: false,
        capacity_held: true,
    };
    let reserve_command = ReserveAnalysis {
        intent_id: intent.intent_id,
        routing_snapshot_id: snapshot.snapshot_id,
        requirement_revision: intent.requirement_revision,
        content_hash: intent.content_hash.clone(),
        expected_reservation_version: 0,
        idempotency_key: "historical-reserve".into(),
    };
    let result_command = SubmitAnalysisResult {
        contract_version: 1,
        assignment_id,
        execution_id: assignment.execution_id,
        agent_id: assignment.agent_id,
        intent_id: intent.intent_id,
        routing_snapshot_id: snapshot.snapshot_id,
        requirement_revision: intent.requirement_revision,
        content_hash: intent.content_hash,
        fencing_token: assignment.fencing_token,
        lease_id: assignment.lease_id,
        expected_lease_version: 1,
        assignment_hash: assignment.assignment_hash.clone(),
        terminal_receipt_ref: "synthetic://retained-analysis/terminal".into(),
        terminal_receipt_sha256: app::sdlc::canonical_hash(&json!({"synthetic":"terminal"}))
            .unwrap(),
        architecture_input_sha256: app::sdlc::canonical_hash(&json!({"synthetic":"analysis"}))
            .unwrap(),
        summary: "Synthetic retained history, not a current execution".into(),
        idempotency_key: "historical-result".into(),
    };
    let result = AnalysisResultReceipt {
        contract_version: 1,
        result_id: Uuid::new_v4(),
        tracker_instance_id: assignment.tracker_instance_id.clone(),
        project_id: assignment.project_id,
        task_id: task,
        root_task_id: task,
        owner_subject: assignment.owner_subject.clone(),
        assignment_id,
        execution_id: assignment.execution_id,
        agent_id: assignment.agent_id,
        intent_id: assignment.intent_id,
        routing_snapshot_id: assignment.routing_snapshot_id,
        requirement_revision: assignment.requirement_revision,
        content_hash: assignment.content_hash.clone(),
        workflow_task_ref: assignment.workflow_task_ref.clone(),
        fencing_token: assignment.fencing_token,
        lease_id: assignment.lease_id,
        lease_version: 1,
        previous_stage: Stage::Analysis,
        next_stage: Stage::Architecture,
        previous_status: "SDLC Analysis".into(),
        next_status: "SDLC Architecture".into(),
        terminal_receipt_ref: result_command.terminal_receipt_ref.clone(),
        terminal_receipt_sha256: result_command.terminal_receipt_sha256.clone(),
        architecture_input_sha256: result_command.architecture_input_sha256.clone(),
        summary: result_command.summary.clone(),
        idempotency_key: result_command.idempotency_key.clone(),
        created_at: now,
    };
    let reservation_operation = AnalysisReservationOperation {
        idempotency_key: reserve_command.idempotency_key.clone(),
        request_sha256: app::sdlc::canonical_hash(
            &json!({"operation":"reserve_analysis","payload":&reserve_command}),
        )
        .unwrap(),
        result: reservation,
    };
    let result_operation = AnalysisResultOperation {
        idempotency_key: result_command.idempotency_key.clone(),
        request_sha256: app::sdlc::canonical_hash(
            &json!({"operation":"submit_analysis_result","payload":&result_command}),
        )
        .unwrap(),
        result,
    };
    // Import a settled predecessor in the isolated DB. All production guards stay on;
    // the result trigger releases the temporary hold before this transaction commits.
    for (query, values) in [
        (
            "INSERT INTO sdlc_analysis_reservations(assignment_id,execution_id,workflow_task_ordinal,task_id,root_task_id,agent_id,intent_id,routing_snapshot_id,fencing_token,lease_id,pool_slot,payload) VALUES($1,$2,$3,$4,$4,$5,$6,$7,$8,$9,1,$10)",
            vec![
                assignment_id.into(),
                assignment.execution_id.into(),
                ordinal.into(),
                task.into(),
                assignment.agent_id.into(),
                assignment.intent_id.into(),
                assignment.routing_snapshot_id.into(),
                assignment.fencing_token.into(),
                assignment.lease_id.into(),
                json!(assignment).into(),
            ],
        ),
        (
            "INSERT INTO sdlc_analysis_reservation_leases(assignment_id,version,holder_subject,claimed_at,heartbeat_at,expires_at) VALUES($1,1,'scheduler',$2,$2,$3)",
            vec![assignment_id.into(), now.into(), lease.expires_at.into()],
        ),
        (
            "INSERT INTO sdlc_analysis_reservation_operations(task_id,idempotency_key,actor_subject,payload_hash,assignment_id,lease_version,result) VALUES($1,$2,'scheduler',$3,$4,1,$5)",
            vec![
                task.into(),
                reservation_operation.idempotency_key.clone().into(),
                reservation_operation.request_sha256.clone().into(),
                assignment_id.into(),
                json!(reservation_operation.result).into(),
            ],
        ),
        (
            "INSERT INTO sdlc_analysis_capacity_holds(assignment_id,root_task_id,agent_id,pool_slot) VALUES($1,$2,$3,1)",
            vec![
                assignment_id.into(),
                task.into(),
                assignment.agent_id.into(),
            ],
        ),
        (
            "INSERT INTO sdlc_analysis_results(result_id,assignment_id,execution_id,task_id,intent_id,lease_version,terminal_receipt_ref,terminal_receipt_sha256,architecture_input_sha256,payload) VALUES($1,$2,$3,$4,$5,1,$6,$7,$8,$9)",
            vec![
                result_operation.result.result_id.into(),
                assignment_id.into(),
                assignment.execution_id.into(),
                task.into(),
                assignment.intent_id.into(),
                result_command.terminal_receipt_ref.into(),
                result_command.terminal_receipt_sha256.into(),
                result_command.architecture_input_sha256.into(),
                json!(result_operation.result).into(),
            ],
        ),
        (
            "INSERT INTO sdlc_analysis_result_operations(task_id,idempotency_key,actor_subject,payload_hash,result_id,result) VALUES($1,$2,'scheduler',$3,$4,$5)",
            vec![
                task.into(),
                result_operation.idempotency_key.clone().into(),
                result_operation.request_sha256.clone().into(),
                result_operation.result.result_id.into(),
                json!(result_operation.result).into(),
            ],
        ),
    ] {
        tx.execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            query,
            values,
        ))
        .await
        .unwrap();
    }
    state.stage = Stage::Architecture;
    tx.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "INSERT INTO sdlc_analysis_state_write_permits(task_id,transaction_id,next_state) VALUES($1,pg_current_xact_id(),$2)",
        [task.into(),json!(state).into()])).await.unwrap();
    tx.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "UPDATE sdlc_tasks SET state=$2 WHERE task_id=$1",
        [task.into(), json!(state).into()],
    ))
    .await
    .unwrap();
    tx.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "UPDATE issues SET status_id=(SELECT id FROM statuses WHERE name='SDLC Architecture' ORDER BY id LIMIT 1),sprint_id=NULL WHERE id=$1", [task.into()])).await.unwrap();
    let settled = tx.query_one(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "SELECT NOT EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=$1)
            AND NOT EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=$1)
            AND NOT EXISTS(SELECT 1 FROM sdlc_pm_executions WHERE task_id=$1)
            AND NOT EXISTS(SELECT 1 FROM sdlc_analysis_state_write_permits WHERE task_id=$1) AS settled",
        [task.into()])).await.unwrap().unwrap();
    assert!(settled.try_get::<bool>("", "settled").unwrap());
    tx.commit().await.unwrap();
    let current = read(client, &format!("{root}/analysis-reservation"), owner, 200).await;
    assert!(current["current"].is_null());
    assert_eq!(current["lease_state"], "unreserved");
    assert_eq!(current["capacity_held"], false);
    assert_eq!(current["dispatch_allowed"], false);
    [
        (
            format!(
                "/api/v1/issues/{task}/sdlc/analysis-reservation/operations/historical-reserve"
            ),
            json!(reservation_operation),
        ),
        (
            format!("/api/v1/issues/{task}/sdlc/analysis-result/operations/historical-result"),
            json!(result_operation),
        ),
    ]
}

pub async fn check(
    db: &DatabaseConnection,
    client: &Client,
    base: &str,
    task: Uuid,
    actors: &routing_policy::Actors<'_>,
    ready: &TaskState,
) -> (Value, [(String, Value); 2]) {
    let url = format!("{base}/api/v1/issues/{task}/sdlc");
    let path = format!("{url}/analysis-reservation");
    let reader = "sdlc_pat_reservation_reader";
    let context = read(client, &format!("{url}/context"), actors.owner, 200).await;
    assert_eq!(context["stage"], "Analysis");
    let intent = analysis_intent::read(client, &url, actors.owner, 200).await;
    let project: Uuid = serde_json::from_value(intent["project_id"].clone()).unwrap();
    let routing = routing_policy::snapshot(client, base, task, actors.owner).await;
    let mut retained = read(client, &path, reader, 200).await;
    assert!(retained["current"].is_null());
    assert_eq!(retained["lease_state"], "unreserved");
    assert_eq!(retained["reason"], "pm_quiescence_unverified");
    assert_eq!(retained["reconciliation_needed"], true);
    assert_eq!(retained["capacity_held"], false);
    assert_eq!(retained["dispatch_allowed"], false);
    assert!(
        retained
            .as_object_mut()
            .unwrap()
            .remove("observed_at")
            .is_some()
    );

    let command = json!({
        "intent_id":intent["intent_id"],
        "routing_snapshot_id":routing["snapshot_id"],
        "requirement_revision":intent["requirement_revision"],
        "content_hash":intent["content_hash"],
        "expected_reservation_version":0,
        "idempotency_key":"retired-http-write"
    });
    let history = seed_retained_history(db, client, base, actors.owner, ready).await;
    let before = business_snapshot(db).await;
    // These methods are absent, not an alternate way to claim or complete work.
    for token in [actors.owner, "sdlc_pat_reservation_scheduler"] {
        for (suffix, status) in [
            ("analysis-reservation", 405),
            ("analysis-reservation/heartbeat", 404),
            ("analysis-result", 404),
        ] {
            let retired_url = format!("{url}/{suffix}");
            let response = client
                .post(&retired_url)
                .bearer_auth(token)
                .json(&command)
                .send()
                .await
                .unwrap();
            let actual = response.status().as_u16();
            let body = response.text().await.unwrap();
            assert_eq!(actual, status, "{retired_url}: {body}");
        }
    }
    assert_eq!(business_snapshot(db).await, before);

    for (suffix, expected) in &history {
        let history_url = format!("{base}{suffix}");
        for token in [actors.owner, reader] {
            assert_eq!(read(client, &history_url, token, 200).await, *expected);
        }
        for token in [
            actors.foreign,
            "sdlc_pat_reservation_foreign",
            "sdlc_pat_reservation_compound",
            "sdlc_pat_pm",
        ] {
            read(client, &history_url, token, 403).await;
        }
        read(client, &history_url, "local-token", 401).await;
    }

    for (suffix, status) in [
        ("analysis-reservation", 200),
        ("analysis-reservation/configuration-preflight", 409),
        ("analysis-reservation/operations/retired-http-write", 404),
        ("analysis-result/operations/retired-http-write", 404),
    ] {
        let read_url = format!("{url}/{suffix}");
        for token in [actors.owner, reader] {
            read(client, &read_url, token, status).await;
        }
        for token in [
            actors.foreign,
            "sdlc_pat_reservation_foreign",
            "sdlc_pat_reservation_compound",
            "sdlc_pat_pm",
        ] {
            read(client, &read_url, token, 403).await;
        }
        read(client, &read_url, "local-token", 401).await;
    }
    assert_eq!(business_snapshot(db).await, before);
    sql(
        db,
        "DELETE FROM project_members WHERE project_id=$1 AND user_id=(SELECT id FROM users WHERE central_sub='scheduler')",
        vec![project.into()],
    )
    .await;
    read(client, &path, reader, 403).await;
    for (suffix, _) in &history {
        read(client, &format!("{base}{suffix}"), reader, 403).await;
    }
    sql(
        db,
        "INSERT INTO project_members(project_id,user_id,role) SELECT $1,id,'developer' FROM users WHERE central_sub='scheduler'",
        vec![project.into()],
    )
    .await;
    let mut after = read(client, &path, reader, 200).await;
    assert!(
        after
            .as_object_mut()
            .unwrap()
            .remove("observed_at")
            .is_some()
    );
    assert_eq!(after, retained);
    assert_eq!(
        read(client, &format!("{url}/context"), actors.owner, 200).await,
        context
    );
    assert_eq!(
        analysis_intent::read(client, &url, actors.owner, 200).await,
        intent
    );
    assert_eq!(
        routing_policy::snapshot(client, base, task, actors.owner).await,
        routing
    );
    println!("ANALYSIS_RESERVATION retired writers/no writes/protected retained history passed");
    (retained, history)
}
