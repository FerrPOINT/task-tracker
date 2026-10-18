//! HTTP transport fixtures, not live Workflow/Fleet acceptance.
use super::*;

const TASK: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const SNAPSHOT: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const AFTER: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

#[tokio::test]
async fn discussion_pagination_and_original_operation_are_explicit_reads() {
    let page = json!({"task_id":TASK,"title":"Actual ticket","description":"Full body",
        "comments":[{"id":AFTER,"author_name":"Other participant","body":"Previous finding"}],
        "snapshot":SNAPSHOT,"next_after":null});
    let server = Server::start(vec![
        (200, page.clone()),
        (200, json!({"comment_id":AFTER})),
    ])
    .await;
    let output = server
        .run(
            &[
                "sdlc",
                "discussion",
                "read",
                "--task",
                TASK,
                "--limit",
                "1",
                "--after",
                AFTER,
                "--snapshot",
                SNAPSHOT,
            ],
            None,
        )
        .await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        page
    );
    let output = server
        .run(
            &[
                "sdlc",
                "discussion",
                "operation",
                "--task",
                TASK,
                "--key",
                "fleet-stage-comment:original-run",
            ],
            None,
        )
        .await;
    assert!(output.status.success());
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].path,
        format!("/api/v1/issues/{TASK}/sdlc/discussion?limit=1&after={AFTER}&snapshot={SNAPSHOT}")
    );
    assert_eq!(requests[1].method, "GET");
    assert!(requests[1].path.starts_with(&format!(
        "/api/v1/issues/{TASK}/sdlc/discussion/operations/"
    )));
    assert!(requests[1].path.contains("original"));
}

#[tokio::test]
async fn comment_payload_and_unicode_are_preserved_without_an_automatic_retry() {
    let command = json!({"body":"Проверил чужие замечания.\nРезультат требует доработки.",
        "fence":{"kind":"delivery","fence":{"assignment_id":AFTER,"execution_id":SNAPSHOT,
            "fencing_token":1,"lease_id":TASK,"expected_lease_version":1}},
        "idempotency_key":"fleet-stage-comment:original-run"});
    let server = Server::start(vec![(
        503,
        json!({"error":{"code":"UNAVAILABLE","message":"unknown acceptance"}}),
    )])
    .await;
    let output = server
        .run(
            &[
                "sdlc",
                "discussion",
                "publish",
                "--task",
                TASK,
                "--from-file",
                "-",
            ],
            Some(&command.to_string()),
        )
        .await;
    assert!(!output.status.success());
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(
        requests[0].path,
        format!("/api/v1/issues/{TASK}/sdlc/discussion")
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[0].body).unwrap(),
        command
    );
}

#[tokio::test]
async fn invalid_pagination_is_rejected_before_any_network_request() {
    let server = Server::start(vec![]).await;
    for tail in [
        vec!["--after", AFTER],
        vec!["--limit", "0"],
        vec!["--limit", "501"],
    ] {
        let mut args = vec!["sdlc", "discussion", "read", "--task", TASK];
        args.extend(tail);
        let output = server.run(&args, None).await;
        assert!(!output.status.success());
    }
    assert!(server.requests().is_empty());
}
