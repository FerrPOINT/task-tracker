use super::*;

const TASK: &str = "11111111-1111-1111-1111-111111111111";

fn command(key: &str) -> Value {
    json!({"blocker_id":"22222222-2222-2222-2222-222222222222","blocker_sha256":"a".repeat(64),
        "expected_blocker_version":2,"expected_task_version":9,"expected_pm_owner_version":17,
        "expected_pm_assignment_version":17,"idempotency_key":key})
}

// Source-shaped synthetic receipt only, not a dispatched PM/native/Workflow observation.
fn receipt(key: &str) -> Value {
    let value = json!({"purpose":"critical_business_review","reservation":{
        "contract_version":1,"variant":"pm_draft_reserved","binding":{
            "tracker_instance_id":"synthetic-tracker","project_id":"33333333-3333-3333-3333-333333333333",
            "task_id":TASK,"root_task_id":TASK,"owner_subject":"synthetic-owner"},
        "owner_cas":{"expected_version":17,"version":18},"assignment":{
            "assignment_id":"44444444-4444-4444-4444-444444444444",
            "execution_id":"55555555-5555-5555-5555-555555555555",
            "agent_id":"66666666-6666-6666-6666-666666666666","version":18,"machine_subject":"synthetic-pm"},
        "execution":{"ordinal":"23","key":"SDLC-23"},"input":{
            "snapshot_ref":"77777777-7777-7777-7777-777777777777","sha256":"b".repeat(64)},
        "assignment_operation_key":key,"admission_state":"reserved","dispatch_allowed":false},
        "context":{"blocker_id":"22222222-2222-2222-2222-222222222222","blocker_sha256":"a".repeat(64),
            "blocker_version":3,"requirements":{"revision":4,"content_hash":"c".repeat(64)}}});
    let typed: domain::sdlc_critical_pm_reservation::CriticalPmReservationResult =
        serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(typed).unwrap(), value);
    value
}

fn reserve_args() -> Vec<&'static str> {
    vec![
        "sdlc",
        "critical-blocker",
        "pm-review",
        "reserve",
        "--task",
        TASK,
        "--from-file",
        "-",
    ]
}

#[tokio::test]
async fn critical_pm_review_original_post_replay_and_separate_get_keep_exact_payload_and_key() {
    let key = "pm-review:original?x=+%#";
    let result = receipt(key);
    let server = Server::start(vec![
        (201, result.clone()),
        (200, result.clone()),
        (200, result.clone()),
    ])
    .await;
    let body = command(key).to_string();
    assert_eq!(
        success(&server.run(&reserve_args(), Some(&body)).await),
        result
    );
    assert_eq!(
        success(&server.run(&reserve_args(), Some(&body)).await),
        result
    );
    assert_eq!(
        success(
            &server
                .run(
                    &[
                        "sdlc",
                        "critical-blocker",
                        "pm-review",
                        "operation",
                        "--task",
                        TASK,
                        "--key",
                        key
                    ],
                    None
                )
                .await
        ),
        result
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 3);
    for request in &requests[..2] {
        assert_eq!(request.method, "POST");
        assert_eq!(
            request.path,
            format!("/api/v1/issues/{TASK}/sdlc/critical-blockers/pm-reviews")
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&request.body).unwrap(),
            command(key)
        );
        assert_eq!(
            request.authorization.as_deref(),
            Some("Bearer fixture-token")
        );
    }
    assert_eq!(requests[0].body, requests[1].body);
    assert_eq!(requests[2].method, "GET");
    assert_eq!(
        requests[2].path,
        format!(
            "/api/v1/issues/{TASK}/sdlc/critical-blockers/pm-reviews/operations/pm-review%3Aoriginal%3Fx%3D%2B%25%23"
        )
    );
    assert_eq!(
        requests[2].authorization.as_deref(),
        Some("Bearer fixture-token")
    );
    assert!(requests[2].body.is_empty());
}

#[tokio::test]
async fn critical_pm_review_closed_invalid_input_is_rejected_before_http_without_echo() {
    let server = Server::start(vec![]).await;
    for mismatch in 0..9 {
        let mut body = command("original");
        match mismatch {
            0 => body["requested_agent_id"] = json!("private-input-must-not-echo"),
            1 => body["terminal"] = json!({"private":"private-input-must-not-echo"}),
            2 => body["expected_pm_owner_version"] = json!(null),
            3 => body["expected_pm_assignment_version"] = json!(9_007_199_254_740_992_i64),
            4 => body["expected_blocker_version"] = json!(0),
            5 => body["expected_task_version"] = json!("9"),
            6 => body["blocker_id"] = json!(uuid::Uuid::nil()),
            7 => body["blocker_sha256"] = json!("A".repeat(64)),
            _ => {
                body.as_object_mut().unwrap().remove("idempotency_key");
            }
        }
        let output = server.run(&reserve_args(), Some(&body.to_string())).await;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("private-input-must-not-echo"));
    }
    for key in ["", ".", "..", "bad/key", "bad\\key", "bad key"] {
        let output = server
            .run(&reserve_args(), Some(&command(key).to_string()))
            .await;
        assert!(!output.status.success());
        let output = server
            .run(
                &[
                    "sdlc",
                    "critical-blocker",
                    "pm-review",
                    "operation",
                    "--task",
                    TASK,
                    "--key",
                    key,
                ],
                None,
            )
            .await;
        assert!(!output.status.success());
    }
    for args in [
        vec![
            "sdlc",
            "critical-blocker",
            "pm-review",
            "reserve",
            "--task",
            "TT-1",
            "--from-file",
            "-",
        ],
        vec![
            "sdlc",
            "critical-blocker",
            "pm-review",
            "reserve",
            "--task",
            TASK,
            "--from-file",
            "-",
            "--owner",
            "synthetic-owner",
        ],
        vec![
            "sdlc",
            "critical-blocker",
            "pm-review",
            "operation",
            "--task",
            TASK,
            "--key",
            "original",
            "--scheduler",
            "foreign",
        ],
    ] {
        assert_eq!(server.run(&args, Some("{}")).await.status.code(), Some(2));
    }
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn critical_pm_review_denials_and_unknown_ack_do_not_retry_post_and_use_original_get() {
    for status in [401, 403, 404, 409, 422, 503] {
        let server = Server::start(vec![(
            status,
            json!({"error":{"code":"FIXTURE_DENIAL","message":"fixture-token"}}),
        )])
        .await;
        let mut args = vec!["--error-format", "json"];
        args.extend(reserve_args());
        let output = server
            .run(&args, Some(&command("original").to_string()))
            .await;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(server.requests().len(), 1);
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stderr).unwrap()["error"]["status"],
            status
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains("fixture-token"));
    }
    let key = "pm-review:unknown-ack";
    let saved = receipt(key);
    let server = Server::bytes(
        vec![
            (201, b"{".to_vec()),
            (200, serde_json::to_vec(&saved).unwrap()),
        ],
        Duration::ZERO,
    )
    .await;
    let output = server
        .run(&reserve_args(), Some(&command(key).to_string()))
        .await;
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(server.requests().len(), 1);
    assert_eq!(
        success(
            &server
                .run(
                    &[
                        "sdlc",
                        "critical-blocker",
                        "pm-review",
                        "operation",
                        "--task",
                        TASK,
                        "--key",
                        key
                    ],
                    None
                )
                .await
        ),
        saved
    );
    assert_eq!(server.requests().len(), 2);
    assert_eq!(server.requests()[1].method, "GET");
    assert_eq!(
        server.requests()[1].path,
        format!(
            "/api/v1/issues/{TASK}/sdlc/critical-blockers/pm-reviews/operations/pm-review%3Aunknown-ack"
        )
    );
}
