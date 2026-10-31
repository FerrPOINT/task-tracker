use super::*;
const TASK: &str = "11111111-1111-4111-8111-111111111111";

#[tokio::test]
async fn lifecycle_commands_keep_revision_and_original_idempotency_key_on_wire() {
    let response = json!({"task_id":TASK,"state":{"stage":"Backlog","status":"InProgress","revision":2},"allowed_actions":[],"execution_busy":false});
    let server = Server::start(vec![
        (200, json!({"enabled":true})),
        (200, json!({"id":TASK})),
        (200, response.clone()),
        (200, json!({"id":TASK})),
        (200, response.clone()),
        (200, json!({"id":TASK})),
        (200, response.clone()),
    ])
    .await;
    assert_eq!(
        success(&server.run(&["lifecycle", "catalog"], None).await),
        json!({"enabled":true})
    );
    assert_eq!(
        success(
            &server
                .run(&["lifecycle", "get", "--issue", TASK], None)
                .await
        ),
        response
    );
    let args = [
        "lifecycle",
        "transition",
        "--issue",
        TASK,
        "--action",
        "start",
        "--expected-revision",
        "1",
        "--idempotency-key",
        "human:original?+=#",
    ];
    for _ in 0..2 {
        assert_eq!(success(&server.run(&args, None).await), response);
    }
    let requests = server.requests();
    assert_eq!(requests.len(), 7);
    assert_eq!(requests[0].path, "/api/v1/task-lifecycle");
    assert_eq!(requests[2].path, format!("/api/v1/issues/{TASK}/lifecycle"));
    for request in [&requests[4], &requests[6]] {
        assert_eq!(request.method, "POST");
        assert_eq!(
            request.path,
            format!("/api/v1/issues/{TASK}/lifecycle/transitions")
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&request.body).unwrap(),
            json!({"action":"start","expected_revision":1,"idempotency_key":"human:original?+=#"})
        );
        assert_eq!(
            request.authorization.as_deref(),
            Some("Bearer fixture-token")
        );
    }
    assert_eq!(requests[4].body, requests[6].body);
}

#[tokio::test]
async fn lifecycle_invalid_action_or_revision_does_not_send_a_request() {
    let server = Server::start(vec![]).await;
    for args in [
        vec![
            "lifecycle",
            "transition",
            "--issue",
            TASK,
            "--action",
            "skip",
            "--expected-revision",
            "1",
            "--idempotency-key",
            "one",
        ],
        vec![
            "lifecycle",
            "transition",
            "--issue",
            TASK,
            "--action",
            "start",
            "--expected-revision",
            "0",
            "--idempotency-key",
            "one",
        ],
    ] {
        assert!(!server.run(&args, None).await.status.success());
    }
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn handoff_commands_preserve_human_payload_and_reconcile_the_original_operation() {
    let handoff = "22222222-2222-4222-8222-222222222222";
    let body = json!({"expected_revision":7,"expected_routing_policy_version":3,"confirmed_issue_sha256":"a".repeat(64),"requirements":{"goal":"Кириллица + quoted input"},"idempotency_key":"original?+=#"});
    let server = Server::start(vec![
        (200, json!({"id":TASK})),
        (200, json!({"blockers":[]})),
        (200, json!({"id":TASK})),
        (202, json!({"receipt":{"handoff_id":handoff}})),
        (200, json!({"id":TASK})),
        (200, json!({"cancellation":null})),
        (200, json!({"id":TASK})),
        (200, json!({"resulting_revision":9})),
    ])
    .await;
    success(
        &server
            .run(&["handoff", "preview", "--issue", TASK], None)
            .await,
    );
    success(
        &server
            .run(
                &["handoff", "request", "--issue", TASK, "--from-file", "-"],
                Some(&body.to_string()),
            )
            .await,
    );
    success(
        &server
            .run(
                &[
                    "handoff",
                    "operation",
                    "--issue",
                    TASK,
                    "--key",
                    "original?+=#",
                ],
                None,
            )
            .await,
    );
    success(
        &server
            .run(
                &[
                    "handoff",
                    "cancel",
                    "--issue",
                    TASK,
                    "--handoff-id",
                    handoff,
                    "--expected-revision",
                    "8",
                    "--idempotency-key",
                    "cancel?+=#",
                ],
                None,
            )
            .await,
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 8);
    assert_eq!(requests[1].path, format!("/api/v1/issues/{TASK}/handoff"));
    assert_eq!(requests[3].method, "POST");
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[3].body).unwrap(),
        body
    );
    assert_eq!(
        requests[5].path,
        format!("/api/v1/issues/{TASK}/handoff/operations/original%3F%2B%3D%23")
    );
    assert_eq!(
        requests[7].path,
        format!("/api/v1/issues/{TASK}/handoff/{handoff}/cancel")
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[7].body).unwrap(),
        json!({"expected_revision":8,"idempotency_key":"cancel?+=#"})
    );
}

#[tokio::test]
async fn handoff_failure_does_not_retry_mutations_or_replace_the_original_key() {
    let server = Server::start(vec![
        (200, json!({"id":TASK})),
        (503, json!({"error":"owner unavailable"})),
    ])
    .await;
    let payload =
        json!({"expected_revision":7,"idempotency_key":"retain-after-failure"}).to_string();
    assert!(
        !server
            .run(
                &["handoff", "request", "--issue", TASK, "--from-file", "-"],
                Some(&payload)
            )
            .await
            .status
            .success()
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].method, "POST");
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[1].body).unwrap()["idempotency_key"],
        "retain-after-failure"
    );
}
