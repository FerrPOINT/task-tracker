//! Child of sdlc. Atomic critical hold/terminal return, without PM allocation or another queue.
use super::*;
use crate::entities::attachment;
use domain::attachment_artifact::Seal;
use domain::sdlc_critical_blockers::*;
use domain::sdlc_lifecycle::*;
use sea_orm::EntityTrait;

#[path = "attachment_pm_review.rs"]
pub(super) mod pm_evidence;

fn invalid_history() -> AppError {
    AppError::conflict("critical blocker history inconsistent")
}

fn decode<T: serde::de::DeserializeOwned>(row: &QueryResult, field: &str) -> Result<T, AppError> {
    serde_json::from_value(row.try_get::<Json>("", field).map_err(map_db)?)
        .map_err(|_| invalid_history())
}

async fn owner_access(tx: &DatabaseTransaction, state: &TaskState) -> Result<(), AppError> {
    let owner: Uuid = tx
        .query_one(statement(
            "SELECT id FROM users WHERE central_sub=$1 AND is_active FOR SHARE",
            vec![state.owner_subject.clone().into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?
        .try_get("", "id")
        .map_err(map_db)?;
    let project_owner: Uuid = tx
        .query_one(statement(
            "SELECT owner_id FROM projects WHERE id=$1 FOR SHARE",
            vec![state.project_id.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?
        .try_get("", "owner_id")
        .map_err(map_db)?;
    if owner != project_owner {
        tx.query_one(statement(
            "SELECT user_id FROM project_members WHERE project_id=$1 AND user_id=$2 FOR SHARE",
            vec![state.project_id.into(), owner.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    }
    Ok(())
}

fn scope(state: &TaskState, saved: &CriticalBlockerReceipt) -> Result<(), AppError> {
    if saved.task_id != state.task_id
        || saved.root_task_id != state.task_id
        || saved.project_id != state.project_id
        || saved.owner_subject != state.owner_subject
        || saved.tracker_instance_id != state.tracker_instance_id
        || saved.blocker_sha256 != app::sdlc::canonical_hash(&saved.proposal)?
    {
        return Err(invalid_history());
    }
    saved.proposal.validate(saved.role)
}

async fn current_producer(
    tx: &DatabaseTransaction,
    state: &TaskState,
    actor: &Principal,
    saved: &CriticalBlockerReceipt,
    write: bool,
) -> Result<(), AppError> {
    let grant = app::sdlc_lifecycle::producer(actor, write)?;
    let current = super::lifecycle::current(tx, state.task_id)
        .await?
        .ok_or(AppError::Forbidden)?;
    super::lifecycle::exact_revision(state, &current)?;
    receipt_producer(&current, actor, saved, &grant)
}

// No separate connection/readback: task, current selector, journal and lease share this lock.
async fn current_pm_reader(
    tx: &DatabaseTransaction,
    state: &TaskState,
    actor: &Principal,
    saved: &CriticalBlockerReceipt,
) -> Result<(), AppError> {
    let assignment = state.assignment.as_ref().ok_or(AppError::Forbidden)?;
    let row = tx
        .query_one(statement(
            "SELECT result,after_blocker FROM sdlc_critical_pm_reviews
         WHERE task_id=$1 AND blocker_id=$2 AND assignment_id=$3 AND execution_id=$4",
            vec![
                state.task_id.into(),
                saved.blocker_id.into(),
                assignment.assignment_id.into(),
                assignment.execution_id.into(),
            ],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    let review: domain::sdlc_critical_pm_reservation::CriticalPmReservationResult =
        decode(&row, "result")?;
    let retained: CriticalBlockerReceipt = decode(&row, "after_blocker")?;
    if retained != *saved {
        return Err(invalid_history());
    }
    let cursor = tx.query_one(statement(
        "SELECT pm_owner_version,pm_execution_id,pm_admission_state FROM sdlc_tasks WHERE task_id=$1",
        vec![state.task_id.into()],
    )).await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
    let execution: Option<Uuid> = cursor.try_get("", "pm_execution_id").map_err(map_db)?;
    let admission: Option<String> = cursor.try_get("", "pm_admission_state").map_err(map_db)?;
    let current = PostgresSdlcRepository::reservation(tx, state, assignment.execution_id).await?;
    if execution != Some(assignment.execution_id)
        || admission.as_deref() != Some("reserved")
        || cursor
            .try_get::<i64>("", "pm_owner_version")
            .map_err(map_db)?
            != current.owner_cas.version
    {
        return Err(AppError::Forbidden);
    }
    let lease = PostgresSdlcRepository::lease_current(tx, &current)
        .await?
        .ok_or(AppError::Forbidden)?;
    // lease_current validates the retained operation/fence, row columns, holder and TTL.
    let now = super::execution_lease::clock(tx).await?;
    app::sdlc_critical_blockers::authorize_critical_pm_blocker_read(
        state, actor, saved, &review, &current, &lease, now,
    )
}

fn receipt_producer(
    current: &LifecycleReceipt,
    actor: &Principal,
    saved: &CriticalBlockerReceipt,
    grant: &DeliveryGrant,
) -> Result<(), AppError> {
    let assignment = current.assignment.as_ref().ok_or(AppError::Forbidden)?;
    app::sdlc_lifecycle::authorize_delivery_assignment(grant, assignment)?;
    let binding = current.binding.as_ref().ok_or(AppError::Forbidden)?;
    if saved.producer_subject != actor.subject
        || binding.machine_subject != actor.subject
        || saved.proposal.fence.assignment_id != assignment.assignment_id
        || saved.proposal.fence.execution_id != assignment.execution_id
        || saved.proposal.fence.fencing_token != assignment.fencing_token
        || saved.proposal.fence.lease_id != assignment.lease_id
        || saved.assignment_hash != assignment.assignment_hash
        || saved.agent_id != assignment.agent_id
        || saved.role != assignment.role
        || saved.chat_id != binding.chat_id
        || saved.run_id != binding.run_id
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

/// Only original-key replay/read can use archived custody. This never admits fresh writes.
async fn original_producer(
    tx: &DatabaseTransaction,
    state: &TaskState,
    actor: &Principal,
    saved: &CriticalBlockerReceipt,
    write: bool,
) -> Result<(), AppError> {
    let row = tx
        .query_one(statement(
            "SELECT r.before_receipt FROM sdlc_critical_pm_returns r
         WHERE r.task_id=$1 AND r.blocker_id=$2",
            vec![state.task_id.into(), saved.blocker_id.into()],
        ))
        .await
        .map_err(map_db)?;
    if let Some(row) = row {
        let grant = app::sdlc_lifecycle::producer(actor, write)?;
        let archived: LifecycleReceipt = decode(&row, "before_receipt")?;
        exact_history(tx, &archived).await?;
        receipt_producer(&archived, actor, saved, &grant)
    } else {
        current_producer(tx, state, actor, saved, write).await
    }
}

async fn original_operation(
    tx: &DatabaseTransaction,
    task: Uuid,
    actor: &Principal,
    key: &str,
) -> Result<Option<CriticalBlockerReceipt>, AppError> {
    let row = tx
        .query_one(statement(
            "SELECT payload_hash,result FROM sdlc_critical_blocker_operations
         WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![task.into(), actor.subject.clone().into(), key.into()],
        ))
        .await
        .map_err(map_db)?;
    row.map(|row| {
        let receipt: CriticalBlockerReceipt = decode(&row, "result")?;
        if receipt.blocker_sha256 != row.try_get::<String>("", "payload_hash").map_err(map_db)?
            || receipt.proposal.idempotency_key != key
            || receipt.producer_subject != actor.subject
        {
            return Err(invalid_history());
        }
        app::sdlc_critical_blockers::replay_blocker(&receipt, &receipt.proposal)
    })
    .transpose()
}

async fn return_operation(
    tx: &DatabaseTransaction,
    task: Uuid,
    actor: &Principal,
    key: &str,
) -> Result<Option<(ReturnCriticalToPm, CriticalPolicyChange)>, AppError> {
    let row = tx.query_one(statement(
        "SELECT request_sha256,command,after_receipt,after_blocker FROM sdlc_critical_pm_returns
         WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
        vec![task.into(), actor.subject.clone().into(), key.into()],
    )).await.map_err(map_db)?;
    row.map(|row| {
        let command: ReturnCriticalToPm = decode(&row, "command")?;
        let plan = CriticalPolicyChange {
            lifecycle: decode(&row, "after_receipt")?,
            blocker: decode(&row, "after_blocker")?,
        };
        if command.review.idempotency_key != key
            || row
                .try_get::<String>("", "request_sha256")
                .map_err(map_db)?
                != app::sdlc::canonical_hash(&command)?
            || plan.blocker.blocker_id != command.review.blocker_id
            || plan.blocker.blocker_sha256 != command.review.blocker_sha256
            || plan.blocker.technical_terminal.as_ref() != Some(&command.terminal)
            || plan.lifecycle.task.task_id != task
            || plan.blocker.task_id != task
            || plan.lifecycle.task.owner_subject != plan.blocker.owner_subject
        {
            return Err(invalid_history());
        }
        Ok((command, plan))
    })
    .transpose()
}

pub(super) async fn ensure_progress_allowed(
    tx: &DatabaseTransaction,
    current: &LifecycleReceipt,
) -> Result<(), AppError> {
    let row = tx
        .query_one(statement(
            "SELECT sdlc_critical_pm_detail(h.blocker_id) AS payload FROM sdlc_critical_blocker_heads h
         JOIN sdlc_critical_blockers b ON b.blocker_id=h.blocker_id AND b.task_id=h.task_id
         WHERE h.task_id=$1 FOR SHARE OF h,b",
            vec![current.task.task_id.into()],
        ))
        .await
        .map_err(map_db)?;
    let blocker = row
        .map(|row| decode::<CriticalBlockerReceipt>(&row, "payload"))
        .transpose()?;
    app::sdlc_critical_blockers::ensure_technical_progress_allowed(current, blocker.as_ref())
}

async fn exact_history(
    tx: &DatabaseTransaction,
    current: &LifecycleReceipt,
) -> Result<i64, AppError> {
    let assignment = current.assignment.as_ref().ok_or(AppError::Forbidden)?;
    let binding = current.binding.as_ref().ok_or(AppError::Forbidden)?;
    let saved: DeliveryAssignment = decode(&tx.query_one(statement(
        "SELECT payload FROM sdlc_delivery_assignments WHERE assignment_id=$1 AND task_id=$2 FOR SHARE",
        vec![assignment.assignment_id.into(), assignment.task_id.into()],
    )).await.map_err(map_db)?.ok_or_else(invalid_history)?, "payload")?;
    let run: RunBinding = decode(&tx.query_one(statement(
        "SELECT payload FROM sdlc_delivery_run_bindings WHERE assignment_id=$1 AND run_id=$2 FOR SHARE",
        vec![assignment.assignment_id.into(), binding.run_id.into()],
    )).await.map_err(map_db)?.ok_or_else(invalid_history)?, "payload")?;
    if saved != *assignment || run != *binding {
        return Err(invalid_history());
    }
    let row = tx
        .query_one(statement(
            "SELECT generation FROM sdlc_routing_snapshot_inventory
         WHERE task_id=$1 AND snapshot_id=$2 AND payload=$3",
            vec![
                assignment.task_id.into(),
                assignment.routing.snapshot_id.into(),
                json_value(&assignment.routing)?,
            ],
        ))
        .await
        .map_err(map_db)?
        .ok_or_else(invalid_history)?;
    row.try_get("", "generation").map_err(map_db)
}

async fn verify_evidence(
    tx: &DatabaseTransaction,
    current: &LifecycleReceipt,
    proposal: &SubmitCriticalBlocker,
    storage: &dyn domain::FileStorage,
) -> Result<(), AppError> {
    let binding = current.binding.as_ref().ok_or(AppError::Forbidden)?;
    for evidence in &proposal.evidence {
        let id: Uuid = evidence
            .receipt
            .reference
            .strip_prefix("tracker-attachment:")
            .ok_or(AppError::Forbidden)?
            .parse()
            .map_err(|_| AppError::Forbidden)?;
        tx.query_one(statement(
            "SELECT a.id FROM attachments a JOIN users u ON u.id=a.author_id AND u.is_active
             WHERE a.id=$1 AND a.issue_id=$2 AND u.central_sub=$3 FOR SHARE OF a,u",
            vec![
                id.into(),
                current.task.task_id.into(),
                binding.machine_subject.clone().into(),
            ],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
        let model = attachment::Entity::find_by_id(id)
            .one(tx)
            .await
            .map_err(map_db)?
            .ok_or(AppError::Forbidden)?;
        let item = crate::repos::map_attachment(model);
        let seal = Seal::parse(item.storage_key.as_ref())?.ok_or(AppError::Forbidden)?;
        if seal.artifact() != *evidence || seal.run_id != binding.run_id {
            return Err(AppError::conflict(
                "critical blocker evidence provenance mismatch",
            ));
        }
        seal.validate_attachment(&item)?;
        let bytes = storage
            .get(&current.task.task_id.to_string(), item.storage_key.as_ref())
            .await?;
        app::attachment_artifacts::verify_bytes(&item, &bytes)?;
    }
    Ok(())
}

impl PostgresSdlcRepository {
    /// Main delegates the shared trait/API here and supplies the existing attachment FileStorage.
    pub async fn submit_critical_blocker_with_storage(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SubmitCriticalBlocker,
        storage: &dyn domain::FileStorage,
    ) -> Result<(CriticalBlockerReceipt, bool), AppError> {
        let grant = app::sdlc_lifecycle::producer(actor, true)?;
        if grant.task_id != task {
            return Err(AppError::Forbidden);
        }
        app::sdlc::validate_key(&command.idempotency_key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        owner_access(&tx, &state).await?;
        if let Some(saved) = original_operation(&tx, task, actor, &command.idempotency_key).await? {
            scope(&state, &saved)?;
            original_producer(&tx, &state, actor, &saved, true).await?;
            let receipt = app::sdlc_critical_blockers::replay_blocker(&saved, &command)?;
            tx.commit().await.map_err(map_db)?;
            return Ok((receipt, true));
        }
        let current = super::lifecycle::current(&tx, task)
            .await?
            .ok_or_else(invalid_history)?;
        super::lifecycle::exact_revision(&state, &current)?;
        ensure_progress_allowed(&tx, &current).await?;
        let routing_generation = exact_history(&tx, &current).await?;
        // Keep the existing capacity row locked and retained. POST is not a release/dispatch.
        tx.query_one(statement(
            "SELECT assignment_id FROM sdlc_delivery_holds WHERE task_id=$1 AND assignment_id=$2 FOR UPDATE",
            vec![task.into(), command.fence.assignment_id.into()],
        )).await.map_err(map_db)?.ok_or_else(invalid_history)?;
        let blocker_id = Uuid::now_v7();
        app::sdlc_critical_blockers::prepare_and_hold(
            &current,
            actor,
            &self.config,
            &command,
            app::sdlc_critical_blockers::BlockerAllocation {
                blocker_id,
                routing_generation,
                observed_at: super::lifecycle::now(&tx).await?,
            },
        )?;
        verify_evidence(&tx, &current, &command, storage).await?;
        // Raw I/O may have consumed the lease. Nothing has been written yet.
        let plan = app::sdlc_critical_blockers::prepare_and_hold(
            &current,
            actor,
            &self.config,
            &command,
            app::sdlc_critical_blockers::BlockerAllocation {
                blocker_id,
                routing_generation,
                observed_at: super::lifecycle::now(&tx).await?,
            },
        )?;
        let receipt = plan.blocker;
        exec(&tx, "INSERT INTO sdlc_critical_blockers(blocker_id,task_id,assignment_id,payload,previous_receipt)
            VALUES($1,$2,$3,$4,$5)", vec![blocker_id.into(), task.into(), command.fence.assignment_id.into(),
                json_value(&receipt)?, json_value(&current)?]).await?;
        exec(
            &tx,
            "INSERT INTO sdlc_critical_blocker_operations(task_id,actor_subject,idempotency_key,
            payload_hash,blocker_id,result,event_id) VALUES($1,$2,$3,$4,$5,$6,$5)",
            vec![
                task.into(),
                actor.subject.clone().into(),
                command.idempotency_key.into(),
                receipt.blocker_sha256.clone().into(),
                blocker_id.into(),
                json_value(&receipt)?,
            ],
        )
        .await?;
        let changed = tx.execute(statement(
            "INSERT INTO sdlc_critical_blocker_heads(task_id,blocker_id) VALUES($1,$2)
             ON CONFLICT(task_id) DO UPDATE SET blocker_id=EXCLUDED.blocker_id
             WHERE sdlc_critical_pm_detail(sdlc_critical_blocker_heads.blocker_id)->>'state'='resolved'",
            vec![task.into(), blocker_id.into()],
        )).await.map_err(map_db)?.rows_affected();
        if changed != 1 {
            return Err(AppError::conflict(
                "current critical blocker is not resolved",
            ));
        }
        Self::save_delivery_projection(&tx, actor, &plan.lifecycle, false).await?;
        let event = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
            "project_id":state.project_id,"task_id":task,"root_task_id":task,"owner_subject":state.owner_subject,
            "stage":current.task.stage,"requirement_revision":receipt.proposal.requirement_revision,"result":&receipt});
        exec(
            &tx,
            "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload)
            VALUES($1,$2,'critical.blocker_held',$3)",
            vec![blocker_id.into(), task.into(), event.into()],
        )
        .await?;
        tx.commit().await.map_err(map_db)?;
        Ok((receipt, false))
    }

    /// Original-key read is scoped to the original child (archived after return), not a broad key search.
    pub async fn critical_blocker_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<CriticalBlockerReceipt, AppError> {
        app::sdlc_lifecycle::producer(actor, false)?;
        app::sdlc::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        owner_access(&tx, &state).await?;
        let saved = original_operation(&tx, task, actor, key)
            .await?
            .ok_or_else(|| AppError::not_found("critical blocker operation", key))?;
        scope(&state, &saved)?;
        original_producer(&tx, &state, actor, &saved, false).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(saved)
    }

    /// Current held/awaiting detail; original held history is never overwritten.
    pub async fn critical_blocker(
        &self,
        task: Uuid,
        actor: &Principal,
        blocker_id: Uuid,
    ) -> Result<CriticalBlockerReceipt, AppError> {
        let pm = app::sdlc::has_pm_grant(&actor.scopes);
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = if pm {
            self.load_read(&tx, task, actor).await?
        } else {
            self.load(&tx, task, actor).await?
        };
        owner_access(&tx, &state).await?;
        let row = tx
            .query_one(statement(
                "SELECT sdlc_critical_pm_detail(b.blocker_id) AS payload FROM sdlc_critical_blockers b
             JOIN sdlc_critical_blocker_heads h ON h.blocker_id=b.blocker_id AND h.task_id=b.task_id
             WHERE b.task_id=$1 AND b.blocker_id=$2 FOR SHARE OF h,b",
                vec![task.into(), blocker_id.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::not_found("critical blocker", blocker_id))?;
        let saved: CriticalBlockerReceipt = decode(&row, "payload")?;
        scope(&state, &saved)?;
        if pm {
            current_pm_reader(&tx, &state, actor, &saved).await?;
        } else if app::sdlc_lifecycle::has_delivery_grant(&actor.scopes) {
            current_producer(&tx, &state, actor, &saved, false).await?;
        } else if !(actor.human_session
            && actor.trusted_human
            && actor.subject == state.owner_subject)
        {
            app::sdlc_lifecycle::scheduler_read(actor, &self.config)?;
        }
        tx.commit().await.map_err(map_db)?;
        Ok(saved)
    }

    /// Proposed inherent interface only. Trusted Fleet must read actual native terminal first;
    /// Tracker validates scheduler identity, current CAS, archived binding and terminal identity.
    pub async fn return_critical_to_pm_with_terminal(
        &self,
        task: Uuid,
        actor: &Principal,
        command: ReturnCriticalToPm,
    ) -> Result<(app::sdlc_critical_blockers::CriticalPolicyChange, bool), AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        if actor.trusted_human {
            return Err(AppError::Forbidden);
        }
        app::sdlc::validate_key(&command.review.idempotency_key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        owner_access(&tx, &state).await?;
        if let Some((original, plan)) =
            return_operation(&tx, task, actor, &command.review.idempotency_key).await?
        {
            if original != command {
                return Err(AppError::conflict("critical return key payload mismatch"));
            }
            scope(&state, &plan.blocker)?;
            tx.commit().await.map_err(map_db)?;
            return Ok((plan, true));
        }
        let current = super::lifecycle::current(&tx, task)
            .await?
            .ok_or_else(invalid_history)?;
        super::lifecycle::exact_revision(&state, &current)?;
        let row = tx.query_one(statement(
            "SELECT sdlc_critical_pm_detail(h.blocker_id) AS payload FROM sdlc_critical_blocker_heads h
             JOIN sdlc_critical_blockers b ON b.task_id=h.task_id AND b.blocker_id=h.blocker_id
             WHERE h.task_id=$1 AND h.blocker_id=$2 FOR SHARE OF h,b",
            vec![task.into(), command.review.blocker_id.into()],
        )).await.map_err(map_db)?.ok_or_else(invalid_history)?;
        let blocker: CriticalBlockerReceipt = decode(&row, "payload")?;
        scope(&state, &blocker)?;
        let plan = app::sdlc_critical_blockers::return_to_pm(
            &current,
            &blocker,
            actor,
            &self.config,
            &command,
        )?;
        exact_history(&tx, &current).await?;
        tx.query_one(statement(
            "SELECT assignment_id FROM sdlc_delivery_holds WHERE task_id=$1 AND assignment_id=$2 FOR UPDATE",
            vec![task.into(), command.review.fence.assignment_id.into()],
        )).await.map_err(map_db)?.ok_or_else(invalid_history)?;
        let return_id = Uuid::now_v7();
        let hash = app::sdlc::canonical_hash(&command)?;
        exec(&tx, "INSERT INTO sdlc_critical_pm_returns(return_id,task_id,blocker_id,assignment_id,actor_subject,
            idempotency_key,request_sha256,command,before_receipt,after_receipt,after_blocker)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)", vec![return_id.into(), task.into(), blocker.blocker_id.into(),
                command.review.fence.assignment_id.into(), actor.subject.clone().into(), command.review.idempotency_key.clone().into(),
                hash.into(), json_value(&command)?, json_value(&current)?, json_value(&plan.lifecycle)?, json_value(&plan.blocker)?]).await?;
        // Both old and critical release guards require the actual immutable return journal above.
        exec(
            &tx,
            "DELETE FROM sdlc_delivery_holds WHERE task_id=$1 AND assignment_id=$2",
            vec![task.into(), command.review.fence.assignment_id.into()],
        )
        .await?;
        Self::save_delivery_projection(&tx, actor, &plan.lifecycle, false).await?;
        let event = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
            "project_id":state.project_id,"task_id":task,"root_task_id":task,"owner_subject":state.owner_subject,
            "stage":plan.lifecycle.task.stage,"requirement_revision":blocker.proposal.requirement_revision,
            "result":&plan.blocker,"lifecycle":&plan.lifecycle});
        exec(
            &tx,
            "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload)
            VALUES($1,$2,'critical.returned_to_pm',$3)",
            vec![return_id.into(), task.into(), event.into()],
        )
        .await?;
        tx.commit().await.map_err(map_db)?;
        Ok((plan, false))
    }

    /// Scheduler READ recovery only, not current task detail or a search across operation owners.
    pub async fn critical_pm_return_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<CriticalPolicyChange, AppError> {
        app::sdlc_lifecycle::scheduler_read(actor, &self.config)?;
        if actor.trusted_human {
            return Err(AppError::Forbidden);
        }
        app::sdlc::validate_key(key)?;
        if task.is_nil() || !valid_operation_key(key) {
            return Err(AppError::validation("invalid critical return task or key"));
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        owner_access(&tx, &state).await?;
        let (_, plan) = return_operation(&tx, task, actor, key)
            .await?
            .ok_or_else(|| AppError::not_found("critical PM return operation", key))?;
        scope(&state, &plan.blocker)?;
        tx.commit().await.map_err(map_db)?;
        Ok(plan)
    }
}
