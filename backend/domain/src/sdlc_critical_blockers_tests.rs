use super::*;
use crate::attachment_artifact::ArtifactBinding;
use crate::sdlc_lifecycle::ArtifactReceipt;
use serde_json::json;

fn proposal() -> SubmitCriticalBlocker {
    let assignment_id = Uuid::new_v4();
    let execution_id = Uuid::new_v4();
    SubmitCriticalBlocker {
        fence: DeliveryFence {
            assignment_id,
            execution_id,
            fencing_token: 1,
            lease_id: Uuid::new_v4(),
            expected_lease_version: 1,
        },
        requirement_revision: 1,
        content_hash: "a".repeat(64),
        kind: CriticalBlockerKind::RequirementsContradiction,
        question: "Which of the contradictory retention requirements is authoritative?".into(),
        rationale: "Permanent deletion and indefinite retention cannot both apply.".into(),
        options: vec![
            CriticalBlockerOption {
                option_id: Uuid::new_v4(),
                label: "Delete".into(),
                consequences: "Cannot restore deleted records.".into(),
            },
            CriticalBlockerOption {
                option_id: Uuid::new_v4(),
                label: "Retain".into(),
                consequences: "Requires a retention exception.".into(),
            },
        ],
        recommendation: None,
        evidence: vec![AttachmentArtifact {
            receipt: ArtifactReceipt {
                reference: format!("tracker-attachment:{}", Uuid::new_v4()),
                sha256: "b".repeat(64),
            },
            binding: ArtifactBinding {
                assignment_id,
                execution_id,
                role: DeliveryRole::Analyst,
            },
        }],
        idempotency_key: "synthetic-blocker".into(),
    }
}

#[test]
fn structured_options_are_bounded_stable_and_never_preselected() {
    let p = proposal();
    p.validate(DeliveryRole::Analyst).unwrap();
    assert_eq!(p.recommendation, None);
    let mut recommended = p.clone();
    recommended.recommendation = Some(p.options[0].option_id);
    recommended.validate(DeliveryRole::Analyst).unwrap();
    let mut bad = p.clone();
    bad.recommendation = Some(Uuid::new_v4());
    assert!(bad.validate(DeliveryRole::Analyst).is_err());
    bad = p.clone();
    bad.options[1].option_id = bad.options[0].option_id;
    assert!(bad.validate(DeliveryRole::Analyst).is_err());
    bad = p.clone();
    bad.options[0].option_id = Uuid::nil();
    assert!(bad.validate(DeliveryRole::Analyst).is_err());
    for count in [0, 1, MAX_OPTIONS] {
        bad = p.clone();
        bad.options = (0..count)
            .map(|_| CriticalBlockerOption {
                option_id: Uuid::new_v4(),
                ..p.options[0].clone()
            })
            .collect();
        bad.validate(DeliveryRole::Analyst).unwrap();
    }
    bad.options.push(CriticalBlockerOption {
        option_id: Uuid::new_v4(),
        ..p.options[0].clone()
    });
    assert!(bad.validate(DeliveryRole::Analyst).is_err());
    bad.options.clear();
    bad.recommendation = Some(p.options[0].option_id);
    assert!(bad.validate(DeliveryRole::Analyst).is_err());
    let mut omitted = serde_json::to_value(&p).unwrap();
    omitted.as_object_mut().unwrap().remove("options");
    let omitted: SubmitCriticalBlocker = serde_json::from_value(omitted).unwrap();
    assert!(omitted.options.is_empty());
    omitted.validate(DeliveryRole::Analyst).unwrap();
}

#[test]
fn evidence_requires_own_sealed_artifact_not_workflow_terminal_or_job_url() {
    let p = proposal();
    for reference in [
        "workflow-terminal:123",
        "https://forge.invalid/jobs/123",
        "tracker-attachment:00000000-0000-0000-0000-000000000000",
        "tracker-attachment:AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
    ] {
        let mut bad = p.clone();
        bad.evidence[0].receipt.reference = reference.into();
        assert!(bad.validate(DeliveryRole::Analyst).is_err());
    }
    for change in 0..4 {
        let mut bad = p.clone();
        match change {
            0 => bad.evidence[0].binding.assignment_id = Uuid::new_v4(),
            1 => bad.evidence[0].binding.execution_id = Uuid::new_v4(),
            2 => bad.evidence[0].binding.role = DeliveryRole::DevOps,
            _ => bad.evidence[0].receipt.sha256 = "A".repeat(64),
        }
        assert!(bad.validate(DeliveryRole::Analyst).is_err());
    }
    for count in [0, 2, MAX_EVIDENCE + 1] {
        let mut bad = p.clone();
        bad.evidence = vec![p.evidence[0].clone(); count];
        assert!(bad.validate(DeliveryRole::Analyst).is_err());
    }
}

#[test]
fn text_fence_key_and_revision_bounds_apply_to_typed_callers_too() {
    let p = proposal();
    for change in 0..12 {
        let mut bad = p.clone();
        match change {
            0 => bad.question = " ".into(),
            1 => bad.question = "x".repeat(4097),
            2 => bad.rationale.push('\0'),
            3 => bad.rationale = "x".repeat(8193),
            4 => bad.options[0].label = "x".repeat(257),
            5 => bad.options[0].consequences = " ".into(),
            6 => bad.fence.assignment_id = Uuid::nil(),
            7 => bad.fence.expected_lease_version = 0,
            8 => bad.fence.fencing_token = crate::sdlc::MAX_SAFE_VERSION + 1,
            9 => bad.idempotency_key = "key with spaces".into(),
            10 => bad.requirement_revision = 0,
            _ => bad.content_hash = "not-a-sha".into(),
        }
        assert!(
            bad.validate(DeliveryRole::Analyst).is_err(),
            "case {change}"
        );
    }
    for key in [".", ".."] {
        let mut bad = p.clone();
        bad.idempotency_key = key.into();
        assert!(bad.validate(DeliveryRole::Analyst).is_err());
    }
}

#[test]
fn only_business_kinds_are_public_and_no_actor_or_owner_claim_is_accepted() {
    let wire = serde_json::to_value(proposal()).unwrap();
    for kind in ["requirements_contradiction", "missing_business_decision"] {
        let mut value = wire.clone();
        value["kind"] = json!(kind);
        assert!(serde_json::from_value::<SubmitCriticalBlocker>(value).is_ok());
    }
    for kind in ["infra", "missing_access", "rework", "ordinary_engineering"] {
        let mut value = wire.clone();
        value["kind"] = json!(kind);
        assert!(serde_json::from_value::<SubmitCriticalBlocker>(value).is_err());
    }
    for field in [
        "actor",
        "owner_subject",
        "agent_id",
        "terminal",
        "verified",
        "selected_option_id",
    ] {
        let mut value = wire.clone();
        value[field] = json!(true);
        assert!(serde_json::from_value::<SubmitCriticalBlocker>(value).is_err());
    }
    for revision in [
        json!(0),
        json!(-1),
        json!(9007199254740992i64),
        json!(1.0),
        json!("1"),
    ] {
        let mut value = wire.clone();
        value["requirement_revision"] = revision;
        assert!(serde_json::from_value::<SubmitCriticalBlocker>(value).is_err());
    }
}

#[test]
fn scheduler_contract_rejects_stop_ack_unknown_and_fake_proof_fields() {
    let p = proposal();
    let request = CriticalPmReviewGuard {
        blocker_id: Uuid::new_v4(),
        blocker_sha256: "c".repeat(64),
        expected_blocker_version: 1,
        expected_task_version: 1,
        fence: p.fence,
        idempotency_key: "hold".into(),
    };
    let command = ReturnCriticalToPm {
        review: request,
        terminal: RuntimeTerminalObservation {
            assignment_hash: "d".repeat(64),
            chat_id: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
            state: crate::sdlc_lifecycle::RuntimeTerminalState::Cancelled,
            receipt: ArtifactReceipt {
                reference: "synthetic-native-terminal".into(),
                sha256: "e".repeat(64),
            },
        },
    };
    let wire = serde_json::to_value(command).unwrap();
    for state in ["unknown", "running", "stop_ack", "eof", "expired"] {
        let mut value = wire.clone();
        value["terminal"]["state"] = json!(state);
        assert!(serde_json::from_value::<ReturnCriticalToPm>(value).is_err());
    }
    for field in ["verified", "owner_confirmed", "workflow_complete"] {
        let mut value = wire.clone();
        value[field] = json!(true);
        assert!(serde_json::from_value::<ReturnCriticalToPm>(value).is_err());
    }
}
