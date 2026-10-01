use super::*;
use serde_json::json;

fn actor(subject: &str, human: bool, scopes: Vec<String>) -> Principal {
    Principal {
        subject: subject.into(),
        human_session: human,
        scopes: scopes.into_iter().collect(),
    }
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
    assert!(matches!(state.stage, Stage::Backlog));
    publish(&mut state, &config, &pm, &fence);
    assert!(!state.ready());
    assert!(matches!(
        apply(&mut state, &owner, &config, &command),
        Err(AppError::Conflict(_))
    ));
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
