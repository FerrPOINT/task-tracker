use domain::sdlc::*;
use shared::AppError;
use uuid::Uuid;

pub const AGENT_SCOPE_PREFIX: &str = "task-tracker:sdlc:agent:";

pub fn configured_agent(actor: &Principal) -> Result<Option<Uuid>, AppError> {
    let mut scopes = actor
        .scopes
        .iter()
        .filter_map(|s| s.strip_prefix(AGENT_SCOPE_PREFIX));
    let Some(raw) = scopes.next() else {
        return Ok(None);
    };
    let id = crate::sdlc::canonical_pm_uuid(raw).ok_or(AppError::Forbidden)?;
    if actor.human_session
        || actor.trusted_human
        || scopes.next().is_some()
        || crate::sdlc::has_pm_grant(&actor.scopes)
    {
        return Err(AppError::Forbidden);
    }
    Ok(Some(id))
}

pub fn authorize_configured_pm(state: &TaskState, actor: &Principal) -> Result<(), AppError> {
    let agent = configured_agent(actor)?.ok_or(AppError::Forbidden)?;
    let assignment = state.assignment.as_ref().ok_or(AppError::Forbidden)?;
    if !state.configured_cli()
        || assignment.agent_id != agent
        || assignment.machine_subject != actor.subject
        || actor.subject == state.owner_subject
    {
        return Err(AppError::Forbidden);
    }
    state.require_root()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> (TaskState, Principal, SdlcConfig, MachineFence) {
        let task = Uuid::from_u128(1);
        let agent = Uuid::from_u128(2);
        let assignment = PmAssignment {
            assignment_id: Uuid::from_u128(3),
            execution_id: Uuid::from_u128(4),
            agent_id: agent,
            version: 1,
            machine_subject: Uuid::from_u128(5).to_string(),
        };
        let state: TaskState = serde_json::from_value(json!({
            "tracker_instance_id":"configured-test","project_id":Uuid::from_u128(6),
            "task_id":task,"root_task_id":task,"owner_subject":"owner","stage":"Draft",
            "draft_submode":"brainstorming","confirmation_revision":null,"assignment":assignment,
            "questions":[],"revisions":[],"confirmations":[],"evidence":[],
            "flow":"single-task-v1","interaction_protocol":"configured-cli-v1",
            "workflow_task_key":"SDLC-12"
        }))
        .unwrap();
        let actor = Principal {
            subject: assignment.machine_subject.clone(),
            human_session: false,
            trusted_human: false,
            scopes: [
                format!("{AGENT_SCOPE_PREFIX}{agent}"),
                "task-tracker:read".into(),
                "task-tracker:write".into(),
            ]
            .into_iter()
            .collect(),
        };
        let config = SdlcConfig {
            single_task_project_ids: Default::default(),
            reservation_scheduler_subject: "scheduler".into(),
            instance_id: "configured-test".into(),
            orchestrator_subject: "orchestrator".into(),
            verifier_subject: "verifier".into(),
        };
        let fence = MachineFence {
            assignment_id: assignment.assignment_id,
            execution_id: assignment.execution_id,
            agent_id: agent,
            assignment_version: 1,
        };
        (state, actor, config, fence)
    }

    #[test]
    fn stable_service_scope_is_current_assignment_not_a_task_credential() {
        let (mut state, actor, _, _) = fixture();
        authorize_configured_pm(&state, &actor).unwrap();
        crate::sdlc::authorize_pm_read(&state, &actor).unwrap();
        state.assignment.as_mut().unwrap().agent_id = Uuid::from_u128(99);
        assert!(authorize_configured_pm(&state, &actor).is_err());
        state.assignment.as_mut().unwrap().agent_id = Uuid::from_u128(2);
        state.assignment.as_mut().unwrap().machine_subject = "foreign".into();
        assert!(authorize_configured_pm(&state, &actor).is_err());
    }

    #[test]
    fn human_scheduler_legacy_and_mixed_grants_cannot_impersonate_configured_pm() {
        let (state, actor, _, _) = fixture();
        let mut human = actor.clone();
        human.trusted_human = true;
        assert!(authorize_configured_pm(&state, &human).is_err());
        let mut scheduler = actor.clone();
        scheduler.subject = "scheduler".into();
        assert!(authorize_configured_pm(&state, &scheduler).is_err());
        let mut mixed = actor.clone();
        mixed
            .scopes
            .insert(state.assignment.as_ref().unwrap().scope(state.task_id));
        assert!(authorize_configured_pm(&state, &mixed).is_err());
        let mut legacy = state.clone();
        legacy.retained_fields.remove("interaction_protocol");
        assert!(authorize_configured_pm(&legacy, &actor).is_err());
    }

    #[test]
    fn explicit_protocol_is_closed_and_legacy_creation_bytes_are_unchanged() {
        let legacy = json!({"title":"Task","description":"","idempotency_key":"create"});
        let command: CreateDraftCommand = serde_json::from_value(legacy.clone()).unwrap();
        assert_eq!(serde_json::to_value(command).unwrap(), legacy);
        for marker in [json!(null), json!("legacy"), json!(false)] {
            let mut bad = legacy.clone();
            bad["interaction_protocol"] = marker;
            assert!(serde_json::from_value::<CreateDraftCommand>(bad).is_err());
        }
    }

    #[test]
    fn configured_question_and_revision_keep_business_fence_and_owner_confirmation() {
        let (mut state, actor, config, fence) = fixture();
        let revision: PublishRevision = serde_json::from_value(json!({
            "fence":fence,"expected_requirement_revision":null,"idempotency_key":"revision",
            "document":{"goal":"A goal","scope":["scope"],"exclusions":[],"constraints":[],"dependencies":[],"assumptions":[],"scenarios":["scenario"],
                "acceptance_criteria":["criterion"],"checklist":["check"],"prerequisites":["prerequisite"]}
        })).unwrap();
        crate::sdlc::apply(
            &mut state,
            &actor,
            &config,
            &SdlcCommand::PublishRevision(revision.clone()),
        )
        .unwrap();
        let question: PublishQuestion = serde_json::from_value(json!({
            "fence":fence,"request_id":Uuid::from_u128(7),"question_id":Uuid::from_u128(8),
            "expected_question_version":null,"requirement_revision":1,"requirement_reference":null,
            "text":"Which option?","rationale":"Business choice","required":true,"mode":"text",
            "options":[],"recommended_option_id":null,"idempotency_key":"question"
        }))
        .unwrap();
        assert!(question.checkpoint_id.is_none());
        crate::sdlc::apply(
            &mut state,
            &actor,
            &config,
            &SdlcCommand::PublishQuestion(question),
        )
        .unwrap();
        assert!(state.questions[0].checkpoint_id.is_none());
        assert!(!state.context(&actor).permissions.can_confirm);
        assert_eq!(
            state.context(&actor).workflow_task_key.as_deref(),
            Some("SDLC-12")
        );
        assert!(
            crate::sdlc::apply(
                &mut state,
                &actor,
                &config,
                &SdlcCommand::PublishRevision(revision)
            )
            .is_err()
        );
        let confirm = SdlcCommand::Confirm {
            revision: 1,
            command: ConfirmCommand {
                content_hash: state.revisions[0].content_hash.clone(),
                idempotency_key: "confirm".into(),
                expected_routing_policy_version: None,
            },
        };
        assert!(crate::sdlc::authorize(&state, &actor, &config, &confirm).is_err());
    }
}
