use super::*;

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

pub async fn confirm(
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
    ];
    // Fault after intent, consent, aggregate, issue history and command receipt writes.
    db.execute_unprepared("CREATE FUNCTION test_analysis_outbox_failure() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_type='analysis.intent_created' THEN RAISE EXCEPTION 'SDLC test injected outbox failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER test_analysis_outbox_failure BEFORE INSERT ON sdlc_outbox FOR EACH ROW EXECUTE FUNCTION test_analysis_outbox_failure();").await.unwrap();
    let confirm_url = format!("{url}/requirements/2/confirm");
    post(client, &confirm_url, owner, command, 409).await;
    assert_eq!(
        before,
        [
            count(db, "sdlc_confirmations").await,
            count(db, "sdlc_analysis_intents").await,
            count(db, "sdlc_outbox").await,
            count(db, "sdlc_idempotency").await,
            count(db, "issue_status_history").await
        ]
    );
    read(client, url, owner, 404).await;
    db.execute_unprepared("DROP TRIGGER test_analysis_outbox_failure ON sdlc_outbox; DROP FUNCTION test_analysis_outbox_failure();").await.unwrap();

    let (first, duplicate) = tokio::join!(
        post(client, &confirm_url, owner, command, 200),
        post(client, &confirm_url, owner, command, 200)
    );
    assert_eq!(first, duplicate);
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
        "ANALYSIS_INTENT rollback/atomic consent+queue/concurrent replay/conflicts/stale/ACL/immutability passed"
    );
    (first, intent)
}
