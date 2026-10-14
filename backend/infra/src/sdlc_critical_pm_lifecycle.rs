//! Companion only. Every entry uses the caller's already locked transaction; never commits it.
use super::*;
use app::sdlc_critical_pm_lifecycle::{
    prepare_critical_pm_command, prepare_critical_pm_completion,
};
use domain::sdlc_critical_blockers::{CriticalBlockerReceipt, CriticalBlockerState};
use domain::sdlc_critical_pm_lifecycle::*;
use domain::sdlc_critical_pm_reservation::CriticalPmReservationResult;
use domain::sdlc_execution_lease::{
    ClaimExecutionLease, ExecutionLeaseReceipt, ResumePmExecutionLease,
};
use domain::sdlc_lifecycle::{LifecycleReceipt, LifecycleStatus, RuntimeState};
use domain::sdlc_reservation::{CompletePmDraft, PmDraftCompletionReceipt};
use domain::sdlc_routing::TaskRoutingSnapshot;

fn inconsistent() -> AppError {
    AppError::conflict("critical PM lifecycle current review inconsistent")
}

fn decode<T: serde::de::DeserializeOwned>(row: &QueryResult, field: &str) -> Result<T, AppError> {
    serde_json::from_value(row.try_get::<Json>("", field).map_err(map_db)?)
        .map_err(|_| inconsistent())
}

struct Review {
    saved: CriticalPmReservationResult,
    blocker: CriticalBlockerReceipt,
    lifecycle: LifecycleReceipt,
    initial_pm: PmAssignment,
    initial_routing: TaskRoutingSnapshot,
}

impl Review {
    fn source<'a>(
        &'a self,
        state: &'a TaskState,
        execution: CriticalPmExecutionContext<'a>,
    ) -> CriticalPmLifecycleSource<'a> {
        CriticalPmLifecycleSource {
            state,
            lifecycle: &self.lifecycle,
            review: &self.saved,
            blocker: &self.blocker,
            initial_pm_assignment: &self.initial_pm,
            initial_routing: &self.initial_routing,
            execution,
        }
    }
    fn before(&self, state: &TaskState) -> CriticalPmProjection {
        CriticalPmProjection {
            state: state.clone(),
            lifecycle: self.lifecycle.clone(),
            blocker: self.blocker.clone(),
        }
    }
}

async fn owner_access(tx: &DatabaseTransaction, state: &TaskState) -> Result<(), AppError> {
    let owner = tx
        .query_one(statement(
            "SELECT u.id,p.owner_id FROM users u JOIN projects p ON p.id=$1
        WHERE u.central_sub=$2 AND u.is_active FOR SHARE OF u,p",
            vec![state.project_id.into(), state.owner_subject.clone().into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    let id = owner.try_get::<Uuid>("", "id").map_err(map_db)?;
    if owner.try_get::<Uuid>("", "owner_id").map_err(map_db)? != id {
        tx.query_one(statement(
            "SELECT user_id FROM project_members WHERE project_id=$1 AND user_id=$2 FOR SHARE",
            vec![state.project_id.into(), id.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    }
    Ok(())
}

impl PostgresSdlcRepository {
    async fn current_critical_pm_review(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        observed: Option<&LifecycleReceipt>,
    ) -> Result<Option<Review>, AppError> {
        let row=tx.query_one(statement("SELECT r.result,r.after_state,r.execution_id,r.owner_version,t.pm_execution_id,
            t.pm_owner_version,t.pm_admission_state,sdlc_critical_pm_detail(r.blocker_id) AS blocker
            FROM sdlc_critical_pm_reviews r JOIN sdlc_tasks t ON t.task_id=r.task_id
            JOIN sdlc_critical_blocker_heads h ON h.task_id=r.task_id AND h.blocker_id=r.blocker_id
            WHERE r.task_id=$1 AND r.execution_id=t.pm_execution_id AND t.pm_admission_state='reserved' FOR SHARE OF r",
            vec![state.task_id.into()])).await.map_err(map_db)?;
        let Some(row) = row else {
            let found=tx.query_one(statement("SELECT EXISTS(SELECT 1 FROM sdlc_critical_pm_reviews r JOIN sdlc_tasks t ON t.task_id=r.task_id
                    WHERE r.task_id=$1 AND r.execution_id=t.pm_execution_id AND t.pm_admission_state='reserved')
                OR EXISTS(SELECT 1 FROM sdlc_critical_blocker_heads h WHERE h.task_id=$1
                    AND sdlc_critical_pm_detail(h.blocker_id)->>'state'='in_pm_review') AS found",
                vec![state.task_id.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
            if found.try_get::<bool>("", "found").map_err(map_db)? {
                return Err(inconsistent());
            }
            return Ok(None);
        };
        owner_access(tx, state).await?;
        let saved: CriticalPmReservationResult = decode(&row, "result")?;
        let blocker: CriticalBlockerReceipt = decode(&row, "blocker")?;
        let lifecycle = match observed {
            Some(receipt) => receipt.clone(),
            None => super::lifecycle::current(tx, state.task_id)
                .await?
                .ok_or_else(inconsistent)?,
        };
        let initial_pm = decode(
            &tx.query_one(statement(
                "SELECT payload FROM sdlc_assignments
            WHERE task_id=$1 ORDER BY version LIMIT 1 FOR SHARE",
                vec![state.task_id.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(inconsistent)?,
            "payload",
        )?;
        let initial_routing = decode(
            &tx.query_one(statement(
                "SELECT payload FROM sdlc_task_routing_snapshots
            WHERE task_id=$1 FOR SHARE",
                vec![state.task_id.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(inconsistent)?,
            "payload",
        )?;
        if state.draft_flow()? != Some(DraftFlow::SingleTaskV1)
            || state.stage != Stage::Draft
            || !self
                .config
                .single_task_project_ids
                .contains(&state.project_id)
            || blocker.state != CriticalBlockerState::InPmReview
            || blocker.resolution.is_some()
            || state.assignment.as_ref() != Some(&saved.reservation.assignment)
            || row
                .try_get::<Option<Uuid>>("", "pm_execution_id")
                .map_err(map_db)?
                != Some(saved.reservation.assignment.execution_id)
            || row.try_get::<Uuid>("", "execution_id").map_err(map_db)?
                != saved.reservation.assignment.execution_id
            || row.try_get::<i64>("", "pm_owner_version").map_err(map_db)?
                != saved.reservation.owner_cas.version
            || row.try_get::<i64>("", "owner_version").map_err(map_db)?
                != saved.reservation.owner_cas.version
            || row
                .try_get::<Option<String>>("", "pm_admission_state")
                .map_err(map_db)?
                .as_deref()
                != Some("reserved")
            || blocker.pm_review.as_ref().is_none_or(|r| {
                r.execution_id != saved.reservation.assignment.execution_id
                    || r.assignment_id != saved.reservation.assignment.assignment_id
            })
            || blocker.blocker_id != saved.context.blocker_id
            || blocker.version != saved.context.blocker_version
            || blocker.blocker_sha256 != saved.context.blocker_sha256
            || blocker.task_id != state.task_id
            || blocker.owner_subject != state.owner_subject
            || blocker.project_id != state.project_id
            || blocker.technical_terminal.is_none()
            || blocker.blocker_sha256 != app::sdlc::canonical_hash(&blocker.proposal)?
            || blocker.routing_snapshot_id != lifecycle.task.routing.snapshot_id
            || lifecycle.task.routing.requirement_revision != saved.context.requirements.revision
            || lifecycle.task.routing.content_hash != saved.context.requirements.content_hash
            || row.try_get::<Json>("", "after_state").map_err(map_db)?["lifecycle"]["routing"]
                != serde_json::to_value(&lifecycle.task.routing).map_err(AppError::internal)?
            || state.tracker_instance_id != self.config.instance_id
            || lifecycle.assignment.is_some()
            || lifecycle.lease.is_some()
            || lifecycle.binding.is_some()
            || lifecycle.result.is_some()
            || app::sdlc_lifecycle::canonical_projection(
                &serde_json::to_value(state).map_err(AppError::internal)?,
                &lifecycle.task,
            )? != serde_json::to_value(state).map_err(AppError::internal)?
        {
            return Err(inconsistent());
        }
        let review = Review {
            saved,
            blocker,
            lifecycle,
            initial_pm,
            initial_routing,
        };
        validate_review_snapshot(tx, state, &review).await?;
        Ok(Some(review))
    }

    /// Root/project are already locked by the existing caller. No recursive call to current().
    /// None means no current review, not a stale/missing proof for an otherwise active review.
    pub(super) async fn review_snapshot(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        receipt: &LifecycleReceipt,
    ) -> Result<Option<TaskRoutingSnapshot>, AppError> {
        let Some(review) = self
            .current_critical_pm_review(tx, state, Some(receipt))
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(review.lifecycle.task.routing))
    }

    /// Called instead of generic apply/finish, after replay and existing producer verification.
    pub(super) async fn execute_critical_pm_review_command(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
        command: &SdlcCommand,
        request_sha256: &str,
    ) -> Result<Option<Json>, AppError> {
        let Some(review) = self.current_critical_pm_review(tx, state, None).await? else {
            return Ok(None);
        };
        if app::sdlc::command_hash(command)? != request_sha256 {
            return Err(inconsistent());
        }
        let lease = Self::lease_current(tx, &review.saved.reservation)
            .await?
            .ok_or_else(inconsistent)?;
        let now = super::execution_lease::clock(tx).await?;
        let generated = matches!(
            command,
            SdlcCommand::Answer { .. } | SdlcCommand::Confirm { .. }
        )
        .then(Uuid::now_v7);
        let plan = prepare_critical_pm_command(
            review.source(
                state,
                CriticalPmExecutionContext::Open {
                    lease: &lease,
                    observed_at: now,
                },
            ),
            actor,
            &self.config,
            command,
            CriticalPmCommandAllocation {
                now,
                generated_receipt_id: generated,
            },
        )?;
        let event_id = Uuid::now_v7();
        write_operation(
            tx,
            actor,
            "command",
            command.key(),
            request_sha256,
            &serde_json::to_value(command).map_err(AppError::internal)?,
            &plan.result,
            &lease,
            &plan.before,
            &plan.after,
            event_id,
            None,
        )
        .await?;
        let event = if let SdlcCommand::Confirm {
            revision,
            command: c,
        } = command
        {
            let confirmation: Confirmation =
                serde_json::from_value(plan.result.clone()).map_err(AppError::internal)?;
            exec(tx,"INSERT INTO sdlc_confirmations(id,task_id,revision,content_hash,payload) VALUES($1,$2,$3,$4,$5)",
                vec![confirmation.id.into(),state.task_id.into(),(*revision).into(),c.content_hash.clone().into(),plan.result.clone().into()]).await?;
            "requirements.confirmed"
        } else {
            self.persist_result(tx, state.task_id, command, &plan.result, &plan.after.state)
                .await?
        };
        save_projection(tx, actor, &plan.after, false).await?;
        exec(tx,"INSERT INTO sdlc_idempotency(task_id,actor_subject,idempotency_key,payload_hash,result) VALUES($1,$2,$3,$4,$5)",
            vec![state.task_id.into(),actor.subject.clone().into(),command.key().into(),request_sha256.into(),plan.result.clone().into()]).await?;
        write_outbox(
            tx,
            event_id,
            event,
            actor,
            &plan.after.state,
            &plan.result,
            Stage::Draft,
        )
        .await?;
        Ok(Some(plan.result))
    }

    pub(super) async fn project_critical_pm_claim(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
        command: &ClaimExecutionLease,
        request_sha256: &str,
        receipt: &ExecutionLeaseReceipt,
    ) -> Result<Option<()>, AppError> {
        let Some(review) = self.current_critical_pm_review(tx, state, None).await? else {
            return Ok(None);
        };
        if app::sdlc::canonical_hash(
            &json!({"operation":"claim_pm_execution_lease","payload":command}),
        )? != request_sha256
            || actor.human_session
            || actor.trusted_human
        {
            return Err(inconsistent());
        }
        super::execution_lease::valid_receipt(receipt, &review.saved.reservation)?;
        super::execution_lease::check_fence(
            &review.saved.reservation,
            command.expected_owner_version,
            &command.fence,
        )?;
        if actor.subject != review.saved.reservation.assignment.machine_subject
            || !actor
                .scopes
                .contains(&review.saved.reservation.assignment.scope(state.task_id))
            || receipt.lease.version != 1
        {
            return Err(inconsistent());
        }
        lease_projection(
            tx,
            actor,
            "claim",
            &command.idempotency_key,
            request_sha256,
            &serde_json::to_value(command).map_err(AppError::internal)?,
            receipt,
            review.before(state),
        )
        .await?;
        Ok(Some(()))
    }

    /// Replaces only the old unconditional delivery-head overlap and stage validation for this review.
    pub(super) async fn allow_critical_pm_retained_head(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        command: &ResumePmExecutionLease,
    ) -> Result<Option<()>, AppError> {
        let Some(review) = self.current_critical_pm_review(tx, state, None).await? else {
            return Ok(None);
        };
        app::sdlc_pm_resume::validate_command(command)?;
        super::execution_lease::check_fence(
            &review.saved.reservation,
            command.expected_owner_version,
            &command.checkpoint.fence,
        )?;
        if command.checkpoint.requirement_revision != state.current_revision()
            || command.checkpoint.content_hash.as_deref()
                != state.revisions.last().map(|r| r.content_hash.as_str())
            || matches!(
                command.input,
                domain::sdlc_execution_lease::PmResumeInput::Confirmation { .. }
            ) && review.lifecycle.task.waiting_reason.as_deref() != Some(PM_FINALIZATION_REASON)
        {
            return Err(inconsistent());
        }
        let allowed = tx
            .query_one(statement(
                "SELECT sdlc_critical_pm_retained_head_allowed($1,$2) AS allowed",
                vec![
                    state.task_id.into(),
                    review.saved.reservation.assignment.execution_id.into(),
                ],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(inconsistent)?;
        if !allowed.try_get::<bool>("", "allowed").map_err(map_db)? {
            return Err(inconsistent());
        }
        Ok(Some(()))
    }

    pub(super) async fn project_critical_pm_resume(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
        command: &ResumePmExecutionLease,
        request_sha256: &str,
        receipt: &ExecutionLeaseReceipt,
    ) -> Result<Option<()>, AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        if actor.trusted_human {
            return Err(AppError::Forbidden);
        }
        let Some(review) = self.current_critical_pm_review(tx, state, None).await? else {
            return Ok(None);
        };
        if app::sdlc_pm_resume::request_hash(command)? != request_sha256 {
            return Err(inconsistent());
        }
        super::execution_lease::valid_receipt(receipt, &review.saved.reservation)?;
        if command.lease_id != receipt.lease.lease_id
            || command.expected_lease_version.checked_add(1) != Some(receipt.lease.version)
            || command.checkpoint.fence != receipt.fence
        {
            return Err(inconsistent());
        }
        lease_projection(
            tx,
            actor,
            "resume",
            &command.idempotency_key,
            request_sha256,
            &serde_json::to_value(command).map_err(AppError::internal)?,
            receipt,
            review.before(state),
        )
        .await?;
        Ok(Some(()))
    }

    /// Parent has verified actual Workflow/native terminal and prepared THIS receipt in THIS transaction.
    /// Must run BEFORE the generic completion INSERT/tail. It inserts all completion effects itself.
    pub(super) async fn finalize_critical_pm_review(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
        command: &CompletePmDraft,
        request_sha256: &str,
        prepared: &PmDraftCompletionReceipt,
    ) -> Result<Option<()>, AppError> {
        let Some(review) = self.current_critical_pm_review(tx, state, None).await? else {
            return Ok(None);
        };
        if app::sdlc::canonical_hash(&json!({"operation":"complete_pm_draft","payload":command}))?
            != request_sha256
        {
            return Err(inconsistent());
        }
        let lease = Self::lease_current(tx, &review.saved.reservation)
            .await?
            .ok_or_else(inconsistent)?;
        let plan = prepare_critical_pm_completion(
            review.source(
                state,
                CriticalPmExecutionContext::Finalizing {
                    lease: &lease,
                    completion: prepared,
                },
            ),
            actor,
            &self.config,
            command,
            CriticalPmCompletionAllocation {
                snapshot_id: Uuid::now_v7(),
                analysis_intent_id: Uuid::now_v7(),
            },
        )?;
        let generation = &plan.routing;
        let intent = &plan.analysis_intent;
        let result = &plan.completion;
        let operation_id = write_operation(
            tx,
            actor,
            "complete",
            &command.idempotency_key,
            request_sha256,
            &serde_json::to_value(command).map_err(AppError::internal)?,
            &serde_json::to_value(result).map_err(AppError::internal)?,
            &lease,
            &plan.before,
            &plan.after,
            result.completion_id,
            Some(result.completion_id),
        )
        .await?;
        exec(tx,"INSERT INTO sdlc_pm_completions(completion_id,task_id,execution_id,assignment_id,assignment_version,
            lease_id,lease_version,requirement_revision,content_hash,terminal_receipt_ref,terminal_receipt_sha256,
            workflow_receipt_ref,workflow_receipt_sha256,result) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
            vec![result.completion_id.into(),state.task_id.into(),result.fence.execution_id.into(),result.fence.assignment_id.into(),
                result.fence.assignment_version.into(),result.lease_id.into(),result.lease_version.into(),result.requirement_revision.into(),
                result.content_hash.clone().into(),result.terminal_receipt_ref.clone().into(),result.terminal_receipt_sha256.clone().into(),
                result.workflow_receipt_ref.clone().into(),result.workflow_receipt_sha256.clone().into(),json_value(result)?]).await?;
        exec(tx,"INSERT INTO sdlc_pm_completion_operations(task_id,idempotency_key,actor_subject,payload_hash,completion_id,result)
            VALUES($1,$2,$3,$4,$5,$6)",vec![state.task_id.into(),command.idempotency_key.clone().into(),actor.subject.clone().into(),
                request_sha256.into(),result.completion_id.into(),json_value(result)?]).await?;
        exec(tx,"INSERT INTO sdlc_task_routing_generations(task_id,generation,snapshot_id,previous_snapshot_id,blocker_id,
            execution_id,completion_id,confirmation_id,requirement_revision,content_hash,project_id,policy_version,payload,operation_id)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",vec![state.task_id.into(),generation.generation.into(),
                generation.snapshot.snapshot_id.into(),generation.previous_snapshot_id.into(),plan.before.blocker.blocker_id.into(),
                result.fence.execution_id.into(),result.completion_id.into(),generation.snapshot.confirmation_id.into(),
                generation.snapshot.requirement_revision.into(),generation.snapshot.content_hash.clone().into(),state.project_id.into(),
                generation.snapshot.policy.version.into(),json_value(&generation.snapshot)?,operation_id.into()]).await?;
        exec(tx,"INSERT INTO sdlc_analysis_intents(intent_id,task_id,requirement_revision,content_hash,confirmation_id,
            operation_key,payload,routing_snapshot_id,routing_generation,review_routing_snapshot_id)
            VALUES($1,$2,$3,$4,$5,$6,$7,NULL,$8,$9)",vec![intent.intent_id.into(),state.task_id.into(),intent.requirement_revision.into(),
                intent.content_hash.clone().into(),intent.confirmation_id.into(),intent.operation_key.clone().into(),json_value(intent)?,
                generation.generation.into(),generation.snapshot.snapshot_id.into()]).await?;
        save_projection(tx, actor, &plan.after, true).await?;
        write_outbox(
            tx,
            result.completion_id,
            "pm.completed",
            actor,
            &plan.after.state,
            &serde_json::to_value(result).map_err(AppError::internal)?,
            Stage::Analysis,
        )
        .await?;
        write_outbox(
            tx,
            intent.intent_id,
            "analysis.intent_created",
            actor,
            &plan.after.state,
            &serde_json::to_value(intent).map_err(AppError::internal)?,
            Stage::Analysis,
        )
        .await?;
        Ok(Some(()))
    }
}

async fn validate_review_snapshot(
    tx: &DatabaseTransaction,
    state: &TaskState,
    review: &Review,
) -> Result<(), AppError> {
    let snapshot = &review.lifecycle.task.routing;
    app::sdlc_routing::validate_routes(&snapshot.policy.routes)?;
    let row = tx
        .query_one(statement(
            "SELECT payload,generation FROM sdlc_routing_snapshot_inventory
        WHERE task_id=$1 AND snapshot_id=$2",
            vec![state.task_id.into(), snapshot.snapshot_id.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or_else(inconsistent)?;
    if row.try_get::<Json>("", "payload").map_err(map_db)?
        != serde_json::to_value(snapshot).map_err(AppError::internal)?
        || row.try_get::<i64>("", "generation").map_err(map_db)?
            != review.blocker.routing_generation
        || snapshot.policy != review.initial_routing.policy
    {
        return Err(inconsistent());
    }
    // Only this journal-verified review can retain its approved baseline while latest changes.
    let mut baseline = state.clone();
    baseline
        .revisions
        .retain(|r| r.revision <= snapshot.requirement_revision);
    super::lifecycle::exact_snapshot(&baseline, snapshot)
}

fn execution(receipt: &LifecycleReceipt) -> Result<Json, AppError> {
    let mut v = serde_json::to_value(receipt).map_err(AppError::internal)?;
    v.as_object_mut().ok_or_else(inconsistent)?.remove("task");
    Ok(v)
}

#[allow(clippy::too_many_arguments)]
async fn write_operation(
    tx: &DatabaseTransaction,
    actor: &Principal,
    kind: &str,
    key: &str,
    hash: &str,
    command: &Json,
    result: &Json,
    lease: &ExecutionLeaseReceipt,
    before: &CriticalPmProjection,
    after: &CriticalPmProjection,
    outbox: Uuid,
    completion: Option<Uuid>,
) -> Result<Uuid, AppError> {
    let operation_id = Uuid::now_v7();
    exec(tx,"INSERT INTO sdlc_critical_pm_lifecycle_operations(operation_id,task_id,blocker_id,execution_id,actor_subject,
        kind,idempotency_key,request_sha256,command,result,lease_id,lease_version,before_state,after_state,
        before_execution,after_execution,before_blocker,after_blocker,outbox_event_id,completion_id,owner_version)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21)",
        vec![operation_id.into(),before.state.task_id.into(),before.blocker.blocker_id.into(),lease.fence.execution_id.into(),
            actor.subject.clone().into(),kind.into(),key.into(),hash.into(),command.clone().into(),result.clone().into(),lease.lease.lease_id.into(),
            lease.lease.version.into(),json_value(&before.state)?,json_value(&after.state)?,execution(&before.lifecycle)?.into(),
            execution(&after.lifecycle)?.into(),json_value(&before.blocker)?,json_value(&after.blocker)?,outbox.into(),completion.into(),lease.owner_version.into()]).await?;
    Ok(operation_id)
}

async fn save_projection(
    tx: &DatabaseTransaction,
    actor: &Principal,
    after: &CriticalPmProjection,
    completed: bool,
) -> Result<(), AppError> {
    let task = after.state.task_id;
    let state = serde_json::to_value(&after.state).map_err(AppError::internal)?;
    let head = execution(&after.lifecycle)?;
    exec(tx,"INSERT INTO sdlc_delivery_projection_permits(task_id,transaction_id,next_state,next_execution)
        VALUES($1,pg_current_xact_id(),$2,$3)",vec![task.into(),state.clone().into(),head.clone().into()]).await?;
    exec(tx,"INSERT INTO sdlc_pm_state_write_permits(task_id,transaction_id,next_state)
        SELECT $1,pg_current_xact_id(),$2 FROM sdlc_tasks WHERE task_id=$1 AND state IS DISTINCT FROM $2",
        vec![task.into(),state.clone().into()]).await?;
    let affected=tx.execute(statement("UPDATE sdlc_tasks SET state=$2,
        pm_admission_state=CASE WHEN $3 THEN 'completed' ELSE pm_admission_state END
        WHERE task_id=$1 AND pm_admission_state='reserved' AND pm_execution_id=($2->'assignment'->>'execution_id')::uuid",
        vec![task.into(),state.into(),completed.into()])).await.map_err(map_db)?.rows_affected();
    if affected != 1 {
        return Err(inconsistent());
    }
    exec(
        tx,
        "UPDATE sdlc_delivery_heads SET execution=$2 WHERE task_id=$1",
        vec![task.into(), head.into()],
    )
    .await?;
    let status =
        super::lifecycle::mapped_issue_status(tx, after.state.stage, after.lifecycle.task.status)
            .await?;
    super::lifecycle::project_issue_status(tx, task, actor, status).await
}

#[allow(clippy::too_many_arguments)]
async fn lease_projection(
    tx: &DatabaseTransaction,
    actor: &Principal,
    kind: &str,
    key: &str,
    hash: &str,
    command: &Json,
    receipt: &ExecutionLeaseReceipt,
    before: CriticalPmProjection,
) -> Result<(), AppError> {
    let mut after = before.clone();
    after.lifecycle.task.version = after
        .lifecycle
        .task
        .version
        .checked_add(1)
        .filter(|v| *v <= MAX_SAFE_VERSION)
        .ok_or_else(inconsistent)?;
    // Actual lease ownership is Reserved, not a fabricated native-running observation.
    after.lifecycle.task.runtime = RuntimeState::Reserved;
    if after.lifecycle.task.waiting_reason.as_deref() != Some(PM_FINALIZATION_REASON) {
        after.lifecycle.task.status = LifecycleStatus::InProgress;
    }
    after.state = serde_json::from_value(app::sdlc_lifecycle::canonical_projection(
        &serde_json::to_value(&after.state).map_err(AppError::internal)?,
        &after.lifecycle.task,
    )?)
    .map_err(AppError::internal)?;
    let event_id = Uuid::now_v7();
    let result = serde_json::to_value(receipt).map_err(AppError::internal)?;
    write_operation(
        tx, actor, kind, key, hash, command, &result, receipt, &before, &after, event_id, None,
    )
    .await?;
    save_projection(tx, actor, &after, false).await?;
    write_outbox(
        tx,
        event_id,
        if kind == "claim" {
            "critical.pm_review_lease_claimed"
        } else {
            "critical.pm_review_resumed"
        },
        actor,
        &after.state,
        &result,
        Stage::Draft,
    )
    .await
}

async fn write_outbox(
    tx: &DatabaseTransaction,
    event_id: Uuid,
    event: &str,
    _actor: &Principal,
    state: &TaskState,
    result: &Json,
    stage: Stage,
) -> Result<(), AppError> {
    exec(tx,"INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,$3,$4)",
        vec![event_id.into(),state.task_id.into(),event.into(),json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
            "project_id":state.project_id,"task_id":state.task_id,"root_task_id":state.root_task_id,"owner_subject":state.owner_subject,
            "stage":stage,"requirement_revision":state.current_revision(),"result":result}).into()]).await
}

#[cfg(test)]
mod tests {
    const SQL: &str = include_str!("../../migration/src/critical_pm_review_lifecycle.sql");
    #[test]
    fn generation_family_has_both_exact_fk_and_closed_null_rules() {
        for required in [
            "sdlc_analysis_intent_routing_family",
            "review_routing_snapshot_id IS NOT NULL",
            "routing_snapshot_id IS NULL",
            "REFERENCES sdlc_task_routing_generations",
            "sdlc_analysis_intents",
            "DEFERRABLE INITIALLY DEFERRED",
        ] {
            assert!(SQL.contains(required), "{required}");
        }
        assert!(!SQL.contains("DROP TABLE sdlc_task_routing_snapshots"));
    }
    #[test]
    fn finalization_is_journal_first_and_deferred_atomic_not_prior_completion() {
        for required in [
            "SDLC current review completion already exists",
            "sdlc_critical_pm_lifecycle_commit_gate",
            "NEW.transaction_id=pg_current_xact_id()",
            "sdlc_pm_completion_operations",
            "sdlc_task_routing_generations",
            "sdlc_delivery_projection_permits",
            "sdlc_pm_state_write_permits",
        ] {
            assert!(SQL.contains(required), "{required}");
        }
    }

    #[test]
    fn inventory_is_read_only_and_collision_guard_serializes_both_authorities() {
        assert!(SQL.contains("CREATE VIEW sdlc_routing_snapshot_inventory AS"));
        assert!(SQL.contains("1::bigint AS generation FROM sdlc_task_routing_snapshots"));
        assert!(SQL.contains("UNION ALL SELECT snapshot_id,task_id,project_id,policy_version,confirmation_id,requirement_revision,content_hash,payload"));
        assert!(
            SQL.contains("pg_advisory_xact_lock(hashtextextended(NEW.snapshot_id::text,61014))")
        );
        assert!(SQL.contains("BEFORE INSERT ON sdlc_task_routing_snapshots"));
        assert!(SQL.contains("BEFORE INSERT ON sdlc_task_routing_generations"));
        assert!(SQL.contains("NEW.generation=previous_generation+1"));
        assert!(!SQL.contains("UPDATE sdlc_task_routing_snapshots"));
        assert!(!SQL.contains("INSERT INTO sdlc_task_routing_snapshots"));
    }

    #[test]
    fn narrow_exceptions_require_exact_transaction_and_old_guards_remain() {
        assert!(SQL.contains("j.transaction_id=pg_current_xact_id() AND j.before_state=p_before AND j.after_state=p_after"));
        assert!(SQL.contains(
            "SDLC review completion requires same-transaction resolution/generation journal"
        ));
        assert!(SQL.contains("SDLC only resolved blocker head may CAS"));
        assert!(SQL.contains("IS DISTINCT FROM ''resolved''"));
        assert!(SQL.contains("sdlc_pm_state_write_permits"));
        assert!(!SQL.contains("DISABLE TRIGGER"));
        assert!(!SQL.contains("DROP FUNCTION"));
        assert!(!SQL.contains("UPDATE sdlc_project_routing_revisions"));
    }
}
