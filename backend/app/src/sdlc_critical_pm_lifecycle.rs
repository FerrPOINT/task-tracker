//! Pure review transitions. Repository replay/locks and actual terminal verification stay outside.
use chrono::Duration;
use domain::sdlc::*;
use domain::sdlc_critical_blockers::*;
use domain::sdlc_critical_pm_lifecycle::*;
use domain::sdlc_execution_lease::ExecutionLeaseReceipt;
use domain::sdlc_lifecycle::*;
use domain::sdlc_reservation::CompletePmDraft;
use domain::sdlc_routing::TaskRoutingSnapshot;
use serde::Serialize;
use serde_json::Value;
use shared::AppError;
use std::collections::HashSet;
use uuid::Uuid;

fn conflict() -> AppError {
    AppError::conflict("critical PM lifecycle source or transition differs")
}

fn value<T: Serialize>(data: &T) -> Result<Value, AppError> {
    serde_json::to_value(data).map_err(AppError::internal)
}

fn next(version: i64) -> Result<i64, AppError> {
    version
        .checked_add(1)
        .filter(|n| version > 0 && *n <= MAX_SAFE_VERSION)
        .ok_or_else(conflict)
}

fn bounded(version: i64) -> bool {
    (1..=MAX_SAFE_VERSION).contains(&version)
}

fn frame(source: &CriticalPmLifecycleSource<'_>) -> CriticalPmProjection {
    CriticalPmProjection {
        state: source.state.clone(),
        lifecycle: source.lifecycle.clone(),
        blocker: source.blocker.clone(),
    }
}

fn sync(projection: &mut CriticalPmProjection) -> Result<(), AppError> {
    projection.state = serde_json::from_value(crate::sdlc_lifecycle::canonical_projection(
        &value(&projection.state)?,
        &projection.lifecycle.task,
    )?)
    .map_err(AppError::internal)?;
    Ok(())
}

fn fence(assignment: &PmAssignment) -> MachineFence {
    MachineFence {
        assignment_id: assignment.assignment_id,
        execution_id: assignment.execution_id,
        agent_id: assignment.agent_id,
        assignment_version: assignment.version,
    }
}

fn own_question(q: &Question, assignment: &PmAssignment) -> bool {
    q.assignment_id == assignment.assignment_id
        && q.execution_id == assignment.execution_id
        && q.agent_id == assignment.agent_id
        && q.assignment_version == assignment.version
}

fn snapshot(state: &TaskState, routing: &TaskRoutingSnapshot) -> Result<(), AppError> {
    crate::sdlc_routing::validate_routes(&routing.policy.routes)?;
    let revision = state
        .revisions
        .iter()
        .find(|r| r.revision == routing.requirement_revision)
        .ok_or_else(conflict)?;
    if routing.contract_version != 2
        || routing.policy.contract_version != 2
        || routing.snapshot_id.is_nil()
        || routing.confirmation_id.is_nil()
        || routing.task_id != state.task_id
        || routing.root_task_id != state.task_id
        || routing.project_id != state.project_id
        || routing.tracker_instance_id != state.tracker_instance_id
        || routing.policy.project_id != state.project_id
        || routing.policy.tracker_instance_id != state.tracker_instance_id
        || !bounded(routing.policy.version)
        || routing.policy.native_ready
        || routing.policy.dispatch_allowed
        || routing.policy.routing_hash != crate::sdlc::canonical_hash(&routing.policy.routes)?
        || routing.content_hash != revision.content_hash
    {
        return Err(conflict());
    }
    let confirmation = crate::sdlc_confirmation::exact_receipt(
        state,
        revision.revision,
        state
            .confirmations
            .iter()
            .find(|c| c.id == routing.confirmation_id),
        Some(&revision.content_hash),
    )?;
    if confirmation.id != routing.confirmation_id {
        return Err(conflict());
    }
    Ok(())
}

fn validate_source<'a>(
    source: &'a CriticalPmLifecycleSource<'_>,
    config: &SdlcConfig,
) -> Result<Option<&'a ExecutionLeaseReceipt>, AppError> {
    let state = source.state;
    let current = source.lifecycle;
    let blocker = source.blocker;
    let review = source.review;
    let pm = &review.reservation;
    let initial = source.initial_pm_assignment;
    state.require_root()?;
    blocker.proposal.validate(blocker.role)?;
    let base = state
        .revisions
        .iter()
        .find(|r| r.revision == review.context.requirements.revision)
        .ok_or_else(conflict)?;
    let latest = state.revisions.last().ok_or_else(conflict)?;
    if state.draft_flow()? != Some(DraftFlow::SingleTaskV1)
        || !config.single_task_project_ids.contains(&state.project_id)
        || state.tracker_instance_id != config.instance_id
        || state.owner_subject.trim().is_empty()
        || state.stage != Stage::Draft
        || current.task.stage != LifecycleStage::Draft
        || !matches!(
            current.task.status,
            LifecycleStatus::Ready | LifecycleStatus::InProgress
        )
        || state.draft_submode.is_none()
        || current.task.task_id != state.task_id
        || current.task.owner_subject != state.owner_subject
        || !bounded(current.task.version)
        || current.task.cycle > MAX_REWORK_CYCLES
        || !matches!(
            current.task.runtime,
            RuntimeState::Terminal | RuntimeState::Reserved | RuntimeState::Running
        )
        || current.task.previous_result_id.is_some()
        || current.task.resume_chat_id.is_some()
        || current.assignment.is_some()
        || current.lease.is_some()
        || current.binding.is_some()
        || current.result.is_some()
        || !matches!(
            current.task.waiting_reason.as_deref(),
            Some(PM_REVIEW_REASON | PM_FINALIZATION_REASON)
        )
        || crate::sdlc_lifecycle::canonical_projection(&value(state)?, &current.task)?
            != value(state)?
        || state.assignment.as_ref() != Some(&pm.assignment)
        || blocker.state != CriticalBlockerState::InPmReview
        || blocker.resolution.is_some()
        || !bounded(blocker.version)
        || !bounded(blocker.routing_generation)
        || blocker.blocker_id.is_nil()
        || blocker.task_id != state.task_id
        || blocker.root_task_id != state.task_id
        || blocker.owner_subject != state.owner_subject
        || blocker.project_id != state.project_id
        || blocker.tracker_instance_id != state.tracker_instance_id
        || blocker.blocker_sha256 != crate::sdlc::canonical_hash(&blocker.proposal)?
        || blocker.routing_snapshot_id != current.task.routing.snapshot_id
        || review.context.blocker_id != blocker.blocker_id
        || review.context.blocker_version != blocker.version
        || review.context.blocker_sha256 != blocker.blocker_sha256
        || base.revision != blocker.proposal.requirement_revision
        || base.content_hash != blocker.proposal.content_hash
        || review.context.requirements.content_hash != base.content_hash
        || current.task.routing.requirement_revision != base.revision
        || current.task.routing.content_hash != base.content_hash
        || latest.revision < base.revision
        || blocker.pm_review.as_ref().is_none_or(|r| {
            r.assignment_id != pm.assignment.assignment_id
                || r.execution_id != pm.assignment.execution_id
        })
        || pm.contract_version != 1
        || pm.dispatch_allowed
        || pm.binding.task_id != state.task_id
        || pm.binding.root_task_id != state.task_id
        || pm.binding.project_id != state.project_id
        || pm.binding.owner_subject != state.owner_subject
        || pm.binding.tracker_instance_id != state.tracker_instance_id
        || initial.assignment_id.is_nil()
        || initial.execution_id.is_nil()
        || initial.agent_id.is_nil()
        || !bounded(initial.version)
        || pm.assignment.version <= initial.version
        || !bounded(pm.assignment.version)
        || !bounded(pm.owner_cas.version)
        || pm.owner_cas.expected_version.checked_add(1) != Some(pm.owner_cas.version)
        || pm.owner_cas.expected_version < 1
        || pm.assignment.agent_id != initial.agent_id
        || pm.interaction_protocol != state.interaction_protocol()?
        || (if state.configured_cli() {
            crate::sdlc::canonical_pm_uuid(&pm.assignment.machine_subject).is_none()
                || pm.assignment.machine_subject != initial.machine_subject
                || pm.assignment.machine_subject == config.orchestrator_subject
        } else {
            pm.assignment.machine_subject != config.orchestrator_subject
        })
        || pm.assignment.machine_subject.is_empty()
        || pm.assignment.machine_subject == state.owner_subject
        || pm.assignment.machine_subject == config.reservation_scheduler_subject
        || pm.assignment.machine_subject == config.verifier_subject
        || pm.assignment.assignment_id.is_nil()
        || pm.assignment.execution_id.is_nil()
        || pm.assignment.assignment_id == pm.assignment.execution_id
        || [
            initial.assignment_id,
            initial.execution_id,
            blocker.proposal.fence.assignment_id,
            blocker.proposal.fence.execution_id,
        ]
        .iter()
        .any(|id| *id == pm.assignment.assignment_id || *id == pm.assignment.execution_id)
        || pm.assignment_operation_key != format!("pm-draft:{}", pm.assignment.assignment_id)
        || pm
            .execution
            .ordinal
            .parse::<i64>()
            .ok()
            .is_none_or(|n| n <= 0 || n.to_string() != pm.execution.ordinal)
        || pm.execution.key != format!("SDLC-{}", pm.execution.ordinal)
        || pm.input.snapshot_ref.is_nil()
        || !sha256(&pm.input.sha256)
    {
        return Err(conflict());
    }
    let mut previous_revision = 0;
    for revision in &state.revisions {
        if !bounded(revision.revision)
            || revision.revision <= previous_revision
            || revision.content_hash != crate::sdlc::canonical_hash(&revision.document)?
        {
            return Err(conflict());
        }
        previous_revision = revision.revision;
    }
    snapshot(state, source.initial_routing)?;
    snapshot(state, &current.task.routing)?;
    if (blocker.routing_generation == 1
        && value(source.initial_routing)? != value(&current.task.routing)?)
        || (blocker.routing_generation > 1
            && source.initial_routing.snapshot_id == current.task.routing.snapshot_id)
        || value(&source.initial_routing.policy)? != value(&current.task.routing.policy)?
    {
        return Err(conflict());
    }
    let terminal = blocker.technical_terminal.as_ref().ok_or_else(conflict)?;
    crate::sdlc_lifecycle::validate_artifact(&terminal.receipt)?;
    if blocker.chat_id.is_nil()
        || blocker.run_id.is_nil()
        || blocker.agent_id.is_nil()
        || !sha256(&blocker.assignment_hash)
        || terminal.chat_id != blocker.chat_id
        || terminal.run_id != blocker.run_id
        || terminal.assignment_hash != blocker.assignment_hash
    {
        return Err(conflict());
    }
    let lease = match &source.execution {
        CriticalPmExecutionContext::Configured { .. } if state.configured_cli() => return Ok(None),
        CriticalPmExecutionContext::Configured { .. } => return Err(conflict()),
        CriticalPmExecutionContext::Open { lease, observed_at } => {
            if state.configured_cli() || *observed_at < lease.lease.heartbeat_at {
                return Err(conflict());
            }
            *lease
        }
        CriticalPmExecutionContext::Finalizing { lease, .. } if !state.configured_cli() => *lease,
        _ => return Err(conflict()),
    };
    if lease.contract_version != 1
        || lease.binding != pm.binding
        || lease.owner_version != pm.owner_cas.version
        || lease.fence != fence(&pm.assignment)
        || lease.dispatch_allowed
        || lease.ttl_seconds != 30
        || lease.heartbeat_seconds != 10
        || lease.lease.lease_id.is_nil()
        || !bounded(lease.lease.version)
        || lease.lease.holder_subject != pm.assignment.machine_subject
        || lease.lease.claimed_at > lease.lease.heartbeat_at
        || lease
            .lease
            .heartbeat_at
            .checked_add_signed(Duration::seconds(30))
            != Some(lease.lease.expires_at)
    {
        return Err(conflict());
    }
    Ok(Some(lease))
}

fn nonblank(text: &str) -> Result<(), AppError> {
    if text.trim().is_empty() || text.len() > 100_000 || text.contains('\0') {
        return Err(AppError::validation(
            "invalid critical PM question or evidence text",
        ));
    }
    Ok(())
}

fn receipt_id(
    state: &TaskState,
    allocation: &CriticalPmCommandAllocation,
    required: bool,
) -> Result<Option<Uuid>, AppError> {
    match allocation.generated_receipt_id {
        Some(id)
            if required
                && !id.is_nil()
                && !state.confirmations.iter().any(|c| c.id == id)
                && !state.questions.iter().any(|q| {
                    q.id == id
                        || q.request_id == id
                        || q.answer.as_ref().is_some_and(|a| a.id == id)
                }) =>
        {
            Ok(Some(id))
        }
        None if !required => Ok(None),
        _ => Err(conflict()),
    }
}

fn question_index(state: &TaskState, id: Uuid) -> Result<usize, AppError> {
    let index = state
        .questions
        .iter()
        .position(|q| q.id == id)
        .ok_or_else(conflict)?;
    if state.questions[index].task_id != state.task_id
        || state.questions[index].root_task_id != state.task_id
        || !own_question(
            &state.questions[index],
            state.assignment.as_ref().ok_or_else(conflict)?,
        )
    {
        return Err(AppError::Forbidden);
    }
    Ok(index)
}

fn ready(state: &TaskState, config: &SdlcConfig) -> bool {
    if state.configured_cli() {
        // The same readiness rule as initial PM. Exact owner consent and the
        // saved Workflow result are checked independently at completion.
        return state.ready();
    }
    state.ready()
        && !config.verifier_subject.is_empty()
        && state.revisions.last().is_some_and(|r| {
            r.document
                .checklist
                .iter()
                .chain(&r.document.prerequisites)
                .all(|check| {
                    state.evidence.iter().any(|e| {
                        e.requirement_revision == r.revision
                            && e.content_hash == r.content_hash
                            && e.check_id == *check
                            && e.verifier_subject == config.verifier_subject
                            && state
                                .questions
                                .iter()
                                .filter(|q| q.requirement_revision == r.revision)
                                .filter_map(|q| q.answer.as_ref())
                                .all(|a| e.created_at >= a.created_at)
                    })
                })
        })
}

/// Existing command semantics with explicit server time/IDs, no clock, randomness or I/O.
fn apply_command(
    state: &mut TaskState,
    actor: &Principal,
    config: &SdlcConfig,
    command: &SdlcCommand,
    allocation: &CriticalPmCommandAllocation,
) -> Result<Value, AppError> {
    crate::sdlc::authorize(state, actor, config, command)?;
    if matches!(
        command,
        SdlcCommand::PublishQuestion(_)
            | SdlcCommand::PublishRevision(_)
            | SdlcCommand::Cancel { .. }
    ) && !state.configured_cli()
        && (crate::sdlc::pm_grant_task(actor)? != Some(state.task_id)
            || !actor.scopes.contains("task-tracker:write"))
    {
        return Err(AppError::Forbidden);
    }
    crate::sdlc::validate_key(command.key())?;
    let id = receipt_id(
        state,
        allocation,
        matches!(
            command,
            SdlcCommand::Answer { .. } | SdlcCommand::Confirm { .. }
        ),
    )?;
    let now = allocation.now;
    match command {
        SdlcCommand::Assign(_) => Err(AppError::Forbidden),
        SdlcCommand::PublishRevision(c) => {
            if state.current_revision() != c.expected_requirement_revision
                || state
                    .questions
                    .iter()
                    .any(|q| q.required && q.state == QuestionState::Open)
            {
                return Err(conflict());
            }
            crate::sdlc::validate_document(&c.document)?;
            let revision = RequirementsRevision {
                revision: next(state.current_revision().ok_or_else(conflict)?)?,
                content_hash: crate::sdlc::canonical_hash(&c.document)?,
                document: c.document.clone(),
                author_subject: actor.subject.clone(),
                created_at: now,
            };
            for q in &mut state.questions {
                if q.state == QuestionState::Open {
                    q.state = QuestionState::Superseded;
                }
            }
            state.confirmation_revision = Some(revision.revision);
            state.revisions.push(revision.clone());
            state.draft_submode = Some(DraftSubmode::AwaitingConfirmation);
            state.status = Some(LifecycleStatus::Ready);
            value(&revision)
        }
        SdlcCommand::PublishQuestion(c) => {
            if state.current_revision() != Some(c.requirement_revision)
                || c.question_id.is_nil()
                || c.request_id.is_nil()
                || !state.configured_cli()
            {
                return Err(conflict());
            }
            let assignment = state.assignment.as_ref().ok_or_else(conflict)?;
            if state
                .questions
                .iter()
                .any(|q| q.request_id == c.request_id && !own_question(q, assignment))
            {
                return Err(AppError::Forbidden);
            }
            nonblank(&c.text)?;
            nonblank(&c.rationale)?;
            let ids: HashSet<_> = c.options.iter().map(|o| o.id).collect();
            if ids.len() != c.options.len()
                || ids.contains(&Uuid::nil())
                || (c.mode == QuestionMode::Text && !c.options.is_empty())
                || (c.mode != QuestionMode::Text && c.options.is_empty())
                || c.recommended_option_id.is_some_and(|id| !ids.contains(&id))
            {
                return Err(AppError::validation(
                    "invalid question options/recommendation",
                ));
            }
            for o in &c.options {
                nonblank(&o.label)?;
                nonblank(&o.consequences)?;
            }
            let old = state.questions.iter().position(|q| q.id == c.question_id);
            let version = if let Some(index) = old {
                question_index(state, c.question_id)?;
                let q = &state.questions[index];
                if c.expected_question_version != Some(q.version) || c.request_id != q.request_id {
                    return Err(conflict());
                }
                for o in &c.options {
                    if q.options.iter().any(|old| {
                        old.id == o.id
                            && (old.label != o.label
                                || old.consequences != o.consequences
                                || old.is_custom != o.is_custom)
                    }) {
                        return Err(AppError::validation(
                            "changed options require new stable IDs",
                        ));
                    }
                }
                next(q.version)?
            } else {
                if c.expected_question_version.is_some() {
                    return Err(conflict());
                }
                1
            };
            let q = Question {
                id: c.question_id,
                request_id: c.request_id,
                task_id: state.task_id,
                root_task_id: state.root_task_id,
                version,
                requirement_revision: c.requirement_revision,
                requirement_reference: c.requirement_reference.clone(),
                assignment_id: c.fence.assignment_id,
                execution_id: c.fence.execution_id,
                agent_id: c.fence.agent_id,
                assignment_version: c.fence.assignment_version,
                checkpoint_id: None,
                text: c.text.clone(),
                rationale: c.rationale.clone(),
                required: c.required,
                mode: c.mode.clone(),
                options: c.options.clone(),
                recommended_option_id: c.recommended_option_id,
                state: QuestionState::Open,
                answer: None,
                author_subject: actor.subject.clone(),
                created_at: now,
            };
            if let Some(index) = old {
                state.questions[index] = q.clone();
            } else {
                state.questions.push(q.clone());
            }
            state.confirmation_revision = None;
            state.draft_submode = Some(DraftSubmode::Clarification);
            state.status = Some(LifecycleStatus::Ready);
            value(&q)
        }
        SdlcCommand::Answer {
            question_id,
            command: c,
        } => {
            if state.current_revision() != Some(c.requirement_revision) {
                return Err(conflict());
            }
            let index = question_index(state, *question_id)?;
            let q = &mut state.questions[index];
            if q.version != c.expected_question_version
                || q.requirement_revision != c.requirement_revision
                || q.state != QuestionState::Open
            {
                return Err(conflict());
            }
            crate::sdlc::validate_answer(q, c)?;
            let answer = Answer {
                id: id.ok_or_else(conflict)?,
                question_id: *question_id,
                question_version: q.version,
                requirement_revision: c.requirement_revision,
                selected_option_ids: c.selected_option_ids.clone(),
                text: c.text.clone(),
                comment: c.comment.clone(),
                author_subject: actor.subject.clone(),
                created_at: now,
            };
            q.answer = Some(answer.clone());
            q.state = QuestionState::Answered;
            state.status = Some(
                if state
                    .questions
                    .iter()
                    .any(|q| q.state == QuestionState::Open)
                {
                    LifecycleStatus::Ready
                } else {
                    LifecycleStatus::InProgress
                },
            );
            value(&answer)
        }
        SdlcCommand::Cancel {
            question_id,
            command: c,
        } => {
            let index = question_index(state, *question_id)?;
            let q = &mut state.questions[index];
            if q.version != c.expected_question_version || q.state != QuestionState::Open {
                return Err(conflict());
            }
            q.state = QuestionState::Cancelled;
            let result = value(q)?;
            state.status = Some(
                if state
                    .questions
                    .iter()
                    .any(|q| q.state == QuestionState::Open)
                {
                    LifecycleStatus::Ready
                } else {
                    LifecycleStatus::InProgress
                },
            );
            Ok(result)
        }
        SdlcCommand::Evidence(c) => {
            let r = state.revisions.last().ok_or_else(conflict)?;
            if r.revision != c.requirement_revision
                || r.content_hash != c.content_hash
                || (!r.document.checklist.contains(&c.check_id)
                    && !r.document.prerequisites.contains(&c.check_id))
            {
                return Err(conflict());
            }
            nonblank(&c.evidence_reference)?;
            let evidence = Evidence {
                requirement_revision: r.revision,
                content_hash: r.content_hash.clone(),
                check_id: c.check_id.clone(),
                evidence_reference: c.evidence_reference.clone(),
                verifier_subject: actor.subject.clone(),
                created_at: now,
            };
            state.evidence.push(evidence.clone());
            value(&evidence)
        }
        SdlcCommand::Confirm {
            revision,
            command: c,
        } => {
            let r = state.revisions.last().ok_or_else(conflict)?;
            if (!state.configured_cli() && config.verifier_subject.is_empty())
                || *revision != r.revision
                || c.content_hash != r.content_hash
                || !ready(state, config)
                || state
                    .confirmations
                    .iter()
                    .any(|old| old.revision == *revision)
            {
                return Err(conflict());
            }
            let confirmation = Confirmation {
                id: id.ok_or_else(conflict)?,
                task_id: state.task_id,
                revision: *revision,
                content_hash: c.content_hash.clone(),
                owner_subject: actor.subject.clone(),
                created_at: now,
                stage: state.confirmation_stage(),
            };
            state.confirmations.push(confirmation.clone());
            state.status = Some(if state.configured_cli() {
                LifecycleStatus::InProgress
            } else {
                LifecycleStatus::Ready
            });
            state.draft_submode = Some(DraftSubmode::AwaitingConfirmation);
            value(&confirmation)
        }
    }
}

pub fn prepare_critical_pm_command(
    source: CriticalPmLifecycleSource<'_>,
    actor: &Principal,
    config: &SdlcConfig,
    command: &SdlcCommand,
    allocation: CriticalPmCommandAllocation,
) -> Result<CriticalPmCommandPlan, AppError> {
    let lease = validate_source(&source, config)?;
    let observed_at = match source.execution {
        CriticalPmExecutionContext::Open { observed_at, .. }
        | CriticalPmExecutionContext::Configured { observed_at } => observed_at,
        _ => return Err(conflict()),
    };
    if allocation.now < observed_at
        || allocation.now
            < source
                .state
                .revisions
                .last()
                .ok_or_else(conflict)?
                .created_at
    {
        return Err(conflict());
    }
    if matches!(
        command,
        SdlcCommand::PublishQuestion(_)
            | SdlcCommand::PublishRevision(_)
            | SdlcCommand::Cancel { .. }
    ) && lease.is_some_and(|lease| lease.lease.expires_at <= allocation.now)
    {
        return Err(conflict());
    }
    if let SdlcCommand::Confirm {
        revision,
        command: c,
    } = command
    {
        if *revision <= source.review.context.requirements.revision
            || c.expected_routing_policy_version
                .is_some_and(|v| v != source.lifecycle.task.routing.policy.version)
        {
            return Err(conflict());
        }
    }
    let before = frame(&source);
    let mut after = before.clone();
    let result = apply_command(&mut after.state, actor, config, command, &allocation)?;
    after.lifecycle.task.version = next(before.lifecycle.task.version)?;
    after.lifecycle.task.status = after.state.status.ok_or_else(conflict)?;
    if matches!(command, SdlcCommand::Confirm { .. }) {
        after.lifecycle.task.waiting_reason = Some(PM_FINALIZATION_REASON.into());
    } else if matches!(
        command,
        SdlcCommand::PublishRevision(_) | SdlcCommand::PublishQuestion(_)
    ) {
        after.lifecycle.task.waiting_reason = Some(PM_REVIEW_REASON.into());
    }
    sync(&mut after)?;
    Ok(CriticalPmCommandPlan {
        before,
        after,
        result,
    })
}

pub fn prepare_critical_pm_completion(
    source: CriticalPmLifecycleSource<'_>,
    actor: &Principal,
    config: &SdlcConfig,
    command: &CompletePmDraft,
    allocation: CriticalPmCompletionAllocation,
) -> Result<CriticalPmCompletionPlan, AppError> {
    crate::sdlc_reservation::scheduler(actor, config)?;
    if actor.trusted_human {
        return Err(AppError::Forbidden);
    }
    crate::sdlc_reservation::validate_pm_completion(command)?;
    let lease = validate_source(&source, config)?.ok_or_else(conflict)?;
    let CriticalPmExecutionContext::Finalizing { completion, .. } = source.execution else {
        return Err(conflict());
    };
    let state = source.state;
    let r = state.revisions.last().ok_or_else(conflict)?;
    let pm = &source.review.reservation;
    let confirmation = crate::sdlc_confirmation::exact_receipt(
        state,
        r.revision,
        state
            .confirmations
            .iter()
            .find(|c| c.revision == r.revision),
        Some(&r.content_hash),
    )?;
    let prepared_command = CompletePmDraft {
        contract_version: completion.contract_version,
        fence: completion.fence.clone(),
        lease_id: completion.lease_id,
        lease_version: completion.lease_version,
        requirement_revision: completion.requirement_revision,
        content_hash: completion.content_hash.clone(),
        terminal_receipt_ref: completion.terminal_receipt_ref.clone(),
        terminal_receipt_sha256: completion.terminal_receipt_sha256.clone(),
        workflow_receipt_ref: completion.workflow_receipt_ref.clone(),
        workflow_receipt_sha256: completion.workflow_receipt_sha256.clone(),
        idempotency_key: completion.idempotency_key.clone(),
    };
    if value(command)? != value(&prepared_command)?
        || completion.completion_id.is_nil()
        || completion.task_id != state.task_id
        || completion.root_task_id != state.task_id
        || completion.project_id != state.project_id
        || completion.tracker_instance_id != state.tracker_instance_id
        || completion.owner_subject != state.owner_subject
        || completion.owner_version != pm.owner_cas.version
        || completion.fence != lease.fence
        || completion.lease_id != lease.lease.lease_id
        || completion.lease_version != lease.lease.version
        || completion.requirement_revision != r.revision
        || completion.content_hash != r.content_hash
        || completion.created_at < confirmation.created_at
        || completion.previous_pm_admission_state != "reserved"
        || completion.next_pm_admission_state != "completed"
        || r.revision <= source.review.context.requirements.revision
        || !ready(state, config)
        || source.lifecycle.task.waiting_reason.as_deref() != Some(PM_FINALIZATION_REASON)
        || allocation.snapshot_id.is_nil()
        || allocation.analysis_intent_id.is_nil()
        || allocation.snapshot_id == allocation.analysis_intent_id
        || [
            source.initial_routing.snapshot_id,
            source.lifecycle.task.routing.snapshot_id,
            confirmation.id,
        ]
        .iter()
        .any(|id| *id == allocation.snapshot_id || *id == allocation.analysis_intent_id)
    {
        return Err(conflict());
    }
    let routing = CriticalPmRoutingGeneration {
        generation: next(source.blocker.routing_generation)?,
        previous_snapshot_id: source.lifecycle.task.routing.snapshot_id,
        snapshot: TaskRoutingSnapshot {
            snapshot_id: allocation.snapshot_id,
            confirmation_id: confirmation.id,
            requirement_revision: r.revision,
            content_hash: r.content_hash.clone(),
            created_at: confirmation.created_at,
            ..source.lifecycle.task.routing.clone()
        },
    };
    let before = frame(&source);
    let mut after = before.clone();
    after.blocker.version = next(before.blocker.version)?;
    after.blocker.state = CriticalBlockerState::Resolved;
    after.blocker.resolution = Some(CriticalResolutionRef {
        requirement_revision: r.revision,
        content_hash: r.content_hash.clone(),
        routing_generation: routing.generation,
    });
    after.lifecycle.task.version = next(before.lifecycle.task.version)?;
    after.lifecycle.task.stage = LifecycleStage::Analysis;
    after.lifecycle.task.status = LifecycleStatus::Ready;
    after.lifecycle.task.runtime = RuntimeState::Queued;
    after.lifecycle.task.waiting_reason = Some("queued_for_assignment".into());
    after.lifecycle.task.routing = routing.snapshot.clone();
    after.lifecycle.task.cycle = 0;
    after.lifecycle.task.transient_retries = 0;
    after.state.draft_submode = None;
    sync(&mut after)?;
    let mut analysis_intent =
        crate::sdlc::analysis_intent(&after.state, &confirmation, allocation.analysis_intent_id)?;
    analysis_intent.workflow = routing.snapshot.policy.routes.analyst.workflow_key.clone();
    Ok(CriticalPmCompletionPlan {
        before,
        after,
        completion: completion.clone(),
        routing,
        analysis_intent,
    })
}

/// Reuse current assignment and exact owner consent. The server has already read
/// Workflow's terminal operation; it is not a caller-provided native receipt.
pub fn prepare_configured_critical_pm_completion(
    source: CriticalPmLifecycleSource<'_>,
    actor: &Principal,
    config: &SdlcConfig,
    command: &domain::sdlc_configured_cli::CompleteConfiguredPm,
    completion: &domain::sdlc_configured_cli::ConfiguredPmCompletionReceipt,
    snapshot_id: Uuid,
) -> Result<ConfiguredCriticalPmCompletionPlan, AppError> {
    if validate_source(&source, config)?.is_some()
        || !matches!(
            source.execution,
            CriticalPmExecutionContext::Configured { .. }
        )
    {
        return Err(conflict());
    }
    crate::sdlc_configured_cli::validate_completion(source.state, actor, command)?;
    let expected = crate::sdlc_configured_cli::completion_result(
        source.state,
        command,
        completion.workflow_receipt.clone(),
        source.review.reservation.owner_cas.version,
        completion.completion_id,
        completion.created_at,
    )?;
    let state = source.state;
    let confirmation = state.current_confirmation().ok_or_else(conflict)?;
    if completion != &expected
        || completion.completion_id.is_nil()
        || completion.created_at < confirmation.created_at
        || command.requirement_revision <= source.review.context.requirements.revision
        || source.lifecycle.task.waiting_reason.as_deref() != Some(PM_FINALIZATION_REASON)
        || snapshot_id.is_nil()
        || [
            source.initial_routing.snapshot_id,
            source.lifecycle.task.routing.snapshot_id,
            confirmation.id,
            completion.completion_id,
        ]
        .contains(&snapshot_id)
    {
        return Err(conflict());
    }
    let routing = CriticalPmRoutingGeneration {
        generation: next(source.blocker.routing_generation)?,
        previous_snapshot_id: source.lifecycle.task.routing.snapshot_id,
        snapshot: TaskRoutingSnapshot {
            snapshot_id,
            confirmation_id: confirmation.id,
            requirement_revision: confirmation.revision,
            content_hash: confirmation.content_hash.clone(),
            created_at: confirmation.created_at,
            ..source.lifecycle.task.routing.clone()
        },
    };
    let before = frame(&source);
    let mut after = before.clone();
    after.blocker.version = next(before.blocker.version)?;
    after.blocker.state = CriticalBlockerState::Resolved;
    after.blocker.resolution = Some(CriticalResolutionRef {
        requirement_revision: confirmation.revision,
        content_hash: confirmation.content_hash.clone(),
        routing_generation: routing.generation,
    });
    after.lifecycle.task.version = next(before.lifecycle.task.version)?;
    after.lifecycle.task.stage = LifecycleStage::Backlog;
    after.lifecycle.task.status = LifecycleStatus::Ready;
    after.lifecycle.task.runtime = RuntimeState::Terminal;
    after.lifecycle.task.waiting_reason = None;
    after.lifecycle.task.routing = routing.snapshot.clone();
    after.lifecycle.task.cycle = 0;
    after.lifecycle.task.transient_retries = 0;
    after.state.draft_submode = None;
    sync(&mut after)?;
    Ok(ConfiguredCriticalPmCompletionPlan {
        before,
        after,
        routing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, TimeZone, Utc};
    use domain::sdlc_critical_pm_reservation::CriticalPmReservationResult;
    use domain::sdlc_reservation::PmDraftCompletionReceipt;
    use serde_json::json;

    // Entirely synthetic producer-shaped records. No live/DB/terminal proof is claimed.
    struct Fixture {
        state: TaskState,
        lifecycle: LifecycleReceipt,
        blocker: CriticalBlockerReceipt,
        review: CriticalPmReservationResult,
        initial: PmAssignment,
        initial_routing: TaskRoutingSnapshot,
        lease: ExecutionLeaseReceipt,
        config: SdlcConfig,
        now: DateTime<Utc>,
    }

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn actor(subject: &str, human: bool, scopes: &[String]) -> Principal {
        Principal {
            subject: subject.into(),
            human_session: human,
            trusted_human: human,
            scopes: scopes.iter().cloned().collect(),
        }
    }

    fn decode<T: serde::de::DeserializeOwned>(v: Value) -> T {
        serde_json::from_value(v).unwrap()
    }

    impl Fixture {
        fn new(generation: i64) -> Self {
            let now = Utc.with_ymd_and_hms(2026, 10, 14, 12, 0, 0).unwrap();
            let old = now - Duration::minutes(5);
            let task = id(1);
            let project = id(2);
            let document = json!({"goal":"Synthetic reviewed business goal","scope":["scope"],"exclusions":[],
                "scenarios":["scenario"],"acceptance_criteria":["acceptance"],"constraints":[],
                "dependencies":[],"assumptions":[],"checklist":["check"],"prerequisites":["prerequisite"]});
            let hash = crate::sdlc::canonical_hash(&document).unwrap();
            let mut routes = serde_json::Map::new();
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
                let hyphen = role.replace('_', "-");
                routes.insert((*role).into(), json!({"agent_id":id(100+i as u128),"fleet_config_revision":1,
                    "instruction_profile":"single-task-v1","instruction_manifest_sha256":"a".repeat(64),
                        "namespace_id":(i+1).to_string(),"namespace_name":format!("single-task-{hyphen}"),
                        "workflow_id":(i+11).to_string(),"workflow_key":format!("single-task-v1:{role}"),
                    "profile":format!("single-task-v1-{hyphen}"),"workflow_catalog_version":1,
                    "workflow_catalog_sha256":"b".repeat(64)}));
            }
            let policy = json!({"contract_version":2,"tracker_instance_id":"fixture","project_id":project,
                "version":1,"routing_hash":crate::sdlc::canonical_hash(&routes).unwrap(),"routes":routes,
                "verification":"declared","native_ready":false,"dispatch_allowed":false,
                "author_subject":"owner","created_at":old});
            let initial_routing: TaskRoutingSnapshot = decode(
                json!({"contract_version":2,"snapshot_id":id(10),
                "tracker_instance_id":"fixture","project_id":project,"task_id":task,"root_task_id":task,
                "confirmation_id":id(1001),"requirement_revision":1,"content_hash":hash,"policy":policy,"created_at":old}),
            );
            let mut routing = initial_routing.clone();
            if generation > 1 {
                routing.snapshot_id = id(10 + generation as u128);
                routing.requirement_revision = generation;
                routing.confirmation_id = id(1000 + generation as u128);
            }
            let initial: PmAssignment =
                decode(json!({"assignment_id":id(20),"execution_id":id(21),
                "agent_id":id(100),"version":1,"machine_subject":"pm"}));
            let assignment = json!({"assignment_id":id(30),"execution_id":id(31),
                "agent_id":id(100),"version":generation+1,"machine_subject":"pm"});
            let binding = json!({"tracker_instance_id":"fixture","project_id":project,"task_id":task,
                "root_task_id":task,"owner_subject":"owner"});
            let review: CriticalPmReservationResult = decode(
                json!({"purpose":"critical_business_review",
                "reservation":{"contract_version":1,"variant":"pm_draft_reserved","binding":binding,
                    "owner_cas":{"expected_version":generation,"version":generation+1},"assignment":assignment,
                    "execution":{"ordinal":"70","key":"SDLC-70"},"input":{"snapshot_ref":id(22),"sha256":"c".repeat(64)},
                    "assignment_operation_key":format!("pm-draft:{}",id(30)),"admission_state":"reserved","dispatch_allowed":false},
                "context":{"blocker_id":id(40),"blocker_sha256":"d".repeat(64),"blocker_version":3,
                    "requirements":{"revision":generation,"content_hash":hash}}}),
            );
            let proposal = json!({"fence":{"assignment_id":id(50),"execution_id":id(51),"lease_id":id(52),
                "expected_lease_version":4,"fencing_token":7},"requirement_revision":generation,"content_hash":hash,
                "kind":"missing_business_decision","question":"Synthetic decision?","rationale":"Synthetic contradiction",
                "options":[],"recommendation":null,"evidence":[{"receipt":{"reference":format!("tracker-attachment:{}",id(53)),
                "sha256":"e".repeat(64)},"binding":{"assignment_id":id(50),"execution_id":id(51),"role":"developer"}}],
                "idempotency_key":"fixture:blocker"});
            let blocker: CriticalBlockerReceipt = decode(json!({"blocker_id":id(40),"version":3,
                "tracker_instance_id":"fixture","project_id":project,"task_id":task,"root_task_id":task,
                "owner_subject":"owner","producer_subject":"technical","agent_id":id(103),"role":"developer",
                "assignment_hash":"f".repeat(64),"chat_id":id(54),"run_id":id(55),"routing_snapshot_id":routing.snapshot_id,
                "routing_generation":generation,"blocker_sha256":crate::sdlc::canonical_hash(&proposal).unwrap(),"proposal":proposal,
                "state":"in_pm_review","pm_review":{"assignment_id":id(30),"execution_id":id(31)},"resolution":null,
                "technical_terminal":{"assignment_hash":"f".repeat(64),"chat_id":id(54),"run_id":id(55),"state":"cancelled",
                    "receipt":{"reference":"fixture://synthetic-old-terminal","sha256":"a".repeat(64)}},"created_at":old}));
            let mut review = review;
            review.context.blocker_sha256 = blocker.blocker_sha256.clone();
            let lifecycle: LifecycleReceipt = decode(
                json!({"task":{"task_id":task,"owner_subject":"owner","version":8,
                "stage":"Draft","status":"Ready","cycle":2,"transient_retries":0,"waiting_reason":PM_REVIEW_REASON,
                "runtime":"terminal","routing":routing,"previous_result_id":null,"resume_chat_id":null},
                "assignment":null,"lease":null,"binding":null,"result":null}),
            );
            let mut revisions = Vec::new();
            let mut confirmations = Vec::new();
            for revision in 1..=generation {
                let mut r = document.clone();
                r.as_object_mut().unwrap().extend(
                    json!({"revision":revision,"content_hash":hash,
                    "author_subject":"pm","created_at":old})
                    .as_object()
                    .unwrap()
                    .clone(),
                );
                revisions.push(r);
                confirmations.push(json!({"id":id(1000+revision as u128),"task_id":task,"revision":revision,
                    "content_hash":hash,"owner_subject":"owner","created_at":old,"stage":"Backlog"}));
            }
            let state: TaskState = decode(
                json!({"tracker_instance_id":"fixture","project_id":project,
                "task_id":task,"root_task_id":task,"owner_subject":"owner","stage":"Draft","status":"Ready",
                "draft_submode":"brainstorming","confirmation_revision":null,"assignment":assignment,
                "questions":[],"revisions":revisions,"confirmations":confirmations,"evidence":[],
                "flow":"single-task-v1","waiting_reason":PM_REVIEW_REASON,"runtime_state":"terminal",
                "lifecycle":lifecycle.task,"future_retained_marker":{"must":"survive"}}),
            );
            let lease: ExecutionLeaseReceipt = decode(
                json!({"contract_version":1,"binding":binding,
                "owner_version":generation+1,"fence":fence(&review.reservation.assignment),
                "lease":{"lease_id":id(60),"version":4,"holder_subject":"pm","claimed_at":now-Duration::seconds(10),
                    "heartbeat_at":now,"expires_at":now+Duration::seconds(30)},
                "ttl_seconds":30,"heartbeat_seconds":10,"dispatch_allowed":false}),
            );
            let config = SdlcConfig {
                instance_id: "fixture".into(),
                orchestrator_subject: "pm".into(),
                verifier_subject: "verifier".into(),
                reservation_scheduler_subject: "scheduler".into(),
                single_task_project_ids: [project].into(),
            };
            Self {
                state,
                lifecycle,
                blocker,
                review,
                initial,
                initial_routing,
                lease,
                config,
                now,
            }
        }

        fn source(&self) -> CriticalPmLifecycleSource<'_> {
            CriticalPmLifecycleSource {
                state: &self.state,
                lifecycle: &self.lifecycle,
                blocker: &self.blocker,
                review: &self.review,
                initial_pm_assignment: &self.initial,
                initial_routing: &self.initial_routing,
                execution: if self.state.configured_cli() {
                    CriticalPmExecutionContext::Configured {
                        observed_at: self.now,
                    }
                } else {
                    CriticalPmExecutionContext::Open {
                        lease: &self.lease,
                        observed_at: self.now,
                    }
                },
            }
        }
        fn pm(&self) -> Principal {
            if self.state.configured_cli() {
                return actor(
                    &self.review.reservation.assignment.machine_subject,
                    false,
                    &["task-tracker:read".into(), "task-tracker:write".into()],
                );
            }
            actor(
                "pm",
                false,
                &[
                    "task-tracker:write".into(),
                    self.review.reservation.assignment.scope(self.state.task_id),
                ],
            )
        }
        fn owner(&self) -> Principal {
            actor("owner", true, &[])
        }
        fn scheduler(&self) -> Principal {
            actor("scheduler", false, &["task-tracker:write".into()])
        }
        fn command(
            &self,
            command: &SdlcCommand,
            who: &Principal,
            generated: Option<Uuid>,
        ) -> Result<CriticalPmCommandPlan, AppError> {
            prepare_critical_pm_command(
                self.source(),
                who,
                &self.config,
                command,
                CriticalPmCommandAllocation {
                    now: self.now,
                    generated_receipt_id: generated,
                },
            )
        }
        fn accept(&mut self, plan: CriticalPmCommandPlan) {
            self.state = plan.after.state;
            self.lifecycle = plan.after.lifecycle;
            self.blocker = plan.after.blocker;
        }
        fn revision_command(&self) -> SdlcCommand {
            let mut document = self.state.revisions.last().unwrap().document.clone();
            document
                .goal
                .push_str("; synthetic owner decision incorporated");
            SdlcCommand::PublishRevision(PublishRevision {
                fence: fence(&self.review.reservation.assignment),
                expected_requirement_revision: self.state.current_revision(),
                document,
                idempotency_key: "fixture:revision".into(),
            })
        }
        fn question_command(&self) -> SdlcCommand {
            SdlcCommand::PublishQuestion(PublishQuestion {
                fence: fence(&self.review.reservation.assignment),
                request_id: id(200),
                question_id: id(201),
                expected_question_version: None,
                requirement_revision: self.state.current_revision().unwrap(),
                requirement_reference: None,
                text: "Synthetic clarification?".into(),
                rationale: "Synthetic business choice".into(),
                required: true,
                mode: QuestionMode::Text,
                options: vec![],
                recommended_option_id: None,
                idempotency_key: "fixture:question".into(),
            })
        }
        fn confirm_command(&self) -> SdlcCommand {
            let r = self.state.revisions.last().unwrap();
            SdlcCommand::Confirm {
                revision: r.revision,
                command: ConfirmCommand {
                    content_hash: r.content_hash.clone(),
                    expected_routing_policy_version: Some(1),
                    idempotency_key: "fixture:consent".into(),
                },
            }
        }
        fn ready_for_confirmation(&mut self) {
            self.accept(
                self.command(&self.revision_command(), &self.pm(), None)
                    .unwrap(),
            );
            for check in ["check", "prerequisite"] {
                let r = self.state.revisions.last().unwrap();
                let command = SdlcCommand::Evidence(EvidenceCommand {
                    fence: fence(&self.review.reservation.assignment),
                    requirement_revision: r.revision,
                    content_hash: r.content_hash.clone(),
                    check_id: check.into(),
                    evidence_reference: "fixture://synthetic-readiness".into(),
                    idempotency_key: format!("fixture:evidence:{check}"),
                });
                let verifier = actor(
                    "verifier",
                    false,
                    &[
                        "task-tracker:write".into(),
                        format!(
                            "task-tracker:sdlc:evidence:{}:{}",
                            self.state.task_id, self.review.reservation.assignment.version
                        ),
                    ],
                );
                self.accept(self.command(&command, &verifier, None).unwrap());
            }
        }
        fn confirmed(&mut self) {
            self.ready_for_confirmation();
            self.accept(
                self.command(&self.confirm_command(), &self.owner(), Some(id(203)))
                    .unwrap(),
            );
        }
        fn completion(&self) -> (CompletePmDraft, PmDraftCompletionReceipt) {
            let r = self.state.revisions.last().unwrap();
            let command = CompletePmDraft {
                contract_version: 1,
                fence: self.lease.fence.clone(),
                lease_id: self.lease.lease.lease_id,
                lease_version: self.lease.lease.version,
                requirement_revision: r.revision,
                content_hash: r.content_hash.clone(),
                terminal_receipt_ref: "fixture://synthetic-new-terminal".into(),
                terminal_receipt_sha256: "a".repeat(64),
                workflow_receipt_ref: "fixture://synthetic-new-workflow".into(),
                workflow_receipt_sha256: "b".repeat(64),
                idempotency_key: "fixture:complete".into(),
            };
            let mut raw = value(&command).unwrap();
            raw.as_object_mut().unwrap().extend(json!({"completion_id":id(204),"tracker_instance_id":"fixture",
                "project_id":self.state.project_id,"task_id":self.state.task_id,"root_task_id":self.state.task_id,
                "owner_subject":"owner","owner_version":self.review.reservation.owner_cas.version,
                "previous_pm_admission_state":"reserved","next_pm_admission_state":"completed","created_at":self.now})
                .as_object().unwrap().clone());
            (command, decode(raw))
        }
        fn finalize(
            &self,
            who: &Principal,
            command: &CompletePmDraft,
            completion: &PmDraftCompletionReceipt,
        ) -> Result<CriticalPmCompletionPlan, AppError> {
            let mut source = self.source();
            source.execution = CriticalPmExecutionContext::Finalizing {
                lease: &self.lease,
                completion,
            };
            prepare_critical_pm_completion(
                source,
                who,
                &self.config,
                command,
                CriticalPmCompletionAllocation {
                    snapshot_id: id(205),
                    analysis_intent_id: id(206),
                },
            )
        }
    }

    #[test]
    fn question_answer_revision_confirmation_stays_one_pm_execution_and_draft() {
        let mut f = Fixture::new(1);
        let history = json!({"review":f.review,"routing":f.initial_routing,"initial":f.initial,"confirmations":f.state.confirmations});
        let q = f.command(&f.question_command(), &f.pm(), None).unwrap();
        assert_eq!(q.before.state.current_revision(), Some(1));
        assert_eq!(
            q.after.state.draft_submode,
            Some(DraftSubmode::Clarification)
        );
        f.accept(q);
        assert!(f.command(&f.revision_command(), &f.pm(), None).is_err());
        let answer = SdlcCommand::Answer {
            question_id: id(201),
            command: AnswerCommand {
                expected_question_version: 1,
                requirement_revision: 1,
                selected_option_ids: vec![],
                text: Some("Synthetic actual owner input".into()),
                comment: None,
                idempotency_key: "fixture:answer".into(),
            },
        };
        let plan = f.command(&answer, &f.owner(), Some(id(207))).unwrap();
        assert_eq!(plan.result["id"], json!(id(207)));
        f.accept(plan);
        f.confirmed();
        assert!(!f.state.context(&f.owner()).permissions.can_confirm);
        assert_eq!(f.state.stage, Stage::Draft);
        assert_eq!(f.lifecycle.task.stage, LifecycleStage::Draft);
        assert_eq!(
            f.lifecycle.task.waiting_reason.as_deref(),
            Some(PM_FINALIZATION_REASON)
        );
        assert_eq!(f.blocker.state, CriticalBlockerState::InPmReview);
        assert!(f.blocker.resolution.is_none());
        assert!(
            f.lifecycle.assignment.is_none()
                && f.lifecycle.lease.is_none()
                && f.lifecycle.binding.is_none()
                && f.lifecycle.result.is_none()
        );
        assert_eq!(
            f.state.assignment.as_ref(),
            Some(&f.review.reservation.assignment)
        );
        assert_eq!(
            f.lifecycle.task.runtime,
            RuntimeState::Terminal,
            "planner must not invent native running"
        );
        assert_eq!(
            json!({"review":f.review,"routing":f.initial_routing,"initial":f.initial,
            "confirmations":&f.state.confirmations[..1]}),
            history
        );
        assert_eq!(f.state.confirmations.last().unwrap().stage, Stage::Backlog);
        assert_eq!(
            f.state.retained_fields["future_retained_marker"],
            json!({"must":"survive"})
        );
    }

    #[test]
    fn finalizing_is_one_uncommitted_plan_and_never_needs_a_previous_completion() {
        let mut f = Fixture::new(1);
        f.confirmed();
        let before = json!({"state":f.state,"blocker":f.blocker,"lease":f.lease,"routing":f.initial_routing});
        let (command, receipt) = f.completion();
        let plan = f.finalize(&f.scheduler(), &command, &receipt).unwrap();
        assert_eq!(value(&plan.before.state).unwrap(), value(&f.state).unwrap());
        assert_eq!(plan.before.blocker.state, CriticalBlockerState::InPmReview);
        assert_eq!(plan.after.blocker.state, CriticalBlockerState::Resolved);
        assert_eq!(plan.after.blocker.version, f.blocker.version + 1);
        assert_eq!(
            plan.after
                .blocker
                .resolution
                .as_ref()
                .unwrap()
                .routing_generation,
            2
        );
        assert_eq!(plan.routing.generation, 2);
        assert_eq!(
            plan.routing.previous_snapshot_id,
            f.initial_routing.snapshot_id
        );
        assert_eq!(
            value(&plan.routing.snapshot.policy).unwrap(),
            value(&f.initial_routing.policy).unwrap()
        );
        assert_eq!(plan.after.lifecycle.task.stage, LifecycleStage::Analysis);
        assert_eq!(plan.after.lifecycle.task.status, LifecycleStatus::Ready);
        assert_eq!(plan.after.lifecycle.task.runtime, RuntimeState::Queued);
        assert_eq!(plan.after.lifecycle.task.cycle, 0);
        assert_eq!(
            plan.after.lifecycle.task.version,
            f.lifecycle.task.version + 1
        );
        assert!(
            plan.after.lifecycle.assignment.is_none()
                && plan.after.lifecycle.lease.is_none()
                && plan.after.lifecycle.binding.is_none()
                && plan.after.lifecycle.result.is_none()
        );
        assert_eq!(plan.analysis_intent.requirement_revision, 2);
        assert_eq!(plan.analysis_intent.workflow, "single-task-v1:analyst");
        assert_eq!(value(&plan.completion).unwrap(), value(&receipt).unwrap());
        assert_eq!(
            crate::sdlc_lifecycle::canonical_projection(
                &value(&plan.after.state).unwrap(),
                &plan.after.lifecycle.task
            )
            .unwrap(),
            value(&plan.after.state).unwrap()
        );
        assert_eq!(
            before,
            json!({"state":f.state,"blocker":f.blocker,"lease":f.lease,"routing":f.initial_routing})
        );
    }

    #[test]
    fn later_reviews_keep_generation_one_and_increment_actual_generation_not_rework_count() {
        for generation in [2, 17] {
            let mut f = Fixture::new(generation);
            let original = value(&f.initial_routing).unwrap();
            f.confirmed();
            let (command, receipt) = f.completion();
            let plan = f.finalize(&f.scheduler(), &command, &receipt).unwrap();
            assert_eq!(plan.routing.generation, generation + 1);
            assert_eq!(plan.analysis_intent.requirement_revision, generation + 1);
            assert_eq!(
                plan.after.state.assignment.as_ref().unwrap().version,
                generation + 1
            );
            assert_eq!(
                plan.routing.previous_snapshot_id,
                f.lifecycle.task.routing.snapshot_id
            );
            assert_eq!(original, value(&f.initial_routing).unwrap());
            assert_eq!(plan.before.blocker.routing_generation, generation);
        }
    }

    #[test]
    fn old_pm_grants_owner_impersonation_and_machine_consent_are_denied() {
        let mut f = Fixture::new(1);
        let old = actor(
            "pm",
            false,
            &[
                "task-tracker:write".into(),
                f.initial.scope(f.state.task_id),
            ],
        );
        for denied in [&old, &f.owner(), &f.scheduler()] {
            assert!(f.command(&f.revision_command(), denied, None).is_err());
        }
        f.ready_for_confirmation();
        for denied in [
            &f.pm(),
            &f.scheduler(),
            &actor("owner", false, &[]),
            &actor("other-owner", true, &[]),
        ] {
            assert!(
                f.command(&f.confirm_command(), denied, Some(id(203)))
                    .is_err()
            );
        }
        let (command, receipt) = f.completion();
        for denied in [
            &f.owner(),
            &f.pm(),
            &actor("scheduler", false, &["task-tracker:read".into()]),
            &actor(
                "scheduler",
                false,
                &["task-tracker:read".into(), "task-tracker:write".into()],
            ),
        ] {
            assert!(f.finalize(denied, &command, &receipt).is_err());
        }
    }

    #[test]
    fn expired_machine_write_fails_but_owner_consent_and_finalizing_do_not_renew_lease() {
        let mut f = Fixture::new(1);
        f.ready_for_confirmation();
        f.now += Duration::seconds(40);
        let before = value(&f.lease).unwrap();
        assert!(f.command(&f.revision_command(), &f.pm(), None).is_err());
        f.accept(
            f.command(&f.confirm_command(), &f.owner(), Some(id(203)))
                .unwrap(),
        );
        let (command, receipt) = f.completion();
        f.finalize(&f.scheduler(), &command, &receipt).unwrap();
        assert_eq!(before, value(&f.lease).unwrap());
    }

    #[test]
    fn old_consent_missing_checks_and_stale_hash_cannot_release_review() {
        let mut f = Fixture::new(1);
        assert!(
            f.command(&f.confirm_command(), &f.owner(), Some(id(203)))
                .is_err()
        );
        f.accept(f.command(&f.revision_command(), &f.pm(), None).unwrap());
        assert!(
            f.command(&f.confirm_command(), &f.owner(), Some(id(203)))
                .is_err()
        );
        let (command, receipt) = f.completion();
        assert!(f.finalize(&f.scheduler(), &command, &receipt).is_err());
        let mut f = Fixture::new(1);
        f.ready_for_confirmation();
        let mut confirm = f.confirm_command();
        if let SdlcCommand::Confirm { command, .. } = &mut confirm {
            command.content_hash = "f".repeat(64);
        }
        assert!(f.command(&confirm, &f.owner(), Some(id(203))).is_err());
    }

    #[test]
    fn finalizing_requires_exact_current_command_lease_and_prepared_receipt() {
        let mut f = Fixture::new(1);
        f.confirmed();
        let (command, receipt) = f.completion();
        for change in 0..13 {
            let mut bad = receipt.clone();
            match change {
                0 => bad.fence.assignment_id = f.initial.assignment_id,
                1 => bad.fence.execution_id = f.initial.execution_id,
                2 => bad.fence.agent_id = id(999),
                3 => bad.lease_version += 1,
                4 => bad.lease_id = id(999),
                5 => bad.owner_version -= 1,
                6 => bad.requirement_revision = 1,
                7 => bad.workflow_receipt_sha256 = "c".repeat(64),
                8 => bad.idempotency_key.push('x'),
                9 => bad.owner_subject = "other".into(),
                10 => bad.completion_id = Uuid::nil(),
                11 => bad.previous_pm_admission_state = "completed".into(),
                _ => bad.created_at = f.now - Duration::seconds(1),
            }
            assert!(
                f.finalize(&f.scheduler(), &command, &bad).is_err(),
                "change {change}"
            );
        }
        assert!(
            prepare_critical_pm_completion(
                f.source(),
                &f.scheduler(),
                &f.config,
                &command,
                CriticalPmCompletionAllocation {
                    snapshot_id: id(205),
                    analysis_intent_id: id(206)
                }
            )
            .is_err()
        );
        let mut source = f.source();
        source.execution = CriticalPmExecutionContext::Finalizing {
            lease: &f.lease,
            completion: &receipt,
        };
        assert!(
            prepare_critical_pm_command(
                source,
                &f.pm(),
                &f.config,
                &f.revision_command(),
                CriticalPmCommandAllocation {
                    now: f.now,
                    generated_receipt_id: None
                }
            )
            .is_err()
        );
    }

    #[test]
    fn wrong_source_canonical_binding_original_pm_and_technical_terminal_fail_closed() {
        for change in 0..12 {
            let mut f = Fixture::new(1);
            match change {
                0 => f.state.assignment = Some(f.initial.clone()),
                1 => f.state.retained_fields.get_mut("lifecycle").unwrap()["version"] = json!(99),
                2 => f.blocker.pm_review.as_mut().unwrap().execution_id = id(999),
                3 => f.blocker.technical_terminal = None,
                4 => f.initial.agent_id = id(999),
                5 => f.blocker.proposal.question.push('?'),
                6 => f.review.context.requirements.content_hash = "c".repeat(64),
                7 => f.lease.fence.execution_id = id(999),
                8 => f.lease.dispatch_allowed = true,
                9 => f.blocker.state = CriticalBlockerState::AwaitingPmReview,
                10 => {
                    assert!(f.state.retained_fields.remove("flow").is_some());
                }
                _ => f.config.single_task_project_ids.clear(),
            }
            assert!(
                f.command(&f.revision_command(), &f.pm(), None).is_err(),
                "change {change}"
            );
        }
    }

    #[test]
    fn explicit_allocation_is_deterministic_and_missing_or_reused_receipt_id_is_denied() {
        let mut f = Fixture::new(1);
        f.ready_for_confirmation();
        for bad in [None, Some(Uuid::nil()), Some(f.state.confirmations[0].id)] {
            assert!(f.command(&f.confirm_command(), &f.owner(), bad).is_err());
        }
        let one = f
            .command(&f.confirm_command(), &f.owner(), Some(id(203)))
            .unwrap();
        let two = f
            .command(&f.confirm_command(), &f.owner(), Some(id(203)))
            .unwrap();
        assert_eq!(one.result, two.result);
        assert_eq!(
            value(&one.after.state).unwrap(),
            value(&two.after.state).unwrap()
        );
        assert!(
            f.command(&f.revision_command(), &f.pm(), Some(id(203)))
                .is_err()
        );
    }

    #[test]
    fn feedback_rework_after_consent_needs_new_revision_and_new_actual_owner_consent() {
        let mut f = Fixture::new(1);
        f.confirmed();
        let old_consent = value(f.state.confirmations.last().unwrap()).unwrap();
        f.accept(f.command(&f.revision_command(), &f.pm(), None).unwrap());
        let (command, receipt) = f.completion();
        assert!(f.finalize(&f.scheduler(), &command, &receipt).is_err());
        assert_eq!(
            old_consent,
            value(f.state.confirmations.last().unwrap()).unwrap()
        );
        assert_eq!(
            f.state.assignment.as_ref(),
            Some(&f.review.reservation.assignment)
        );
        assert_eq!(f.blocker.version, 3);
    }

    #[test]
    fn bounded_versions_fail_instead_of_wrapping_or_resetting() {
        let mut f = Fixture::new(1);
        f.lifecycle.task.version = MAX_SAFE_VERSION;
        f.state
            .retained_fields
            .insert("lifecycle".into(), value(&f.lifecycle.task).unwrap());
        assert!(f.command(&f.revision_command(), &f.pm(), None).is_err());
        for change in 0..2 {
            let mut f = Fixture::new(2);
            f.confirmed();
            if change == 0 {
                f.blocker.routing_generation = MAX_SAFE_VERSION;
            } else {
                f.blocker.version = MAX_SAFE_VERSION;
                f.review.context.blocker_version = MAX_SAFE_VERSION;
            }
            let (command, receipt) = f.completion();
            assert!(validate_source(&f.source(), &f.config).is_ok());
            assert!(f.finalize(&f.scheduler(), &command, &receipt).is_err());
        }
    }

    #[test]
    fn question_and_cancel_cannot_rewrite_previous_pm_execution_question() {
        let mut f = Fixture::new(1);
        f.accept(f.command(&f.question_command(), &f.pm(), None).unwrap());
        f.state.questions[0].assignment_id = f.initial.assignment_id;
        let answer = SdlcCommand::Answer {
            question_id: id(201),
            command: AnswerCommand {
                expected_question_version: 1,
                requirement_revision: 1,
                selected_option_ids: vec![],
                text: Some("owner".into()),
                comment: None,
                idempotency_key: "answer".into(),
            },
        };
        assert!(f.command(&answer, &f.owner(), Some(id(207))).is_err());
        let cancel = SdlcCommand::Cancel {
            question_id: id(201),
            command: CancelQuestion {
                fence: fence(&f.review.reservation.assignment),
                expected_question_version: 1,
                idempotency_key: "cancel".into(),
            },
        };
        assert!(f.command(&cancel, &f.pm(), None).is_err());
        let mut question = f.question_command();
        if let SdlcCommand::PublishQuestion(c) = &mut question {
            c.expected_question_version = Some(1);
        }
        assert!(f.command(&question, &f.pm(), None).is_err());
    }

    #[test]
    fn structured_single_choice_has_no_answer_preselection_and_owned_cancel_is_exact() {
        let mut f = Fixture::new(1);
        let mut command = f.question_command();
        if let SdlcCommand::PublishQuestion(c) = &mut command {
            c.mode = QuestionMode::Single;
            c.options.push(QuestionOption {
                id: id(208),
                label: "One real alternative".into(),
                consequences: "Synthetic business consequence".into(),
                is_custom: false,
            });
            c.recommended_option_id = Some(id(208));
        }
        let plan = f.command(&command, &f.pm(), None).unwrap();
        assert!(plan.result["answer"].is_null());
        assert_eq!(plan.result["recommended_option_id"], json!(id(208)));
        f.accept(plan);
        let cancel = SdlcCommand::Cancel {
            question_id: id(201),
            command: CancelQuestion {
                fence: fence(&f.review.reservation.assignment),
                expected_question_version: 1,
                idempotency_key: "fixture:cancel".into(),
            },
        };
        let plan = f.command(&cancel, &f.pm(), None).unwrap();
        assert_eq!(plan.result["state"], json!("cancelled"));
        assert_eq!(
            plan.after.state.assignment.as_ref(),
            Some(&f.review.reservation.assignment)
        );
        assert_eq!(plan.after.blocker.state, CriticalBlockerState::InPmReview);
    }

    #[test]
    fn matching_old_completion_command_and_receipt_still_cannot_finalize_current_review() {
        let mut f = Fixture::new(1);
        f.confirmed();
        let (mut command, mut receipt) = f.completion();
        command.fence = fence(&f.initial);
        receipt.fence = command.fence.clone();
        command.requirement_revision = 1;
        receipt.requirement_revision = 1;
        command.content_hash = f.initial_routing.content_hash.clone();
        receipt.content_hash = command.content_hash.clone();
        assert!(f.finalize(&f.scheduler(), &command, &receipt).is_err());
    }

    #[test]
    fn duplicate_consent_and_routing_allocation_are_not_fresh_operations() {
        let mut f = Fixture::new(1);
        f.confirmed();
        assert!(
            f.command(&f.confirm_command(), &f.owner(), Some(id(209)))
                .is_err()
        );
        let (command, receipt) = f.completion();
        for allocation in [
            CriticalPmCompletionAllocation {
                snapshot_id: Uuid::nil(),
                analysis_intent_id: id(206),
            },
            CriticalPmCompletionAllocation {
                snapshot_id: f.initial_routing.snapshot_id,
                analysis_intent_id: id(206),
            },
            CriticalPmCompletionAllocation {
                snapshot_id: id(205),
                analysis_intent_id: id(205),
            },
        ] {
            let mut source = f.source();
            source.execution = CriticalPmExecutionContext::Finalizing {
                lease: &f.lease,
                completion: &receipt,
            };
            assert!(
                prepare_critical_pm_completion(
                    source,
                    &f.scheduler(),
                    &f.config,
                    &command,
                    allocation
                )
                .is_err()
            );
        }
    }

    #[test]
    fn readiness_cannot_be_supplied_by_pm_or_an_unconfigured_verifier() {
        let mut f = Fixture::new(1);
        f.ready_for_confirmation();
        assert!(ready(&f.state, &f.config));
        for who in ["pm", "other-verifier"] {
            let mut state = f.state.clone();
            for evidence in &mut state.evidence {
                evidence.verifier_subject = who.into();
            }
            let mut source = f.source();
            source.state = &state;
            assert!(
                prepare_critical_pm_command(
                    source,
                    &f.owner(),
                    &f.config,
                    &f.confirm_command(),
                    CriticalPmCommandAllocation {
                        now: f.now,
                        generated_receipt_id: Some(id(203))
                    }
                )
                .is_err()
            );
        }
    }

    fn configured_fixture() -> Fixture {
        let mut f = Fixture::new(1);
        let subject = id(700).to_string();
        f.state
            .retained_fields
            .insert("interaction_protocol".into(), json!("configured-cli-v1"));
        f.state
            .retained_fields
            .insert("workflow_task_key".into(), json!("SDLC-1"));
        f.review.reservation.interaction_protocol = Some(InteractionProtocol::ConfiguredCliV1);
        f.review.reservation.assignment.machine_subject = subject.clone();
        f.state.assignment = Some(f.review.reservation.assignment.clone());
        f.initial.machine_subject = subject;
        for confirmation in &mut f.state.confirmations {
            confirmation.stage = Stage::Draft;
        }
        f.config.orchestrator_subject = "legacy-orchestrator".into();
        f.config.verifier_subject.clear();
        // Deliberately unusable: configured commands must never inspect this.
        f.lease.lease.expires_at = f.now - Duration::days(1);
        f.lease.fence.execution_id = Uuid::nil();
        f
    }

    #[test]
    fn configured_review_publishes_and_confirms_without_a_native_lease_or_verifier() {
        let mut f = configured_fixture();
        let command = f.revision_command();
        f.accept(f.command(&command, &f.pm(), None).unwrap());
        assert_eq!(f.state.current_revision(), Some(2));
        let consent = f.confirm_command();
        assert!(f.command(&consent, &f.pm(), Some(id(209))).is_err());
        f.accept(f.command(&consent, &f.owner(), Some(id(209))).unwrap());
        assert_eq!(f.state.stage, Stage::Draft);
        assert_eq!(f.state.status, Some(LifecycleStatus::InProgress));
        assert_eq!(f.state.current_confirmation().unwrap().revision, 2);
        assert_eq!(
            f.lifecycle.task.waiting_reason.as_deref(),
            Some(PM_FINALIZATION_REASON)
        );
        assert!(f.lifecycle.assignment.is_none() && f.lifecycle.lease.is_none());
        assert_eq!(f.blocker.state, CriticalBlockerState::InPmReview);
    }

    #[test]
    fn configured_review_questions_have_no_checkpoint_and_reject_stale_or_foreign_writes() {
        let mut f = configured_fixture();
        let command = f.question_command();
        for kind in 0..4 {
            let mut changed = command.clone();
            let mut principal = f.pm();
            if let SdlcCommand::PublishQuestion(q) = &mut changed {
                match kind {
                    0 => q.question_id = Uuid::nil(),
                    1 => q.fence.assignment_version += 1,
                    2 => principal.subject = id(701).to_string(),
                    _ => {
                        principal.scopes.remove("task-tracker:write");
                    }
                };
            }
            assert!(
                f.command(&changed, &principal, None).is_err(),
                "case {kind}"
            );
        }
        let plan = f.command(&command, &f.pm(), None).unwrap();
        assert!(plan.result["checkpoint_id"].is_null());
        f.accept(plan);
        // An unanswered required question blocks the next revision and consent.
        assert!(f.command(&f.revision_command(), &f.pm(), None).is_err());
        assert!(
            f.command(&f.confirm_command(), &f.owner(), Some(id(209)))
                .is_err()
        );
    }

    #[test]
    fn configured_review_completion_resolves_to_backlog_and_keeps_the_frozen_policy() {
        use domain::sdlc_configured_cli::{CompleteConfiguredPm, ConfiguredWorkflowReceipt};
        let mut f = configured_fixture();
        f.accept(f.command(&f.revision_command(), &f.pm(), None).unwrap());
        f.accept(
            f.command(&f.confirm_command(), &f.owner(), Some(id(209)))
                .unwrap(),
        );
        let revision = f.state.revisions.last().unwrap();
        let command = CompleteConfiguredPm {
            interaction_protocol: InteractionProtocol::ConfiguredCliV1,
            fence: fence(&f.review.reservation.assignment),
            requirement_revision: revision.revision,
            content_hash: revision.content_hash.clone(),
            namespace_id: 1,
            workflow_execution_ref: id(800),
            workflow_operation_key: "fixture:workflow-report".into(),
            workflow_result_version: 7,
            idempotency_key: "fixture:complete-business".into(),
        };
        let receipt = ConfiguredWorkflowReceipt {
            interaction_protocol: InteractionProtocol::ConfiguredCliV1,
            execution_ref: command.workflow_execution_ref,
            task: "SDLC-1".into(),
            tracker_task_id: f.state.task_id,
            namespace_id: 1,
            concrete_agent_ref: command.fence.agent_id,
            assignment_ref: command.fence.assignment_id,
            tracker_execution_id: command.fence.execution_id,
            assignment_version: command.fence.assignment_version,
            machine_subject: f.pm().subject,
            operation_key: command.workflow_operation_key.clone(),
            request_sha256: "a".repeat(64),
            result_version: command.workflow_result_version,
            requirements_revision: Some(command.requirement_revision),
            requirements_sha256: Some(command.content_hash.clone()),
            owner_subject: f.state.owner_subject.clone(),
            confirmation_id: Some(id(209)),
            history_id: 85,
            status: "done".into(),
        };
        let completion = crate::sdlc_configured_cli::completion_result(
            &f.state,
            &command,
            receipt,
            f.review.reservation.owner_cas.version,
            id(801),
            f.now,
        )
        .unwrap();
        let before = value(&f.state).unwrap();
        let plan = prepare_configured_critical_pm_completion(
            f.source(),
            &f.pm(),
            &f.config,
            &command,
            &completion,
            id(802),
        )
        .unwrap();
        assert_eq!(plan.after.state.stage, Stage::Backlog);
        assert_eq!(plan.after.lifecycle.task.status, LifecycleStatus::Ready);
        assert_eq!(plan.after.lifecycle.task.runtime, RuntimeState::Terminal);
        assert!(plan.after.lifecycle.task.waiting_reason.is_none());
        assert!(plan.after.lifecycle.assignment.is_none() && plan.after.lifecycle.lease.is_none());
        assert_eq!(plan.after.blocker.state, CriticalBlockerState::Resolved);
        assert_eq!(plan.routing.generation, 2);
        assert_eq!(plan.routing.snapshot.policy, f.initial_routing.policy);
        assert_eq!(plan.routing.snapshot.confirmation_id, id(209));
        assert_eq!(value(&f.state).unwrap(), before);
        for kind in 0..5 {
            let mut altered = completion.clone();
            let mut principal = f.pm();
            match kind {
                0 => altered.workflow_receipt.requirements_revision = Some(1),
                1 => altered.workflow_receipt.confirmation_id = Some(id(1001)),
                2 => altered.completion_id = Uuid::nil(),
                3 => altered.created_at = f.now - Duration::seconds(1),
                _ => principal.subject = id(701).to_string(),
            }
            assert!(
                prepare_configured_critical_pm_completion(
                    f.source(),
                    &principal,
                    &f.config,
                    &command,
                    &altered,
                    id(802),
                )
                .is_err(),
                "case {kind}"
            );
        }
    }
}
