use super::*;
use domain::sdlc_lifecycle::ArtifactReceipt;
use serde_json::json;

struct Fixture {
    state: TaskState,
    reservation: PmDraftReservation,
    input: PmDraftInputResponse,
    lease: ExecutionLeaseReceipt,
    actor: Principal,
    config: SdlcConfig,
    command: BootstrapRetryCommand,
    readback: BootstrapFailureReadback,
    now: DateTime<Utc>,
}
fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}
fn fixture() -> Fixture {
    let now = DateTime::parse_from_rfc3339("2026-10-21T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let state: TaskState = serde_json::from_value(json!({
        "tracker_instance_id":"tracker","project_id":id(1),"task_id":id(2),"root_task_id":id(2),
        "owner_subject":"owner","stage":"Draft","status":"InProgress","draft_submode":"brainstorming",
        "flow":"single-task-v1","waiting_reason":"waiting_for_pm","confirmation_revision":null,
        "assignment":{"assignment_id":id(3),"execution_id":id(4),"agent_id":id(5),"version":1,"machine_subject":"pm"},
        "questions":[],"revisions":[],"evidence":[],"confirmations":[],"preserved":{"original":"history"}
    })).unwrap();
    let hash =
        crate::sdlc::pm_draft_input_hash("Service Pulse", "Synthetic initial request").unwrap();
    let reservation: PmDraftReservation = serde_json::from_value(json!({
        "contract_version":1,"variant":"pm_draft_reserved","binding":{"tracker_instance_id":"tracker",
        "project_id":id(1),"task_id":id(2),"root_task_id":id(2),"owner_subject":"owner"},
        "owner_cas":{"expected_version":0,"version":1},"assignment":state.assignment,
        "execution":{"ordinal":"12","key":"SDLC-12"},"input":{"snapshot_ref":id(6),"sha256":hash},
        "assignment_operation_key":format!("pm-draft:{}",id(3)),"admission_state":"reserved","dispatch_allowed":false
    })).unwrap();
    let input = PmDraftInputResponse {
        contract_version: 1,
        tracker_instance_id: "tracker".into(),
        project_id: id(1),
        task_id: id(2),
        root_task_id: id(2),
        owner_subject: "owner".into(),
        input: PmDraftInput {
            snapshot_ref: id(6),
            title: "Service Pulse".into(),
            description: "Synthetic initial request".into(),
            sha256: hash,
        },
    };
    let fence = MachineFence {
        assignment_id: id(3),
        execution_id: id(4),
        agent_id: id(5),
        assignment_version: 1,
    };
    let lease = ExecutionLeaseReceipt {
        contract_version: 1,
        binding: reservation.binding.clone(),
        owner_version: 1,
        fence: fence.clone(),
        lease: ExecutionLease {
            lease_id: id(7),
            version: 4,
            holder_subject: "pm".into(),
            claimed_at: now - Duration::seconds(100),
            heartbeat_at: now - Duration::seconds(31),
            expires_at: now - Duration::seconds(1),
        },
        ttl_seconds: 30,
        heartbeat_seconds: 10,
        dispatch_allowed: false,
    };
    let command = BootstrapRetryCommand {
        expected_owner_version: 1,
        fence,
        lease_id: id(7),
        expected_lease_version: 4,
        creation_operation_id: id(8),
        previous_session_run_id: id(8),
        next_session_run_id: id(10),
        previous_native_run_ref: "run_original_failed".into(),
        chat_id: id(9),
        barrier_id: id(11),
        predecessor: WorkflowBootstrapPredecessor {
            task_id: 1,
            assignment_operation_key: reservation.assignment_operation_key.clone(),
            assignment_revision: 1,
            attempt: 1,
            finalized_binding: Some(FinalizedBootstrapBinding {
                bind_operation_key: "original-bind".into(),
                bind_request_sha256: "b".repeat(64),
                binding_ref: "fixture://finalized-binding".into(),
                session_run_id: id(8),
                native_run_ref: "run_original_failed".into(),
            }),
        },
        idempotency_key: "original-technical-retry".into(),
    };
    let readback = BootstrapFailureReadback {
        contract_version: 1,
        observed_at: now,
        identity: identity(&reservation),
        owner_subject: "owner".into(),
        input: reservation.input.clone(),
        owner_version: 1,
        lease_id: id(7),
        lease_version: 4,
        creation_operation_id: id(8),
        session_run_id: id(8),
        native_run_ref: command.previous_native_run_ref.clone(),
        chat_id: id(9),
        barrier_id: id(11),
        next_session_run_id: id(10),
        terminal: Some(BootstrapNativeTerminal::Failed),
        predecessor: command.predecessor.clone(),
        initial_phase_id: 1,
        current_phase_id: 1,
        workflow_status: "active".into(),
        first_instruction: BootstrapReadbackState::Absent,
        pm_enrollment: BootstrapReadbackState::Absent,
        generic_binding: BootstrapReadbackState::Present,
        business_result: BootstrapReadbackState::Absent,
        inflight_work: BootstrapReadbackState::Absent,
        terminal_receipt: ArtifactReceipt {
            reference: "fixture://native-terminal".into(),
            sha256: "a".repeat(64),
        },
        bootstrap_receipt: ArtifactReceipt {
            reference: "fixture://workflow-predecessor".into(),
            sha256: "b".repeat(64),
        },
    };
    Fixture {
        state,
        reservation,
        input,
        lease,
        command,
        readback,
        now,
        actor: Principal {
            subject: "scheduler".into(),
            human_session: false,
            trusted_human: false,
            scopes: ["task-tracker:write".into()].into(),
        },
        config: SdlcConfig {
            instance_id: "tracker".into(),
            orchestrator_subject: "pm".into(),
            verifier_subject: "verifier".into(),
            reservation_scheduler_subject: "scheduler".into(),
            single_task_project_ids: [id(1)].into(),
        },
    }
}
impl Fixture {
    fn source(&self) -> BootstrapRetrySource<'_> {
        BootstrapRetrySource {
            state: &self.state,
            reservation: &self.reservation,
            original_input: &self.input,
            lease: &self.lease,
            owner_version: 1,
            admission_state: "reserved",
            predecessor_consumed: false,
            delivery_owned: false,
            now: self.now,
        }
    }
    async fn plan(&self) -> Result<BootstrapRetryPlan, AppError> {
        let observation =
            BootstrapRetryService::trusted_observation(&self.command, self.readback.clone())?;
        BootstrapRetryService::prepare(
            self.source(),
            &self.actor,
            &self.config,
            &self.command,
            &observation,
        )
    }
}

#[tokio::test]
async fn renews_only_exact_expired_lease_preserving_assignment_input_history_and_claim() {
    let f = fixture();
    let before = serde_json::to_value(&f.lease).unwrap();
    let state = serde_json::to_value(&f.state).unwrap();
    let plan = f.plan().await.unwrap();
    let mut expected = before.clone();
    expected["lease"]["version"] = json!(5);
    expected["lease"]["heartbeat_at"] =
        json!(f.now.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true));
    expected["lease"]["expires_at"] =
        json!((f.now + Duration::seconds(30)).to_rfc3339_opts(chrono::SecondsFormat::Nanos, true));
    assert_eq!(
        serde_json::to_value(&plan.operation.result).unwrap(),
        expected
    );
    assert_eq!(serde_json::to_value(&f.lease).unwrap(), before);
    assert_eq!(serde_json::to_value(&f.state).unwrap(), state);
    assert_eq!(
        plan.expected_state_sha256,
        crate::sdlc::canonical_hash(&f.state).unwrap()
    );
    assert_eq!(
        plan.expected_reservation_sha256,
        crate::sdlc::canonical_hash(&f.reservation).unwrap()
    );
    assert_eq!(plan.previous_session_run_id, id(8));
    assert_eq!(plan.next_session_run_id, id(10));
    assert_eq!(plan.chat_id, id(9));
    assert!(!plan.operation.result.dispatch_allowed);
}

#[tokio::test]
async fn human_owner_pm_and_extra_scopes_never_authorize_a_retry_plan() {
    for case in 0..5 {
        let mut f = fixture();
        match case {
            0 => f.actor.subject = "owner".into(),
            1 => f.actor.subject = "pm".into(),
            2 => f.actor.human_session = true,
            3 => {
                f.actor.scopes.insert("task-tracker:read".into());
            }
            _ => f.config.reservation_scheduler_subject.clear(),
        }
        assert!(f.plan().await.is_err());
    }
}

#[tokio::test]
async fn unknown_pending_accepted_step_enrollment_binding_or_inflight_work_is_not_absence() {
    for state in [
        BootstrapReadbackState::Present,
        BootstrapReadbackState::Pending,
        BootstrapReadbackState::Unknown,
    ] {
        for case in 0..4 {
            let mut f = fixture();
            match case {
                0 => f.readback.first_instruction = state,
                1 => f.readback.pm_enrollment = state,
                2 => f.readback.business_result = state,
                _ => f.readback.inflight_work = state,
            }
            assert!(f.plan().await.is_err(), "case {case}");
        }
    }
    for terminal in [None] {
        let mut f = fixture();
        f.readback.terminal = terminal;
        assert!(f.plan().await.is_err());
    }
}

#[tokio::test]
async fn all_identity_input_run_barrier_and_receipt_coordinates_require_trusted_exact_readback() {
    for case in 0..27 {
        let mut f = fixture();
        match case {
            0 => f.readback.identity.tracker_instance_ref = "foreign".into(),
            1 => f.readback.identity.tracker_project_ref = id(99).to_string(),
            2 => f.readback.identity.task_ref = id(99).to_string(),
            3 => f.readback.identity.root_ref = id(99).to_string(),
            4 => f.readback.identity.agent_ref = id(99).to_string(),
            5 => f.readback.identity.assignment_ref = id(99).to_string(),
            6 => f.readback.identity.execution_ref = id(99).to_string(),
            7 => f.readback.identity.assignment_revision = 2,
            8 => f.readback.identity.assignment_operation_key = "foreign".into(),
            9 => f.readback.identity.task = "SDLC-99".into(),
            10 => f.readback.session_run_id = id(99),
            11 => f.readback.native_run_ref = "foreign".into(),
            12 => f.readback.chat_id = id(99),
            13 => f.readback.barrier_id = id(99),
            14 => f.readback.owner_version = 2,
            15 => f.readback.lease_id = id(99),
            16 => f.readback.lease_version = 3,
            17 => f.readback.predecessor.task_id = 99,
            18 => f.readback.predecessor.assignment_revision = 2,
            19 => f.readback.predecessor.attempt = 2,
            20 => {
                f.readback
                    .predecessor
                    .finalized_binding
                    .as_mut()
                    .unwrap()
                    .bind_request_sha256 = "c".repeat(64)
            }
            21 => f.readback.owner_subject = "foreign".into(),
            22 => f.readback.input.snapshot_ref = id(99),
            23 => f.readback.input.sha256 = "c".repeat(64),
            24 => f.readback.next_session_run_id = id(99),
            25 => f.readback.creation_operation_id = id(99),
            _ => f.readback.contract_version = 2,
        }
        assert!(f.plan().await.is_err(), "case {case}");
    }
}

#[tokio::test]
async fn successor_active_lease_stale_observation_and_consumed_predecessor_block_fresh_plan() {
    let f = fixture();
    let observation =
        BootstrapRetryService::trusted_observation(&f.command, f.readback.clone()).unwrap();
    for case in 0..6 {
        let mut source = f.source();
        match case {
            0 => source.owner_version = 2,
            1 => source.admission_state = "completed",
            2 => source.predecessor_consumed = true,
            3 => source.delivery_owned = true,
            4 => source.now = f.now + Duration::seconds(31),
            _ => source.now = f.lease.lease.heartbeat_at,
        }
        assert!(
            BootstrapRetryService::prepare(source, &f.actor, &f.config, &f.command, &observation)
                .is_err(),
            "case {case}"
        );
    }
}

#[tokio::test]
async fn changed_business_input_lease_claim_or_assignment_cannot_be_normalized() {
    for case in 0..13 {
        let mut f = fixture();
        match case {
            0 => f.state.owner_subject = "foreign".into(),
            1 => f.state.project_id = id(99),
            2 => f.state.stage = Stage::Analysis,
            3 => f.state.confirmation_revision = Some(1),
            4 => f.state.draft_submode = Some(DraftSubmode::AwaitingConfirmation),
            5 => f.state.assignment.as_mut().unwrap().version = 2,
            6 => f.input.input.description.push_str("changed"),
            7 => f.input.input.snapshot_ref = id(99),
            8 => f.lease.lease.holder_subject = "foreign".into(),
            9 => f.lease.lease.version = 5,
            10 => f.lease.lease.expires_at += Duration::seconds(1),
            11 => f.lease.lease.heartbeat_at = f.lease.lease.claimed_at - Duration::seconds(1),
            _ => f.config.single_task_project_ids.clear(),
        }
        assert!(f.plan().await.is_err(), "case {case}");
    }
}

#[tokio::test]
async fn original_key_lost_ack_replay_returns_exact_receipt_without_reading_or_issuing_work() {
    let mut f = fixture();
    let operation = f.plan().await.unwrap().operation;
    f.now += Duration::hours(2);
    let replay = BootstrapRetryService::replay(
        &f.actor,
        &f.config,
        &f.command,
        (&f.reservation, &f.lease),
        ("scheduler", &operation),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(replay).unwrap(),
        serde_json::to_value(&operation.result).unwrap()
    );
}

#[tokio::test]
async fn same_key_modified_payload_actor_or_saved_receipt_is_rejected() {
    for case in 0..9 {
        let mut f = fixture();
        let mut saved = f.plan().await.unwrap().operation;
        let actor = if case == 0 { "foreign" } else { "scheduler" };
        match case {
            1 => f.command.next_session_run_id = id(99),
            2 => {
                f.command
                    .predecessor
                    .finalized_binding
                    .as_mut()
                    .unwrap()
                    .bind_request_sha256 = "c".repeat(64)
            }
            3 => f.command.previous_native_run_ref = "foreign".into(),
            4 => saved.result.lease.claimed_at += Duration::seconds(1),
            5 => saved.result.lease.version += 1,
            6 => saved.result.fence.assignment_version = 2,
            7 => saved.result.dispatch_allowed = true,
            8 => f.command.expected_lease_version = i64::MAX,
            _ => {}
        }
        assert!(
            BootstrapRetryService::replay(
                &f.actor,
                &f.config,
                &f.command,
                (&f.reservation, &f.lease),
                (actor, &saved)
            )
            .is_err(),
            "case {case}"
        );
    }
}

#[tokio::test]
async fn finalized_predecessor_accepts_all_four_native_terminals_without_business_progress() {
    for terminal in [
        BootstrapNativeTerminal::Completed,
        BootstrapNativeTerminal::Failed,
        BootstrapNativeTerminal::Cancelled,
        BootstrapNativeTerminal::Stopped,
    ] {
        let mut f = fixture();
        f.readback.terminal = Some(terminal);
        assert!(f.plan().await.is_ok());
        f.readback.business_result = BootstrapReadbackState::Present;
        assert!(f.plan().await.is_err());
    }
}

#[tokio::test]
async fn finalized_or_unbound_cursor_must_be_exact_never_pending_or_unknown() {
    for state in [
        BootstrapReadbackState::Absent,
        BootstrapReadbackState::Pending,
        BootstrapReadbackState::Unknown,
    ] {
        let mut f = fixture();
        f.readback.generic_binding = state;
        assert!(f.plan().await.is_err());
    }
    let mut f = fixture();
    let finalized = f.command.predecessor.finalized_binding.clone();
    f.command.predecessor.finalized_binding = None;
    f.readback.predecessor = f.command.predecessor.clone();
    f.readback.generic_binding = BootstrapReadbackState::Absent;
    assert!(f.plan().await.is_ok());
    f.readback.generic_binding = BootstrapReadbackState::Present;
    assert!(f.plan().await.is_err());
    // A late original bind changes the authoritative predecessor after an
    // unbound request was frozen. It must not consume a lease using that request.
    f.readback.predecessor.finalized_binding = finalized;
    let lease_before = serde_json::to_value(&f.lease).unwrap();
    assert!(f.plan().await.is_err());
    assert_eq!(serde_json::to_value(&f.lease).unwrap(), lease_before);
}

#[tokio::test]
async fn readback_must_keep_initial_phase_status_and_valid_original_receipt_hashes() {
    for case in 0..9 {
        let mut f = fixture();
        match case {
            0 => f.readback.initial_phase_id = 0,
            1 => f.readback.current_phase_id = 2,
            2 => f.readback.workflow_status = "done".into(),
            3 => f.readback.terminal_receipt.sha256 = "bad".into(),
            4 => f.readback.bootstrap_receipt.reference.clear(),
            5 => f.readback.observed_at = f.now + Duration::seconds(1),
            6 => f.readback.observed_at = f.now - Duration::seconds(31),
            7 => {
                f.command
                    .predecessor
                    .finalized_binding
                    .as_mut()
                    .unwrap()
                    .native_run_ref = "foreign".into()
            }
            _ => {
                f.command
                    .predecessor
                    .finalized_binding
                    .as_mut()
                    .unwrap()
                    .session_run_id = id(99)
            }
        }
        assert!(f.plan().await.is_err(), "case {case}");
    }
}

#[test]
fn scheduler_post_has_no_proof_flags_receipts_or_optional_unknown_binding() {
    let f = fixture();
    let raw = serde_json::to_value(&f.command).unwrap();
    assert!(serde_json::from_value::<BootstrapRetryCommand>(raw.clone()).is_ok());
    for field in [
        "witness",
        "terminal",
        "no_enrollment",
        "no_step",
        "dispatch_allowed",
        "terminal_receipt",
        "bootstrap_receipt",
    ] {
        let mut bad = raw.clone();
        bad[field] = json!(true);
        assert!(serde_json::from_value::<BootstrapRetryCommand>(bad).is_err());
    }
    let mut bad = raw.clone();
    bad["predecessor"]
        .as_object_mut()
        .unwrap()
        .remove("finalized_binding");
    assert!(serde_json::from_value::<BootstrapRetryCommand>(bad).is_err());
    for field in [
        "creation_operation_id",
        "previous_session_run_id",
        "next_session_run_id",
        "barrier_id",
        "lease_id",
        "chat_id",
    ] {
        let mut bad = raw.clone();
        bad[field] = json!(Uuid::nil());
        assert!(serde_json::from_value::<BootstrapRetryCommand>(bad).is_err());
    }
    for terminal in ["running", "unknown", "succeeded"] {
        let mut bad = serde_json::to_value(&f.readback).unwrap();
        bad["terminal"] = json!(terminal);
        assert!(serde_json::from_value::<BootstrapFailureReadback>(bad).is_err());
    }
}
