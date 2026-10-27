use domain::sdlc::*;
use domain::sdlc_configured_cli::*;
use shared::AppError;
use uuid::Uuid;

/// Only an untouched business Draft can acquire its first execution protocol.
/// Repository admission additionally checks retained assignment/history ledgers.
pub fn adopt_unstarted_draft(state: &mut TaskState) -> Result<(), AppError> {
    state.require_root()?;
    if state.interaction_protocol()?.is_some()
        || state.stage != Stage::Draft
        || state
            .status
            .is_some_and(|value| value != domain::sdlc_lifecycle::LifecycleStatus::Ready)
        || state.assignment.is_some()
        || state
            .draft_submode
            .is_some_and(|value| value != DraftSubmode::Brainstorming)
        || state.confirmation_revision.is_some()
        || !state.questions.is_empty()
        || !state.revisions.is_empty()
        || !state.confirmations.is_empty()
        || !state.evidence.is_empty()
        || state.retained_fields.contains_key("lifecycle")
        || state.retained_fields.contains_key("workflow_task_key")
    {
        return Err(AppError::conflict(
            "configured adoption requires an unstarted Draft",
        ));
    }
    state.draft_flow()?;
    state.retained_fields.insert(
        "interaction_protocol".into(),
        serde_json::json!("configured-cli-v1"),
    );
    state
        .retained_fields
        .insert("flow".into(), serde_json::json!("single-task-v1"));
    state.status = Some(domain::sdlc_lifecycle::LifecycleStatus::Ready);
    state.draft_submode = Some(DraftSubmode::Brainstorming);
    Ok(())
}

pub fn authorize_configured_identity(actor: &Principal) -> Result<(), AppError> {
    if actor.human_session
        || actor.trusted_human
        || actor.scopes.contains("admin")
        || !actor.scopes.contains("task-tracker:read")
        || !actor.scopes.contains("task-tracker:write")
        || crate::sdlc::has_pm_grant(&actor.scopes)
        || crate::sdlc_lifecycle::has_delivery_grant(&actor.scopes)
        || crate::sdlc::canonical_pm_uuid(&actor.subject).is_none()
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub fn authorize_configured_pm(state: &TaskState, actor: &Principal) -> Result<(), AppError> {
    authorize_configured_identity(actor)?;
    let assignment = state.assignment.as_ref().ok_or(AppError::Forbidden)?;
    if !state.configured_cli()
        || assignment.agent_id.is_nil()
        || assignment.machine_subject != actor.subject
        || actor.subject == state.owner_subject
    {
        return Err(AppError::Forbidden);
    }
    state.require_root()
}

/// Project access is checked by the repository. A read-only service can inspect
/// the saved business result without becoming its assigned producer.
pub fn authorize_completion_read(state: &TaskState, actor: &Principal) -> Result<(), AppError> {
    if !state.configured_cli() {
        return Err(AppError::Forbidden);
    }
    if state.owner(actor)
        || (!actor.human_session
            && !actor.trusted_human
            && crate::sdlc_reservation::reader(actor).is_ok())
    {
        return state.require_root();
    }
    authorize_configured_pm(state, actor)
}

pub fn validate_completion(
    state: &TaskState,
    actor: &Principal,
    command: &CompleteConfiguredPm,
) -> Result<(), AppError> {
    authorize_configured_pm(state, actor)?;
    crate::sdlc::validate_key(&command.idempotency_key)?;
    crate::sdlc::validate_key(&command.workflow_operation_key)?;
    let assignment = state.assignment.as_ref().ok_or(AppError::Forbidden)?;
    let revision = state
        .revisions
        .last()
        .ok_or_else(|| AppError::conflict("requirements absent"))?;
    let confirmation = state
        .current_confirmation()
        .ok_or_else(|| AppError::conflict("exact owner confirmation required"))?;
    crate::sdlc::validate_document(&revision.document)?;
    if !((state.stage == Stage::Draft
        && state.status == Some(domain::sdlc_lifecycle::LifecycleStatus::InProgress)
        && state.ready())
        || (state.stage == Stage::Backlog
            && state.status == Some(domain::sdlc_lifecycle::LifecycleStatus::Ready)))
        || command.fence.assignment_id != assignment.assignment_id
        || command.fence.execution_id != assignment.execution_id
        || command.fence.agent_id != assignment.agent_id
        || command.fence.assignment_version != assignment.version
        || command.requirement_revision != revision.revision
        || command.content_hash != revision.content_hash
        || crate::sdlc::canonical_hash(&revision.document)? != revision.content_hash
        || confirmation.owner_subject != state.owner_subject
        || command.workflow_execution_ref.is_nil()
        || !(1..=MAX_SAFE_VERSION).contains(&command.namespace_id)
        || !(2..=MAX_SAFE_VERSION).contains(&command.workflow_result_version)
    {
        return Err(AppError::conflict(
            "configured completion fence or owner confirmation changed",
        ));
    }
    Ok(())
}

pub fn validate_receipt(
    state: &TaskState,
    command: &CompleteConfiguredPm,
    receipt: &ConfiguredWorkflowReceipt,
) -> Result<(), AppError> {
    let assignment = state.assignment.as_ref().ok_or(AppError::Forbidden)?;
    let confirmation = state
        .current_confirmation()
        .ok_or_else(|| AppError::conflict("exact owner confirmation required"))?;
    let key = state
        .retained_fields
        .get("workflow_task_key")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| AppError::conflict("workflow business key absent"))?;
    let hash = &receipt.request_sha256;
    if receipt.interaction_protocol != InteractionProtocol::ConfiguredCliV1
        || receipt.execution_ref != command.workflow_execution_ref
        || receipt.namespace_id != command.namespace_id
        || receipt.task != key
        || receipt.tracker_task_id != state.task_id
        || receipt.concrete_agent_ref != assignment.agent_id
        || receipt.assignment_ref != assignment.assignment_id
        || receipt.tracker_execution_id != assignment.execution_id
        || receipt.assignment_version != assignment.version
        || receipt.machine_subject != assignment.machine_subject
        || receipt.operation_key != command.workflow_operation_key
        || receipt.result_version != command.workflow_result_version
        || receipt.requirements_revision != Some(command.requirement_revision)
        || receipt.requirements_sha256.as_ref() != Some(&command.content_hash)
        || receipt.confirmation_id != Some(confirmation.id)
        || receipt.owner_subject != state.owner_subject
        || receipt.status != "done"
        || receipt.history_id <= 0
        || hash.len() != 64
        || !hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(AppError::conflict(
            "authoritative Workflow terminal receipt mismatch",
        ));
    }
    Ok(())
}

pub fn completion_result(
    state: &TaskState,
    command: &CompleteConfiguredPm,
    receipt: ConfiguredWorkflowReceipt,
    owner_version: i64,
    completion_id: Uuid,
    created_at: chrono::DateTime<chrono::Utc>,
) -> Result<ConfiguredPmCompletionReceipt, AppError> {
    validate_receipt(state, command, &receipt)?;
    let workflow_receipt_sha256 = crate::sdlc::canonical_hash(&receipt)?;
    Ok(ConfiguredPmCompletionReceipt {
        interaction_protocol: InteractionProtocol::ConfiguredCliV1,
        completion_id,
        tracker_instance_id: state.tracker_instance_id.clone(),
        project_id: state.project_id,
        task_id: state.task_id,
        root_task_id: state.root_task_id,
        owner_subject: state.owner_subject.clone(),
        fence: command.fence.clone(),
        owner_version,
        requirement_revision: command.requirement_revision,
        content_hash: command.content_hash.clone(),
        confirmation_id: receipt
            .confirmation_id
            .ok_or_else(|| AppError::conflict("PM confirmation absent"))?,
        workflow_receipt: receipt,
        workflow_receipt_sha256,
        stage: Stage::Backlog,
        status: domain::sdlc_lifecycle::LifecycleStatus::Ready,
        previous_pm_admission_state: "reserved".into(),
        next_pm_admission_state: "completed".into(),
        idempotency_key: command.idempotency_key.clone(),
        created_at,
    })
}

/// Reconstruct only the business tuple used to verify an immutable old receipt.
/// This value is never persisted or used to authorize a new command. Authorization
/// belongs to the current task/identity; history retains its original assignment
/// and approved requirements when a later PM review changes them.
pub fn completion_history_state(
    current: &TaskState,
    assignment: &PmAssignment,
    command: &CompleteConfiguredPm,
) -> Result<TaskState, AppError> {
    crate::sdlc::validate_key(&command.idempotency_key)?;
    crate::sdlc::validate_key(&command.workflow_operation_key)?;
    if !current.configured_cli()
        || command.interaction_protocol != InteractionProtocol::ConfiguredCliV1
        || command.workflow_execution_ref.is_nil()
        || !(1..=MAX_SAFE_VERSION).contains(&command.namespace_id)
        || !(2..=MAX_SAFE_VERSION).contains(&command.workflow_result_version)
        || !(1..=MAX_SAFE_VERSION).contains(&assignment.version)
        || assignment.assignment_id.is_nil()
        || assignment.execution_id.is_nil()
        || assignment.agent_id.is_nil()
        || crate::sdlc::canonical_pm_uuid(&assignment.machine_subject).is_none()
        || assignment.machine_subject == current.owner_subject
    {
        return Err(AppError::conflict("invalid completion business history"));
    }
    let mut saved = current.clone();
    let index = saved
        .revisions
        .iter()
        .position(|revision| revision.revision == command.requirement_revision)
        .ok_or_else(|| AppError::conflict("completion requirements history absent"))?;
    saved.revisions.truncate(index + 1);
    saved.assignment = Some(assignment.clone());
    if command.fence.assignment_id != assignment.assignment_id
        || command.fence.execution_id != assignment.execution_id
        || command.fence.agent_id != assignment.agent_id
        || command.fence.assignment_version != assignment.version
        || saved.revisions[index].content_hash != command.content_hash
        || crate::sdlc::canonical_hash(&saved.revisions[index].document)? != command.content_hash
        || saved.current_confirmation().is_none()
    {
        return Err(AppError::conflict("completion business history mismatch"));
    }
    Ok(saved)
}

impl crate::sdlc::SdlcService {
    pub async fn complete_configured_pm(
        &self,
        task: Uuid,
        actor: &Principal,
        command: CompleteConfiguredPm,
    ) -> Result<(ConfiguredPmCompletionReceipt, bool), AppError> {
        let state = self.repository.read(task, actor).await?;
        authorize_configured_pm(&state, actor)?;
        crate::sdlc::validate_key(&command.idempotency_key)?;
        let hash = crate::sdlc::canonical_hash(&command)?;
        if let Some(saved) = self
            .repository
            .configured_pm_completion(task, actor, Some(&command.idempotency_key))
            .await?
        {
            if saved.request_sha256 != hash {
                return Err(AppError::conflict("PM completion key payload conflict"));
            }
            return Ok((saved.result, true));
        }
        validate_completion(&state, actor, &command)?;
        if state.stage != Stage::Draft {
            return Err(AppError::conflict(
                "configured completion requires confirmed Draft",
            ));
        }
        let reader = self.configured_workflow.as_ref().ok_or_else(|| {
            AppError::Unavailable("Workflow receipt reader is not configured".into())
        })?;
        let receipt = reader.read(&state, &command).await?;
        // The repository reauthorizes and rechecks the complete business tuple
        // under its lock after IO. No Tracker transaction spans Workflow HTTP.
        self.repository
            .complete_configured_pm(task, actor, command, receipt)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub(super) fn fixture() -> (TaskState, Principal, SdlcConfig, MachineFence) {
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
            scopes: ["task-tracker:read".into(), "task-tracker:write".into()]
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
    fn initial_adoption_preserves_business_identity_and_refuses_started_work() {
        let (mut initial, _, _, _) = fixture();
        initial.assignment = None;
        initial.retained_fields.remove("interaction_protocol");
        initial.retained_fields.remove("workflow_task_key");
        initial.retained_fields.remove("flow");
        let original = serde_json::to_value(&initial).unwrap();
        let mut adopted = initial.clone();
        adopt_unstarted_draft(&mut adopted).unwrap();
        let mut expected = original;
        expected["interaction_protocol"] = json!("configured-cli-v1");
        expected["flow"] = json!("single-task-v1");
        expected["status"] = json!("Ready");
        expected["draft_submode"] = json!("brainstorming");
        assert_eq!(serde_json::to_value(&adopted).unwrap(), expected);
        assert!(adopt_unstarted_draft(&mut adopted).is_err());
        for field in ["lifecycle", "workflow_task_key"] {
            let mut started = initial.clone();
            started.retained_fields.insert(field.into(), json!(null));
            assert!(adopt_unstarted_draft(&mut started).is_err());
        }
        let mut active = initial.clone();
        active.status = Some(domain::sdlc_lifecycle::LifecycleStatus::InProgress);
        assert!(adopt_unstarted_draft(&mut active).is_err());
        let mut assigned = initial;
        assigned.assignment = fixture().0.assignment;
        assert!(adopt_unstarted_draft(&mut assigned).is_err());
    }

    #[test]
    fn standard_service_scopes_use_current_assignment_subject_not_a_task_credential() {
        let (mut state, actor, _, _) = fixture();
        authorize_configured_pm(&state, &actor).unwrap();
        crate::sdlc::authorize_pm_read(&state, &actor).unwrap();
        state.assignment.as_mut().unwrap().agent_id = Uuid::nil();
        assert!(authorize_configured_pm(&state, &actor).is_err());
        state.assignment.as_mut().unwrap().agent_id = Uuid::from_u128(2);
        state.assignment.as_mut().unwrap().machine_subject = Uuid::from_u128(42).to_string();
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
        let mut mixed = actor.clone();
        mixed
            .scopes
            .insert("task-tracker:sdlc:delivery:foreign".into());
        assert!(authorize_configured_pm(&state, &mixed).is_err());
        let mut human_pat = actor.clone();
        human_pat.human_session = true;
        assert!(authorize_configured_pm(&state, &human_pat).is_err());
        let mut administrator = actor;
        administrator.scopes.insert("admin".into());
        assert!(authorize_configured_pm(&state, &administrator).is_err());
    }

    #[test]
    fn configured_pm_requires_both_standard_scopes_and_canonical_exact_subject() {
        let (mut state, actor, _, _) = fixture();
        let mut with_workflow = actor.clone();
        with_workflow.scopes.extend([
            "project-workflow:read".into(),
            "project-workflow:write".into(),
        ]);
        authorize_configured_pm(&state, &with_workflow).unwrap();
        crate::sdlc_confirmation::authorize_read(&state, &with_workflow).unwrap();
        for scope in ["task-tracker:read", "task-tracker:write"] {
            let mut partial = actor.clone();
            partial.scopes.remove(scope);
            assert!(authorize_configured_pm(&state, &partial).is_err());
            assert_eq!(
                crate::sdlc::authorize_pm_read(&state, &partial).is_ok(),
                scope == "task-tracker:write"
            );
        }
        for subject in [
            Uuid::nil().to_string(),
            "not-a-uuid".into(),
            "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA".into(),
        ] {
            let mut bad = actor.clone();
            bad.subject = subject.clone();
            state.assignment.as_mut().unwrap().machine_subject = subject;
            assert!(authorize_configured_pm(&state, &bad).is_err());
        }
    }

    #[test]
    fn server_reader_is_readonly_and_never_assigned_pm_authority() {
        let (state, _, _, _) = fixture();
        let mut reader = Principal {
            subject: Uuid::from_u128(42).to_string(),
            human_session: false,
            trusted_human: false,
            scopes: ["task-tracker:read".into()].into(),
        };
        crate::sdlc::authorize_pm_read(&state, &reader).unwrap();
        crate::sdlc_confirmation::authorize_read(&state, &reader).unwrap();
        authorize_completion_read(&state, &reader).unwrap();
        assert!(authorize_configured_pm(&state, &reader).is_err());
        for scope in ["task-tracker:write", "project-workflow:read"] {
            let mut extra = reader.clone();
            extra.scopes.insert(scope.into());
            assert!(crate::sdlc::authorize_pm_read(&state, &extra).is_err());
            assert!(crate::sdlc_confirmation::authorize_read(&state, &extra).is_err());
            assert!(authorize_completion_read(&state, &extra).is_err());
        }
        reader.human_session = true;
        assert!(crate::sdlc::authorize_pm_read(&state, &reader).is_err());
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
        let mut stale = revision.clone();
        stale.fence.agent_id = Uuid::from_u128(99);
        assert!(
            crate::sdlc::authorize(
                &state,
                &actor,
                &config,
                &SdlcCommand::PublishRevision(stale)
            )
            .is_err()
        );
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
        let mut retired = serde_json::to_value(&question).unwrap();
        retired["checkpoint_id"] = json!(Uuid::new_v4());
        assert!(serde_json::from_value::<PublishQuestion>(retired).is_err());
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

#[cfg(test)]
#[path = "sdlc_configured_completion_tests.rs"]
mod completion_tests;
