use super::*;
use serde_json::{Value, json};

fn confirmed() -> (
    TaskState,
    Principal,
    CompleteConfiguredPm,
    ConfiguredWorkflowReceipt,
) {
    let (mut state, pm, config, fence) = super::tests::fixture();
    state.status = Some(domain::sdlc_lifecycle::LifecycleStatus::Ready);
    let revision:PublishRevision=serde_json::from_value(json!({"fence":fence,"expected_requirement_revision":null,
        "idempotency_key":"revision","document":{"goal":"Goal","scope":["Scope"],"exclusions":[],"scenarios":["Scenario"],
        "acceptance_criteria":["Acceptance"],"constraints":[],"dependencies":[],"assumptions":[],"checklist":["Check"],"prerequisites":["Prerequisite"]}})).unwrap();
    crate::sdlc::apply(
        &mut state,
        &pm,
        &config,
        &SdlcCommand::PublishRevision(revision),
    )
    .unwrap();
    let verifier = Principal {
        subject: config.verifier_subject.clone(),
        human_session: false,
        trusted_human: false,
        scopes: [format!("task-tracker:sdlc:evidence:{}:1", state.task_id)]
            .into_iter()
            .collect(),
    };
    for check in ["Check", "Prerequisite"] {
        let evidence = EvidenceCommand {
            fence: fence.clone(),
            requirement_revision: 1,
            content_hash: state.revisions[0].content_hash.clone(),
            check_id: check.into(),
            evidence_reference: "fixture://readiness".into(),
            idempotency_key: format!("evidence-{check}"),
        };
        crate::sdlc::apply(
            &mut state,
            &verifier,
            &config,
            &SdlcCommand::Evidence(evidence),
        )
        .unwrap();
    }
    let owner = Principal {
        subject: state.owner_subject.clone(),
        human_session: true,
        trusted_human: true,
        scopes: Default::default(),
    };
    let confirm = ConfirmCommand {
        content_hash: state.revisions[0].content_hash.clone(),
        idempotency_key: "owner-confirm".into(),
        expected_routing_policy_version: None,
    };
    crate::sdlc::apply(
        &mut state,
        &owner,
        &config,
        &SdlcCommand::Confirm {
            revision: 1,
            command: confirm,
        },
    )
    .unwrap();
    let command = CompleteConfiguredPm {
        interaction_protocol: InteractionProtocol::ConfiguredCliV1,
        fence: fence.clone(),
        requirement_revision: 1,
        content_hash: state.revisions[0].content_hash.clone(),
        namespace_id: 2,
        workflow_execution_ref: Uuid::from_u128(123),
        workflow_operation_key: "final-a01".into(),
        workflow_result_version: 4,
        idempotency_key: "finish-a01".into(),
    };
    let receipt = ConfiguredWorkflowReceipt {
        interaction_protocol: InteractionProtocol::ConfiguredCliV1,
        execution_ref: command.workflow_execution_ref,
        task: "SDLC-12".into(),
        tracker_task_id: state.task_id,
        namespace_id: 2,
        concrete_agent_ref: fence.agent_id,
        assignment_ref: fence.assignment_id,
        tracker_execution_id: fence.execution_id,
        assignment_version: 1,
        machine_subject: pm.subject.clone(),
        operation_key: command.workflow_operation_key.clone(),
        request_sha256: "a".repeat(64),
        result_version: 4,
        requirements_revision: 1,
        requirements_sha256: command.content_hash.clone(),
        owner_subject: state.owner_subject.clone(),
        confirmation_id: state.current_confirmation().unwrap().id,
        history_id: 17,
        status: "done".into(),
    };
    (state, pm, command, receipt)
}

fn response(receipt: &ConfiguredWorkflowReceipt) -> Value {
    json!({"ok":true,"complete":true,"receipt":receipt,"workflow_receipt":{
        "reference":format!("workflow-configured-cli:{}:{}",receipt.execution_ref,receipt.history_id),
        "sha256":crate::sdlc::canonical_hash(receipt).unwrap()}})
}

#[test]
fn completion_requires_real_owner_confirmation_and_current_service_fence() {
    let (state, pm, command, receipt) = confirmed();
    validate_completion(&state, &pm, &command).unwrap();
    validate_receipt(&state, &command, &receipt).unwrap();
    let mut owner = pm.clone();
    owner.subject = state.owner_subject.clone();
    owner.human_session = true;
    assert!(validate_completion(&state, &owner, &command).is_err());
    let mut scheduler = pm.clone();
    scheduler.subject = "scheduler".into();
    assert!(validate_completion(&state, &scheduler, &command).is_err());
    let mut stale = command.clone();
    stale.fence.assignment_version += 1;
    assert!(validate_completion(&state, &pm, &stale).is_err());
    let mut unconfirmed = state.clone();
    unconfirmed.confirmations.clear();
    assert!(validate_completion(&unconfirmed, &pm, &command).is_err());
    let mut native = serde_json::to_value(&command).unwrap();
    native["terminal_receipt"] = json!(receipt);
    assert!(serde_json::from_value::<CompleteConfiguredPm>(native).is_err());
}

#[test]
fn authoritative_receipt_cannot_substitute_another_business_or_owner_tuple() {
    let (state, _, command, receipt) = confirmed();
    for (key, value) in [
        ("assignment_version", json!(2)),
        ("assignment_ref", json!(Uuid::from_u128(88))),
        ("tracker_execution_id", json!(Uuid::from_u128(89))),
        ("concrete_agent_ref", json!(Uuid::from_u128(90))),
        ("tracker_task_id", json!(Uuid::from_u128(91))),
        ("task", json!("SDLC-13")),
        ("machine_subject", json!("foreign")),
        ("owner_subject", json!("foreign")),
        ("confirmation_id", json!(Uuid::from_u128(92))),
        ("requirements_revision", json!(2)),
        ("requirements_sha256", json!("b".repeat(64))),
        ("result_version", json!(5)),
        ("namespace_id", json!(3)),
        ("operation_key", json!("foreign")),
        ("status", json!("active")),
    ] {
        let mut changed = serde_json::to_value(&receipt).unwrap();
        changed[key] = value;
        let changed: ConfiguredWorkflowReceipt = serde_json::from_value(changed).unwrap();
        assert!(
            validate_receipt(&state, &command, &changed).is_err(),
            "{key}"
        );
    }
}

#[test]
fn terminal_lookup_requires_complete_and_exact_sealed_receipt() {
    let (state, _, command, receipt) = confirmed();
    let value = response(&receipt);
    let decode = |v: &Value| {
        crate::sdlc_workflow_receipt::decode(&serde_json::to_vec(v).unwrap(), &state, &command)
    };
    assert_eq!(decode(&value).unwrap(), receipt);
    for field in ["ok", "complete"] {
        let mut changed = value.clone();
        changed[field] = json!(false);
        assert!(decode(&changed).is_err());
    }
    let mut changed = value.clone();
    changed["receipt"]["status"] = json!("active");
    assert!(decode(&changed).is_err());
    let mut changed = value.clone();
    changed["receipt"]
        .as_object_mut()
        .unwrap()
        .remove("assignment_version");
    assert!(decode(&changed).is_err());
    let mut changed = value.clone();
    changed["workflow_receipt"]["sha256"] = json!("0".repeat(64));
    assert!(decode(&changed).is_err());
}

#[tokio::test]
async fn workflow_reader_uses_one_read_only_lookup_post_without_retry() {
    use axum::{Json, Router, extract::State, http::HeaderMap, routing::post};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let (state, _, command, receipt) = confirmed();
    let count = Arc::new(AtomicUsize::new(0));
    let shared = (response(&receipt), count.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new()
        .route(
            "/internal/runtime/terminal-receipt/readback",
            post(
                |State((body, count)): State<(Value, Arc<AtomicUsize>)>,
                 headers: HeaderMap,
                 Json(selector): Json<Value>| async move {
                    assert_eq!(
                        headers["authorization"],
                        "Bearer synthetic-service-read-token"
                    );
                    assert_eq!(selector["interaction_protocol"], "configured-cli-v1");
                    assert_eq!(selector["task"], "SDLC-12");
                    assert_eq!(selector["operation_key"], "final-a01");
                    assert_eq!(selector.as_object().unwrap().len(), 10);
                    count.fetch_add(1, Ordering::SeqCst);
                    Json(body)
                },
            ),
        )
        .with_state(shared);
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let reader = crate::sdlc_workflow_receipt::WorkflowReceiptReader::new(
        &origin,
        "synthetic-service-read-token",
    )
    .unwrap();
    let result = reader.read(&state, &command).await;
    server.abort();
    assert_eq!(result.unwrap(), receipt);
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn failed_lookup_is_not_retried_and_private_error_body_is_not_returned() {
    use axum::{Router, extract::State, http::StatusCode, routing::post};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let (state, _, command, _) = confirmed();
    let count = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new()
        .route(
            "/internal/runtime/terminal-receipt/readback",
            post(|State(count): State<Arc<AtomicUsize>>| async move {
                count.fetch_add(1, Ordering::SeqCst);
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "synthetic-private-error-body",
                )
            }),
        )
        .with_state(count.clone());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let reader = crate::sdlc_workflow_receipt::WorkflowReceiptReader::new(
        &origin,
        "synthetic-service-read-token",
    )
    .unwrap();
    let result = reader.read(&state, &command).await;
    server.abort();
    let error = result.unwrap_err().to_string();
    assert!(!error.contains("synthetic-private-error-body"));
    assert_eq!(count.load(Ordering::SeqCst), 1);
}
