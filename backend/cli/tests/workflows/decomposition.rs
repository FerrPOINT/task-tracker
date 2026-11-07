use super::*;

#[tokio::test]
async fn decomposition_cli_routes_payload_cas_and_operation_readback() {
    let task = "11111111-1111-4111-8111-111111111111";
    for key in [".", ".."] {
        let server = Server::start(vec![(200, json!({"plan":null}))]).await;
        let output = server
            .run(
                &["decomposition", "operation", "--task", task, "--key", key],
                None,
            )
            .await;
        assert!(!output.status.success());
        assert!(server.requests().is_empty());
    }
    for (args, suffix) in [
        (vec!["read", "--task", task], ""),
        (
            vec!["history", "--task", task, "--before", "7", "--limit", "2"],
            "/revisions?before=7&limit=2",
        ),
        (
            vec!["revision", "--task", task, "--revision", "3"],
            "/revisions/3",
        ),
        (
            vec!["operation", "--task", task, "--key", "original:save"],
            "/operations/original%3Asave",
        ),
    ] {
        let server = Server::start(vec![(200, json!({"revision":3}))]).await;
        let mut command = vec!["decomposition"];
        command.extend(args);
        let output = server.run(&command, None).await;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(
            requests[0].path,
            format!("/api/v1/issues/{task}/decomposition{suffix}")
        );
    }
    let body = json!({"expected_revision":2,"idempotency_key":"original-save","reason":"Clarify criteria","fence":null,"content":{"overview":"Existing API","source":null,"sections":[]}});
    let server = Server::start(vec![(409, json!({"error":"plan revision changed"}))]).await;
    let output = server
        .run(
            &["decomposition", "save", "--task", task, "--from-file", "-"],
            Some(&body.to_string()),
        )
        .await;
    assert!(!output.status.success());
    let requests = server.requests();
    assert_eq!(
        requests.len(),
        1,
        "a conflict must not retry or generate a new operation"
    );
    assert_eq!(requests[0].method, "POST");
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[0].body).unwrap(),
        body
    );
}
