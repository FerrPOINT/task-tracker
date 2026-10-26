use domain::sdlc::*;
use domain::sdlc_execution_lease::*;
use domain::sdlc_pm_draft::PmDraftReservation;
use shared::AppError;
use uuid::Uuid;

impl crate::sdlc::SdlcService {
    pub async fn resume_pm_execution_lease(
        &self,
        task: Uuid,
        actor: &Principal,
        command: ResumePmExecutionLease,
    ) -> Result<(ExecutionLeaseReceipt, bool), AppError> {
        validate_command(&command)?;
        if let Some(recovery) = &command.recovery {
            if let Some(receipt) = self
                .repository
                .preflight_settled_pm_recovery(task, actor, &command)
                .await?
            {
                return Ok((receipt, true));
            }
            let reader = self.fleet_configuration.as_ref().ok_or_else(|| {
                AppError::Unavailable("Fleet PM readback is not configured".into())
            })?;
            let witness = reader
                .read_settled_pm_input_recovery(command.previous_run_id, recovery.barrier_id)
                .await?;
            return self
                .repository
                .recover_settled_pm_execution_lease(task, actor, command, witness)
                .await;
        }
        self.repository
            .resume_pm_execution_lease(task, actor, command)
            .await
    }
    pub async fn pm_resume_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<ExecutionLeaseOperation, AppError> {
        self.repository.pm_resume_operation(task, actor, key).await
    }
}

fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

pub fn validate_command(c: &ResumePmExecutionLease) -> Result<(), AppError> {
    crate::sdlc::validate_key(&c.idempotency_key)?;
    if let Some(recovery) = &c.recovery {
        crate::sdlc::validate_key(&recovery.original_key)?;
        crate::sdlc::validate_key(&recovery.predecessor_key)?;
        if recovery.barrier_id.is_nil()
            || !hash(&recovery.original_request_sha256)
            || !hash(&recovery.predecessor_request_sha256)
            || c.idempotency_key == recovery.original_key
            || c.idempotency_key == recovery.predecessor_key
            || (recovery.predecessor_key == recovery.original_key
                && recovery.predecessor_request_sha256 != recovery.original_request_sha256)
        {
            return Err(AppError::validation("invalid settled PM recovery anchor"));
        }
    }
    let p = &c.checkpoint;
    let f = &p.fence;
    if [
        c.lease_id,
        c.input.input_id(),
        p.checkpoint_id,
        c.previous_run_id,
        f.assignment_id,
        f.execution_id,
        f.agent_id,
    ]
    .iter()
    .any(Uuid::is_nil)
        || [
            c.expected_owner_version,
            c.expected_lease_version,
            p.checkpoint_revision,
            f.assignment_version,
        ]
        .iter()
        .any(|n| !(1..=MAX_SAFE_VERSION).contains(n))
        || !hash(&p.checkpoint_sha256)
        || !match (p.requirement_revision, p.content_hash.as_deref()) {
            (None, None) => true,
            (Some(n), Some(h)) => (1..=MAX_SAFE_VERSION).contains(&n) && hash(h),
            _ => false,
        }
    {
        return Err(AppError::validation(
            "invalid PM resume identity/checkpoint",
        ));
    }
    crate::sdlc_lifecycle::validate_artifact(&c.previous_run_terminal)
}

pub fn request_hash(c: &ResumePmExecutionLease) -> Result<String, AppError> {
    crate::sdlc::canonical_hash(
        &serde_json::json!({"operation":"resume_pm_execution_lease","payload":c}),
    )
}

pub fn validate_settled_witness(
    witness: &PmSettledInputRecoveryWitness,
    reservation: &PmDraftReservation,
    command: &ResumePmExecutionLease,
) -> Result<(), AppError> {
    validate_command(command)?;
    let recovery = command
        .recovery
        .as_ref()
        .ok_or_else(|| AppError::validation("settled PM recovery discriminator required"))?;
    if witness.contract_version != 1
        || witness.session_run_id != command.previous_run_id
        || witness.barrier_id != recovery.barrier_id
        || witness.identity != crate::sdlc_pm_bootstrap_retry::identity(reservation)
        || witness.request_sha256 != request_hash(command)?
        || serde_json::to_value(&witness.command).map_err(AppError::internal)?
            != serde_json::to_value(command).map_err(AppError::internal)?
    {
        return Err(AppError::conflict(
            "Fleet settled PM recovery barrier changed",
        ));
    }
    if let Some(budget) = &witness.manual_budget {
        validate_manual_budget(budget, command)?;
    }
    Ok(())
}

fn validate_manual_budget(
    budget: &PmSettledInputManualBudget,
    command: &ResumePmExecutionLease,
) -> Result<(), AppError> {
    let recovery = command
        .recovery
        .as_ref()
        .ok_or_else(|| AppError::conflict("manual PM recovery budget requires a settled input"))?;
    if budget.contract_version != 1
        || budget.root_operation_id.is_nil()
        || budget.control_operation_id.is_nil()
        || !budget
            .retry_operation_key
            .strip_prefix("fleet-pm-resume-retry:")
            .is_some_and(hash)
        || !hash(&budget.retry_request_sha256)
        || !hash(&budget.original_resume_request_sha256)
        || !hash(&budget.prefix_sha256)
        || budget.original_resume_request_sha256 != recovery.original_request_sha256
        || budget.allowance != 3
        || budget.prefix_count < 3
        || budget
            .prefix_count
            .checked_add(budget.allowance)
            .is_none_or(|end| end > MAX_SAFE_VERSION as usize)
    {
        return Err(AppError::conflict(
            "Fleet manual PM recovery budget changed",
        ));
    }
    // Fleet validates its own audit/prefix hash. Its root/control IDs are not
    // Tracker execution IDs or necessarily the previous native run ID.
    Ok(())
}

pub fn validate_settled_budget_position(
    witness: &PmSettledInputRecoveryWitness,
    attempts: usize,
) -> Result<(), AppError> {
    let allowed = if let Some(budget) = &witness.manual_budget {
        validate_manual_budget(budget, &witness.command)?;
        budget.prefix_count <= attempts && attempts < budget.prefix_count + budget.allowance
    } else {
        attempts < 3
    };
    if !allowed {
        return Err(AppError::conflict(
            "settled PM recovery attempt limit reached",
        ));
    }
    Ok(())
}

pub fn validate_context(
    state: &TaskState,
    r: &PmDraftReservation,
    c: &ResumePmExecutionLease,
) -> Result<(), AppError> {
    state.require_root()?;
    validate_command(c)?;
    let a = state
        .assignment
        .as_ref()
        .ok_or_else(|| AppError::conflict("current PM assignment absent"))?;
    let p = &c.checkpoint;
    let f = &p.fence;
    if r.assignment != *a
        || r.binding.task_id != state.task_id
        || r.binding.root_task_id != state.root_task_id
        || r.binding.project_id != state.project_id
        || r.binding.owner_subject != state.owner_subject
        || r.binding.tracker_instance_id != state.tracker_instance_id
        || r.owner_cas.version != c.expected_owner_version
        || a.assignment_id != f.assignment_id
        || a.execution_id != f.execution_id
        || a.agent_id != f.agent_id
        || a.version != f.assignment_version
        || p.requirement_revision != state.current_revision()
        || p.content_hash.as_deref() != state.revisions.last().map(|r| r.content_hash.as_str())
    {
        return Err(AppError::conflict("stale PM resume binding/checkpoint"));
    }
    let stage_ok = match c.input {
        PmResumeInput::Answer { .. } | PmResumeInput::Feedback { .. } => {
            matches!(state.stage, Stage::Draft | Stage::Clarification)
        }
        PmResumeInput::Confirmation { .. } => {
            matches!(state.stage, Stage::Backlog | Stage::Analysis)
        }
    };
    if !stage_ok {
        return Err(AppError::conflict(
            "PM resume cannot reopen business lifecycle",
        ));
    }
    Ok(())
}

pub fn validate_answer(
    state: &TaskState,
    c: &ResumePmExecutionLease,
    answer: &Answer,
) -> Result<(), AppError> {
    let f = &c.checkpoint.fence;
    let q = state
        .questions
        .iter()
        .find(|q| q.id == answer.question_id)
        .ok_or_else(|| AppError::conflict("PM resume answer question absent"))?;
    if answer.id != c.input.input_id()
        || answer.author_subject != state.owner_subject
        || Some(answer.requirement_revision) != c.checkpoint.requirement_revision
        || q.task_id != state.task_id
        || q.root_task_id != state.task_id
        || q.version != answer.question_version
        || q.requirement_revision != answer.requirement_revision
        || q.state != QuestionState::Answered
        || q.checkpoint_id != Some(c.checkpoint.checkpoint_id)
        || q.assignment_id != f.assignment_id
        || q.execution_id != f.execution_id
        || q.agent_id != f.agent_id
        || q.assignment_version != f.assignment_version
        || q.answer
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(AppError::internal)?
            != Some(serde_json::to_value(answer).map_err(AppError::internal)?)
    {
        return Err(AppError::conflict("stale/foreign PM resume answer"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub(super) fn fixture() -> (TaskState, PmDraftReservation, ResumePmExecutionLease) {
        let task = Uuid::new_v4();
        let assignment = PmAssignment {
            assignment_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            agent_id: Uuid::new_v4(),
            version: 1,
            machine_subject: "pm".into(),
        };
        let state: TaskState = serde_json::from_value(json!({
            "tracker_instance_id":"resume-test","project_id":Uuid::new_v4(),
            "task_id":task,"root_task_id":task,"owner_subject":"owner","stage":"Draft",
            "confirmation_revision":null,"assignment":assignment,"questions":[],
            "revisions":[],"evidence":[],"confirmations":[]
        }))
        .unwrap();
        let reservation: PmDraftReservation = serde_json::from_value(json!({
            "contract_version":1,"variant":"pm_draft_reserved",
            "binding":{"tracker_instance_id":state.tracker_instance_id,"project_id":state.project_id,
                "task_id":task,"root_task_id":task,"owner_subject":state.owner_subject},
            "owner_cas":{"expected_version":0,"version":1},"assignment":assignment,
            "execution":{"ordinal":"1","key":"SDLC-1"},
            "input":{"snapshot_ref":Uuid::new_v4(),"sha256":"a".repeat(64)},
            "assignment_operation_key":"reserve","admission_state":"reserved","dispatch_allowed":false
        })).unwrap();
        let command = ResumePmExecutionLease {
            expected_owner_version: 1,
            lease_id: Uuid::new_v4(),
            expected_lease_version: 1,
            input: PmResumeInput::Feedback {
                input_id: Uuid::new_v4(),
            },
            checkpoint: PmResumeCheckpoint {
                checkpoint_id: Uuid::new_v4(),
                checkpoint_revision: 1,
                checkpoint_sha256: "b".repeat(64),
                fence: MachineFence {
                    assignment_id: assignment.assignment_id,
                    execution_id: assignment.execution_id,
                    agent_id: assignment.agent_id,
                    assignment_version: 1,
                },
                requirement_revision: None,
                content_hash: None,
            },
            previous_run_id: Uuid::new_v4(),
            previous_run_terminal: domain::sdlc_lifecycle::ArtifactReceipt {
                reference: "fixture://trusted-terminal-observation".into(),
                sha256: "c".repeat(64),
            },
            idempotency_key: "resume".into(),
            recovery: None,
        };
        (state, reservation, command)
    }

    #[test]
    fn resume_validation_requires_paired_hash_revision_and_safe_identity() {
        let (_, _, command) = fixture();
        assert!(validate_command(&command).is_ok());
        let value = serde_json::to_value(&command).unwrap();
        for (field, invalid) in [
            ("checkpoint_sha256", json!("A".repeat(64))),
            ("content_hash", json!("a".repeat(64))),
            ("requirement_revision", json!(1)),
        ] {
            let mut bad = value.clone();
            bad["checkpoint"][field] = invalid;
            assert!(validate_command(&serde_json::from_value(bad).unwrap()).is_err());
        }
        for field in ["expected_owner_version", "expected_lease_version"] {
            let mut bad = command.clone();
            if field == "expected_owner_version" {
                bad.expected_owner_version = 0;
            } else {
                bad.expected_lease_version = 0;
            }
            assert!(validate_command(&bad).is_err());
        }
        let mut bad = command.clone();
        bad.previous_run_id = Uuid::nil();
        assert!(validate_command(&bad).is_err());
        let mut bad = command.clone();
        bad.previous_run_terminal.reference.clear();
        assert!(validate_command(&bad).is_err());
        let mut paired = command.clone();
        paired.checkpoint.requirement_revision = Some(1);
        paired.checkpoint.content_hash = Some("a".repeat(64));
        assert!(validate_command(&paired).is_ok());
        assert_ne!(
            request_hash(&paired).unwrap(),
            request_hash(&command).unwrap()
        );
    }

    #[test]
    fn resume_context_is_exact_and_cannot_reopen_pm_business_writes() {
        let (state, reservation, command) = fixture();
        let before = serde_json::to_value(&state).unwrap();
        assert!(validate_context(&state, &reservation, &command).is_ok());
        for field in [
            "task_id",
            "root_task_id",
            "project_id",
            "owner_subject",
            "tracker_instance_id",
        ] {
            let mut bad = before.clone();
            bad[field] = if field.ends_with("id") {
                json!(Uuid::new_v4())
            } else {
                json!("foreign")
            };
            let bad: TaskState = serde_json::from_value(bad).unwrap();
            assert!(validate_context(&bad, &reservation, &command).is_err());
        }
        for stage in [
            "Backlog",
            "Analysis",
            "Architecture",
            "Development",
            "Review",
            "Testing",
            "Rework",
            "Deployment",
        ] {
            let mut bad = before.clone();
            bad["stage"] = json!(stage);
            let bad: TaskState = serde_json::from_value(bad).unwrap();
            assert!(validate_context(&bad, &reservation, &command).is_err());
        }
        let mut legacy = state.clone();
        legacy.stage = Stage::Clarification;
        assert!(validate_context(&legacy, &reservation, &command).is_ok());
        let mut closing = command.clone();
        closing.input = PmResumeInput::Confirmation {
            input_id: Uuid::new_v4(),
        };
        assert!(validate_context(&state, &reservation, &closing).is_err());
        for stage in [Stage::Backlog, Stage::Analysis] {
            let mut consented = state.clone();
            consented.stage = stage;
            assert!(validate_context(&consented, &reservation, &closing).is_ok());
            // This only validates a context; repository still requires real exact owner consent.
        }
        let mut stale = command.clone();
        stale.expected_owner_version += 1;
        assert!(validate_context(&state, &reservation, &stale).is_err());
        let mut stale = command.clone();
        stale.checkpoint.fence.assignment_version += 1;
        assert!(validate_context(&state, &reservation, &stale).is_err());
        let mut stale = command.clone();
        stale.checkpoint.requirement_revision = Some(1);
        stale.checkpoint.content_hash = Some("a".repeat(64));
        assert!(validate_context(&state, &reservation, &stale).is_err());
        assert_eq!(serde_json::to_value(state).unwrap(), before);
    }
}

#[cfg(test)]
#[path = "sdlc_settled_input_recovery_tests.rs"]
mod settled_input_tests;
