use super::*;
use chrono::{Duration, Utc};

fn fixture() -> (
    TaskState,
    PmDraftReservation,
    PermitPmIdleCheckpointRequest,
    PmFeedbackReceipt,
    PmTerminalIdleRecoveryWitness,
) {
    let assignment = PmAssignment {
        assignment_id: Uuid::new_v4(),
        execution_id: Uuid::new_v4(),
        agent_id: Uuid::new_v4(),
        version: 1,
        machine_subject: "synthetic-pm".into(),
    };
    let task = Uuid::new_v4();
    let state: TaskState = serde_json::from_value(json!({"tracker_instance_id":"idle-test", "project_id":Uuid::new_v4(),
        "task_id":task,"root_task_id":task,"owner_subject":"owner","stage":"Draft","draft_submode":"brainstorming",
        "flow":"single-task-v1","confirmation_revision":null,"assignment":assignment,"questions":[],"revisions":[],
        "evidence":[],"confirmations":[]})).unwrap();
    let r: PmDraftReservation = serde_json::from_value(json!({"contract_version":1,"variant":"pm_draft_reserved",
        "binding":{"tracker_instance_id":state.tracker_instance_id,"project_id":state.project_id,"task_id":task,"root_task_id":task,
            "owner_subject":state.owner_subject},"owner_cas":{"expected_version":0,"version":1},"assignment":assignment,
        "execution":{"ordinal":"3","key":"SDLC-3"},"input":{"snapshot_ref":Uuid::new_v4(),"sha256":"a".repeat(64)},
        "assignment_operation_key":"reserve","admission_state":"reserved","dispatch_allowed":false})).unwrap();
    let fence = MachineFence {
        assignment_id: assignment.assignment_id,
        execution_id: assignment.execution_id,
        agent_id: assignment.agent_id,
        assignment_version: assignment.version,
    };
    let feedback = PmFeedbackReceipt {
        message_id: Uuid::new_v4(),
        tracker_instance_id: state.tracker_instance_id.clone(),
        project_id: state.project_id,
        task_id: task,
        root_task_id: task,
        owner_subject: state.owner_subject.clone(),
        fence: fence.clone(),
        text: "Saved original owner feedback".into(),
        content_hash: crate::sdlc::canonical_hash(&json!({"text":"Saved original owner feedback"}))
            .unwrap(),
        requirement_revision: None,
        created_at: Utc::now(),
    };
    let identity = domain::sdlc_execution_lease::PmExecutionIdentity {
        task: r.execution.key.clone(),
        execution_ref: assignment.execution_id.to_string(),
        tracker_instance_ref: state.tracker_instance_id.clone(),
        tracker_project_ref: state.project_id.to_string(),
        task_ref: task.to_string(),
        root_ref: task.to_string(),
        agent_ref: assignment.agent_id.to_string(),
        assignment_operation_key: r.assignment_operation_key.clone(),
        assignment_ref: assignment.assignment_id.to_string(),
        assignment_revision: 1,
    };
    let run = Uuid::new_v4();
    let native = PmIdleNativeObservation {
        identity: identity.clone(),
        observation_ref: Uuid::new_v4(),
        binding_ref: format!("fleet:pm:{run}"),
        hermes_run_ref: "run_accepted".into(),
        session_run_id: run,
        status: PmIdleNativeStatus::Completed,
        dispatch_operation_key: format!("fleet-pm-bind:{run}"),
        checkpoint_ref: None,
        fence: 1,
    };
    let checkpoint = Uuid::new_v4();
    let checkpoint_key = format!("fleet-pm-checkpoint:{checkpoint}");
    let mut body = serde_json::to_value(&identity).unwrap();
    body.as_object_mut().unwrap().extend(json!({"checkpoint_ref":checkpoint,"operation_key":checkpoint_key,
        "session_run_id":run,"binding_ref":native.binding_ref,"hermes_run_ref":native.hermes_run_ref,
        "expected_fence":1,"expected_version":1,"instruction_profile":"single-task-v1","instruction_manifest_sha256":"b".repeat(64),
        "requirements_revision":null,"requirements_sha256":null,"clarification_request_ref":null,"clarification_version":null
    }).as_object().unwrap().clone());
    let c = PermitPmIdleCheckpointRequest {
        expected_owner_version: 1,
        lease_id: Uuid::new_v4(),
        expected_lease_version: 9,
        input: PmResumeInput::Feedback {
            input_id: feedback.message_id,
        },
        previous_run_id: run,
        previous_run_terminal: domain::sdlc_lifecycle::ArtifactReceipt {
            reference: format!("/internal/runtime/v1/pm/runs/{run}"),
            sha256: crate::sdlc::canonical_hash(&serde_json::to_value(&native).unwrap()).unwrap(),
        },
        fence,
        terminal_idle_recovery: PmTerminalIdleRecovery {
            contract_version: 1,
            barrier_id: Uuid::new_v4(),
            checkpoint_operation_key: checkpoint_key.clone(),
            checkpoint_sha256: crate::sdlc::canonical_hash(&body).unwrap(),
        },
        idempotency_key: format!("fleet-pm-terminal-idle:{run}:{}", feedback.message_id),
    };
    let w = PmTerminalIdleRecoveryWitness {
        contract_version: 1,
        barrier_id: c.terminal_idle_recovery.barrier_id,
        owner_subject: state.owner_subject.clone(),
        identity,
        request_sha256: request_hash(&c).unwrap(),
        command: c.clone(),
        native_terminal: native,
        feedback_receipt: domain::sdlc_lifecycle::ArtifactReceipt {
            reference: "synthetic://stored-feedback".into(),
            sha256: crate::sdlc::canonical_hash(&serde_json::to_value(&feedback).unwrap()).unwrap(),
        },
        instruction_operation_key: format!("fleet-pm-first-step:{run}"),
        instruction_request_sha256: "c".repeat(64),
        instruction_result_sha256: "d".repeat(64),
        checkpoint_request: PmIdleCheckpointRequest {
            operation_key: checkpoint_key,
            body,
        },
    };
    (state, r, c, feedback, w)
}

#[test]
fn terminal_idle_wire_is_closed_disjoint_and_old_resume_json_is_unchanged() {
    let (_, _, c, _, w) = fixture();
    let raw = serde_json::to_value(&c).unwrap();
    let parsed: PmLeaseResumeRequest = serde_json::from_value(raw.clone()).unwrap();
    assert!(matches!(&parsed, PmLeaseResumeRequest::TerminalIdle(_)));
    assert_eq!(serde_json::to_value(parsed).unwrap(), raw);
    for key in ["checkpoint", "recovery", "caller_proof"] {
        let mut bad = raw.clone();
        bad[key] = Value::Null;
        assert!(
            serde_json::from_value::<PmLeaseResumeRequest>(bad).is_err(),
            "{key}"
        );
    }
    for kind in ["answer", "confirmation"] {
        let mut bad = raw.clone();
        bad["input"]["input_kind"] = json!(kind);
        assert!(serde_json::from_value::<PmLeaseResumeRequest>(bad).is_err());
    }
    let ordinary = json!({"expected_owner_version":1,"lease_id":c.lease_id,"expected_lease_version":1,
        "input":c.input,"checkpoint":{"checkpoint_id":Uuid::new_v4(),"checkpoint_revision":1,"checkpoint_sha256":"e".repeat(64),
            "fence":c.fence,"requirement_revision":null,"content_hash":null},"previous_run_id":c.previous_run_id,
        "previous_run_terminal":c.previous_run_terminal,"idempotency_key":"ordinary-resume"});
    let old: domain::sdlc_execution_lease::ResumePmExecutionLease =
        serde_json::from_value(ordinary.clone()).unwrap();
    let union: PmLeaseResumeRequest = serde_json::from_value(ordinary).unwrap();
    assert_eq!(
        serde_json::to_value(old).unwrap(),
        serde_json::to_value(union).unwrap()
    );
    let mut extra = serde_json::to_value(&w).unwrap();
    extra["native_terminal"]["authority"] = json!(true);
    assert!(serde_json::from_value::<PmTerminalIdleRecoveryWitness>(extra).is_err());
    let mut missing = serde_json::to_value(&w).unwrap();
    missing["native_terminal"]
        .as_object_mut()
        .unwrap()
        .remove("checkpoint_ref");
    assert!(serde_json::from_value::<PmTerminalIdleRecoveryWitness>(missing).is_err());
}

#[test]
fn terminal_idle_requires_feedback_only_predocument_brainstorming_and_exact_owner() {
    let (state, r, c, feedback, w) = fixture();
    validate_witness(&w, &state, &r, &c, &feedback).unwrap();
    for case in 0..8 {
        let mut state = state.clone();
        let mut c = c.clone();
        match case {
            0 => {
                c.input = PmResumeInput::Answer {
                    input_id: c.input.input_id(),
                }
            }
            1 => {
                c.input = PmResumeInput::Confirmation {
                    input_id: c.input.input_id(),
                }
            }
            2 => state.stage = Stage::Backlog,
            3 => state.draft_submode = Some(DraftSubmode::Clarification),
            4 => {
                state.retained_fields.remove("flow");
            }
            5 => state.confirmation_revision = Some(1),
            6 => state.owner_subject = "other-owner".into(),
            _ => c.expected_owner_version += 1,
        }
        assert!(validate_context(&state, &r, &c).is_err(), "case {case}");
    }
}

#[test]
fn terminal_idle_rejects_foreign_native_instruction_checkpoint_and_unknown_receipts() {
    let (state, r, c, feedback, w) = fixture();
    let original = serde_json::to_value(&w).unwrap();
    for case in 0..12 {
        let mut w = w.clone();
        match case {
            0 => w.barrier_id = Uuid::new_v4(),
            1 => w.request_sha256 = "e".repeat(64),
            2 => w.command.expected_lease_version += 1,
            3 => w.native_terminal.status = PmIdleNativeStatus::Running,
            4 => w.native_terminal.session_run_id = Uuid::new_v4(),
            5 => w.native_terminal.checkpoint_ref = Some(Uuid::new_v4().to_string()),
            6 => w.instruction_result_sha256.clear(),
            7 => w.instruction_operation_key = "foreign".into(),
            8 => w.feedback_receipt.sha256 = "e".repeat(64),
            9 => w.checkpoint_request.body["requirements_revision"] = json!(1),
            10 => w.checkpoint_request.body["instruction_profile"] = json!("legacy"),
            _ => w.identity.assignment_ref = Uuid::new_v4().to_string(),
        }
        assert!(
            validate_witness(&w, &state, &r, &c, &feedback).is_err(),
            "case {case}"
        );
    }
    assert_eq!(serde_json::to_value(w).unwrap(), original);
}

#[test]
fn terminal_idle_permit_replay_can_expire_but_never_extend_or_grant_dispatch() {
    let (_, _, c, _, _) = fixture();
    let issued = Utc::now() - Duration::seconds(60);
    let mut response = PermitPmIdleCheckpointResponse {
        contract_version: 1,
        operation: IdleCheckpointOperation::PermitPmIdleCheckpoint,
        idempotency_key: c.idempotency_key.clone(),
        request_sha256: request_hash(&c).unwrap(),
        permit: PmIdleCheckpointPermit {
            permit_id: Uuid::new_v4(),
            scope: IdleCheckpointScope::IdleCheckpoint,
            barrier_id: c.terminal_idle_recovery.barrier_id,
            session_run_id: c.previous_run_id,
            input_id: c.input.input_id(),
            checkpoint_operation_key: c.terminal_idle_recovery.checkpoint_operation_key.clone(),
            checkpoint_sha256: c.terminal_idle_recovery.checkpoint_sha256.clone(),
            issued_at: issued,
            expires_at: issued + Duration::seconds(30),
        },
        observed_at: issued,
    };
    let core = serde_json::to_value(&response.permit).unwrap();
    response.observed_at = Utc::now();
    validate_response(&c, &response).unwrap();
    assert_eq!(serde_json::to_value(&response.permit).unwrap(), core);
    assert!(
        serde_json::to_value(&response)
            .unwrap()
            .get("lease")
            .is_none()
    );
    response.permit.expires_at += Duration::seconds(1);
    assert!(validate_response(&c, &response).is_err());
}

#[tokio::test]
async fn terminal_idle_callback_uses_existing_auth_exact_get_and_closed_response() {
    use std::io::{Read, Write};
    let (_, _, c, _, w) = fixture();
    let barrier = c.terminal_idle_recovery.barrier_id;
    for bad in [false, true] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let body = if bad {
            json!({"eligible":true}).to_string()
        } else {
            serde_json::to_string(&w).unwrap()
        };
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let n = stream.read(&mut chunk).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&chunk[..n]);
                if request.windows(4).any(|v| v == b"\r\n\r\n") {
                    break;
                }
                assert!(request.len() < 16384);
            }
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            String::from_utf8(request).unwrap()
        });
        let token = format!("sdlc_pat_{}", "a".repeat(64));
        let reader =
            crate::sdlc_configuration::FleetConfigurationReader::new(&origin, &token).unwrap();
        let result = reader.read_terminal_idle_pm_recovery(barrier).await;
        assert_eq!(result.is_ok(), !bad);
        let request = server.join().unwrap();
        assert!(request.starts_with(&format!(
            "GET /internal/runtime/v1/pm/terminal-idle-recoveries/{barrier} HTTP/1.1"
        )));
        assert!(request.contains(&format!("authorization: Bearer {token}")));
        assert!(request.contains("cache-control: no-cache, no-store"));
    }
}
