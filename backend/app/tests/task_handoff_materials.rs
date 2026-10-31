//! Synthetic owner declarations; no live Fleet/Forge/Workflow observation is claimed.
use app::task_handoff::{HandoffAllocation, prepare_materials};
use chrono::Utc;
use domain::{
    sdlc::{Principal, TaskState},
    sdlc_lifecycle::{LifecycleStage, LifecycleStatus},
    task_handoff::{
        CurrentStageEntry, PreparedCurrentStageHandoff, TaskExecutionBlocker, TaskHandoffCommand,
    },
};
use serde_json::json;
use uuid::Uuid;

fn fixture(
    status: LifecycleStatus,
) -> (
    TaskState,
    Principal,
    PreparedCurrentStageHandoff,
    TaskHandoffCommand,
) {
    let now = Utc::now();
    let project = Uuid::new_v4();
    let task = Uuid::new_v4();
    let instance = Uuid::new_v4().to_string();
    let state:TaskState=serde_json::from_value(json!({
        "tracker_instance_id":instance,"project_id":project,"task_id":task,"root_task_id":task,
        "owner_subject":"human-owner","stage":"Development","status":status,"confirmation_revision":null,
        "assignment":null,"questions":[],"revisions":[],"confirmations":[],"evidence":[],"state_origin":"manual_core"
    })).unwrap();
    let actor = Principal {
        subject: "human-owner".into(),
        human_session: true,
        trusted_human: true,
        scopes: Default::default(),
    };
    let mut routes = json!({});
    for (index, role) in [
        "project_manager",
        "analyst",
        "architect",
        "developer",
        "reviewer",
        "tester",
        "devops",
    ]
    .iter()
    .enumerate()
    {
        let suffix = role.replace('_', "-");
        routes[*role] = json!({"agent_id":Uuid::new_v4(),"fleet_config_revision":1,
            "instruction_profile":"single-task-v1","instruction_manifest_sha256":"b".repeat(64),
            "namespace_id":(index+1).to_string(),"namespace_name":format!("single-task-{suffix}"),
            "workflow_id":(index+11).to_string(),"workflow_key":format!("single-task-v1:{role}"),
            "profile":format!("single-task-v1-{suffix}"),"workflow_catalog_version":1,"workflow_catalog_sha256":"c".repeat(64)});
    }
    let routing=serde_json::from_value(json!({"contract_version":2,"tracker_instance_id":instance,"project_id":project,
        "version":3,"routing_hash":app::sdlc::canonical_hash(&routes).unwrap(),"routes":routes,"verification":"declared",
        "native_ready":false,"dispatch_allowed":false,"author_subject":"config-admin","created_at":now})).unwrap();
    let forge = Uuid::new_v4();
    let repository = Uuid::new_v4();
    let mut source:domain::sdlc_source::ProjectSourcePolicy=serde_json::from_value(json!({
        "contract_version":1,"tracker_instance_id":instance,"project_id":project,"source_revision":2,"source_hash":"",
        "source":{"forge_instance_id":forge,"repository_id":repository,"base_ref":"refs/heads/main","base_commit":"d".repeat(40)},
        "verification":{"namespace":{"registry_instance_id":Uuid::new_v4(),"namespace_id":Uuid::new_v4()},
            "forge_instance_id":forge,"repository_id":repository,"public_name":"fixture/source","base_ref":"refs/heads/main",
            "base_commit":"d".repeat(40),"default_ref":"refs/heads/main","protected_refs":[],"observed_at":now},
        "author_subject":"config-admin","created_at":now})).unwrap();
    source.source_hash = app::sdlc_source::sealed_hash(&source).unwrap();
    let core = domain::task_lifecycle::State {
        stage: LifecycleStage::Development,
        status,
        revision: 7,
    };
    let prepared = PreparedCurrentStageHandoff {
        task_id: task,
        project_id: project,
        owner_subject: actor.subject.clone(),
        state: core,
        issue_sha256: "a".repeat(64),
        issue_materials_sha256: "e".repeat(64),
        entry: Some(CurrentStageEntry::derive(core, false).unwrap()),
        routing: Some(routing),
        source: Some(source),
        blockers: vec![],
    };
    let command=serde_json::from_value(json!({"expected_revision":7,"expected_routing_policy_version":3,
        "confirmed_issue_sha256":"a".repeat(64),"idempotency_key":"human-original-key",
        "requirements":{"goal":"Human-confirmed current stage","scope":["Current source"],"exclusions":[],
            "scenarios":["Keep current progress"],"acceptance_criteria":["Verified actual result"],"constraints":[],
            "dependencies":[],"assumptions":[],"checklist":["result-check"],"prerequisites":["source-input"]}})).unwrap();
    (state, actor, prepared, command)
}
fn allocation() -> HandoffAllocation {
    HandoffAllocation {
        handoff_id: Uuid::new_v4(),
        confirmation_id: Uuid::new_v4(),
        routing_snapshot_id: Uuid::new_v4(),
        source_binding_id: Uuid::new_v4(),
        observed_at: Utc::now(),
    }
}
#[test]
fn human_entry_freezes_real_materials_without_resetting_progress_or_faking_a_draft() {
    for status in [LifecycleStatus::Ready, LifecycleStatus::InProgress] {
        let (state, actor, prepared, command) = fixture(status);
        let before = serde_json::to_value(&state).unwrap();
        let result = prepare_materials(&state, &actor, &prepared, &command, allocation()).unwrap();
        assert_eq!(result.receipt.entry.state, prepared.state);
        assert_eq!(result.receipt.resulting_revision, 8);
        assert_eq!(result.receipt.entry.role.key(), "developer");
        assert_eq!(result.receipt.entry.mode.key(), "initial");
        assert_eq!(result.next_state.stage, state.stage);
        assert_eq!(result.next_state.status, state.status);
        assert!(result.next_state.assignment.is_none());
        assert!(!result.next_state.retained_fields.contains_key("flow"));
        assert!(!result.next_state.retained_fields.contains_key("lifecycle"));
        assert_eq!(result.requirement.author_subject, actor.subject);
        assert_eq!(result.confirmation.owner_subject, actor.subject);
        assert_eq!(result.confirmation.stage, state.stage);
        assert_eq!(
            result.source.routing_snapshot_id,
            result.routing.snapshot_id
        );
        assert_eq!(result.source.confirmation_id, result.confirmation.id);
        assert_eq!(
            result.source.source_hash,
            app::sdlc_source::sealed_hash(&result.source).unwrap()
        );
        assert_eq!(result.receipt.idempotency_key, command.idempotency_key);
        assert_eq!(serde_json::to_value(&state).unwrap(), before);
        // Old Draft enrollment has its original contract; a human input is separate.
        assert!(
            app::sdlc_source::freeze_binding(
                &result.next_state,
                &result.confirmation,
                &result.routing,
                prepared.source.as_ref().unwrap()
            )
            .is_err()
        );
    }
}
#[test]
fn changed_revision_materials_routing_or_actor_cannot_create_a_handoff_package() {
    let (state, actor, prepared, command) = fixture(LifecycleStatus::InProgress);
    let before = serde_json::to_value(&state).unwrap();
    for field in ["revision", "hash", "routing", "actor", "machine", "busy"] {
        let mut actor = actor.clone();
        let mut command = command.clone();
        let mut prepared = prepared.clone();
        match field {
            "revision" => command.expected_revision += 1,
            "hash" => command.confirmed_issue_sha256 = "f".repeat(64),
            "routing" => command.expected_routing_policy_version += 1,
            "actor" => actor.subject = "different-human".into(),
            "machine" => {
                actor.human_session = false;
                actor.trusted_human = false;
            }
            "busy" => prepared.blockers.push(TaskExecutionBlocker::ExecutionBusy),
            _ => unreachable!(),
        }
        assert!(
            prepare_materials(&state, &actor, &prepared, &command, allocation()).is_err(),
            "{field}"
        );
    }
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
}
#[test]
fn handoff_does_not_accept_an_incomplete_requirements_document() {
    let (state, actor, prepared, mut command) = fixture(LifecycleStatus::Ready);
    command.requirements.prerequisites.clear();
    assert!(prepare_materials(&state, &actor, &prepared, &command, allocation()).is_err());
}

#[test]
fn every_manual_stage_has_its_real_configured_entry_without_fake_predecessors() {
    use app::task_handoff::{PendingHandoffMaterials, seed_current_stage};
    for stage in domain::task_lifecycle::STAGES {
        for status in [LifecycleStatus::Ready, LifecycleStatus::InProgress] {
            let (mut state, actor, mut prepared, command) = fixture(status);
            state.stage = stage.canonical();
            prepared.state.stage = stage;
            prepared.entry = Some(CurrentStageEntry::derive(prepared.state, false).unwrap());
            let frozen =
                prepare_materials(&state, &actor, &prepared, &command, allocation()).unwrap();
            let pending = PendingHandoffMaterials {
                receipt: frozen.receipt,
                routing: frozen.routing,
                source: frozen.source,
            };
            let seed = seed_current_stage(&pending).unwrap();
            assert_eq!((seed.task.stage, seed.task.status), (stage, status));
            assert_eq!(seed.consent.entry, prepared.entry.unwrap());
            assert!(seed.task.previous_result_id.is_none());
            assert!(
                seed.assignment.is_none()
                    && seed.lease.is_none()
                    && seed.binding.is_none()
                    && seed.result.is_none()
            );
            assert_eq!(
                seed.interaction_protocol,
                domain::sdlc::InteractionProtocol::ConfiguredCliV1
            );
            for invalid_revision in [-1, 0, domain::sdlc::MAX_SAFE_VERSION] {
                let mut invalid = PendingHandoffMaterials {
                    receipt: pending.receipt.clone(),
                    routing: pending.routing.clone(),
                    source: pending.source.clone(),
                };
                invalid.receipt.entry.state.revision = invalid_revision;
                invalid.receipt.resulting_revision = invalid_revision + 1;
                assert!(seed_current_stage(&invalid).is_err());
            }
            // A receipt for another task/source is not a current-stage entry permit.
            let mut changed = pending;
            changed.source.task_id = Uuid::new_v4();
            assert!(seed_current_stage(&changed).is_err());
        }
    }
}

#[test]
fn legacy_unmarked_clarification_uses_agreed_draft_view_without_rewriting_history() {
    let (mut state, actor, mut prepared, command) = fixture(LifecycleStatus::Ready);
    state.stage = domain::sdlc::Stage::Clarification;
    state.status = None;
    prepared.state.stage = LifecycleStage::Draft;
    prepared.entry = Some(CurrentStageEntry::derive(prepared.state, false).unwrap());
    let before = serde_json::to_value(&state).unwrap();
    let frozen = prepare_materials(&state, &actor, &prepared, &command, allocation()).unwrap();
    assert_eq!(frozen.next_state.stage, domain::sdlc::Stage::Clarification);
    assert_eq!(frozen.next_state.status, None);
    assert_eq!(frozen.confirmation.stage, domain::sdlc::Stage::Draft);
    assert_eq!(frozen.receipt.entry.state.stage, LifecycleStage::Draft);
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
}

#[test]
fn pm_accepts_real_initial_materials_without_invented_technical_requirements() {
    for stage in [LifecycleStage::Draft, LifecycleStage::Backlog] {
        let (mut state, actor, mut prepared, mut command) = fixture(LifecycleStatus::Ready);
        state.stage = stage.canonical();
        prepared.state.stage = stage;
        prepared.entry = Some(CurrentStageEntry::derive(prepared.state, false).unwrap());
        command.requirements.scope.clear();
        command.requirements.scenarios.clear();
        command.requirements.acceptance_criteria.clear();
        command.requirements.checklist.clear();
        command.requirements.prerequisites.clear();
        assert!(prepare_materials(&state, &actor, &prepared, &command, allocation()).is_ok());
        // Missing technical requirements remain a real blocker at a technical stage.
        state.stage = domain::sdlc::Stage::Analysis;
        prepared.state.stage = LifecycleStage::Analysis;
        prepared.entry = Some(CurrentStageEntry::derive(prepared.state, false).unwrap());
        assert!(prepare_materials(&state, &actor, &prepared, &command, allocation()).is_err());
        state.stage = stage.canonical();
        prepared.state.stage = stage;
        prepared.entry = Some(CurrentStageEntry::derive(prepared.state, false).unwrap());
        command.requirements.checklist.push(" ".into());
        assert!(prepare_materials(&state, &actor, &prepared, &command, allocation()).is_err());
    }
}

#[test]
fn an_explicit_user_pat_can_confirm_its_own_inputs_without_a_fake_browser_session() {
    let (state, mut actor, prepared, command) = fixture(LifecycleStatus::Ready);
    actor.human_session = false;
    assert!(actor.trusted_human);
    assert!(prepare_materials(&state, &actor, &prepared, &command, allocation()).is_ok());
    // A service principal never becomes a person by claiming a browser session.
    actor.trusted_human = false;
    actor.human_session = true;
    assert!(prepare_materials(&state, &actor, &prepared, &command, allocation()).is_err());
}

#[test]
fn owner_observations_cannot_follow_changed_inputs_or_extend_their_original_age() {
    use app::{
        sdlc_configuration::MatchedConfiguration, task_handoff_observations::MatchedHandoffInputs,
    };
    let (_, _, prepared, _) = fixture(LifecycleStatus::Ready);
    let selection = MatchedConfiguration::selection_hash(
        prepared
            .entry
            .as_ref()
            .unwrap()
            .route(prepared.routing.as_ref().unwrap()),
        prepared.entry.unwrap().role.key(),
        prepared.entry.unwrap().mode.key(),
    )
    .unwrap();
    let fleet = || MatchedConfiguration {
        selection_sha256: selection.clone(),
        observation_ref: Uuid::new_v4(),
        observed_at: Utc::now(),
    };
    let observed_source = prepared.source.as_ref().unwrap().verification.clone();
    let foreign_configuration = MatchedConfiguration {
        selection_sha256: "f".repeat(64),
        observation_ref: Uuid::new_v4(),
        observed_at: Utc::now(),
    };
    assert!(
        MatchedHandoffInputs::bind(&prepared, foreign_configuration, observed_source.clone())
            .is_err()
    );
    let matched = MatchedHandoffInputs::bind(&prepared, fleet(), observed_source.clone()).unwrap();
    assert!(matched.ensure_for(&prepared).is_ok());
    for field in ["revision", "materials", "routing", "source", "owner"] {
        let mut changed = prepared.clone();
        match field {
            "revision" => changed.state.revision += 1,
            "materials" => changed.issue_materials_sha256 = "f".repeat(64),
            "routing" => changed.routing.as_mut().unwrap().version += 1,
            "source" => changed.source.as_mut().unwrap().source_revision += 1,
            "owner" => changed.owner_subject = "another-human".into(),
            _ => unreachable!(),
        }
        assert!(matched.ensure_for(&changed).is_err(), "{field}");
    }
    let expired = MatchedConfiguration {
        selection_sha256: selection.clone(),
        observation_ref: Uuid::new_v4(),
        observed_at: Utc::now() - chrono::Duration::seconds(6),
    };
    assert!(MatchedHandoffInputs::bind(&prepared, expired, observed_source.clone()).is_err());
    let mut expired_source = observed_source.clone();
    expired_source.observed_at = Utc::now() - chrono::Duration::seconds(31);
    assert!(MatchedHandoffInputs::bind(&prepared, fleet(), expired_source).is_err());
    let mut foreign = observed_source;
    foreign.repository_id = Uuid::new_v4();
    assert!(MatchedHandoffInputs::bind(&prepared, fleet(), foreign).is_err());
}
