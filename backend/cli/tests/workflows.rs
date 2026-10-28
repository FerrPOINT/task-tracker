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

#[tokio::test]
async fn empty_mutation_success_preserves_human_messages_and_json() {
    type Case<'a> = (&'a [&'a str], &'a str, &'a str, &'a str);
    let cases: Vec<Case<'_>> = vec![
        (
            &["project", "delete", "CLI"],
            "project CLI deleted\n",
            "DELETE",
            "/api/v1/projects/CLI",
        ),
        (
            &["issue", "delete", "CLI-1"],
            "issue CLI-1 deleted\n",
            "DELETE",
            "/api/v1/issues/CLI-1",
        ),
        (
            &["comment", "delete", "comment"],
            "comment comment deleted\n",
            "DELETE",
            "/api/v1/comments/comment",
        ),
        (
            &["label", "delete", "label"],
            "label label deleted\n",
            "DELETE",
            "/api/v1/labels/label",
        ),
        (
            &[
                "label",
                "detach",
                "--issue-id",
                "CLI-1",
                "--label-id",
                "label",
            ],
            "label detached\n",
            "DELETE",
            "/api/v1/issues/issue-id/labels/label",
        ),
        (
            &["notification", "read", "notification"],
            "notification notification marked as read\n",
            "PATCH",
            "/api/v1/notifications/notification/read",
        ),
        (
            &["notification", "read-all"],
            "all notifications marked as read\n",
            "POST",
            "/api/v1/notifications/read-all",
        ),
        (
            &[
                "member",
                "remove",
                "--project-key",
                "CLI",
                "--user-id",
                "user",
            ],
            "member removed\n",
            "DELETE",
            "/api/v1/projects/CLI/members/user",
        ),
    ];
    for (args, message, method, path) in cases {
        for format in ["table", "compact", "json"] {
            for status in [200, 204] {
                let mut responses = vec![];
                if args[0] == "label" && args[1] == "detach" {
                    responses.push((200, br#"{"id":"issue-id"}"#.to_vec()));
                }
                responses.push((status, vec![]));
                let server = Server::bytes(responses, Duration::ZERO).await;
                let mut cli = vec!["--output", format];
                cli.extend_from_slice(args);
                let output = server.run(&cli, None).await;
                assert_eq!(
                    output.status.code(),
                    Some(0),
                    "{args:?}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(output.stderr.is_empty());
                if format == "json" {
                    assert_eq!(
                        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                        json!({"status":"ok"})
                    );
                } else {
                    assert_eq!(
                        String::from_utf8(output.stdout).unwrap(),
                        message,
                        "{args:?} {format}"
                    );
                }
                let requests = server.requests();
                let last = requests.last().unwrap();
                assert_eq!((&last.method[..], &last.path[..]), (method, path));
            }
            let server = Server::start(vec![(
                403,
                json!({"error":{"code":"FORBIDDEN","message":"denied"}}),
            )])
            .await;
            let mut cli = vec!["--output", format, "--error-format", "json"];
            cli.extend_from_slice(args);
            let output = server.run(&cli, None).await;
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stderr).unwrap()["error"]["status"],
                403
            );
        }
    }
}
#[path = "workflows/critical_pm_reviews.rs"]
mod critical_pm_reviews;

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
        self.run_with_api_url(&url, args, input).await
    }
    async fn run_with_api_url(
        &self,
        api_url: &str,
        args: &[&str],
        input: Option<&str>,
    ) -> std::process::Output {
        let url = api_url.to_string();
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
async fn sdlc_commands_map_to_existing_task_uuid_endpoints() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let server = Server::start(vec![
        (200, json!({"context":true})),
        (200, json!({"input":true})),
        (200, json!({"questions":[]})),
        (200, json!({"revision":1})),
        (200, json!({"role":"Analyst"})),
        (200, json!({"runtime_ready":false})),
        (200, json!({"lease_state":"active"})),
        (200, json!({"operation":"reservation"})),
        (200, json!({"operation":"result"})),
        (200, json!({"context":"plain-base"})),
    ])
    .await;

    for (args, path) in [
        (
            vec!["sdlc", "context", "--task", TASK],
            format!("/api/v1/issues/{TASK}/sdlc/context"),
        ),
        (
            vec!["sdlc", "input", "--task", TASK],
            format!("/api/v1/issues/{TASK}/sdlc/pm-draft-input"),
        ),
        (
            vec!["sdlc", "questions", "--task", TASK],
            format!("/api/v1/issues/{TASK}/sdlc/clarifications"),
        ),
        (
            vec!["sdlc", "requirements", "--task", TASK],
            format!("/api/v1/issues/{TASK}/sdlc/requirements"),
        ),
        (
            vec!["sdlc", "intent", "--task", TASK],
            format!("/api/v1/issues/{TASK}/sdlc/analysis-intent"),
        ),
        (
            vec!["sdlc", "analysis", "preflight", "--task", TASK],
            format!("/api/v1/issues/{TASK}/sdlc/analysis-reservation/configuration-preflight"),
        ),
        (
            vec!["sdlc", "analysis", "reservation", "read", "--task", TASK],
            format!("/api/v1/issues/{TASK}/sdlc/analysis-reservation"),
        ),
        (
            vec![
                "sdlc",
                "analysis",
                "reservation",
                "operation",
                "--task",
                TASK,
                "--key",
                "lease key 1",
            ],
            format!("/api/v1/issues/{TASK}/sdlc/analysis-reservation/operations/lease%20key%201"),
        ),
        (
            vec![
                "sdlc",
                "analysis",
                "result",
                "operation",
                "--task",
                TASK,
                "--key",
                "result key 1",
            ],
            format!("/api/v1/issues/{TASK}/sdlc/analysis-result/operations/result%20key%201"),
        ),
    ] {
        success(&server.run(&args, None).await);
        let last = server.requests().last().unwrap().clone();
        assert_eq!(last.method, "GET");
        assert_eq!(last.path, path);
    }

    success(
        &server
            .run_with_api_url(&server.url, &["sdlc", "context", "--task", TASK], None)
            .await,
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 10);
    assert_eq!(
        requests[9].path,
        format!("/api/v1/issues/{TASK}/sdlc/context")
    );
    assert!(requests.iter().all(|request| request.key.is_none()));

    let invalid = server
        .run(&["sdlc", "context", "--task", "PROJ-1"], None)
        .await;
    assert_eq!(invalid.status.code(), Some(2));
    assert_eq!(server.requests().len(), 10);

    // Base rejects encoded path separators before credentials can be sent.
    for args in [
        vec![
            "sdlc",
            "analysis",
            "reservation",
            "operation",
            "--task",
            TASK,
            "--key",
            "lease key/1",
        ],
        vec![
            "sdlc",
            "analysis",
            "result",
            "operation",
            "--task",
            TASK,
            "--key",
            "result key/1",
        ],
    ] {
        let invalid = server.run(&args, None).await;
        assert_eq!(invalid.status.code(), Some(1));
        assert_eq!(server.requests().len(), 10);
    }
}

#[tokio::test]
async fn sdlc_configured_pm_discovery_help_preserves_error_boundaries_without_network() {
    let server = Server::start(vec![]).await;
    let cases: &[(&str, &[&str])] = &[
        (
            "context",
            &[
                "interaction_protocol=configured-cli-v1",
                "workflow_task_key (SDLC-N), not the Tracker task UUID",
                "project-workflow --json step --task WORKFLOW_TASK_KEY",
                "requirement_revision=null is expected",
                "permissions.can_answer and permissions.can_confirm describe owner-only actions",
                "Both may be false for the assigned PM; they do not prohibit publishing questions",
                "answer or confirm as the owner",
                "A real 403 on publication remains a blocker",
                "Do not publish a placeholder",
                "denied context/Workflow access remains a blocker",
            ],
        ),
        (
            "lifecycle",
            &[
                "not a configured-cli-v1 PM startup prerequisite",
                "task-tracker sdlc context --task TASK_UUID",
                "project-workflow --json step --task WORKFLOW_TASK_KEY",
                "Do not acquire a native lease, rebind or borrow another token",
                "authorization failures on required commands remain blockers",
                "Legacy delivery callers retain this command",
            ],
        ),
        (
            "requirements",
            &[
                "requirement_revision=null",
                "expected ONLY when the successful authorized",
                "context read for the SAME task shows no first revision",
                "never a placeholder",
                "If context names a saved revision, a 404 is NOT explained",
                "Do not ignore denied access, a missing task",
            ],
        ),
    ];
    for &(command, expected) in cases {
        let output = server.run(&["sdlc", command, "--help"], None).await;
        assert!(output.status.success(), "{command}");
        assert!(output.stderr.is_empty(), "{command}");
        let help = String::from_utf8(output.stdout).unwrap();
        for &text in expected {
            assert!(help.contains(text), "{command}: missing {text:?}");
        }
    }
    assert!(server.requests().is_empty());
}

fn sdlc_help_payload(output: &std::process::Output) -> (String, Value) {
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let help = String::from_utf8(output.stdout.clone()).unwrap();
    let (_, example) = help
        .split_once("JSON example (configured-cli-v1):")
        .unwrap();
    let payload = serde_json::Deserializer::from_str(example)
        .into_iter::<Value>()
        .next()
        .unwrap()
        .unwrap();
    (help, payload)
}

#[tokio::test]
async fn sdlc_question_publish_help_has_typed_example_without_network_or_owner_authority() {
    let server = Server::start(vec![]).await;
    let output = server
        .run(&["sdlc", "question", "publish", "--help"], None)
        .await;
    let (help, payload) = sdlc_help_payload(&output);
    let question: domain::sdlc::PublishQuestion = serde_json::from_value(payload.clone()).unwrap();
    assert!(payload.get("checkpoint_id").is_none());
    assert_eq!(question.expected_question_version, None);
    assert_eq!(question.mode, domain::sdlc::QuestionMode::Single);
    assert_eq!(question.options.len(), 2);
    assert_ne!(question.options[0].id, question.options[1].id);
    assert_eq!(question.recommended_option_id, Some(question.options[0].id));
    assert!(!question.text.is_empty());
    assert!(help.contains("--from-file -"));
    assert!(help.contains("current unconfirmed requirements revision must already exist"));
    assert!(help.contains("free-form brainstorming until enough substantive product facts"));
    assert!(help.contains("Never publish a placeholder merely to obtain a revision"));
    assert!(help.contains("without native checkpoint_id"));
    assert!(help.contains("PM cannot answer or confirm"));
    assert!(!help.contains("fixture-token"));

    let mut wrong = payload.clone();
    let text = wrong.as_object_mut().unwrap().remove("text").unwrap();
    wrong["question"] = text;
    assert!(serde_json::from_value::<domain::sdlc::PublishQuestion>(wrong).is_err());
    let mut wrong = payload.clone();
    wrong["options"] = json!(["Internal users", "External users"]);
    assert!(serde_json::from_value::<domain::sdlc::PublishQuestion>(wrong).is_err());

    let mut legacy = payload;
    legacy["checkpoint_id"] = json!("99999999-9999-4999-8999-999999999999");
    assert!(serde_json::from_value::<domain::sdlc::PublishQuestion>(legacy).is_err());
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn sdlc_revision_publish_help_has_typed_working_document_without_owner_authority() {
    let server = Server::start(vec![]).await;
    let output = server
        .run(&["sdlc", "revision", "publish", "--help"], None)
        .await;
    let (help, payload) = sdlc_help_payload(&output);
    let revision: domain::sdlc::PublishRevision = serde_json::from_value(payload).unwrap();
    assert_eq!(revision.expected_requirement_revision, None);
    let document = &revision.document;
    app::sdlc::validate_document(document).unwrap();
    assert!(document.goal.contains("personal browser task list"));
    assert!(
        document
            .scope
            .iter()
            .any(|item| item.starts_with("Create a task"))
    );
    assert!(
        document
            .scope
            .iter()
            .any(|item| item.starts_with("Mark an existing task"))
    );
    assert!(
        document
            .acceptance_criteria
            .iter()
            .any(|item| item.starts_with("Reloading restores"))
    );
    assert!(help.contains("--from-file -"));
    assert!(help.contains("current revision from context"));
    assert!(help.contains("free-form brainstorming until enough substantive product facts"));
    assert!(help.contains("Never publish a placeholder merely to obtain a revision"));
    assert!(help.contains("not a PM-process document"));
    assert!(!help.contains("Agree the first release scope with the owner"));
    assert!(help.contains("PM must not call revision confirm"));
    assert!(!help.contains("fixture-token"));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn sdlc_pm_completion_submit_help_has_closed_configured_example_without_network() {
    use domain::sdlc_configured_cli::CompleteConfiguredPm;

    let server = Server::start(vec![]).await;
    let output = server
        .run(&["sdlc", "pm", "completion", "submit", "--help"], None)
        .await;
    let (help, payload) = sdlc_help_payload(&output);
    let completion: CompleteConfiguredPm = serde_json::from_value(payload.clone()).unwrap();
    assert_eq!(completion.fence.assignment_version, 1);
    assert_eq!(completion.requirement_revision, 2);
    assert_eq!(completion.workflow_result_version, 4);
    assert_ne!(
        completion.fence.execution_id,
        completion.workflow_execution_ref
    );
    assert_eq!(completion.content_hash, "a".repeat(64));
    assert!(
        serde_json::from_value::<domain::sdlc_reservation::CompletePmDraft>(payload.clone())
            .is_err()
    );
    for text in [
        "--from-file -",
        "exact saved human-owner confirmation",
        "complete=true result with the real terminal_receipt",
        "Tracker execution_id is NOT Workflow execution_ref",
        "singular",
        "server-to-server",
        "Historical completion remains readable",
        "no blind resend or replacement key",
        "completion operation --task TASK_UUID --key COMPLETION_KEY",
        "completion read --task TASK_UUID",
        "do not queue Analysis or dispatch Analyst",
    ] {
        assert!(help.contains(text), "missing help boundary: {text}");
    }
    assert!(!help.contains("fixture-token"));
    assert!(serde_json::from_value::<CompleteConfiguredPm>(json!({})).is_err());
    let mut wrong = payload.clone();
    let revision = wrong
        .as_object_mut()
        .unwrap()
        .remove("requirement_revision")
        .unwrap();
    wrong["requirements_revision"] = revision;
    assert!(serde_json::from_value::<CompleteConfiguredPm>(wrong).is_err());
    let mut wrong = payload.clone();
    wrong["terminal_receipt"] = json!({});
    assert!(serde_json::from_value::<CompleteConfiguredPm>(wrong).is_err());
    let mut wrong = payload;
    wrong["lease_id"] = json!("77777777-7777-4777-8777-777777777777");
    assert!(serde_json::from_value::<CompleteConfiguredPm>(wrong).is_err());
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn sdlc_publish_commands_forward_json_payloads_without_extra_contracts() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let question = json!({
        "fence": {
            "assignment_id": "22222222-2222-2222-2222-222222222222",
            "execution_id": "33333333-3333-3333-3333-333333333333",
            "agent_id": "44444444-4444-4444-4444-444444444444",
            "assignment_version": 7
        },
        "request_id": "55555555-5555-5555-5555-555555555555",
        "question_id": "66666666-6666-6666-6666-666666666666",
        "expected_question_version": null,
        "requirement_revision": 3,
        "requirement_reference": null,
        "text": "Choose one",
        "rationale": "blocks scope",
        "required": true,
        "mode": "single",
        "options": [
            {
                "id": "88888888-8888-8888-8888-888888888888",
                "label": "A",
                "consequences": "do A",
                "is_custom": false
            }
        ],
        "recommended_option_id": null,
        "idempotency_key": "question-key"
    });
    let revision = json!({
        "fence": {
            "assignment_id": "22222222-2222-2222-2222-222222222222",
            "execution_id": "33333333-3333-3333-3333-333333333333",
            "agent_id": "44444444-4444-4444-4444-444444444444",
            "assignment_version": 7
        },
        "expected_requirement_revision": null,
        "document": {
            "goal": "Ship CLI",
            "scope": ["thin wrapper"],
            "exclusions": [],
            "scenarios": ["PM publishes"],
            "acceptance_criteria": ["server validates"],
            "constraints": [],
            "dependencies": [],
            "assumptions": [],
            "checklist": ["c1"],
            "prerequisites": ["p1"]
        },
        "idempotency_key": "revision-key"
    });
    let server = Server::start(vec![
        (200, json!({"id":"question"})),
        (200, json!({"revision":4})),
    ])
    .await;

    success(
        &server
            .run(
                &[
                    "sdlc",
                    "question",
                    "publish",
                    "--task",
                    TASK,
                    "--from-file",
                    "-",
                ],
                Some(&question.to_string()),
            )
            .await,
    );
    success(
        &server
            .run(
                &[
                    "sdlc",
                    "revision",
                    "publish",
                    "--task",
                    TASK,
                    "--from-file",
                    "-",
                ],
                Some(&revision.to_string()),
            )
            .await,
    );

    let requests = server.requests();
    assert_eq!(requests[0].method, "POST");
    assert_eq!(
        requests[0].path,
        format!("/api/v1/issues/{TASK}/sdlc/clarifications")
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[0].body).unwrap(),
        question
    );
    assert!(requests[0].key.is_none());

    assert_eq!(requests[1].method, "POST");
    assert_eq!(
        requests[1].path,
        format!("/api/v1/issues/{TASK}/sdlc/requirements")
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[1].body).unwrap(),
        revision
    );
    assert!(requests[1].key.is_none());
}

#[tokio::test]
async fn sdlc_analysis_commands_forward_json_payloads_without_extra_contracts() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let reserve = json!({
        "intent_id": "22222222-2222-2222-2222-222222222222",
        "expected_requirement_revision": 2,
        "content_hash": "a".repeat(64),
        "expected_routing_snapshot_id": "33333333-3333-3333-3333-333333333333",
        "idempotency_key": "reserve-key"
    });
    let heartbeat = json!({
        "assignment_id": "44444444-4444-4444-4444-444444444444",
        "execution_id": "55555555-5555-5555-5555-555555555555",
        "expected_lease_version": 1,
        "idempotency_key": "heartbeat-key"
    });
    let result = json!({
        "assignment_id": "44444444-4444-4444-4444-444444444444",
        "execution_id": "55555555-5555-5555-5555-555555555555",
        "fencing_token": "66666666-6666-6666-6666-666666666666",
        "requirement_revision": 2,
        "content_hash": "a".repeat(64),
        "terminal_receipt_ref": "workflow://terminal/1",
        "terminal_receipt_sha256": "b".repeat(64),
        "architecture_input_sha256": "c".repeat(64),
        "idempotency_key": "result-key"
    });
    let server = Server::start(vec![
        (
            201,
            json!({"assignment":{"assignment_id":"44444444-4444-4444-4444-444444444444"}}),
        ),
        (
            200,
            json!({"assignment":{"assignment_id":"44444444-4444-4444-4444-444444444444"}}),
        ),
        (201, json!({"next_stage":"Architecture"})),
    ])
    .await;

    success(
        &server
            .run(
                &[
                    "sdlc",
                    "analysis",
                    "reservation",
                    "reserve",
                    "--task",
                    TASK,
                    "--from-file",
                    "-",
                ],
                Some(&reserve.to_string()),
            )
            .await,
    );
    success(
        &server
            .run(
                &[
                    "sdlc",
                    "analysis",
                    "reservation",
                    "heartbeat",
                    "--task",
                    TASK,
                    "--from-file",
                    "-",
                ],
                Some(&heartbeat.to_string()),
            )
            .await,
    );
    success(
        &server
            .run(
                &[
                    "sdlc",
                    "analysis",
                    "result",
                    "submit",
                    "--task",
                    TASK,
                    "--from-file",
                    "-",
                ],
                Some(&result.to_string()),
            )
            .await,
    );

    let requests = server.requests();
    for request in &requests {
        assert_eq!(request.method, "POST");
        assert!(request.key.is_none());
    }
    assert_eq!(
        requests[0].path,
        format!("/api/v1/issues/{TASK}/sdlc/analysis-reservation")
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[0].body).unwrap(),
        reserve
    );
    assert_eq!(
        requests[1].path,
        format!("/api/v1/issues/{TASK}/sdlc/analysis-reservation/heartbeat")
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[1].body).unwrap(),
        heartbeat
    );
    assert_eq!(
        requests[2].path,
        format!("/api/v1/issues/{TASK}/sdlc/analysis-result")
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[2].body).unwrap(),
        result
    );
}

#[tokio::test]
async fn sdlc_draft_pm_and_exact_confirmation_use_existing_producers() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    const PROJECT: &str = "22222222-2222-2222-2222-222222222222";
    let payload = json!({"idempotency_key":"stable-key","content_hash":"a".repeat(64)});
    let server = Server::start(vec![(201, json!({"task_id":TASK})); 6]).await;
    let cases = [
        (
            vec![
                "sdlc",
                "draft",
                "create",
                "--project",
                PROJECT,
                "--from-file",
                "-",
            ],
            format!("/api/v1/projects/{PROJECT}/sdlc/drafts"),
        ),
        (
            vec![
                "sdlc",
                "pm",
                "assignment",
                "reserve",
                "--task",
                TASK,
                "--from-file",
                "-",
            ],
            format!("/api/v1/issues/{TASK}/sdlc/pm-draft-assignment"),
        ),
        (
            vec![
                "sdlc",
                "pm",
                "lease",
                "claim",
                "--task",
                TASK,
                "--from-file",
                "-",
            ],
            format!("/api/v1/issues/{TASK}/sdlc/pm-draft-execution-lease"),
        ),
        (
            vec![
                "sdlc",
                "pm",
                "lease",
                "heartbeat",
                "--task",
                TASK,
                "--from-file",
                "-",
            ],
            format!("/api/v1/issues/{TASK}/sdlc/pm-draft-execution-lease/heartbeat"),
        ),
        (
            vec![
                "sdlc",
                "pm",
                "completion",
                "submit",
                "--task",
                TASK,
                "--from-file",
                "-",
            ],
            format!("/api/v1/issues/{TASK}/sdlc/pm-draft-completion"),
        ),
        (
            vec![
                "sdlc",
                "revision",
                "confirm",
                "--task",
                TASK,
                "--revision",
                "7",
                "--from-file",
                "-",
            ],
            format!("/api/v1/issues/{TASK}/sdlc/requirements/7/confirm"),
        ),
    ];
    for (args, _) in &cases {
        success(&server.run(args, Some(&payload.to_string())).await);
    }
    let requests = server.requests();
    assert_eq!(requests.len(), cases.len());
    for (request, (_, path)) in requests.iter().zip(cases) {
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, path);
        assert_eq!(
            serde_json::from_slice::<Value>(&request.body).unwrap(),
            payload
        );
        assert!(request.key.is_none());
    }
}

#[tokio::test]
async fn sdlc_pm_retired_lease_commands_are_rejected_before_http() {
    let server = Server::start(vec![]).await;
    let output = server.run(&["sdlc", "pm", "--help"], None).await;
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for supported in ["feedback", "assignment", "completion"] {
        assert!(help.contains(supported));
    }
    assert!(!help.contains("lease"));
    for operation in ["claim", "heartbeat", "read"] {
        let output = server.run(&["sdlc", "pm", "lease", operation], None).await;
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("unrecognized subcommand 'lease'")
        );
    }
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn sdlc_pm_unknown_acceptance_readbacks_preserve_keys_without_mutation() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    const PROJECT: &str = "22222222-2222-2222-2222-222222222222";
    let server = Server::start(vec![(200, json!({"retained":true})); 4]).await;
    let cases = [
        (
            vec![
                "sdlc",
                "draft",
                "operation",
                "--project",
                PROJECT,
                "--key",
                "lost key",
            ],
            format!("/api/v1/projects/{PROJECT}/sdlc/drafts/operations/lost%20key"),
        ),
        (
            vec![
                "sdlc",
                "pm",
                "assignment",
                "read",
                "--task",
                TASK,
                "--key",
                "lost key",
            ],
            format!("/api/v1/issues/{TASK}/sdlc/pm-draft-assignment?idempotency_key=lost%20key"),
        ),
        (
            vec![
                "sdlc",
                "pm",
                "completion",
                "operation",
                "--task",
                TASK,
                "--key",
                "lost key",
            ],
            format!("/api/v1/issues/{TASK}/sdlc/pm-draft-completion/operations/lost%20key"),
        ),
        (
            vec!["sdlc", "pm", "completion", "read", "--task", TASK],
            format!("/api/v1/issues/{TASK}/sdlc/pm-draft-completion"),
        ),
    ];
    for (args, _) in &cases {
        success(&server.run(args, None).await);
    }
    let requests = server.requests();
    assert_eq!(requests.len(), cases.len());
    for (request, (_, path)) in requests.iter().zip(cases) {
        assert_eq!(request.method, "GET");
        assert_eq!(request.path, path);
        assert!(request.body.is_empty());
    }
}

#[tokio::test]
async fn sdlc_exact_confirmation_rejects_invalid_revision_before_http() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let server = Server::start(vec![]).await;
    for revision in ["0", "9007199254740992", "1.0"] {
        let output = server
            .run(
                &[
                    "sdlc",
                    "revision",
                    "confirm",
                    "--task",
                    TASK,
                    "--revision",
                    revision,
                    "--from-file",
                    "-",
                ],
                Some("{}"),
            )
            .await;
        assert!(!output.status.success());
    }
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn sdlc_confirmation_read_returns_actual_retained_owner_receipt_without_post() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    const CONSENT: &str = "22222222-2222-2222-2222-222222222222";
    let receipt = json!({"id":CONSENT,"task_id":TASK,"revision":7,"content_hash":"a".repeat(64),
        "owner_subject":"owner","created_at":"2026-10-13T12:00:00Z","stage":"Backlog"});
    let server = Server::start(vec![(200, receipt.clone())]).await;
    let output = server
        .run(
            &[
                "sdlc",
                "confirmation",
                "read",
                "--task",
                TASK,
                "--revision",
                "7",
            ],
            None,
        )
        .await;
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        receipt
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests.iter().all(|r| r.method == "GET"));
    assert_eq!(
        requests[0].path,
        format!("/api/v1/issues/{TASK}/sdlc/requirements/7/confirmation")
    );
}

#[tokio::test]
async fn sdlc_confirmation_read_rejects_mismatched_receipt_without_publishing() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    const CONSENT: &str = "22222222-2222-2222-2222-222222222222";
    let server = Server::start(vec![(
        200,
        json!({"id":CONSENT,"task_id":TASK,"revision":8,"content_hash":"a".repeat(64),
        "owner_subject":"owner","created_at":"2026-10-13T12:00:00Z","stage":"Backlog"}),
    )])
    .await;
    let output = server
        .run(
            &[
                "sdlc",
                "confirmation",
                "read",
                "--task",
                TASK,
                "--revision",
                "7",
            ],
            None,
        )
        .await;
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(server.requests().len(), 1);
    assert!(server.requests().iter().all(|r| r.method == "GET"));
}

#[tokio::test]
async fn sdlc_confirmation_read_denials_do_not_fallback_to_intent_outbox_or_post() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    for status in [403, 404, 409] {
        let server = Server::start(vec![(status, json!({"error":"unavailable receipt"}))]).await;
        let output = server
            .run(
                &[
                    "sdlc",
                    "confirmation",
                    "read",
                    "--task",
                    TASK,
                    "--revision",
                    "7",
                ],
                None,
            )
            .await;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(
            requests[0].path,
            format!("/api/v1/issues/{TASK}/sdlc/requirements/7/confirmation")
        );
    }
}

#[tokio::test]
async fn sdlc_confirmation_read_rejects_malformed_receipt_without_fallback() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let valid = json!({"id":"22222222-2222-2222-2222-222222222222","task_id":TASK,"revision":7,
        "content_hash":"a".repeat(64),"owner_subject":"owner","created_at":"2026-10-13T12:00:00Z","stage":"Backlog"});
    for (field, value) in [
        ("id", json!("00000000-0000-0000-0000-000000000000")),
        ("task_id", json!("33333333-3333-3333-3333-333333333333")),
        ("revision", json!("7")),
        ("content_hash", json!("not-a-sha256")),
        ("owner_subject", json!(" ")),
        ("created_at", Value::Null),
        ("stage", json!("Analysis")),
    ] {
        let mut receipt = valid.clone();
        receipt[field] = value;
        let server = Server::start(vec![(200, receipt)]).await;
        let output = server
            .run(
                &[
                    "sdlc",
                    "confirmation",
                    "read",
                    "--task",
                    TASK,
                    "--revision",
                    "7",
                ],
                None,
            )
            .await;
        assert!(!output.status.success(), "accepted invalid {field}");
        assert!(output.stdout.is_empty());
        assert_eq!(server.requests().len(), 1);
        assert_eq!(server.requests()[0].method, "GET");
    }
}

#[tokio::test]
async fn sdlc_confirmation_read_rejects_invalid_revision_before_http() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let server = Server::start(vec![]).await;
    for revision in ["0", "-1", "9007199254740992", "1.0", "not-a-revision"] {
        let output = server
            .run(
                &[
                    "sdlc",
                    "confirmation",
                    "read",
                    "--task",
                    TASK,
                    "--revision",
                    revision,
                ],
                None,
            )
            .await;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn sdlc_lifecycle_and_accepted_result_reads_preserve_response_and_authorization() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    const RESULT: &str = "22222222-2222-2222-2222-222222222222";
    let cases = [
        (
            vec!["sdlc", "lifecycle", "--task", TASK],
            format!("/api/v1/issues/{TASK}/sdlc/lifecycle"),
            json!({"observed_at":"2026-10-14T12:00:00Z","runtime_state":"running",
                "receipt":{"assignment":{"previous_result_id":RESULT},"result":null}}),
        ),
        (
            vec!["sdlc", "result", "read", "--task", TASK, "--result", RESULT],
            format!("/api/v1/issues/{TASK}/sdlc/lifecycle/results/{RESULT}"),
            json!({"source":"delivery","receipt":{"result_id":RESULT,"task_id":TASK,
                "output":{"role":"developer","commit_sha":"a".repeat(40),
                    "changes":{"reference":"tracker-attachment:33333333-3333-3333-3333-333333333333","sha256":"b".repeat(64)},
                    "checks":[]}}}),
        ),
    ];
    for (args, path, receipt) in cases {
        for versioned_base in [false, true] {
            let server = Server::start(vec![(200, receipt.clone())]).await;
            let base = if versioned_base {
                format!("{}/api/v1", server.url)
            } else {
                server.url.clone()
            };
            let output = server.run_with_api_url(&base, &args, None).await;
            assert_eq!(success(&output), receipt);
            let requests = server.requests();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].method, "GET");
            assert_eq!(requests[0].path, path);
            assert_eq!(
                requests[0].authorization.as_deref(),
                Some("Bearer fixture-token")
            );
            assert!(requests[0].body.is_empty());
            assert!(requests[0].key.is_none());
        }
    }
}

#[tokio::test]
async fn sdlc_lifecycle_and_accepted_result_read_errors_never_retry_or_mutate() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    const RESULT: &str = "22222222-2222-2222-2222-222222222222";
    let cases = [
        (
            vec!["sdlc", "lifecycle", "--task", TASK],
            format!("/api/v1/issues/{TASK}/sdlc/lifecycle"),
        ),
        (
            vec!["sdlc", "result", "read", "--task", TASK, "--result", RESULT],
            format!("/api/v1/issues/{TASK}/sdlc/lifecycle/results/{RESULT}"),
        ),
    ];
    for (args, path) in cases {
        for status in [401, 403, 404, 409, 500] {
            let server = Server::start(vec![(
                status,
                json!({"error":{"code":"DENIED","message":"fixture-token"}}),
            )])
            .await;
            let mut cli = vec!["--error-format", "json"];
            cli.extend_from_slice(&args);
            let output = server.run(&cli, None).await;
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stderr).unwrap()["error"]["status"],
                status
            );
            assert!(!String::from_utf8_lossy(&output.stderr).contains("fixture-token"));
            let requests = server.requests();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].method, "GET");
            assert_eq!(requests[0].path, path);
            assert_eq!(
                requests[0].authorization.as_deref(),
                Some("Bearer fixture-token")
            );
            assert!(requests[0].body.is_empty());
            assert!(requests[0].key.is_none());
        }
    }
}

#[tokio::test]
async fn sdlc_lifecycle_and_accepted_result_reads_reject_invalid_uuids_before_http() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let server = Server::start(vec![]).await;
    for args in [
        vec!["sdlc", "lifecycle", "--task", "TASK-1"],
        vec![
            "sdlc", "result", "read", "--task", "TASK-1", "--result", TASK,
        ],
        vec![
            "sdlc",
            "result",
            "read",
            "--task",
            TASK,
            "--result",
            "not-a-uuid",
        ],
    ] {
        let output = server.run(&args, None).await;
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn sdlc_role_proposal_forwards_workflow_evidence_and_reconciles_its_original_operation() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let proposal = json!({"fence":{"assignment_id":"22222222-2222-2222-2222-222222222222",
        "execution_id":"33333333-3333-3333-3333-333333333333","fencing_token":7,
        "lease_id":"44444444-4444-4444-4444-444444444444","expected_lease_version":2},
        "workflow_receipt":{"reference":"workflow/terminal/7","sha256":"a".repeat(64)},
        "result":{"role":"reviewer","verdict":"approved","report":{"reference":"evidence/review.json","sha256":"b".repeat(64)}},
        "idempotency_key":"proposal key"});
    let server = Server::start(vec![
        (201, json!({"proposal_id":"proposal"})),
        (200, json!({"proposal_id":"proposal"})),
    ])
    .await;
    success(
        &server
            .run(
                &[
                    "sdlc",
                    "result",
                    "propose",
                    "--task",
                    TASK,
                    "--from-file",
                    "-",
                ],
                Some(&proposal.to_string()),
            )
            .await,
    );
    success(
        &server
            .run(
                &[
                    "sdlc",
                    "result",
                    "operation",
                    "--task",
                    TASK,
                    "--key",
                    "proposal key",
                ],
                None,
            )
            .await,
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(
        requests[0].path,
        format!("/api/v1/issues/{TASK}/sdlc/delivery-proposals")
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[0].body).unwrap(),
        proposal
    );
    assert_eq!(requests[1].method, "GET");
    assert_eq!(
        requests[1].path,
        format!("/api/v1/issues/{TASK}/sdlc/delivery-proposals/operations/proposal%20key")
    );
}

fn critical_blocker_proposal() -> Value {
    let proposal = json!({
        "fence":{"assignment_id":"22222222-2222-2222-2222-222222222222",
            "execution_id":"33333333-3333-3333-3333-333333333333","fencing_token":1,
            "lease_id":"44444444-4444-4444-4444-444444444444","expected_lease_version":1},
        "requirement_revision":1,"content_hash":"a".repeat(64),"kind":"requirements_contradiction",
        "question":"synthetic private business question","rationale":"Two approved requirements contradict.",
        "options":[],"recommendation":null,
        "evidence":[{"receipt":{"reference":"tracker-attachment:55555555-5555-5555-5555-555555555555",
            "sha256":"b".repeat(64)},"binding":{"assignment_id":"22222222-2222-2222-2222-222222222222",
            "execution_id":"33333333-3333-3333-3333-333333333333","role":"analyst"}}],
        "idempotency_key":"critical:original:1?x=+%#"
    });
    let typed: domain::sdlc_critical_blockers::SubmitCriticalBlocker =
        serde_json::from_value(proposal.clone()).unwrap();
    typed
        .validate(domain::sdlc_lifecycle::DeliveryRole::Analyst)
        .unwrap();
    proposal
}

fn critical_return_command() -> Value {
    let command = json!({
        "review":{"blocker_id":"66666666-6666-6666-6666-666666666666","blocker_sha256":"a".repeat(64),
            "expected_blocker_version":1,"expected_task_version":7,
            "fence":{"assignment_id":"22222222-2222-2222-2222-222222222222",
                "execution_id":"33333333-3333-3333-3333-333333333333","fencing_token":1,
                "lease_id":"44444444-4444-4444-4444-444444444444","expected_lease_version":3},
            "idempotency_key":"return:original:1?x=+%#"},
        "terminal":{"assignment_hash":"b".repeat(64),"chat_id":"77777777-7777-7777-7777-777777777777",
            "run_id":"88888888-8888-8888-8888-888888888888","state":"cancelled",
            "receipt":{"reference":"fixture://SYNTHETIC-native-terminal-readback","sha256":"c".repeat(64)}}
    });
    let typed: domain::sdlc_critical_blockers::ReturnCriticalToPm =
        serde_json::from_value(command.clone()).unwrap();
    assert_eq!(serde_json::to_value(typed).unwrap(), command);
    command
}

#[tokio::test]
async fn sdlc_critical_return_preserves_file_stdin_body_auth_and_exact_endpoint() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let command = critical_return_command();
    // Transport-only partial response stub. Typed plan and actual persistence are tested elsewhere.
    let receipt = json!({"lifecycle":{"task":{"task_id":TASK,"stage":"Draft"}},
        "blocker":{"version":2,"state":"awaiting_pm_review","technical_terminal":command["terminal"]}});
    let mut file = tempfile::NamedTempFile::new().unwrap();
    write!(file, "{command}").unwrap();
    file.flush().unwrap();
    for versioned in [false, true] {
        for from_file in ["-", file.path().to_str().unwrap()] {
            for status in [201, 200] {
                let server = Server::start(vec![(status, receipt.clone())]).await;
                let url = if versioned {
                    format!("{}/api/v1", server.url)
                } else {
                    server.url.clone()
                };
                let output = server
                    .run_with_api_url(
                        &url,
                        &[
                            "sdlc",
                            "critical-blocker",
                            "return-to-pm",
                            "--task",
                            TASK,
                            "--from-file",
                            from_file,
                        ],
                        (from_file == "-").then(|| command.to_string()).as_deref(),
                    )
                    .await;
                assert_eq!(success(&output), receipt);
                let requests = server.requests();
                assert_eq!(requests.len(), 1);
                assert_eq!(requests[0].method, "POST");
                assert_eq!(
                    requests[0].path,
                    format!("/api/v1/issues/{TASK}/sdlc/critical-blockers/returns")
                );
                assert_eq!(
                    serde_json::from_slice::<Value>(&requests[0].body).unwrap(),
                    command
                );
                assert_eq!(
                    requests[0].authorization.as_deref(),
                    Some("Bearer fixture-token")
                );
                assert!(
                    requests[0].key.is_none(),
                    "key is the exact supplied command field, not another header/transport"
                );
            }
        }
    }
}

#[tokio::test]
async fn sdlc_critical_return_unknown_post_requires_explicit_original_key_get_not_lifecycle_transport()
 {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let command = critical_return_command();
    let saved = json!({"lifecycle":{"task":{"task_id":TASK,"version":8}},"blocker":{"version":2,"state":"awaiting_pm_review"}});
    let server = Server::start(vec![
        (
            503,
            json!({"error":{"code":"UNAVAILABLE","message":"unknown outcome"}}),
        ),
        (200, saved.clone()),
    ])
    .await;
    let failed = server
        .run(
            &[
                "sdlc",
                "critical-blocker",
                "return-to-pm",
                "--task",
                TASK,
                "--from-file",
                "-",
            ],
            Some(&command.to_string()),
        )
        .await;
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert_eq!(server.requests().len(), 1);
    let key = command["review"]["idempotency_key"].as_str().unwrap();
    let read = server
        .run(
            &[
                "sdlc",
                "critical-blocker",
                "return-operation",
                "--task",
                TASK,
                "--key",
                key,
            ],
            None,
        )
        .await;
    assert_eq!(success(&read), saved);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[0].body).unwrap(),
        command
    );
    assert_eq!(requests[1].method, "GET");
    assert_eq!(
        requests[1].path,
        format!(
            "/api/v1/issues/{TASK}/sdlc/critical-blockers/returns/operations/{}",
            urlencoding::encode(key)
        )
    );
    assert!(requests[1].body.is_empty());
    assert!(requests[1].key.is_none());
    assert_eq!(
        requests[1].authorization.as_deref(),
        Some("Bearer fixture-token")
    );
}

#[tokio::test]
async fn sdlc_critical_return_errors_do_not_retry_mutate_or_leak_private_body() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let command = critical_return_command().to_string();
    for status in [401, 403, 404, 409, 422, 503] {
        for action in ["return-to-pm", "return-operation"] {
            let server = Server::start(vec![(
                status,
                json!({"error":{"code":"DENIED","message":"denied fixture-token"}}),
            )])
            .await;
            let mut args = vec![
                "--error-format",
                "json",
                "sdlc",
                "critical-blocker",
                action,
                "--task",
                TASK,
            ];
            args.extend_from_slice(if action == "return-to-pm" {
                &["--from-file", "-"]
            } else {
                &["--key", "return:original:1?x=+%#"]
            });
            let output = server
                .run(
                    &args,
                    (action == "return-to-pm").then_some(command.as_str()),
                )
                .await;
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!stderr.contains("fixture-token"));
            assert!(!stderr.contains("SYNTHETIC-native-terminal-readback"));
            assert_eq!(
                serde_json::from_str::<Value>(&stderr).unwrap()["error"]["status"],
                status
            );
            let requests = server.requests();
            assert_eq!(requests.len(), 1);
            assert_eq!(
                requests[0].method,
                if action == "return-to-pm" {
                    "POST"
                } else {
                    "GET"
                }
            );
        }
    }
}

#[tokio::test]
async fn sdlc_critical_return_invalid_cli_inputs_and_impersonation_flags_never_send_http() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let server = Server::start(vec![]).await;
    for args in [
        vec![
            "sdlc",
            "critical-blocker",
            "return-to-pm",
            "--task",
            "TT-1",
            "--from-file",
            "-",
        ],
        vec![
            "sdlc",
            "critical-blocker",
            "return-operation",
            "--task",
            "TT-1",
            "--key",
            "original",
        ],
        vec![
            "sdlc",
            "critical-blocker",
            "return-to-pm",
            "--task",
            TASK,
            "--from-file",
            "-",
            "--owner",
            "owner",
        ],
        vec![
            "sdlc",
            "critical-blocker",
            "return-to-pm",
            "--task",
            TASK,
            "--from-file",
            "-",
            "--scheduler",
            "scheduler",
        ],
    ] {
        let output = server.run(&args, Some("{}")).await;
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    for key in [
        ".",
        "..",
        "space key",
        "",
        "return/original",
        "return\\original",
    ] {
        let output = server
            .run(
                &[
                    "sdlc",
                    "critical-blocker",
                    "return-operation",
                    "--task",
                    TASK,
                    "--key",
                    key,
                ],
                None,
            )
            .await;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let mut body = critical_return_command();
        body["review"]["idempotency_key"] = json!(key);
        let output = server
            .run(
                &[
                    "sdlc",
                    "critical-blocker",
                    "return-to-pm",
                    "--task",
                    TASK,
                    "--from-file",
                    "-",
                ],
                Some(&body.to_string()),
            )
            .await;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    let output = server
        .run(
            &[
                "sdlc",
                "critical-blocker",
                "return-to-pm",
                "--task",
                TASK,
                "--from-file",
                "-",
            ],
            Some("not-json"),
        )
        .await;
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn sdlc_critical_return_nested_help_is_available_without_network() {
    let server = Server::start(vec![]).await;
    for action in ["return-to-pm", "return-operation"] {
        let output = server
            .run(&["sdlc", "critical-blocker", action, "--help"], None)
            .await;
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("--task"));
        assert!(stdout.contains(if action == "return-to-pm" {
            "--from-file"
        } else {
            "--key"
        }));
    }
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn sdlc_critical_blocker_submit_preserves_typed_body_and_auth() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let proposal = critical_blocker_proposal();
    let receipt = json!({"blocker_id":"66666666-6666-6666-6666-666666666666",
        "version":1,"state":"held","proposal":proposal});
    let mut file = tempfile::NamedTempFile::new().unwrap();
    write!(file, "{proposal}").unwrap();
    file.flush().unwrap();
    for versioned in [false, true] {
        for from_file in ["-", file.path().to_str().unwrap()] {
            let server = Server::start(vec![(201, receipt.clone())]).await;
            let url = if versioned {
                format!("{}/api/v1", server.url)
            } else {
                server.url.clone()
            };
            let output = server
                .run_with_api_url(
                    &url,
                    &[
                        "sdlc",
                        "critical-blocker",
                        "submit",
                        "--task",
                        TASK,
                        "--from-file",
                        from_file,
                    ],
                    (from_file == "-").then(|| proposal.to_string()).as_deref(),
                )
                .await;
            assert_eq!(success(&output), receipt);
            let requests = server.requests();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].method, "POST");
            assert_eq!(
                requests[0].path,
                format!("/api/v1/issues/{TASK}/sdlc/critical-blockers")
            );
            assert_eq!(
                serde_json::from_slice::<Value>(&requests[0].body).unwrap(),
                proposal
            );
            assert_eq!(
                requests[0].authorization.as_deref(),
                Some("Bearer fixture-token")
            );
            assert!(requests[0].key.is_none());
        }
    }
}

#[tokio::test]
async fn sdlc_critical_blocker_reads_exact_detail_and_original_key_without_mutation() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    const BLOCKER: &str = "66666666-6666-6666-6666-666666666666";
    let current = json!({"blocker_id":BLOCKER,"version":2,"state":"awaiting_pm_review"});
    let original = json!({"blocker_id":BLOCKER,"version":1,"state":"held"});
    let server = Server::start(vec![(200, current.clone()), (200, original.clone())]).await;
    assert_eq!(
        success(
            &server
                .run(
                    &[
                        "sdlc",
                        "critical-blocker",
                        "read",
                        "--task",
                        TASK,
                        "--blocker",
                        BLOCKER
                    ],
                    None,
                )
                .await
        ),
        current
    );
    assert_eq!(
        success(
            &server
                .run(
                    &[
                        "sdlc",
                        "critical-blocker",
                        "operation",
                        "--task",
                        TASK,
                        "--key",
                        "critical:original:1?x=+%#"
                    ],
                    None,
                )
                .await
        ),
        original
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].path,
        format!("/api/v1/issues/{TASK}/sdlc/critical-blockers/{BLOCKER}")
    );
    assert_eq!(
        requests[1].path,
        format!(
            "/api/v1/issues/{TASK}/sdlc/critical-blockers/operations/critical%3Aoriginal%3A1%3Fx%3D%2B%25%23"
        )
    );
    for request in requests {
        assert_eq!(request.method, "GET");
        assert!(request.body.is_empty());
        assert!(request.key.is_none());
        assert_eq!(
            request.authorization.as_deref(),
            Some("Bearer fixture-token")
        );
    }
}

#[tokio::test]
async fn sdlc_critical_blocker_errors_never_retry_or_publish_success() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let proposal = critical_blocker_proposal().to_string();
    for status in [401, 403, 404, 409, 422, 503] {
        for action in ["submit", "read", "operation"] {
            let server = Server::start(vec![(
                status,
                json!({"error":{"code":"DENIED",
                "message":"request denied fixture-token"}}),
            )])
            .await;
            let mut args = vec![
                "--error-format",
                "json",
                "sdlc",
                "critical-blocker",
                action,
                "--task",
                TASK,
            ];
            args.extend_from_slice(match action {
                "submit" => &["--from-file", "-"],
                "read" => &["--blocker", "66666666-6666-6666-6666-666666666666"],
                _ => &["--key", "critical:original:1?x=+%#"],
            });
            let output = server
                .run(&args, (action == "submit").then_some(proposal.as_str()))
                .await;
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!stderr.contains("fixture-token"));
            assert!(!stderr.contains("synthetic private business question"));
            assert_eq!(
                serde_json::from_str::<Value>(&stderr).unwrap()["error"]["status"],
                status
            );
            let requests = server.requests();
            assert_eq!(requests.len(), 1, "{action} {status}");
            assert_eq!(
                requests[0].method,
                if action == "submit" { "POST" } else { "GET" }
            );
        }
    }
}

#[tokio::test]
async fn sdlc_critical_blocker_unknown_submit_requires_explicit_original_key_read() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let proposal = critical_blocker_proposal();
    let original = json!({"blocker_id":"66666666-6666-6666-6666-666666666666","version":1,"proposal":proposal});
    let server = Server::start(vec![
        (
            503,
            json!({"error":{"code":"UNAVAILABLE","message":"unknown outcome"}}),
        ),
        (200, original.clone()),
    ])
    .await;
    let failed = server
        .run(
            &[
                "sdlc",
                "critical-blocker",
                "submit",
                "--task",
                TASK,
                "--from-file",
                "-",
            ],
            Some(&proposal.to_string()),
        )
        .await;
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert_eq!(server.requests().len(), 1);
    let key = proposal["idempotency_key"].as_str().unwrap();
    let read = server
        .run(
            &[
                "sdlc",
                "critical-blocker",
                "operation",
                "--task",
                TASK,
                "--key",
                key,
            ],
            None,
        )
        .await;
    assert_eq!(success(&read), original);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[0].body).unwrap(),
        proposal
    );
    assert_eq!(requests[1].method, "GET");
    assert_eq!(
        requests[1].path,
        format!(
            "/api/v1/issues/{TASK}/sdlc/critical-blockers/operations/{}",
            urlencoding::encode(key)
        )
    );
}

#[tokio::test]
async fn sdlc_critical_blocker_invalid_inputs_never_make_http_requests() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let server = Server::start(vec![]).await;
    for args in [
        vec![
            "sdlc",
            "critical-blocker",
            "submit",
            "--task",
            "TT-1",
            "--from-file",
            "-",
        ],
        vec![
            "sdlc",
            "critical-blocker",
            "read",
            "--task",
            TASK,
            "--blocker",
            "not-a-uuid",
        ],
        vec![
            "sdlc",
            "critical-blocker",
            "operation",
            "--task",
            "TT-1",
            "--key",
            "original",
        ],
        vec![
            "sdlc",
            "critical-blocker",
            "submit",
            "--task",
            TASK,
            "--from-file",
            "-",
            "--owner",
            "owner",
        ],
        vec![
            "sdlc",
            "critical-blocker",
            "submit",
            "--task",
            TASK,
            "--from-file",
            "-",
            "--scheduler",
            "scheduler",
        ],
    ] {
        let output = server.run(&args, Some("{}")).await;
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    let malformed = server
        .run(
            &[
                "sdlc",
                "critical-blocker",
                "submit",
                "--task",
                TASK,
                "--from-file",
                "-",
            ],
            Some("{"),
        )
        .await;
    assert_eq!(malformed.status.code(), Some(1));
    assert!(malformed.stdout.is_empty());
    for key in ["", ".", "..", "critical/1", "critical\\1", "bad key"] {
        let rejected = server
            .run(
                &[
                    "sdlc",
                    "critical-blocker",
                    "operation",
                    "--task",
                    TASK,
                    "--key",
                    key,
                ],
                None,
            )
            .await;
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty());
        let mut proposal = critical_blocker_proposal();
        proposal["idempotency_key"] = json!(key);
        let rejected = server
            .run(
                &[
                    "sdlc",
                    "critical-blocker",
                    "submit",
                    "--task",
                    TASK,
                    "--from-file",
                    "-",
                ],
                Some(&proposal.to_string()),
            )
            .await;
        assert!(!rejected.status.success());
        assert!(rejected.stdout.is_empty());
    }
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn sdlc_publish_errors_do_not_echo_request_documents_or_retry() {
    const TASK: &str = "11111111-1111-1111-1111-111111111111";
    let server = Server::start(vec![(
        409,
        json!({"error":{"code":"CONFLICT","message":"same idempotency key changed payload"}}),
    )])
    .await;

    let failed = server
        .run(
            &[
                "--error-format",
                "json",
                "sdlc",
                "question",
                "publish",
                "--task",
                TASK,
                "--from-file",
                "-",
            ],
            Some(r#"{"idempotency_key":"same-key","secret_body":"do-not-echo"}"#),
        )
        .await;

    assert_eq!(failed.status.code(), Some(1));
    assert!(failed.stdout.is_empty());
    assert_eq!(server.requests().len(), 1);
    let stderr = String::from_utf8_lossy(&failed.stderr);
    assert!(!stderr.contains("do-not-echo"));
    let value: Value = serde_json::from_slice(&failed.stderr).unwrap();
    assert_eq!(value["error"]["status"], 409);
    assert_eq!(value["error"]["code"], "CONFLICT");
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
