use super::*;
use domain::attachment_artifact::ArtifactBinding;
use domain::sdlc::PmAssignment;
use domain::sdlc_lifecycle::DeliveryRole;
use serde_json::json;
use uuid::Uuid;

fn actor(task: Uuid) -> Principal {
    let assignment = PmAssignment {
        assignment_id: Uuid::new_v4(),
        execution_id: Uuid::new_v4(),
        agent_id: Uuid::new_v4(),
        version: 18,
        machine_subject: "synthetic-pm".into(),
    };
    Principal {
        subject: assignment.machine_subject.clone(),
        human_session: false,
        trusted_human: false,
        scopes: ["task-tracker:read".into(), assignment.scope(task)].into(),
    }
}

fn fixture(role: DeliveryRole) -> (CriticalBlockerReceipt, domain::Attachment, Seal, Vec<u8>) {
    let task = Uuid::new_v4();
    let bytes = b"{ \"critical\": true }\n".to_vec();
    let seal = Seal::new(
        task,
        ArtifactBinding {
            assignment_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            role,
        },
        Uuid::new_v4(),
        "critical.json",
        "application/json",
        crate::attachment_artifacts::raw_sha256(&bytes),
    )
    .unwrap();
    let item = domain::Attachment {
        id: shared::AttachmentId::from_uuid(seal.artifact_id),
        issue_id: shared::IssueId::from_uuid(task),
        author_id: shared::UserId::new(),
        file_name: "critical.json".into(),
        content_type: "application/json".into(),
        size_bytes: bytes.len() as i64,
        storage_key: seal.key().into(),
        created_at: shared::now(),
    };
    let proposal = json!({"fence":{"assignment_id":seal.binding.assignment_id,"execution_id":seal.binding.execution_id,
            "lease_id":Uuid::new_v4(),"fencing_token":5,"expected_lease_version":3},
        "requirement_revision":4,"content_hash":"a".repeat(64),"kind":"requirements_contradiction",
        "question":"Which rule is authoritative?","rationale":"The approved rules conflict.",
        "options":[],"recommendation":null,"idempotency_key":"original-question","evidence":[seal.artifact()]});
    let mut blocker:CriticalBlockerReceipt = serde_json::from_value(json!({
        "blocker_id":Uuid::new_v4(),"version":3,"tracker_instance_id":"synthetic-tracker","project_id":Uuid::new_v4(),
        "task_id":task,"root_task_id":task,"owner_subject":"synthetic-owner","producer_subject":"synthetic-role",
        "agent_id":Uuid::new_v4(),"role":role,"assignment_hash":"b".repeat(64),"chat_id":Uuid::new_v4(),
        "run_id":seal.run_id,"routing_snapshot_id":Uuid::new_v4(),"routing_generation":1,
        "blocker_sha256":crate::sdlc::canonical_hash(&proposal).unwrap(),"proposal":proposal,
        "state":"in_pm_review","pm_review":{"assignment_id":Uuid::new_v4(),"execution_id":Uuid::new_v4()},
        "resolution":null,"technical_terminal":null,"created_at":chrono::Utc::now()
    })).unwrap();
    blocker.technical_terminal = Some(domain::sdlc_lifecycle::RuntimeTerminalObservation {
        assignment_hash: blocker.assignment_hash.clone(),
        chat_id: blocker.chat_id,
        run_id: blocker.run_id,
        state: domain::sdlc_lifecycle::RuntimeTerminalState::Cancelled,
        receipt: domain::sdlc_lifecycle::ArtifactReceipt {
            reference: "fixture://synthetic-terminal-not-live-proof".into(),
            sha256: "e".repeat(64),
        },
    });
    blocker.proposal.validate(role).unwrap();
    (blocker, item, seal, bytes)
}

#[test]
fn pm_route_is_only_single_canonical_get_never_ordinary_list_upload_or_delete() {
    let task = Uuid::new_v4();
    let mut pm = actor(task);
    let id = Uuid::from_u128(0xabcdef);
    let path = format!("/api/v1/attachments/{id}/download");
    assert!(route(&pm, "GET", &path).unwrap());
    for method in ["HEAD", "POST", "PUT", "PATCH", "DELETE"] {
        assert!(route(&pm, method, &path).is_err());
    }
    for path in [
        format!("/api/v1/issues/{task}/attachments"),
        format!("/api/v1/attachments/{id}"),
        format!("{path}/"),
        format!(
            "/api/v1/attachments/{}/download",
            id.to_string().to_uppercase()
        ),
        "/api/v1/attachments/not-uuid/download".into(),
        "/api/v1/attachments/00000000-0000-0000-0000-000000000000/download".into(),
    ] {
        assert!(route(&pm, "GET", &path).is_err());
    }
    pm.scopes.remove("task-tracker:read");
    assert!(route(&pm, "GET", &path).is_err());
    pm = actor(task);
    pm.human_session = true;
    assert!(route(&pm, "GET", &path).is_err());
    pm = actor(task);
    pm.scopes.extend(actor(Uuid::new_v4()).scopes);
    assert!(route(&pm, "GET", &path).is_err());
    pm = actor(task);
    pm.scopes = ["task-tracker:read".into()].into();
    assert!(!route(&pm, "GET", &path).unwrap());
}

#[test]
fn all_six_saved_technical_roles_accept_only_exact_ref_hash_binding_run_and_raw_bytes() {
    for role in [
        DeliveryRole::Analyst,
        DeliveryRole::Architect,
        DeliveryRole::Developer,
        DeliveryRole::Reviewer,
        DeliveryRole::Tester,
        DeliveryRole::DevOps,
    ] {
        let (blocker, item, seal, bytes) = fixture(role);
        evidence(&blocker, &item, &seal).unwrap();
        crate::attachment_artifacts::verify_bytes(&item, &bytes).unwrap();
        assert!(crate::attachment_artifacts::verify_bytes(&item, b"{\"critical\":true}").is_err());
        for mismatch in 0..9 {
            let mut bad = blocker.clone();
            match mismatch {
                0 => bad.task_id = Uuid::new_v4(),
                1 => bad.proposal.evidence.clear(),
                2 => bad.proposal.evidence[0].receipt.sha256 = "f".repeat(64),
                3 => {
                    bad.proposal.evidence[0].receipt.reference =
                        format!("tracker-attachment:{}", Uuid::new_v4())
                }
                4 => bad.proposal.evidence[0].binding.execution_id = Uuid::new_v4(),
                5 => bad.proposal.fence.assignment_id = Uuid::new_v4(),
                6 => bad.run_id = Uuid::new_v4(),
                7 => bad.state = CriticalBlockerState::AwaitingPmReview,
                _ => {
                    bad.role = if role == DeliveryRole::Developer {
                        DeliveryRole::Reviewer
                    } else {
                        DeliveryRole::Developer
                    }
                }
            }
            assert!(evidence(&bad, &item, &seal).is_err());
        }
        let mut ordinary = item.clone();
        ordinary.storage_key = "ordinary-attachment".into();
        assert!(
            Seal::parse(ordinary.storage_key.as_ref())
                .unwrap()
                .is_none()
        );
        assert!(evidence(&blocker, &ordinary, &seal).is_err());
        let mut foreign = item.clone();
        foreign.issue_id = shared::IssueId::new();
        assert!(evidence(&blocker, &foreign, &seal).is_err());
    }
}
