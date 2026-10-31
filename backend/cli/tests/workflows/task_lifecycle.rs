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
