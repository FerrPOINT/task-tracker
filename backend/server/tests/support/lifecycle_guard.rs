use super::*;

pub async fn binding(
    db: &DatabaseConnection,
    client: &Client,
    base: &str,
    owner: &str,
    task: Uuid,
    other: Uuid,
) {
    let before = [
        count(db, "sdlc_tasks").await,
        count(db, "sdlc_idempotency").await,
        count(db, "sdlc_outbox").await,
        count(db, "issue_status_history").await,
    ];
    let first_url = format!("{base}/api/v1/issues/{task}/sdlc/binding");
    let second_url = format!("{base}/api/v1/issues/{other}/sdlc/binding");
    let first_command = json!({"root_task_id":other,"idempotency_key":"unaccepted-child-a"});
    let second_command = json!({"root_task_id":task,"idempotency_key":"unaccepted-child-b"});
    let (first, second) = tokio::join!(
        post(client, &first_url, owner, &first_command, 422),
        post(client, &second_url, owner, &second_command, 422)
    );
    assert!(
        first
            .to_string()
            .contains("accepted Architect decomposition")
    );
    assert!(
        second
            .to_string()
            .contains("accepted Architect decomposition")
    );
    for root in [Uuid::nil(), Uuid::new_v4()] {
        post(
            client,
            &format!("{base}/api/v1/issues/{task}/sdlc/binding"),
            owner,
            &json!({"root_task_id":root,"idempotency_key":"invalid-root"}),
            422,
        )
        .await;
    }
    assert_eq!(
        before,
        [
            count(db, "sdlc_tasks").await,
            count(db, "sdlc_idempotency").await,
            count(db, "sdlc_outbox").await,
            count(db, "issue_status_history").await,
        ]
    );
    println!(
        "ROOT_BINDING same-project child/cyclic-root/nil/missing-root rejected without writes"
    );
}

pub async fn reject_child_row(db: &DatabaseConnection, root: Uuid, child: Uuid) {
    let result = db.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "INSERT INTO sdlc_tasks(task_id,tracker_instance_id,project_id,root_task_id,owner_subject,state)
         SELECT $2,tracker_instance_id,project_id,$1,owner_subject,
           jsonb_set(jsonb_set(state,'{task_id}',to_jsonb($2::text)),'{root_task_id}',to_jsonb($1::text))
         FROM sdlc_tasks WHERE task_id=$1", vec![root.into(),child.into()])).await;
    let error = result.unwrap_err().to_string();
    assert!(error.contains("sdlc_root_only"), "{error}");
}

pub async fn queued_analysis(db: &DatabaseConnection, task: Uuid) {
    let state = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT state FROM sdlc_tasks WHERE task_id=$1",
            vec![task.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Value>("", "state")
        .unwrap();
    let before = [
        count(db, "sdlc_outbox").await,
        count(db, "sdlc_idempotency").await,
        count(db, "issue_status_history").await,
    ];
    let mut changed_revision = state.clone();
    changed_revision["revisions"][1]["content_hash"] = json!("0".repeat(64));
    let mut invented_decomposition = state.clone();
    invented_decomposition["decomposition"] = json!({"accepted":true,"children":[]});
    let mut changed_assignment = state.clone();
    changed_assignment["assignment"]["version"] = json!(999);
    let mut candidates = vec![changed_revision, invented_decomposition, changed_assignment];
    for stage in [
        "Draft",
        "Clarification",
        "Backlog",
        "Architecture",
        "Deployment",
    ] {
        let mut candidate = state.clone();
        candidate["stage"] = json!(stage);
        candidates.push(candidate);
    }
    for candidate in candidates {
        let error = db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "UPDATE sdlc_tasks SET state=$2 WHERE task_id=$1",
                vec![task.into(), candidate.into()],
            ))
            .await
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("guarded admission before lifecycle mutation"),
            "{error}"
        );
    }
    sql(
        db,
        "UPDATE sdlc_tasks SET state=state WHERE task_id=$1",
        vec![task.into()],
    )
    .await;
    let after = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT state FROM sdlc_tasks WHERE task_id=$1",
            vec![task.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get::<Value>("", "state")
        .unwrap();
    assert_eq!(after, state);
    assert_eq!(
        before,
        [
            count(db, "sdlc_outbox").await,
            count(db, "sdlc_idempotency").await,
            count(db, "issue_status_history").await,
        ]
    );
    println!(
        "ANALYSIS_FENCE stage/revision/assignment/invented-decomposition rejected; no-op/readback retained"
    );
}
