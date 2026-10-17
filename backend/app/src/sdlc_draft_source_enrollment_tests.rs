//! Synthetic unit fixtures, not live task or runtime evidence.
use super::*;
use chrono::Utc;
use domain::sdlc::PmAssignment;
use serde_json::json;
use uuid::Uuid;

fn fixture() -> (
    TaskState,
    Principal,
    DraftSourceEnrollmentPreflight,
    EnrollDraftSource,
) {
    let task = Uuid::new_v4();
    let project = Uuid::new_v4();
    let owner = Principal {
        subject: "fixture-owner".into(),
        human_session: true,
        trusted_human: false,
        scopes: Default::default(),
    };
    let assignment = PmAssignment {
        assignment_id: Uuid::new_v4(),
        execution_id: Uuid::new_v4(),
        agent_id: Uuid::new_v4(),
        version: 1,
        machine_subject: "fixture-pm".into(),
    };
    let state: TaskState = serde_json::from_value(json!({
        "tracker_instance_id":"fixture-tracker","project_id":project,"task_id":task,"root_task_id":task,
        "owner_subject":owner.subject,"stage":"Draft","confirmation_revision":null,"assignment":assignment,
        "questions":[],"revisions":[],"confirmations":[],"evidence":[],"flow":"single-task-v1",
        "runtime_state":"running","waiting_reason":"pm_brainstorming"
    })).unwrap();
    let source = SourcePolicySpec {
        forge_instance_id: Uuid::new_v4(),
        repository_id: Uuid::new_v4(),
        base_ref: "refs/heads/main".into(),
        base_commit: "a".repeat(40),
    };
    let now = Utc::now();
    let verification = VerifiedSource {
        namespace: shared::resource_context::NamespaceRef {
            registry_instance_id: Uuid::new_v4(),
            namespace_id: Uuid::new_v4(),
        },
        forge_instance_id: source.forge_instance_id,
        repository_id: source.repository_id,
        public_name: "fixture/repo".into(),
        base_ref: source.base_ref.clone(),
        base_commit: source.base_commit.clone(),
        default_ref: source.base_ref.clone(),
        protected_refs: vec![],
        observed_at: now,
    };
    let mut policy = ProjectSourcePolicy {
        contract_version: 1,
        tracker_instance_id: state.tracker_instance_id.clone(),
        project_id: project,
        source_revision: 1,
        source_hash: String::new(),
        source,
        verification,
        author_subject: owner.subject.clone(),
        created_at: now,
    };
    policy.source_hash = crate::sdlc_source::sealed_hash(&policy).unwrap();
    let mut before = DraftSourceEnrollmentPreflight {
        task_id: task,
        project_id: project,
        state_sha256: crate::sdlc::canonical_hash(&state).unwrap(),
        selection_sha256: crate::sdlc::canonical_hash(&Option::<serde_json::Value>::None).unwrap(),
        policy,
        preflight_sha256: String::new(),
    };
    before.preflight_sha256 = preflight_hash(&before).unwrap();
    let command = EnrollDraftSource {
        expected_preflight_sha256: before.preflight_sha256.clone(),
        source_revision: before.policy.source_revision,
        policy_hash: before.policy.source_hash.clone(),
        idempotency_key: "fixture-enroll".into(),
    };
    (state, owner, before, command)
}

#[test]
fn active_pm_brainstorming_or_absent_runtime_does_not_block_metadata_selection() {
    let (state, owner, before, command) = fixture();
    let original = serde_json::to_value(&state).unwrap();
    authorize_draft(&state, &owner).unwrap();
    verify_cas(&before, &command).unwrap();
    assert_eq!(original, serde_json::to_value(&state).unwrap());
    let mut no_pm_yet = state;
    no_pm_yet.assignment = None;
    authorize_draft(&no_pm_yet, &owner).unwrap();
}

#[test]
fn only_exact_human_owner_unconfirmed_root_draft_is_eligible() {
    let (state, owner, _, _) = fixture();
    for field in ["human", "subject", "machine_grant"] {
        let mut actor = owner.clone();
        match field {
            "human" => actor.human_session = false,
            "subject" => actor.subject = Uuid::new_v4().to_string(),
            _ => {
                actor.scopes.insert("task-tracker:sdlc:pm:spoof".into());
            }
        }
        assert!(authorize_draft(&state, &actor).is_err(), "{field}");
    }
    for field in ["stage", "confirmation_revision", "root_task_id", "flow"] {
        let mut raw = serde_json::to_value(&state).unwrap();
        raw[field] = match field {
            "stage" => json!("Analysis"),
            "confirmation_revision" => json!(1),
            "root_task_id" => json!(Uuid::new_v4()),
            _ => json!("legacy"),
        };
        assert!(
            authorize_draft(&serde_json::from_value(raw).unwrap(), &owner).is_err(),
            "{field}"
        );
    }
}

#[test]
fn changed_task_state_policy_or_selection_invalidates_original_cas() {
    let (_, _, before, command) = fixture();
    for field in ["task_id", "state_sha256", "selection_sha256", "policy"] {
        let mut raw = serde_json::to_value(&before).unwrap();
        match field {
            "task_id" => raw[field] = json!(Uuid::new_v4()),
            "policy" => raw[field]["source_revision"] = json!(2),
            _ => raw[field] = json!("c".repeat(64)),
        }
        let mut changed: DraftSourceEnrollmentPreflight = serde_json::from_value(raw).unwrap();
        changed.preflight_sha256 = preflight_hash(&changed).unwrap();
        assert!(verify_cas(&changed, &command).is_err(), "{field}");
    }
}

#[test]
fn selection_event_is_metadata_only_and_not_a_git_or_dispatch_grant() {
    let (state, _, before, command) = fixture();
    let mut receipt = DraftSourceEnrollmentReceipt {
        enrollment_id: Uuid::new_v4(),
        task_id: state.task_id,
        project_id: state.project_id,
        owner_subject: state.owner_subject.clone(),
        source_revision: before.policy.source_revision,
        policy_hash: before.policy.source_hash,
        request_sha256: crate::sdlc::canonical_hash(&command).unwrap(),
        source_hash: String::new(),
        created_at: Utc::now(),
    };
    receipt.source_hash = crate::sdlc_source::sealed_hash(&receipt).unwrap();
    let event = domain::sdlc::OutboxEvent {
        sequence: 1,
        event_id: receipt.enrollment_id,
        task_id: state.task_id,
        event_type: "source.selected".into(),
        created_at: receipt.created_at,
        payload: json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
            "project_id":state.project_id,"task_id":state.task_id,"root_task_id":state.task_id,
            "owner_subject":state.owner_subject,"stage":"Draft","requirement_revision":null,"result":receipt}),
    };
    let projected = crate::sdlc_metadata::project(
        &event,
        crate::sdlc_metadata::MetadataReferences {
            input: None,
            question: None,
            requirement_hash: None,
        },
    )
    .unwrap();
    let projected = serde_json::to_value(projected).unwrap();
    assert_eq!(projected["event_type"], "source.selected");
    for field in [
        "candidate_ref",
        "assignment_id",
        "execution_id",
        "run_id",
        "fence",
        "credential",
        "dispatch_allowed",
    ] {
        assert!(
            projected["payload"]["resource"].get(field).is_none(),
            "{field}"
        );
    }
    assert!(matches!(
        crate::sdlc_source::readback(None, None).unwrap(),
        TaskSourceReadback::Blocked {
            reason: SourceBlockReason::MissingBinding,
            ..
        }
    ));
}

#[test]
fn pm_machine_cannot_enroll_or_obtain_source_write_authority() {
    let (state, _, _, _) = fixture();
    let assignment = state.assignment.as_ref().unwrap();
    let actor = Principal {
        subject: assignment.machine_subject.clone(),
        human_session: false,
        trusted_human: true,
        scopes: [
            "task-tracker:read".into(),
            "task-tracker:write".into(),
            assignment.scope(state.task_id),
        ]
        .into_iter()
        .collect(),
    };
    assert!(authorize_draft(&state, &actor).is_err());
    assert!(crate::sdlc_lifecycle::producer(&actor, true).is_err());
}
