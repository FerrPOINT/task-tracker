use super::*;
use serde_json::json;

fn actor(subject: &str, human: bool, scopes: Vec<String>) -> Principal {
    Principal {
        subject: subject.into(),
        human_session: human,
        trusted_human: false,
        scopes: scopes.into_iter().collect(),
    }
}

#[test]
fn non_root_cannot_use_pm_lifecycle_or_issue_confirmation_permission() {
    let (mut state, config, owner, pm, _, _) = fixture();
    state.root_task_id = Uuid::new_v4();
    let before = serde_json::to_value(&state).unwrap();
    let command = SdlcCommand::Confirm {
        revision: 1,
        command: ConfirmCommand {
            content_hash: "a".repeat(64),
            idempotency_key: "child-confirm".into(),
            expected_routing_policy_version: None,
        },
    };
    assert!(matches!(state.require_root(), Err(AppError::Conflict(_))));
    assert!(matches!(
        authorize_pm_read(&state, &owner),
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        authorize_pm_read(&state, &pm),
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        apply(&mut state, &owner, &config, &command),
        Err(AppError::Conflict(_))
    ));
    assert!(!state.context(&owner).permissions.can_confirm);
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
    state.root_task_id = state.task_id;
    assert!(state.require_root().is_ok());
    state.task_id = Uuid::nil();
    state.root_task_id = Uuid::nil();
    assert!(state.require_root().is_err());
}

#[test]
fn draft_validation_keeps_exact_content_and_checks_character_and_byte_limits() {
    let human = actor("central-human", true, vec![]);
    let mut command = CreateDraftCommand {
        flow: None,
        title: "\u{43f}".repeat(500),
        description: "x".repeat(100_000),
        idempotency_key: "\u{43a}".repeat(64),
    };
    assert!(validate_draft(&human, &command).is_ok());
    assert_eq!(command.title.chars().count(), 500);
    command.title.push('x');
    assert!(validate_draft(&human, &command).is_err());
    command.title = " Exact title ".into();
    command.description.push('x');
    assert!(validate_draft(&human, &command).is_err());
    command.description.clear();
    command.idempotency_key.push('x');
    assert!(validate_draft(&human, &command).is_err());
    command.idempotency_key = "exact-key".into();
    assert!(validate_draft(&human, &command).is_ok());
    assert_eq!(command.title, " Exact title ");
    assert!(validate_draft(&actor("central-human", false, vec![]), &command).is_err());
    assert!(validate_draft(&actor("", true, vec![]), &command).is_err());
    command.description = "nul\0".into();
    assert!(validate_draft(&human, &command).is_err());
}

#[test]
fn new_pm_feedback_only_accepts_draft_dialogue_and_preserves_legacy_state() {
    use crate::sdlc_lifecycle::validate_new_pm_feedback;
    use domain::sdlc_lifecycle::LifecycleStatus;

    let (mut state, config, _, pm, _, fence) = fixture();
    assert!(validate_new_pm_feedback(&state).is_ok());
    state.stage = Stage::Clarification;
    let legacy = serde_json::to_value(&state).unwrap();
    assert!(validate_new_pm_feedback(&state).is_ok());
    assert_eq!(serde_json::to_value(&state).unwrap(), legacy);

    state.stage = Stage::Draft;
    state
        .retained_fields
        .insert("flow".into(), json!("single-task-v1"));
    for mode in [
        DraftSubmode::Brainstorming,
        DraftSubmode::AwaitingConfirmation,
    ] {
        state.draft_submode = Some(mode);
        for status in [LifecycleStatus::Ready, LifecycleStatus::InProgress] {
            state.status = Some(status);
            let before = serde_json::to_value(&state).unwrap();
            assert!(validate_new_pm_feedback(&state).is_ok());
            assert_eq!(serde_json::to_value(&state).unwrap(), before);
        }
    }
    for status in [None, Some(LifecycleStatus::Completed)] {
        state.status = status;
        assert!(validate_new_pm_feedback(&state).is_err());
    }
    state.status = Some(LifecycleStatus::Ready);
    for mode in [None, Some(DraftSubmode::Clarification)] {
        state.draft_submode = mode;
        assert!(validate_new_pm_feedback(&state).is_err());
    }
    state.draft_submode = Some(DraftSubmode::Brainstorming);
    for stage in [
        Stage::Clarification,
        Stage::Backlog,
        Stage::Analysis,
        Stage::Architecture,
        Stage::Development,
        Stage::Review,
        Stage::Testing,
        Stage::Rework,
        Stage::Deployment,
    ] {
        state.stage = stage;
        assert!(validate_new_pm_feedback(&state).is_err());
    }
    state.stage = Stage::Draft;
    publish(&mut state, &config, &pm, &fence);
    assert_eq!(
        state.draft_submode,
        Some(DraftSubmode::AwaitingConfirmation)
    );
    assert!(validate_new_pm_feedback(&state).is_ok());
    let mut question = question(&fence, QuestionMode::Single);
    question.required = false;
    apply(
        &mut state,
        &pm,
        &config,
        &SdlcCommand::PublishQuestion(question),
    )
    .unwrap();
    assert!(validate_new_pm_feedback(&state).is_err());
    state.draft_submode = Some(DraftSubmode::AwaitingConfirmation);
    // Even an optional open question blocks free feedback in an inconsistent mode.
    assert!(validate_new_pm_feedback(&state).is_err());
    state.retained_fields.remove("flow");
    state.stage = Stage::Clarification;
    state.status = None;
    state.draft_submode = None;
    assert!(validate_new_pm_feedback(&state).is_err());
    state.questions[0].state = QuestionState::Answered;
    assert!(validate_new_pm_feedback(&state).is_ok());
    state.draft_submode = Some(DraftSubmode::Clarification);
    assert!(validate_new_pm_feedback(&state).is_err());
    state.draft_submode = None;
    state.root_task_id = Uuid::new_v4();
    assert!(validate_new_pm_feedback(&state).is_err());
    state.root_task_id = state.task_id;
    state.retained_fields.insert("flow".into(), json!(null));
    assert!(validate_new_pm_feedback(&state).is_err());
}

#[test]
fn draft_flow_omission_preserves_hash_and_marker_survives_pm_transitions() {
    let legacy = json!({"title":"Draft","description":"Input","idempotency_key":"create"});
    let command: CreateDraftCommand = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(command.flow, None);
    assert_eq!(serde_json::to_value(&command).unwrap(), legacy);
    let hash = canonical_hash(&json!({"operation":"create_draft","payload":command})).unwrap();
    assert_eq!(
        hash,
        canonical_hash(&json!({"operation":"create_draft","payload":legacy})).unwrap()
    );
    for value in [
        json!(null),
        json!(false),
        json!(true),
        json!("single-task-v2"),
        json!({}),
    ] {
        let mut bad = legacy.clone();
        bad["flow"] = value;
        assert!(serde_json::from_value::<CreateDraftCommand>(bad).is_err());
    }
    let mut opted = legacy;
    opted["flow"] = json!("single-task-v1");
    let command: CreateDraftCommand = serde_json::from_value(opted.clone()).unwrap();
    assert_eq!(command.flow, Some(DraftFlow::SingleTaskV1));
    assert_eq!(serde_json::to_value(&command).unwrap(), opted);
    assert_ne!(
        hash,
        canonical_hash(&json!({"operation":"create_draft","payload":command})).unwrap()
    );
    let (mut state, config, owner, pm, verifier, fence) = fixture();
    state
        .retained_fields
        .insert("flow".into(), json!("single-task-v1"));
    publish(&mut state, &config, &pm, &fence);
    for check in ["review", "contract"] {
        record_evidence(&mut state, &config, &verifier, &fence, check, check);
    }
    let hash = state.revisions.last().unwrap().content_hash.clone();
    apply(
        &mut state,
        &owner,
        &config,
        &SdlcCommand::Confirm {
            revision: 1,
            command: ConfirmCommand {
                content_hash: hash,
                idempotency_key: "confirm-flow".into(),
                expected_routing_policy_version: None,
            },
        },
    )
    .unwrap();
    let state: TaskState = serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
    assert_eq!(state.stage, Stage::Analysis);
    assert_eq!(state.draft_flow().unwrap(), Some(DraftFlow::SingleTaskV1));
    assert!(
        serde_json::to_value(state.context(&owner))
            .unwrap()
            .get("flow")
            .is_none()
    );
    let mut invalid = state.clone();
    invalid.retained_fields.insert("flow".into(), json!(null));
    assert!(invalid.draft_flow().is_err());
    invalid.retained_fields.remove("flow");
    assert_eq!(invalid.draft_flow().unwrap(), None);
}

fn fixture() -> (
    TaskState,
    SdlcConfig,
    Principal,
    Principal,
    Principal,
    MachineFence,
) {
    let assignment = PmAssignment {
        assignment_id: Uuid::new_v4(),
        execution_id: Uuid::new_v4(),
        agent_id: Uuid::new_v4(),
        version: 1,
        machine_subject: "pm".into(),
    };
    let task_id = Uuid::new_v4();
    let pm = actor("pm", false, vec![assignment.scope(task_id)]);
    let fence = MachineFence {
        assignment_id: assignment.assignment_id,
        execution_id: assignment.execution_id,
        agent_id: assignment.agent_id,
        assignment_version: 1,
    };
    let state = TaskState {
        status: None,
        retained_fields: Default::default(),
        draft_submode: None,
        tracker_instance_id: "test-instance".into(),
        project_id: Uuid::new_v4(),
        task_id,
        root_task_id: task_id,
        owner_subject: "owner".into(),
        stage: Stage::Draft,
        confirmation_revision: None,
        assignment: Some(assignment),
        questions: vec![],
        revisions: vec![],
        evidence: vec![],
        confirmations: vec![],
    };
    let config = SdlcConfig {
        single_task_project_ids: std::collections::HashSet::new(),
        reservation_scheduler_subject: "scheduler".into(),
        instance_id: "test-instance".into(),
        orchestrator_subject: "fleet".into(),
        verifier_subject: "verifier".into(),
    };
    let verifier = actor(
        "verifier",
        false,
        vec![format!("task-tracker:sdlc:evidence:{task_id}:1")],
    );
    (
        state,
        config,
        actor("owner", true, vec![]),
        pm,
        verifier,
        fence,
    )
}

fn document() -> RequirementsDocument {
    RequirementsDocument {
        goal: "Goal".into(),
        scope: vec!["Scope".into()],
        exclusions: vec![],
        scenarios: vec!["Scenario".into()],
        acceptance_criteria: vec!["Acceptance".into()],
        constraints: vec![],
        dependencies: vec![],
        assumptions: vec![],
        checklist: vec!["review".into()],
        prerequisites: vec!["contract".into()],
    }
}

#[test]
fn sdlc_confirmation_read_uses_exact_saved_owner_history_without_current_stage_rewrite() {
    use crate::sdlc_confirmation::exact_receipt;
    let (mut state, config, owner, pm, verifier, fence) = fixture();
    publish(&mut state, &config, &pm, &fence);
    for check in ["review", "contract"] {
        record_evidence(&mut state, &config, &verifier, &fence, check, check);
    }
    let hash = state.revisions[0].content_hash.clone();
    let result = apply(
        &mut state,
        &owner,
        &config,
        &SdlcCommand::Confirm {
            revision: 1,
            command: ConfirmCommand {
                content_hash: hash.clone(),
                idempotency_key: "direct-confirmation".into(),
                expected_routing_policy_version: None,
            },
        },
    )
    .unwrap();
    let receipt: Confirmation = serde_json::from_value(result.clone()).unwrap();
    assert_eq!(state.stage, Stage::Analysis);
    let before = canonical_hash(&state).unwrap();
    assert_eq!(
        serde_json::to_value(exact_receipt(&state, 1, Some(&receipt), Some(&hash)).unwrap())
            .unwrap(),
        result
    );
    assert_eq!(canonical_hash(&state).unwrap(), before);
    assert!(matches!(
        exact_receipt(&state, 2, None, None),
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        exact_receipt(&state, 1, None, Some(&hash)),
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        exact_receipt(&state, 1, Some(&receipt), None),
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        exact_receipt(&state, 1, Some(&receipt), Some("wrong-hash")),
        Err(AppError::Conflict(_))
    ));
    let mut inconsistent_history = state.clone();
    inconsistent_history.confirmations[0].created_at += chrono::Duration::seconds(1);
    assert!(matches!(
        exact_receipt(&inconsistent_history, 1, Some(&receipt), Some(&hash)),
        Err(AppError::Conflict(_))
    ));
    let mut duplicate = state.clone();
    duplicate.confirmations.push(receipt.clone());
    assert!(matches!(
        exact_receipt(&duplicate, 1, Some(&receipt), Some(&hash)),
        Err(AppError::Conflict(_))
    ));
    for field in ["owner", "task", "stage", "hash"] {
        let mut forged = receipt.clone();
        match field {
            "owner" => forged.owner_subject = "foreign".into(),
            "task" => forged.task_id = Uuid::new_v4(),
            "stage" => forged.stage = Stage::Analysis,
            _ => forged.content_hash = "b".repeat(64),
        }
        let mut forged_state = state.clone();
        forged_state.confirmations[0] = forged.clone();
        assert!(matches!(
            exact_receipt(&forged_state, 1, Some(&forged), Some(&hash)),
            Err(AppError::Conflict(_))
        ));
    }
    let mut later_revision = state.revisions[0].clone();
    later_revision.revision = 2;
    state.revisions.push(later_revision);
    assert_eq!(
        exact_receipt(&state, 1, Some(&receipt), Some(&hash)).unwrap(),
        receipt
    );
}

#[test]
fn sdlc_confirmation_read_grant_does_not_grant_owner_confirmation_or_foreign_task_access() {
    use crate::sdlc_confirmation::authorize_read;
    let (state, config, owner, mut pm, _, _) = fixture();
    assert!(matches!(
        authorize_read(&state, &pm),
        Err(AppError::Forbidden)
    ));
    pm.scopes.insert("task-tracker:read".into());
    authorize_read(&state, &pm).unwrap();
    authorize_read(&state, &owner).unwrap();
    let mut foreign = pm.clone();
    foreign.subject = "foreign".into();
    assert!(matches!(
        authorize_read(&state, &foreign),
        Err(AppError::Forbidden)
    ));
    let mut foreign_state = state.clone();
    foreign_state.task_id = Uuid::new_v4();
    foreign_state.root_task_id = foreign_state.task_id;
    assert!(matches!(
        authorize_read(&foreign_state, &pm),
        Err(AppError::Forbidden)
    ));
    let mut stale = state.clone();
    stale.assignment.as_mut().unwrap().version += 1;
    assert!(matches!(
        authorize_read(&stale, &pm),
        Err(AppError::Forbidden)
    ));
    let mut child = state.clone();
    child.root_task_id = Uuid::new_v4();
    assert!(authorize_read(&child, &pm).is_err());
    let command = SdlcCommand::Confirm {
        revision: 1,
        command: ConfirmCommand {
            content_hash: "a".repeat(64),
            idempotency_key: "forbidden".into(),
            expected_routing_policy_version: None,
        },
    };
    assert!(matches!(
        authorize(&state, &pm, &config, &command),
        Err(AppError::Forbidden)
    ));
}

#[test]
fn sdlc_confirmation_read_allowlist_adds_only_exact_get_with_safe_revision() {
    use crate::sdlc_confirmation::{is_confirmation_read, validate_revision};
    for revision in ["1", "9007199254740991"] {
        assert!(is_confirmation_read(
            "GET",
            &["requirements", revision, "confirmation"]
        ));
    }
    for revision in ["0", "01", "+1", "-1", "1.0", "9007199254740992"] {
        assert!(!is_confirmation_read(
            "GET",
            &["requirements", revision, "confirmation"]
        ));
    }
    for method in ["POST", "HEAD", "PATCH", "DELETE"] {
        assert!(!is_confirmation_read(
            method,
            &["requirements", "1", "confirmation"]
        ));
    }
    for resource in [
        vec!["analysis-intent"],
        vec!["events"],
        vec!["requirements", "1", "confirm"],
        vec!["requirements", "1", "confirmation", "extra"],
    ] {
        assert!(!is_confirmation_read("GET", &resource));
    }
    for revision in [0, -1, MAX_SAFE_VERSION + 1] {
        assert!(validate_revision(revision).is_err());
    }
}

#[test]
fn canonical_status_and_extensions_survive_aggregate_roundtrip_without_legacy_defaults() {
    use domain::sdlc_lifecycle::LifecycleStatus;
    let (state, _, owner, _, _, _) = fixture();
    let legacy = serde_json::to_value(&state).unwrap();
    let legacy_context = serde_json::to_value(state.context(&owner)).unwrap();
    for wire in [&legacy, &legacy_context] {
        assert!(wire.get("status").is_none());
        assert!(wire.get("draft_submode").is_none());
    }
    let mut canonical = legacy.clone();
    canonical["status"] = json!("InProgress");
    canonical["draft_submode"] = json!("brainstorming");
    canonical["runtime_state"] = json!("running");
    canonical["waiting_reason"] = serde_json::Value::Null;
    canonical["lifecycle"] = json!({"preserved":"execution-cursor"});
    let decoded: TaskState = serde_json::from_value(canonical.clone()).unwrap();
    assert_eq!(decoded.status, Some(LifecycleStatus::InProgress));
    assert_eq!(serde_json::to_value(&decoded).unwrap(), canonical);
    let context = serde_json::to_value(decoded.context(&owner)).unwrap();
    assert_eq!(context["status"], "InProgress");
    assert_eq!(context["draft_submode"], "brainstorming");
    assert!(context["waiting_reason"].is_null());
}

#[test]
fn pm_waiting_does_not_become_a_stage_or_runtime_status() {
    use domain::sdlc_lifecycle::LifecycleStatus;
    let (mut state, config, owner, pm, _, fence) = fixture();
    state.status = Some(LifecycleStatus::InProgress);
    state.draft_submode = Some(DraftSubmode::Brainstorming);
    publish(&mut state, &config, &pm, &fence);
    assert_eq!(state.stage, Stage::Draft);
    assert_eq!(state.status, Some(LifecycleStatus::Ready));
    assert_eq!(
        state.draft_submode,
        Some(DraftSubmode::AwaitingConfirmation)
    );
    let q = question(&fence, QuestionMode::Single);
    apply(
        &mut state,
        &pm,
        &config,
        &SdlcCommand::PublishQuestion(q.clone()),
    )
    .unwrap();
    assert_eq!(state.stage, Stage::Draft);
    assert_eq!(
        state.context(&owner).waiting_reason.as_deref(),
        Some("waiting_for_owner_answers")
    );
    apply(
        &mut state,
        &owner,
        &config,
        &SdlcCommand::Answer {
            question_id: q.question_id,
            command: AnswerCommand {
                expected_question_version: 1,
                requirement_revision: 1,
                selected_option_ids: vec![q.options[0].id],
                text: None,
                comment: None,
                idempotency_key: "resume-pm".into(),
            },
        },
    )
    .unwrap();
    assert_eq!(state.stage, Stage::Draft);
    assert_eq!(state.status, Some(LifecycleStatus::InProgress));
    assert_eq!(
        state.context(&owner).waiting_reason.as_deref(),
        Some("waiting_for_requirements_or_evidence")
    );
    assert!(!state.context(&owner).permissions.can_confirm);
}

#[test]
fn pm_feedback_resume_uses_server_clock_exact_binding_and_never_reopens_published_analysis() {
    use domain::sdlc_lifecycle::*;
    let (mut state, _, _, _, _, fence) = fixture();
    let observed = chrono::Utc::now();
    let feedback = PmFeedbackReceipt {
        message_id: Uuid::new_v4(),
        tracker_instance_id: state.tracker_instance_id.clone(),
        project_id: state.project_id,
        task_id: state.task_id,
        root_task_id: state.root_task_id,
        owner_subject: state.owner_subject.clone(),
        fence: fence.clone(),
        text: "Additional thought".into(),
        content_hash: canonical_hash(&json!({"text":"Additional thought"})).unwrap(),
        requirement_revision: None,
        created_at: observed,
    };
    let checkpoint = PmFeedbackCheckpoint {
        fence: fence.clone(),
        checkpoint_id: Uuid::new_v4(),
        checkpoint_version: 1,
        previous_run_id: Uuid::new_v4(),
        previous_run_terminal: ArtifactReceipt {
            reference: "fixture://terminal".into(),
            sha256: "a".repeat(64),
        },
    };
    let mut lease = domain::sdlc_execution_lease::ExecutionLeaseReadback {
        contract_version: 1,
        binding: domain::sdlc_pm_draft::PmDraftBinding {
            tracker_instance_id: state.tracker_instance_id.clone(),
            project_id: state.project_id,
            task_id: state.task_id,
            root_task_id: state.root_task_id,
            owner_subject: state.owner_subject.clone(),
        },
        owner_version: 1,
        fence: fence.clone(),
        observed_at: observed,
        state: domain::sdlc_execution_lease::ExecutionLeaseState::Active,
        current: Some(domain::sdlc_execution_lease::ExecutionLease {
            lease_id: Uuid::new_v4(),
            version: 2,
            holder_subject: "pm".into(),
            claimed_at: observed,
            heartbeat_at: observed,
            expires_at: observed + chrono::Duration::seconds(30),
        }),
        operation: None,
        dispatch_allowed: false,
    };
    let resume =
        crate::sdlc_lifecycle::pm_feedback_resume_contract(&state, &feedback, &checkpoint, &lease)
            .unwrap();
    assert_eq!(resume.fence, checkpoint.fence);
    assert_eq!(
        resume.resume_operation_key,
        format!("pm-feedback:{}", feedback.message_id)
    );
    lease.binding.project_id = Uuid::new_v4();
    assert!(
        crate::sdlc_lifecycle::pm_feedback_resume_contract(&state, &feedback, &checkpoint, &lease)
            .is_err()
    );
    lease.binding.project_id = state.project_id;
    lease.observed_at += chrono::Duration::seconds(30);
    assert!(
        crate::sdlc_lifecycle::pm_feedback_resume_contract(&state, &feedback, &checkpoint, &lease)
            .is_err()
    );
    lease.observed_at = observed;
    state.stage = Stage::Analysis;
    assert!(
        crate::sdlc_lifecycle::pm_feedback_resume_contract(&state, &feedback, &checkpoint, &lease)
            .is_err()
    );
    assert!(state.questions.is_empty());
    assert!(state.confirmations.is_empty());
}

fn publish(state: &mut TaskState, config: &SdlcConfig, pm: &Principal, fence: &MachineFence) {
    apply(
        state,
        pm,
        config,
        &SdlcCommand::PublishRevision(PublishRevision {
            fence: fence.clone(),
            expected_requirement_revision: state.current_revision(),
            document: document(),
            idempotency_key: Uuid::new_v4().to_string(),
        }),
    )
    .unwrap();
}

fn record_evidence(
    state: &mut TaskState,
    config: &SdlcConfig,
    verifier: &Principal,
    fence: &MachineFence,
    check: &str,
    key: &str,
) {
    let revision = state.revisions.last().unwrap();
    let revision_number = revision.revision;
    let content_hash = revision.content_hash.clone();
    apply(
        state,
        verifier,
        config,
        &SdlcCommand::Evidence(EvidenceCommand {
            fence: fence.clone(),
            requirement_revision: revision_number,
            content_hash,
            check_id: check.into(),
            evidence_reference: format!("workflow://verified/{key}"),
            idempotency_key: key.into(),
        }),
    )
    .unwrap();
}

fn question(fence: &MachineFence, mode: QuestionMode) -> PublishQuestion {
    PublishQuestion {
        fence: fence.clone(),
        request_id: Uuid::new_v4(),
        question_id: Uuid::new_v4(),
        expected_question_version: None,
        requirement_revision: 1,
        checkpoint_id: Uuid::new_v4(),
        requirement_reference: Some("scope".into()),
        text: "Choose".into(),
        rationale: "Clarify scope".into(),
        required: true,
        mode,
        options: vec![
            QuestionOption {
                id: Uuid::new_v4(),
                label: "Standard".into(),
                consequences: "Known path".into(),
                is_custom: false,
            },
            QuestionOption {
                id: Uuid::new_v4(),
                label: "Custom".into(),
                consequences: "Custom path".into(),
                is_custom: true,
            },
        ],
        recommended_option_id: None,
        idempotency_key: "question".into(),
    }
}

#[test]
fn answered_clarification_requires_republication_even_with_fresh_evidence() {
    let (mut state, config, owner, pm, verifier, fence) = fixture();
    publish(&mut state, &config, &pm, &fence);
    let input = question(&fence, QuestionMode::Single);
    apply(
        &mut state,
        &pm,
        &config,
        &SdlcCommand::PublishQuestion(input.clone()),
    )
    .unwrap();
    for check in ["review", "contract"] {
        record_evidence(
            &mut state,
            &config,
            &verifier,
            &fence,
            check,
            &format!("pre-answer-{check}"),
        );
    }
    apply(
        &mut state,
        &owner,
        &config,
        &SdlcCommand::Answer {
            question_id: input.question_id,
            command: AnswerCommand {
                expected_question_version: 1,
                requirement_revision: 1,
                selected_option_ids: vec![input.options[0].id],
                text: None,
                comment: None,
                idempotency_key: "answer-before-ready".into(),
            },
        },
    )
    .unwrap();
    assert!(!state.ready());
    for check in ["review", "contract"] {
        record_evidence(
            &mut state,
            &config,
            &verifier,
            &fence,
            check,
            &format!("post-answer-{check}"),
        );
    }
    assert!(!state.context(&owner).permissions.can_confirm);
    let old_confirmation = SdlcCommand::Confirm {
        revision: 1,
        command: ConfirmCommand {
            content_hash: state.revisions[0].content_hash.clone(),
            idempotency_key: "old-revision-after-post-answer-evidence".into(),
            expected_routing_policy_version: None,
        },
    };
    assert!(matches!(
        apply(&mut state, &owner, &config, &old_confirmation),
        Err(AppError::Conflict(_))
    ));
    assert!(state.confirmations.is_empty());
    assert_eq!(state.stage, Stage::Clarification);

    publish(&mut state, &config, &pm, &fence);
    assert_eq!(state.current_revision(), Some(2));
    assert!(!state.ready());
    for check in ["review", "contract"] {
        record_evidence(
            &mut state,
            &config,
            &verifier,
            &fence,
            check,
            &format!("republished-{check}"),
        );
    }
    assert!(state.context(&owner).permissions.can_confirm);
    let revision = state.revisions.last().unwrap();
    let revision_number = revision.revision;
    let content_hash = revision.content_hash.clone();
    apply(
        &mut state,
        &owner,
        &config,
        &SdlcCommand::Confirm {
            revision: revision_number,
            command: ConfirmCommand {
                content_hash,
                idempotency_key: "confirm-after-post-answer-evidence".into(),
                expected_routing_policy_version: None,
            },
        },
    )
    .unwrap();
}

#[test]
fn modes_custom_unknown_duplicate_and_recommendation() {
    let (mut state, config, _, pm, _, fence) = fixture();
    publish(&mut state, &config, &pm, &fence);
    let mut input = question(&fence, QuestionMode::Single);
    input.recommended_option_id = Some(input.options[0].id);
    let q: Question = serde_json::from_value(
        apply(
            &mut state,
            &pm,
            &config,
            &SdlcCommand::PublishQuestion(input),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(q.answer.is_none());
    let mut answer = AnswerCommand {
        expected_question_version: 1,
        requirement_revision: 1,
        selected_option_ids: vec![],
        text: None,
        comment: None,
        idempotency_key: "answer".into(),
    };
    assert!(validate_answer(&q, &answer).is_err());
    answer.selected_option_ids = vec![q.options[0].id];
    assert!(validate_answer(&q, &answer).is_ok());
    answer.selected_option_ids.push(q.options[0].id);
    assert!(validate_answer(&q, &answer).is_err());
    answer.selected_option_ids = vec![Uuid::new_v4()];
    assert!(validate_answer(&q, &answer).is_err());
    answer.selected_option_ids = vec![q.options[1].id];
    assert!(validate_answer(&q, &answer).is_err());
    answer.text = Some("Custom explanation".into());
    assert!(validate_answer(&q, &answer).is_ok());
    let mut multiple = q.clone();
    multiple.mode = QuestionMode::Multiple;
    answer.selected_option_ids = q.options.iter().map(|o| o.id).collect();
    assert!(validate_answer(&multiple, &answer).is_ok());
    assert!(validate_answer(&q, &answer).is_err());
    multiple.mode = QuestionMode::Text;
    multiple.options.clear();
    answer.selected_option_ids.clear();
    assert!(validate_answer(&multiple, &answer).is_ok());
    answer.text = Some("  ".into());
    assert!(validate_answer(&multiple, &answer).is_err());
}

#[test]
fn stale_question_revision_fence_and_human_machine_spoof_are_denied() {
    let (mut state, config, owner, pm, _, fence) = fixture();
    publish(&mut state, &config, &pm, &fence);
    let input = question(&fence, QuestionMode::Single);
    let command = SdlcCommand::PublishQuestion(input.clone());
    assert!(matches!(
        apply(&mut state, &owner, &config, &command),
        Err(AppError::Forbidden)
    ));
    let mut no_scope = pm.clone();
    no_scope.scopes.clear();
    assert!(matches!(
        apply(&mut state, &no_scope, &config, &command),
        Err(AppError::Forbidden)
    ));
    apply(&mut state, &pm, &config, &command).unwrap();
    let mut stale = input.clone();
    stale.fence.assignment_version = 2;
    assert!(matches!(
        apply(
            &mut state,
            &pm,
            &config,
            &SdlcCommand::PublishQuestion(stale)
        ),
        Err(AppError::Conflict(_))
    ));
    let mut answer = AnswerCommand {
        expected_question_version: 2,
        requirement_revision: 1,
        selected_option_ids: vec![input.options[0].id],
        text: None,
        comment: None,
        idempotency_key: "answer".into(),
    };
    assert!(matches!(
        apply(
            &mut state,
            &owner,
            &config,
            &SdlcCommand::Answer {
                question_id: input.question_id,
                command: answer.clone()
            }
        ),
        Err(AppError::Conflict(_))
    ));
    answer.expected_question_version = 1;
    answer.requirement_revision = 2;
    assert!(matches!(
        apply(
            &mut state,
            &owner,
            &config,
            &SdlcCommand::Answer {
                question_id: input.question_id,
                command: answer.clone()
            }
        ),
        Err(AppError::Conflict(_))
    ));
    answer.requirement_revision = 1;
    assert!(matches!(
        apply(
            &mut state,
            &actor("operator", true, vec![]),
            &config,
            &SdlcCommand::Answer {
                question_id: input.question_id,
                command: answer
            }
        ),
        Err(AppError::Forbidden)
    ));
}

#[test]
fn owner_confirmation_requires_new_document_and_exact_trusted_evidence() {
    let (mut state, config, owner, pm, verifier, fence) = fixture();
    publish(&mut state, &config, &pm, &fence);
    // Evidence for the old revision must not make it ready after a clarification
    // answer; the PM still has to publish a new full requirements revision.
    for check in ["review", "contract"] {
        let command = SdlcCommand::Evidence(EvidenceCommand {
            fence: fence.clone(),
            requirement_revision: 1,
            content_hash: state.revisions.last().unwrap().content_hash.clone(),
            check_id: check.into(),
            evidence_reference: "workflow://verified/old-revision".into(),
            idempotency_key: format!("old-revision-{check}"),
        });
        apply(&mut state, &verifier, &config, &command).unwrap();
    }
    let input = question(&fence, QuestionMode::Single);
    apply(
        &mut state,
        &pm,
        &config,
        &SdlcCommand::PublishQuestion(input.clone()),
    )
    .unwrap();
    let confirm = |state: &TaskState| SdlcCommand::Confirm {
        revision: state.current_revision().unwrap(),
        command: ConfirmCommand {
            content_hash: state.revisions.last().unwrap().content_hash.clone(),
            idempotency_key: "confirm".into(),
            expected_routing_policy_version: None,
        },
    };
    let command = confirm(&state);
    assert!(apply(&mut state, &owner, &config, &command).is_err());
    apply(
        &mut state,
        &owner,
        &config,
        &SdlcCommand::Answer {
            question_id: input.question_id,
            command: AnswerCommand {
                expected_question_version: 1,
                requirement_revision: 1,
                selected_option_ids: vec![input.options[0].id],
                text: None,
                comment: None,
                idempotency_key: "answer".into(),
            },
        },
    )
    .unwrap();
    assert!(!state.ready());
    publish(&mut state, &config, &pm, &fence);
    for check in ["review", "contract"] {
        let command = SdlcCommand::Evidence(EvidenceCommand {
            fence: fence.clone(),
            requirement_revision: 2,
            content_hash: state.revisions.last().unwrap().content_hash.clone(),
            check_id: check.into(),
            evidence_reference: "workflow://verified/check".into(),
            idempotency_key: check.into(),
        });
        assert!(matches!(
            apply(&mut state, &owner, &config, &command),
            Err(AppError::Forbidden)
        ));
        assert!(matches!(
            apply(&mut state, &pm, &config, &command),
            Err(AppError::Forbidden)
        ));
        apply(&mut state, &verifier, &config, &command).unwrap();
    }
    assert!(state.context(&owner).permissions.can_confirm);
    let mut child = state.clone();
    child.root_task_id = Uuid::new_v4();
    assert!(!child.ready());
    assert!(!child.context(&owner).permissions.can_confirm);
    let stale = SdlcCommand::Confirm {
        revision: 1,
        command: ConfirmCommand {
            content_hash: state.revisions[0].content_hash.clone(),
            idempotency_key: "stale-confirmation".into(),
            expected_routing_policy_version: None,
        },
    };
    assert!(matches!(
        apply(&mut state, &owner, &config, &stale),
        Err(AppError::Conflict(_))
    ));
    let mut wrong_hash = confirm(&state);
    if let SdlcCommand::Confirm { command, .. } = &mut wrong_hash {
        command.content_hash = "bad".into();
    }
    assert!(apply(&mut state, &owner, &config, &wrong_hash).is_err());
    let command = confirm(&state);
    assert!(
        apply(
            &mut state,
            &actor("owner", false, vec![]),
            &config,
            &command
        )
        .is_err()
    );
    apply(&mut state, &owner, &config, &command).unwrap();
    assert!(matches!(state.stage, Stage::Analysis));
    assert!(!state.ready());
    assert_eq!(
        state.context(&owner).waiting_reason.as_deref(),
        Some("queued_for_analysis")
    );
    let intent =
        analysis_intent(&state, state.confirmations.last().unwrap(), Uuid::new_v4()).unwrap();
    assert_eq!(intent.requirement_revision, 2);
    assert_eq!(intent.status, AnalysisStatus::Ready);
    assert_eq!(intent.workflow, "hermes-sdlc:analyst");
    assert_eq!(intent.mode, "analysis");
    assert_eq!(intent.scope, "business");
    let saved = canonical_hash(&state).unwrap();
    assert!(matches!(
        apply(&mut state, &owner, &config, &command),
        Err(AppError::Conflict(_))
    ));
    let mutation = SdlcCommand::PublishRevision(PublishRevision {
        fence,
        expected_requirement_revision: Some(2),
        document: document(),
        idempotency_key: "late-pm-revision".into(),
    });
    assert!(matches!(
        apply(&mut state, &pm, &config, &mutation),
        Err(AppError::Conflict(_))
    ));
    assert_eq!(canonical_hash(&state).unwrap(), saved);
}

#[test]
fn wire_hash_is_flat_canonical_and_unsafe_versions_are_rejected() {
    assert_eq!(
        canonical_hash(&json!({"z":{"b":2,"a":1},"a":[2,1]})).unwrap(),
        canonical_hash(&json!({"a":[2,1],"z":{"a":1,"b":2}})).unwrap()
    );
    assert_ne!(
        canonical_hash(&json!([1, 2])).unwrap(),
        canonical_hash(&json!([2, 1])).unwrap()
    );
    let (mut state, config, _, pm, _, fence) = fixture();
    publish(&mut state, &config, &pm, &fence);
    let value = serde_json::to_value(&state.revisions[0]).unwrap();
    assert_eq!(value["goal"], "Goal");
    assert!(value.get("document").is_none());
    assert!(serde_json::from_value::<RequirementsRevision>(value).is_ok());
    let command = json!({"expected_question_version":9007199254740992i64,"requirement_revision":1,"selected_option_ids":[],"text":null,"comment":null,"idempotency_key":"k"});
    assert!(serde_json::from_value::<AnswerCommand>(command.clone()).is_err());
    let mut valid = command;
    valid["expected_question_version"] = json!(1);
    valid["author_subject"] = json!("spoof");
    assert!(serde_json::from_value::<AnswerCommand>(valid).is_err());
    assert!(next_version(MAX_SAFE_VERSION).is_err());
    assert_eq!(
        next_version(MAX_SAFE_VERSION - 1).unwrap(),
        MAX_SAFE_VERSION
    );
}

#[test]
fn canonical_answer_selection_is_a_set_but_question_binding_is_part_of_hash() {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let question_id = Uuid::new_v4();
    let mut command = SdlcCommand::Answer {
        question_id,
        command: AnswerCommand {
            expected_question_version: 1,
            requirement_revision: 1,
            selected_option_ids: vec![first, second],
            text: None,
            comment: None,
            idempotency_key: "key".into(),
        },
    };
    let hash = command_hash(&command).unwrap();
    if let SdlcCommand::Answer { command, .. } = &mut command {
        command.selected_option_ids.reverse();
    }
    assert_eq!(hash, command_hash(&command).unwrap());
    if let SdlcCommand::Answer { question_id, .. } = &mut command {
        *question_id = Uuid::new_v4();
    }
    assert_ne!(hash, command_hash(&command).unwrap());
}

#[test]
fn pm_draft_input_hash_matches_exact_utf8_vectors_without_normalization_or_key() {
    for (title, description, expected) in [
        (
            "Title",
            "",
            "9e109d116c12f9cee785295ff2193aae78def94c769785cd4e182abf474dc1af",
        ),
        (
            "  \u{0417}\u{0430}\u{0434}\u{0430}\u{0447}\u{0430} \u{1f680}  ",
            "first\r\nsecond\ne\u{0301} \u{00e9}\t\"\\",
            "32b3c95cffc2114b62b969de058f4e3839c3e6b82d7ab09c061e35b0ed0d0b35",
        ),
        (
            "Title",
            "first\nsecond",
            "85e49640e5eb71089824f52c45338b91c206c299cfe9d147ebc6a2517868cd3e",
        ),
    ] {
        assert_eq!(pm_draft_input_hash(title, description).unwrap(), expected);
    }
    for (a, b) in [
        ("e\u{0301}", "\u{00e9}"),
        ("a\r\nb", "a\nb"),
        (" text ", "text"),
    ] {
        assert_ne!(
            pm_draft_input_hash("Title", a).unwrap(),
            pm_draft_input_hash("Title", b).unwrap()
        );
    }
    let hash = pm_draft_input_hash("Title", "").unwrap();
    assert_ne!(
        hash,
        canonical_hash(&json!({"description":"","title":"Title","idempotency_key":"first"}))
            .unwrap()
    );
}
