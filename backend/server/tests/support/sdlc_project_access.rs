use super::*;

async fn read(client: &Client, base: &str, path: &str, token: &str, expected: u16) -> Value {
    let response = client
        .get(format!("{base}/api/v1/{path}"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body = response.text().await.unwrap();
    assert_eq!(status, expected, "{path}: {body}");
    serde_json::from_str(&body).unwrap_or(Value::Null)
}

pub(super) async fn directories(client: &Client, base: &str, token: &str, mut expected: Vec<Uuid>) {
    expected.sort();
    let access = read(client, base, "sdlc/project-access", token, 200).await;
    assert_eq!(access["project_ids"], json!(expected));
    let mut collected = Vec::<Uuid>::new();
    let mut after = None;
    // A denied project's UUID must not leak into a cursor or an empty page.
    for _ in 0..=expected.len() {
        let path = match after {
            Some(id) => format!("sdlc/project-directory?limit=1&after={id}"),
            None => "sdlc/project-directory?limit=1".into(),
        };
        let page = read(client, base, &path, token, 200).await;
        let entries = page["projects"].as_array().unwrap();
        assert!(entries.len() <= 1);
        collected.extend(entries.iter().map(|entry| {
            let id: Uuid = serde_json::from_value(entry["id"].clone()).unwrap();
            assert!(expected.contains(&id));
            assert!(!entry["key"].as_str().unwrap().is_empty());
            id
        }));
        after = serde_json::from_value::<Option<Uuid>>(page["next_cursor"].clone()).unwrap();
        if after.is_none() {
            assert_eq!(collected, expected);
            return;
        }
        assert_eq!(after, collected.last().copied());
    }
    panic!("project directory failed to terminate");
}

pub(super) async fn verify(
    db: &DatabaseConnection,
    client: &Client,
    base: &str,
    project: Uuid,
    task: Uuid,
    actors: &routing_policy::Actors<'_>,
) {
    let other: Uuid = db
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT id FROM projects WHERE key='OTHER'",
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "id")
        .unwrap();
    directories(client, base, actors.owner, vec![project, other]).await;
    directories(client, base, actors.operator, vec![project]).await;
    directories(client, base, actors.foreign, vec![]).await;
    directories(client, base, "sdlc_pat_wrong_scope", vec![project]).await;
    directories(client, base, "sdlc_pat_reservation_foreign", vec![]).await;

    let tables = [
        "users",
        "issues",
        "sdlc_tasks",
        "sdlc_draft_creations",
        "sdlc_idempotency",
        "sdlc_outbox",
        "issue_status_history",
    ];
    let mut before = Vec::new();
    for table in tables {
        before.push(count(db, table).await);
    }
    for token in [actors.foreign, "sdlc_pat_reservation_foreign"] {
        read(
            client,
            base,
            &format!("issues/{task}/sdlc/context"),
            token,
            403,
        )
        .await;
    }
    post(
        client,
        &format!("{base}/api/v1/issues/{task}/sdlc/binding"),
        actors.foreign,
        &json!({"root_task_id":task,"idempotency_key":"foreign-human-binding"}),
        403,
    )
    .await;
    post(
        client,
        &format!("{base}/api/v1/projects/{project}/sdlc/drafts"),
        actors.foreign,
        &json!({"title":"Forbidden draft","description":"Denied request","idempotency_key":"foreign-human-draft"}),
        403,
    )
    .await;
    read(
        client,
        base,
        &format!("projects/{project}/sdlc/drafts/operations/foreign-human-draft"),
        actors.foreign,
        403,
    )
    .await;
    // The fixture keeps a legacy local admin hint, but no central admin role or membership.
    // Keep the legacy public Tracker route's existing policy outside this fix.
    read(client, base, &format!("issues/{task}"), actors.foreign, 200).await;
    for (table, expected) in tables.into_iter().zip(before) {
        assert_eq!(count(db, table).await, expected, "denial mutated {table}");
    }
    for token in [actors.owner, actors.operator] {
        read(
            client,
            base,
            &format!("issues/{task}/sdlc/context"),
            token,
            404,
        )
        .await;
    }
    sql(
        db,
        "UPDATE users SET is_active=false WHERE central_sub='operator'",
        vec![],
    )
    .await;
    for path in ["sdlc/project-access", "sdlc/project-directory"] {
        read(client, base, path, actors.operator, 403).await;
    }
    sql(
        db,
        "UPDATE users SET is_active=true WHERE central_sub='operator'",
        vec![],
    )
    .await;
}
