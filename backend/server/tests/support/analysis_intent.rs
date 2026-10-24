use super::*;
use sea_orm::TransactionTrait;

pub async fn read(client: &Client, url: &str, token: &str, status: u16) -> Value {
    let response = client
        .get(format!("{url}/analysis-intent"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    let actual = response.status().as_u16();
    let value = response.json().await.unwrap_or(Value::Null);
    assert_eq!(actual, status, "Analysis intent readback: {value}");
    value
}

pub async fn confirm_and_seed_analysis_fixture(
    db: &DatabaseConnection,
    client: &Client,
    url: &str,
    owner: &str,
    command: &Value,
    task: Uuid,
) -> (Value, Value) {
    read(client, url, owner, 404).await;
    let before = [
        count(db, "sdlc_confirmations").await,
        count(db, "sdlc_analysis_intents").await,
        count(db, "sdlc_outbox").await,
        count(db, "sdlc_idempotency").await,
        count(db, "issue_status_history").await,
        count(db, "sdlc_task_routing_snapshots").await,
    ];
    // Fault after consent, aggregate, issue history and command receipt writes.
    db.execute_unprepared("CREATE FUNCTION test_analysis_outbox_failure() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_type='requirements.confirmed' THEN RAISE EXCEPTION 'SDLC test injected outbox failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER test_analysis_outbox_failure BEFORE INSERT ON sdlc_outbox FOR EACH ROW EXECUTE FUNCTION test_analysis_outbox_failure();").await.unwrap();
    let confirm_url = format!("{url}/requirements/2/confirm");
    post(client, &confirm_url, owner, command, 409).await;
    assert_eq!(
        before,
        [
            count(db, "sdlc_confirmations").await,
            count(db, "sdlc_analysis_intents").await,
            count(db, "sdlc_outbox").await,
            count(db, "sdlc_idempotency").await,
            count(db, "issue_status_history").await,
            count(db, "sdlc_task_routing_snapshots").await
        ]
    );
    read(client, url, owner, 404).await;
    db.execute_unprepared("DROP TRIGGER test_analysis_outbox_failure ON sdlc_outbox; DROP FUNCTION test_analysis_outbox_failure();").await.unwrap();

    let (first, duplicate) = tokio::join!(
        post(client, &confirm_url, owner, command, 200),
        post(client, &confirm_url, owner, command, 200)
    );
    assert_eq!(first, duplicate);
    assert_eq!(count(db, "sdlc_analysis_intents").await, before[1]);
    assert_eq!(count(db, "sdlc_outbox").await, before[2] + 1);
    read(client, url, owner, 404).await;
    let context = client
        .get(format!("{url}/context"))
        .bearer_auth(owner)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(context["stage"], "Backlog");
    assert_eq!(context["waiting_reason"], Value::Null);
    // Explicit historical fixture for the downstream admission/restart tests below.
    // This is not a production promotion API or a confirmation side effect.
    seed_historical_analysis(db, task).await;
    let intent = read(client, url, owner, 200).await;
    assert_eq!(intent["task_id"], task.to_string());
    assert_eq!(intent["confirmation_id"], first["id"]);
    assert_eq!(intent["requirement_revision"], 2);
    assert_eq!(intent["content_hash"], command["content_hash"]);
    for (field, expected) in [
        ("stage", "Analysis"),
        ("status", "Ready"),
        ("role", "Analyst"),
        ("workflow", "hermes-sdlc:analyst"),
        ("mode", "analysis"),
        ("scope", "business"),
    ] {
        assert_eq!(intent[field], expected);
    }
    assert_eq!(intent["cycle"], 0);
    assert_eq!(intent["attempt"], 0);
    assert_eq!(
        intent["operation_key"],
        format!("analysis:{}", first["id"].as_str().unwrap())
    );
    assert_eq!(count(db, "sdlc_analysis_intents").await, before[1] + 1);
    assert_eq!(count(db, "sdlc_outbox").await, before[2] + 2);

    let mut changed = command.clone();
    changed["content_hash"] = json!("0".repeat(64));
    post(client, &confirm_url, owner, &changed, 409).await;
    changed = command.clone();
    changed["idempotency_key"] = json!("second-confirmation-operation");
    post(client, &confirm_url, owner, &changed, 409).await;
    post(
        client,
        &format!("{url}/requirements/1/confirm"),
        owner,
        &changed,
        409,
    )
    .await;
    read(client, url, "sdlc_pat_pm", 403).await;
    assert_eq!(intent, read(client, url, "sdlc_pat_wrong_scope", 200).await);
    assert_eq!(count(db, "sdlc_analysis_intents").await, before[1] + 1);
    assert_eq!(count(db, "sdlc_outbox").await, before[2] + 2);
    let state = client
        .get(format!("{url}/context"))
        .bearer_auth(owner)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(state["stage"], "Analysis");
    assert_eq!(state["waiting_reason"], "queued_for_analysis");
    assert_eq!(state["permissions"]["can_confirm"], false);
    lifecycle_guard::queued_analysis(db, task).await;
    assert!(db.execute_unprepared(&format!("UPDATE issues SET status_id=(SELECT id FROM statuses WHERE name='Backlog' LIMIT 1) WHERE id='{task}'")).await.is_err());
    assert!(
        db.execute_unprepared("DELETE FROM sdlc_analysis_intents")
            .await
            .is_err()
    );
    assert!(db.execute_unprepared("UPDATE sdlc_analysis_intents SET payload=payload||'{\"mode\":\"decomposition\"}'::jsonb").await.is_err());
    assert!(
        db.execute_unprepared(
            "WITH duplicate AS (SELECT *,gen_random_uuid() AS fresh_id FROM sdlc_analysis_intents) INSERT INTO sdlc_analysis_intents(intent_id,task_id,requirement_revision,content_hash,confirmation_id,operation_key,payload) SELECT fresh_id,task_id,requirement_revision,content_hash,confirmation_id,operation_key,jsonb_set(payload,'{intent_id}',to_jsonb(fresh_id::text)) FROM duplicate"
        )
        .await
        .is_err()
    );
    assert_eq!(intent, read(client, url, owner, 200).await);
    println!(
        "BACKLOG confirmation rollback/replay; explicit Analysis fixture ACL/restart/immutability passed"
    );
    (first, intent)
}

pub(super) async fn seed_historical_analysis(db: &DatabaseConnection, task: Uuid) {
    let tx = db.begin().await.unwrap();
    let row = tx.query_one(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "SELECT state,pm_execution_id IS NULL AS unreserved FROM sdlc_tasks WHERE task_id=$1 FOR UPDATE", [task.into()])).await.unwrap().unwrap();
    assert!(row.try_get::<bool>("", "unreserved").unwrap());
    let mut state: domain::sdlc::TaskState =
        serde_json::from_value(row.try_get("", "state").unwrap()).unwrap();
    assert_eq!(state.stage, domain::sdlc::Stage::Backlog);
    assert!(state.status.is_none());
    state.stage = domain::sdlc::Stage::Analysis;
    let row = tx
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT payload FROM sdlc_task_routing_snapshots WHERE task_id=$1",
            [task.into()],
        ))
        .await
        .unwrap();
    let snapshot: Option<domain::sdlc_routing::TaskRoutingSnapshot> =
        row.map(|row| serde_json::from_value(row.try_get("", "payload").unwrap()).unwrap());
    let mut intent =
        app::sdlc::analysis_intent(&state, state.confirmations.last().unwrap(), Uuid::new_v4())
            .unwrap();
    if let Some(snapshot) = &snapshot {
        intent.workflow = snapshot.policy.routes.analyst.workflow_key.clone();
    }
    tx.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "INSERT INTO sdlc_analysis_intents(intent_id,task_id,requirement_revision,content_hash,confirmation_id,operation_key,payload,routing_snapshot_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
        [intent.intent_id.into(),task.into(),intent.requirement_revision.into(),intent.content_hash.clone().into(),intent.confirmation_id.into(),intent.operation_key.clone().into(),json!(intent).into(),snapshot.map(|snapshot| snapshot.snapshot_id).into()])).await.unwrap();
    tx.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "UPDATE sdlc_tasks SET state=$2 WHERE task_id=$1",
        [task.into(), json!(state).into()],
    ))
    .await
    .unwrap();
    tx.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "INSERT INTO issue_status_history(id,issue_id,from_status_id,to_status_id,changed_by_id,created_at)
         SELECT $1,i.id,i.status_id,s.id,i.reporter_id,now() FROM issues i JOIN statuses s ON s.name='SDLC Analysis' WHERE i.id=$2 AND i.status_id<>s.id",
        [Uuid::new_v4().into(),task.into()])).await.unwrap();
    tx.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "UPDATE issues SET status_id=(SELECT id FROM statuses WHERE name='SDLC Analysis' ORDER BY id LIMIT 1),sprint_id=NULL WHERE id=$1", [task.into()])).await.unwrap();
    let payload = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
        "project_id":state.project_id,"task_id":task,"root_task_id":state.root_task_id,
        "owner_subject":state.owner_subject,"stage":"Analysis","requirement_revision":intent.requirement_revision,"result":intent});
    tx.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,'analysis.intent_created',$3)",
        [intent.intent_id.into(),task.into(),payload.into()])).await.unwrap();
    tx.commit().await.unwrap();
}
