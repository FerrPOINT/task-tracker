use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{Request, StatusCode},
    response::Response,
};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::Write,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};
#[derive(Debug, Clone)]
struct Recorded {
    method: String,
    path: String,
    body: Vec<u8>,
    key: Option<String>,
    authorization: Option<String>,
}
struct TestState {
    responses: Mutex<VecDeque<(u16, Vec<u8>)>>,
    requests: Mutex<Vec<Recorded>>,
    delay: Duration,
}
struct Server {
    url: String,
    state: Arc<TestState>,
    handle: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
impl Server {
    async fn start(responses: Vec<(u16, Value)>) -> Self {
        Self::bytes(
            responses
                .into_iter()
                .map(|(s, v)| (s, serde_json::to_vec(&v).unwrap()))
                .collect(),
            Duration::ZERO,
        )
        .await
    }
    async fn bytes(responses: Vec<(u16, Vec<u8>)>, delay: Duration) -> Self {
        let state = Arc::new(TestState {
            responses: Mutex::new(responses.into()),
            requests: Mutex::new(vec![]),
            delay,
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new().fallback(record).with_state(state.clone());
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { url, state, handle }
    }
    fn requests(&self) -> Vec<Recorded> {
        self.state.requests.lock().unwrap().clone()
    }
    async fn run(&self, args: &[&str], input: Option<&str>) -> std::process::Output {
        let url = format!("{}/api/v1", self.url);
        let args = args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let input = input.map(str::to_string);
        tokio::task::spawn_blocking(move || {
            let mut child = Command::new(env!("CARGO_BIN_EXE_task-tracker"))
                .args(["--api-url", &url, "--token", "fixture-token"])
                .args(args)
                .env_remove("SDLC_API_TOKEN")
                .env_remove("CICD_PROFILE")
                .env_remove("WIKI_TOKEN")
                .env_remove("TASKTRACKER_TOKEN")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            if let Some(input) = input {
                child
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(input.as_bytes())
                    .unwrap();
            } else {
                drop(child.stdin.take());
            }
            child.wait_with_output().unwrap()
        })
        .await
        .unwrap()
    }
}
async fn record(State(state): State<Arc<TestState>>, req: Request<Body>) -> Response {
    let method = req.method().to_string();
    let path = req.uri().to_string();
    let key = req
        .headers()
        .get("idempotency-key")
        .and_then(|h| h.to_str().ok())
        .map(str::to_string);
    let authorization = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .map(str::to_string);
    let body = to_bytes(req.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    state.requests.lock().unwrap().push(Recorded {
        method,
        path,
        body,
        key,
        authorization,
    });
    tokio::time::sleep(state.delay).await;
    let (status, bytes) = state.responses.lock().unwrap().pop_front().unwrap_or((
        500,
        br#"{"error":{"code":"UNEXPECTED_REQUEST","message":"unexpected request"}}"#.to_vec(),
    ));
    Response::builder()
        .status(StatusCode::from_u16(status).unwrap())
        .header("content-type", "application/json")
        .body(Body::from(bytes))
        .unwrap()
}
fn success(output: &std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

#[tokio::test]
async fn task_input_pagination_restore_and_json_success() {
    let server = Server::start(vec![
        (200, json!({"id":"uuid","key":"TT-1"})),
        (200, json!({"issues":[]})),
        (204, Value::Null),
        (200, json!({"id":"uuid"})),
    ])
    .await;
    success(
        &server
            .run(
                &["issue", "update", "TT-1", "--from-file", "-", "--unassign"],
                Some("Описание\nиз stdin"),
            )
            .await,
    );
    success(
        &server
            .run(
                &[
                    "issue",
                    "list",
                    "--project-key",
                    "TT",
                    "--limit",
                    "2",
                    "--offset",
                    "2",
                    "--status",
                    "Open",
                    "--sort-order",
                    "asc",
                ],
                None,
            )
            .await,
    );
    // A real 204 has no JSON payload.
    let deleted = server.run(&["issue", "delete", "TT-1"], None).await;
    assert_eq!(success(&deleted), json!({"status":"ok"}));
    success(&server.run(&["issue", "restore", "TT-1"], None).await);
    let req = server.requests();
    assert_eq!(req[0].method, "PATCH");
    assert_eq!(req[0].path, "/api/v1/issues/TT-1");
    let body: Value = serde_json::from_slice(&req[0].body).unwrap();
    assert_eq!(body["description"], "Описание\nиз stdin");
    assert!(body["assignee_id"].is_null());
    assert!(req[1].path.contains("offset=2"));
    assert!(req[1].path.contains("status=Open"));
    assert_eq!(req[3].path, "/api/v1/issues/TT-1/restore");
    assert_eq!(
        req[0].authorization.as_deref(),
        Some("Bearer fixture-token")
    );
    assert!(req[0].key.is_none());
}
#[tokio::test]
async fn child_commands_resolve_keys_and_filter_transitions() {
    let server=Server::start(vec![(200,json!({"id":"uuid","status_id":"open"})),(200,json!({"comments":[]})),(200,json!({"id":"uuid","status_id":"open"})),(200,json!([{"from_status_id":"open","to_status_id":"done"},{"from_status_id":"done","to_status_id":"open"}]))]).await;
    success(
        &server
            .run(
                &["comment", "list", "TT-1", "--limit", "20", "--offset", "20"],
                None,
            )
            .await,
    );
    let value = success(&server.run(&["transitions", "--issue", "TT-1"], None).await);
    assert_eq!(value["transitions"].as_array().unwrap().len(), 1);
    assert_eq!(
        server.requests()[1].path,
        "/api/v1/issues/uuid/comments?limit=20&offset=20"
    );
}
#[tokio::test]
async fn links_fields_and_worklogs_send_correct_requests() {
    let server = Server::start(vec![
        (200, json!({"id":"uuid"})),
        (201, json!({"id":"link"})),
        (200, json!({"id":"uuid"})),
        (200, json!({"value":null})),
        (200, json!({"id":"uuid"})),
        (201, json!({"id":"worklog"})),
    ])
    .await;
    success(
        &server
            .run(
                &[
                    "link",
                    "add",
                    "--issue",
                    "TT-1",
                    "--target-key",
                    "TT-2",
                    "--link-type",
                    "blocks",
                ],
                None,
            )
            .await,
    );
    success(
        &server
            .run(
                &[
                    "field",
                    "set",
                    "--issue",
                    "TT-1",
                    "--field",
                    "field",
                    "--from-file",
                    "-",
                ],
                Some("null"),
            )
            .await,
    );
    success(
        &server
            .run(
                &[
                    "worklog",
                    "create",
                    "--issue",
                    "TT-1",
                    "--started-at",
                    "2026-10-01T10:00:00Z",
                    "--duration-seconds",
                    "60",
                    "--from-file",
                    "-",
                ],
                Some("Отчёт"),
            )
            .await,
    );
    let req = server.requests();
    assert_eq!(req[1].path, "/api/v1/issues/uuid/links");
    assert_eq!(req[3].method, "PUT");
    assert_eq!(req[3].path, "/api/v1/issues/uuid/custom-fields/field/value");
    assert_eq!(
        serde_json::from_slice::<Value>(&req[5].body).unwrap()["description"],
        "Отчёт"
    );
}
#[tokio::test]
async fn attachment_upload_download_and_no_clobber() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.txt");
    std::fs::write(&input, "fixture file").unwrap();
    let out = dir.path().join("output.txt");
    let server = Server::bytes(
        vec![
            (200, br#"{"id":"uuid"}"#.to_vec()),
            (201, br#"{"id":"attachment"}"#.to_vec()),
            (200, b"downloaded file".to_vec()),
        ],
        Duration::ZERO,
    )
    .await;
    success(
        &server
            .run(
                &[
                    "attachment",
                    "upload",
                    "--issue",
                    "TT-1",
                    "--file",
                    input.to_str().unwrap(),
                ],
                None,
            )
            .await,
    );
    success(
        &server
            .run(
                &[
                    "attachment",
                    "download",
                    "attachment",
                    "--out",
                    out.to_str().unwrap(),
                ],
                None,
            )
            .await,
    );
    assert_eq!(std::fs::read(&out).unwrap(), b"downloaded file");
    let failed = server
        .run(
            &[
                "attachment",
                "download",
                "attachment",
                "--out",
                out.to_str().unwrap(),
            ],
            None,
        )
        .await;
    assert!(!failed.status.success());
    assert_eq!(server.requests().len(), 3);
    assert!(String::from_utf8_lossy(&server.requests()[1].body).contains("fixture file"));
}
#[tokio::test]
async fn structured_errors_redact_credentials_and_input_conflicts_make_no_request() {
    for status in [401, 403, 409, 429, 500] {
        let server=Server::start(vec![(status,json!({"error":{"code":"FIXTURE_ERROR","message":"fixture-token","request_id":"request-1"}}))]).await;
        let failed = server
            .run(&["--error-format", "json", "issue", "get", "TT-1"], None)
            .await;
        assert_eq!(failed.status.code(), Some(1));
        assert!(failed.stdout.is_empty());
        let value: Value = serde_json::from_slice(&failed.stderr).unwrap();
        assert_eq!(value["error"]["status"], status);
        assert_eq!(value["error"]["request_id"], "request-1");
        assert!(!String::from_utf8_lossy(&failed.stderr).contains("fixture-token"));
    }
    let server = Server::start(vec![]).await;
    let failed = server
        .run(
            &[
                "issue",
                "update",
                "TT-1",
                "--description",
                "inline",
                "--from-file",
                "-",
            ],
            None,
        )
        .await;
    assert_eq!(failed.status.code(), Some(2));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn parsing_errors_are_json_and_do_not_echo_credentials() {
    let server = Server::start(vec![]).await;
    let output = server
        .run(
            &[
                "--error-format",
                "json",
                "--token",
                "sensitive-value",
                "unknown-command",
            ],
            None,
        )
        .await;
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let value: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["error"]["code"], "CLI_USAGE");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("sensitive-value"));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn effective_transport_token_is_redacted() {
    for (token, expected) in [
        (" fixture-token ", "fixture-token"),
        (" ", "shared-fixture-token"),
    ] {
        let server = Server::start(vec![(
            403,
            json!({"error": {"code": "DENIED", "message": expected}}),
        )])
        .await;
        let url = server.url.clone();
        let output = tokio::task::spawn_blocking(move || {
            Command::new(env!("CARGO_BIN_EXE_task-tracker"))
                .args([
                    "--api-url",
                    &url,
                    "--token",
                    token,
                    "--error-format",
                    "json",
                    "issue",
                    "get",
                    "TT-1",
                ])
                .env("SDLC_API_TOKEN", "shared-fixture-token")
                .env_remove("CICD_PROFILE")
                .env_remove("CICD_API_TOKEN")
                .env_remove("TASKTRACKER_TOKEN")
                .output()
                .unwrap()
        })
        .await
        .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["status"], 403);
        assert_eq!(error["error"]["message"], "[REDACTED]");
        assert_eq!(
            server.requests()[0].authorization.as_deref(),
            Some(format!("Bearer {expected}").as_str())
        );
    }
}

#[test]
fn help_hides_token_environment_value() {
    let output = Command::new(env!("CARGO_BIN_EXE_task-tracker"))
        .args(["--help"])
        .env("TASKTRACKER_TOKEN", "help-only-fixture-token")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("TASKTRACKER_TOKEN"));
    assert!(!stdout.contains("help-only-fixture-token"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("help-only-fixture-token"));
}
