use super::*;
use chrono::{Duration, TimeZone, Utc};
use domain::sdlc::{MachineFence, TaskState};
use domain::sdlc_critical_pm_reservation::CriticalPmReservationResult;
use domain::sdlc_execution_lease::{ExecutionLease, ExecutionLeaseReceipt};
use serde_json::json;

struct ReadFixture {
    state: TaskState,
    actor: Principal,
    blocker: CriticalBlockerReceipt,
    review: CriticalPmReservationResult,
    lease: ExecutionLeaseReceipt,
    now: chrono::DateTime<Utc>,
}

impl ReadFixture {
    fn new() -> Self {
        let now = Utc.with_ymd_and_hms(2026, 10, 14, 12, 0, 0).unwrap();
        let task = Uuid::new_v4();
        let project = Uuid::new_v4();
        let assignment = domain::sdlc::PmAssignment {
            assignment_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            agent_id: Uuid::new_v4(),
            version: 18,
            machine_subject: "synthetic-pm".into(),
        };
        let state: TaskState = serde_json::from_value(json!({
            "tracker_instance_id":"synthetic-tracker","project_id":project,"task_id":task,
            "root_task_id":task,"owner_subject":"synthetic-owner","stage":"Draft",
            "status":"Ready","draft_submode":"brainstorming","confirmation_revision":null,
            "assignment":assignment,"questions":[],"revisions":[],"confirmations":[],"evidence":[],
            "flow":"single-task-v1"
        }))
        .unwrap();
        let technical_assignment = Uuid::new_v4();
        let technical_execution = Uuid::new_v4();
        let proposal: SubmitCriticalBlocker = serde_json::from_value(json!({
            "fence":{"assignment_id":technical_assignment,"execution_id":technical_execution,
                "lease_id":Uuid::new_v4(),"fencing_token":5,"expected_lease_version":3},
            "requirement_revision":4,"content_hash":"a".repeat(64),"kind":"requirements_contradiction",
            "question":"Which retention rule is authoritative?","rationale":"The two rules conflict.",
            "options":[],"recommendation":null,"idempotency_key":"original-question",
            "evidence":[{"receipt":{"reference":format!("tracker-attachment:{}",Uuid::new_v4()),"sha256":"b".repeat(64)},
                "binding":{"assignment_id":technical_assignment,"execution_id":technical_execution,"role":"developer"}}]
        })).unwrap();
        proposal.validate(DeliveryRole::Developer).unwrap();
        let mut blocker: CriticalBlockerReceipt = serde_json::from_value(json!({
            "blocker_id":Uuid::new_v4(),"version":3,"tracker_instance_id":state.tracker_instance_id,
            "project_id":project,"task_id":task,"root_task_id":task,"owner_subject":state.owner_subject,
            "producer_subject":"synthetic-developer","agent_id":Uuid::new_v4(),"role":"developer",
            "assignment_hash":"c".repeat(64),"chat_id":Uuid::new_v4(),"run_id":Uuid::new_v4(),
            "routing_snapshot_id":Uuid::new_v4(),"routing_generation":1,
            "blocker_sha256":crate::sdlc::canonical_hash(&proposal).unwrap(),"proposal":proposal,
            "state":"in_pm_review","pm_review":{"assignment_id":assignment.assignment_id,"execution_id":assignment.execution_id},
            "resolution":null,"technical_terminal":null,"created_at":now
        })).unwrap();
        blocker.technical_terminal = Some(RuntimeTerminalObservation {
            assignment_hash: blocker.assignment_hash.clone(),
            chat_id: blocker.chat_id,
            run_id: blocker.run_id,
            state: RuntimeTerminalState::Cancelled,
            receipt: ArtifactReceipt {
                reference: "fixture://synthetic-terminal-not-live-proof".into(),
                sha256: "e".repeat(64),
            },
        });
        let review: CriticalPmReservationResult = serde_json::from_value(json!({
            "purpose":"critical_business_review","reservation":{"contract_version":1,"variant":"pm_draft_reserved",
                "binding":{"tracker_instance_id":state.tracker_instance_id,"project_id":project,
                    "task_id":task,"root_task_id":task,"owner_subject":state.owner_subject},
                "assignment":assignment,"owner_cas":{"expected_version":17,"version":18},
                "execution":{"ordinal":"23","key":"SDLC-23"},
                "input":{"snapshot_ref":Uuid::new_v4(),"sha256":"d".repeat(64)},
                "assignment_operation_key":"review-reservation-original","admission_state":"reserved","dispatch_allowed":false},
            "context":{"blocker_id":blocker.blocker_id,"blocker_sha256":blocker.blocker_sha256,
                "blocker_version":3,"requirements":{"revision":4,"content_hash":"a".repeat(64)}}
        })).unwrap();
        let actor = Principal {
            subject: assignment.machine_subject.clone(),
            human_session: false,
            trusted_human: false,
            scopes: [
                "task-tracker:read".into(),
                "task-tracker:write".into(),
                assignment.scope(task),
            ]
            .into(),
        };
        let lease = ExecutionLeaseReceipt {
            contract_version: 1,
            binding: review.reservation.binding.clone(),
            owner_version: 18,
            fence: MachineFence {
                assignment_id: assignment.assignment_id,
                execution_id: assignment.execution_id,
                agent_id: assignment.agent_id,
                assignment_version: assignment.version,
            },
            lease: ExecutionLease {
                lease_id: Uuid::new_v4(),
                version: 2,
                holder_subject: actor.subject.clone(),
                claimed_at: now - Duration::seconds(10),
                heartbeat_at: now,
                expires_at: now + Duration::seconds(30),
            },
            ttl_seconds: 30,
            heartbeat_seconds: 10,
            dispatch_allowed: false,
        };
        Self {
            state,
            actor,
            blocker,
            review,
            lease,
            now,
        }
    }

    fn allowed(&self) -> Result<(), AppError> {
        authorize_critical_pm_blocker_read(
            &self.state,
            &self.actor,
            &self.blocker,
            &self.review,
            &self.review.reservation,
            if self.state.configured_cli() {
                None
            } else {
                Some(&self.lease)
            },
            self.now,
        )
    }
}

#[test]
fn configured_pm_reads_only_the_selected_current_blocker_without_native_authority() {
    let mut f = ReadFixture::new();
    let subject = Uuid::new_v4().to_string();
    f.state
        .retained_fields
        .insert("interaction_protocol".into(), json!("configured-cli-v1"));
    f.state.assignment.as_mut().unwrap().machine_subject = subject.clone();
    f.review.reservation.assignment = f.state.assignment.clone().unwrap();
    f.review.reservation.interaction_protocol =
        Some(domain::sdlc::InteractionProtocol::ConfiguredCliV1);
    f.actor.subject = subject;
    f.actor.scopes = ["task-tracker:read".into(), "task-tracker:write".into()].into();
    f.lease.lease.expires_at = f.now - Duration::days(1);
    assert!(f.allowed().is_ok());
    for change in 0..4 {
        let mut blocker = f.blocker.clone();
        let mut actor = f.actor.clone();
        match change {
            0 => blocker.blocker_id = Uuid::new_v4(),
            1 => blocker.pm_review.as_mut().unwrap().execution_id = Uuid::new_v4(),
            2 => blocker.state = CriticalBlockerState::Resolved,
            _ => actor.subject = Uuid::new_v4().to_string(),
        }
        assert!(
            authorize_critical_pm_blocker_read(
                &f.state,
                &actor,
                &blocker,
                &f.review,
                &f.review.reservation,
                None,
                f.now,
            )
            .is_err()
        );
    }
    assert!(
        authorize_critical_pm_blocker_read(
            &f.state,
            &f.actor,
            &f.blocker,
            &f.review,
            &f.review.reservation,
            Some(&f.lease),
            f.now,
        )
        .is_err()
    );
}

#[test]
fn current_review_reads_only_its_blocker_in_all_draft_submodes_without_mutation() {
    let mut f = ReadFixture::new();
    for mode in [
        domain::sdlc::DraftSubmode::Brainstorming,
        domain::sdlc::DraftSubmode::Clarification,
        domain::sdlc::DraftSubmode::AwaitingConfirmation,
    ] {
        f.state.draft_submode = Some(mode);
        let before = json!({"state":f.state,"blocker":f.blocker,"review":f.review,"lease":f.lease});
        f.allowed().unwrap();
        assert_eq!(
            json!({"state":f.state,"blocker":f.blocker,"review":f.review,"lease":f.lease}),
            before
        );
    }
}

#[test]
fn old_pm_foreign_task_agent_execution_version_and_missing_read_scope_are_denied() {
    for mismatch in 0..8 {
        let mut f = ReadFixture::new();
        match mismatch {
            0 => {
                f.actor.scopes.remove("task-tracker:read");
            }
            1 => f.actor.human_session = true,
            2 => f.actor.subject = "foreign".into(),
            _ => {
                let mut wrong = f.state.assignment.clone().unwrap();
                let mut task = f.state.task_id;
                match mismatch {
                    3 => wrong.assignment_id = Uuid::new_v4(),
                    4 => wrong.execution_id = Uuid::new_v4(),
                    5 => wrong.agent_id = Uuid::new_v4(),
                    6 => wrong.version -= 1,
                    _ => task = Uuid::new_v4(),
                }
                f.actor.scopes = ["task-tracker:read".into(), wrong.scope(task)].into();
            }
        }
        assert!(f.allowed().is_err());
    }
}

#[test]
fn held_returned_resolved_foreign_and_unlinked_review_context_are_denied() {
    for mismatch in 0..12 {
        let mut f = ReadFixture::new();
        match mismatch {
            0 => f.blocker.state = CriticalBlockerState::Held,
            1 => f.blocker.state = CriticalBlockerState::AwaitingPmReview,
            2 => f.blocker.state = CriticalBlockerState::Resolved,
            3 => f.blocker.task_id = Uuid::new_v4(),
            4 => f.blocker.project_id = Uuid::new_v4(),
            5 => f.blocker.owner_subject = "foreign-owner".into(),
            6 => f.blocker.pm_review = None,
            7 => f.blocker.pm_review.as_mut().unwrap().execution_id = Uuid::new_v4(),
            8 => f.review.context.blocker_id = Uuid::new_v4(),
            9 => f.review.context.blocker_sha256 = "f".repeat(64),
            10 => f.review.context.requirements.revision += 1,
            _ => f.review.context.blocker_version -= 1,
        }
        assert!(f.allowed().is_err());
    }
}

#[test]
fn lease_expiry_future_time_wrong_holder_owner_and_every_fence_component_deny() {
    for mismatch in 0..12 {
        let mut f = ReadFixture::new();
        match mismatch {
            0 => f.now = f.lease.lease.expires_at,
            1 => f.now = f.lease.lease.heartbeat_at - Duration::nanoseconds(1),
            2 => f.lease.lease.holder_subject = "old-pm".into(),
            3 => f.lease.owner_version -= 1,
            4 => f.lease.fence.assignment_id = Uuid::new_v4(),
            5 => f.lease.fence.execution_id = Uuid::new_v4(),
            6 => f.lease.fence.agent_id = Uuid::new_v4(),
            7 => f.lease.fence.assignment_version -= 1,
            8 => f.lease.binding.owner_subject = "foreign-owner".into(),
            9 => f.lease.lease.lease_id = Uuid::nil(),
            10 => f.lease.lease.version = 0,
            _ => f.lease.lease.claimed_at = f.now + Duration::seconds(1),
        }
        assert!(f.allowed().is_err());
    }
    let f = ReadFixture::new();
    let mut changed = f.review.reservation.clone();
    changed.owner_cas.version += 1;
    assert!(
        authorize_critical_pm_blocker_read(
            &f.state,
            &f.actor,
            &f.blocker,
            &f.review,
            &changed,
            Some(&f.lease),
            f.now
        )
        .is_err()
    );
}

#[test]
fn only_canonical_blocker_detail_get_shape_is_allowed_never_operations_or_attachments() {
    let id = Uuid::from_u128(0xabcdef).to_string();
    assert!(is_pm_critical_blocker_read(
        "GET",
        &["critical-blockers", &id]
    ));
    for method in ["POST", "PATCH", "DELETE"] {
        assert!(!is_pm_critical_blocker_read(
            method,
            &["critical-blockers", &id]
        ));
    }
    for parts in [
        vec!["critical-blockers"],
        vec!["critical-blockers", "returns"],
        vec!["critical-blockers", "pm-reviews"],
        vec!["critical-blockers", "operations", "original"],
        vec!["critical-blockers", "pm-reviews", "operations", "original"],
        vec!["critical-blockers", &id, ""],
        vec!["attachments", &id, "download"],
        vec!["critical-blockers", "00000000-0000-0000-0000-000000000000"],
    ] {
        assert!(!is_pm_critical_blocker_read("GET", &parts));
    }
    assert!(!is_pm_critical_blocker_read(
        "GET",
        &["critical-blockers", &id.to_uppercase()]
    ));
}
