//! Synthetic unit fixtures, never live owner/source verification evidence.
use super::*;
use domain::sdlc::{ConfirmCommand, Stage};
use serde_json::json;
use uuid::Uuid;

fn fixture() -> (
    TaskState,
    Confirmation,
    TaskRoutingSnapshot,
    ProjectSourcePolicy,
) {
    let now = Utc::now();
    let project = Uuid::new_v4();
    let task = Uuid::new_v4();
    let instance = Uuid::new_v4().to_string();
    let confirmation = Confirmation {
        id: Uuid::new_v4(),
        task_id: task,
        revision: 1,
        content_hash: "a".repeat(64),
        owner_subject: "central-owner".into(),
        created_at: now,
        stage: Stage::Backlog,
    };
    let state: TaskState = serde_json::from_value(json!({
        "tracker_instance_id":instance,"project_id":project,"task_id":task,"root_task_id":task,
        "owner_subject":"central-owner","stage":"Analysis","confirmation_revision":1,
        "assignment":null,"questions":[],"revisions":[],"confirmations":[confirmation],"evidence":[],"flow":"single-task-v1"
    }))
    .unwrap();
    let mut routes = json!({});
    for (i, role) in [
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
        routes[*role] = json!({"agent_id":Uuid::new_v4(),"fleet_config_revision":4,
            "instruction_profile":"single-task-v1","instruction_manifest_sha256":"b".repeat(64),
            "namespace_id":(i+1).to_string(),"namespace_name":format!("single-task-{suffix}"),
            "workflow_id":(i+11).to_string(),"workflow_key":format!("single-task-v1:{role}"),
            "profile":format!("single-task-v1-{suffix}"),"workflow_catalog_version":1,"workflow_catalog_sha256":"c".repeat(64)});
    }
    let routing: TaskRoutingSnapshot = serde_json::from_value(json!({
        "contract_version":2,"snapshot_id":Uuid::new_v4(),"tracker_instance_id":instance,
        "project_id":project,"task_id":task,"root_task_id":task,"confirmation_id":confirmation.id,
        "requirement_revision":1,"content_hash":confirmation.content_hash,"created_at":now,
        "policy":{"contract_version":2,"tracker_instance_id":instance,"project_id":project,
            "version":1,"routing_hash":crate::sdlc::canonical_hash(&routes).unwrap(),"routes":routes,
            "verification":"declared","native_ready":false,"dispatch_allowed":false,
            "author_subject":"central-owner","created_at":now}
    })).unwrap();
    let source = SourcePolicySpec {
        forge_instance_id: Uuid::new_v4(),
        repository_id: Uuid::new_v4(),
        base_ref: "refs/heads/main".into(),
        base_commit: "d".repeat(40),
    };
    let verification = VerifiedSource {
        public_name: "fixture/repository".into(),
        namespace: NamespaceRef {
            registry_instance_id: Uuid::new_v4(),
            namespace_id: Uuid::new_v4(),
        },
        forge_instance_id: source.forge_instance_id,
        repository_id: source.repository_id,
        base_ref: source.base_ref.clone(),
        base_commit: source.base_commit.clone(),
        default_ref: "refs/heads/main".into(),
        protected_refs: vec!["refs/heads/release".into()],
        observed_at: now,
    };
    let mut policy = ProjectSourcePolicy {
        contract_version: 1,
        tracker_instance_id: instance,
        project_id: project,
        source_revision: 1,
        source_hash: String::new(),
        source,
        verification,
        author_subject: "central-owner".into(),
        created_at: now,
    };
    policy.source_hash = sealed_hash(&policy).unwrap();
    (state, confirmation, routing, policy)
}

fn current(binding: &TaskSourceBinding, routing: &TaskRoutingSnapshot) -> LifecycleReadback {
    let now = Utc::now();
    let lease = Uuid::new_v4();
    let mut assignment = json!({"assignment_id":Uuid::new_v4(),"execution_id":Uuid::new_v4(),
        "task_id":binding.task_id,"owner_subject":binding.owner_subject,"role":"developer","stage":"Development",
        "cycle":0,"attempt":1,"agent_id":routing.policy.routes.developer.agent_id,"routing":routing,
        "previous_result_id":null,"fencing_token":1,"lease_id":lease});
    assignment["assignment_hash"] = json!(crate::sdlc::canonical_hash(&assignment).unwrap());
    serde_json::from_value(json!({"observed_at":now,"runtime_state":"running","receipt":{
        "task":{"task_id":binding.task_id,"owner_subject":binding.owner_subject,"version":3,"stage":"Development",
            "status":"InProgress","cycle":0,"transient_retries":0,"waiting_reason":null,"runtime":"running",
            "routing":routing,"previous_result_id":null},"assignment":assignment,
        "lease":{"lease_id":lease,"version":2,"holder_subject":"central-scheduler","claimed_at":now,
            "heartbeat_at":now,"expires_at":now+chrono::Duration::seconds(30)},
        "binding":{"chat_id":Uuid::new_v4(),"run_id":Uuid::new_v4(),"machine_subject":"central-machine",
            "dispatch_receipt":{"reference":"fixture://synthetic-dispatch","sha256":"e".repeat(64)}},"result":null
    }})).unwrap()
}

#[test]
fn owner_confirmation_freezes_exact_ref_and_hash_without_assignment_mutation() {
    let (state, confirmation, routing, policy) = fixture();
    let before = serde_json::to_value(&state).unwrap();
    let binding = freeze_binding(&state, &confirmation, &routing, &policy).unwrap();
    assert_eq!(
        binding.candidate_ref,
        format!("refs/heads/sdlc/{}", state.task_id)
    );
    assert_eq!(binding.base_commit, policy.source.base_commit);
    assert_eq!(binding.source_hash, sealed_hash(&binding).unwrap());
    assert_eq!(binding.task_id, binding.root_task_id);
    assert_eq!(serde_json::to_value(state).unwrap(), before);
    let live = current(&binding, &routing);
    assert!(matches!(
        readback(Some(binding), Some(live)).unwrap(),
        TaskSourceReadback::Bound { .. }
    ));
}

#[test]
fn default_protected_foreign_and_unconfirmed_source_are_rejected() {
    let (state, confirmation, routing, policy) = fixture();
    for candidate in [
        "main",
        "refs/tags/v1",
        "refs/heads/../other",
        "refs/heads/a.lock",
        "refs/heads/a\nrefs/heads/b",
        "refs/heads/@{bad}",
    ] {
        assert!(!branch_ref(candidate), "{candidate}");
    }
    for protected in [false, true] {
        let mut changed = policy.clone();
        if protected {
            changed
                .verification
                .protected_refs
                .push(candidate_ref(state.task_id));
        } else {
            changed.verification.default_ref = candidate_ref(state.task_id);
        }
        changed.source_hash = sealed_hash(&changed).unwrap();
        assert!(freeze_binding(&state, &confirmation, &routing, &changed).is_err());
    }
    let mut foreign = state.clone();
    foreign.root_task_id = Uuid::new_v4();
    assert!(freeze_binding(&foreign, &confirmation, &routing, &policy).is_err());
    foreign = state;
    foreign.confirmations.clear();
    assert!(freeze_binding(&foreign, &confirmation, &routing, &policy).is_err());
}

#[test]
fn forged_sha_or_namespace_and_stale_verification_never_become_verified() {
    let (_, _, _, policy) = fixture();
    let now = policy.verification.observed_at;
    validate_verification(
        &policy.source,
        &policy.verification.namespace,
        &policy.verification,
        now,
    )
    .unwrap();
    for field in [
        "namespace",
        "base_commit",
        "base_ref",
        "repository_id",
        "forge_instance_id",
        "observed_at",
    ] {
        let mut value = serde_json::to_value(&policy.verification).unwrap();
        value[field] = match field {
            "namespace" => {
                json!({"registry_instance_id":Uuid::new_v4(),"namespace_id":Uuid::new_v4()})
            }
            "base_commit" => json!("f".repeat(40)),
            "base_ref" => json!("refs/heads/other"),
            "observed_at" => json!(now - chrono::Duration::seconds(31)),
            _ => json!(Uuid::new_v4()),
        };
        let observed = serde_json::from_value(value).unwrap();
        assert!(
            validate_verification(
                &policy.source,
                &policy.verification.namespace,
                &observed,
                now
            )
            .is_err(),
            "{field}"
        );
    }
}

#[test]
fn missing_legacy_binding_and_inactive_context_are_typed_blocks() {
    assert!(matches!(
        readback(None, None).unwrap(),
        TaskSourceReadback::Blocked {
            reason: SourceBlockReason::MissingBinding,
            ..
        }
    ));
    let (state, confirmation, routing, policy) = fixture();
    let binding = freeze_binding(&state, &confirmation, &routing, &policy).unwrap();
    assert!(matches!(
        readback(Some(binding.clone()), None).unwrap(),
        TaskSourceReadback::Blocked {
            reason: SourceBlockReason::MissingCurrentExecution,
            ..
        }
    ));
    let original = current(&binding, &routing);
    let mut changed = original.clone();
    changed.receipt.task.runtime = RuntimeState::Unknown;
    assert!(matches!(
        readback(Some(binding.clone()), Some(changed)).unwrap(),
        TaskSourceReadback::Blocked {
            reason: SourceBlockReason::ExecutionNotRunning,
            ..
        }
    ));
    changed = original.clone();
    changed.observed_at += chrono::Duration::seconds(31);
    assert!(matches!(
        readback(Some(binding.clone()), Some(changed)).unwrap(),
        TaskSourceReadback::Blocked {
            reason: SourceBlockReason::LeaseExpired,
            ..
        }
    ));
    changed = original;
    changed.receipt.task.routing.requirement_revision += 1;
    assert!(matches!(
        readback(Some(binding), Some(changed)).unwrap(),
        TaskSourceReadback::Blocked {
            reason: SourceBlockReason::SourceContextChanged,
            ..
        }
    ));
}

#[test]
fn legacy_confirmation_hash_is_unchanged_and_machine_cannot_choose_source() {
    let raw = json!({"content_hash":"a".repeat(64),"idempotency_key":"old-key"});
    let legacy: ConfirmCommand = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(serde_json::to_value(legacy).unwrap(), raw);
    let mut arbitrary = raw.clone();
    arbitrary["candidate_ref"] = json!("refs/heads/not-owned");
    assert!(serde_json::from_value::<ConfirmCommand>(arbitrary).is_err());
    let machine = Principal {
        subject: "central-machine".into(),
        human_session: false,
        trusted_human: true,
        scopes: ["task-tracker:write".into()].into_iter().collect(),
    };
    assert!(human_owner(&machine).is_err());
    let task = Uuid::new_v4();
    let assignment = Uuid::new_v4();
    let execution = Uuid::new_v4();
    let agent = Uuid::new_v4();
    let mut machine = machine;
    machine.scopes = [
        "task-tracker:read".into(),
        format!("task-tracker:sdlc:delivery:{task}:{assignment}:{execution}:{agent}:1"),
    ]
    .into_iter()
    .collect();
    assert!(
        crate::sdlc_lifecycle::delivery_request_policy(
            &machine,
            "GET",
            &format!("/api/v1/issues/{task}/sdlc/source-binding")
        )
        .is_ok()
    );
    for (method, path) in [
        ("POST", format!("/api/v1/issues/{task}/sdlc/lifecycle")),
        (
            "GET",
            format!("/api/v1/issues/{}/sdlc/source-binding", Uuid::new_v4()),
        ),
        (
            "POST",
            format!("/api/v1/projects/{}/sdlc/source-policy", Uuid::new_v4()),
        ),
    ] {
        assert!(crate::sdlc_lifecycle::delivery_request_policy(&machine, method, &path).is_err());
    }
}

#[tokio::test]
async fn missing_registered_source_counterpart_is_not_verified_by_metadata_or_sha_syntax() {
    let (_, _, _, policy) = fixture();
    let reader = domain::task_repositories::UnavailableTaskRepositories;
    let result = domain::task_repositories::TaskRepositoriesReader::verify_source(
        &reader,
        &policy.verification.namespace,
        &policy.source,
    )
    .await;
    assert!(
        matches!(result, Err(shared::AppError::Unavailable(ref reason))
        if reason == "forge_source_verification_counterpart_missing")
    );
}
