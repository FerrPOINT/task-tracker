use super::*;
use chrono::{DateTime, Duration, Utc};
use domain::sdlc_execution_lease::ExecutionLease;
use domain::sdlc_lifecycle::*;
use domain::sdlc_routing::TaskRoutingSnapshot;

#[cfg(test)]
#[path = "sdlc_lifecycle_pg_tests.rs"]
mod regression_tests;

fn corrupt() -> AppError {
    AppError::conflict("delivery ledger inconsistent")
}

pub(super) async fn mapped_issue_status(
    tx: &DatabaseTransaction,
    stage: Stage,
    status: LifecycleStatus,
) -> Result<Uuid, AppError> {
    let stage = serde_json::to_value(stage).map_err(AppError::internal)?;
    let status = serde_json::to_value(status).map_err(AppError::internal)?;
    let stage = stage.as_str().ok_or_else(corrupt)?;
    let status = status.as_str().ok_or_else(corrupt)?;
    tx.query_one(statement(
        "SELECT m.status_id FROM sdlc_issue_status_mapping m JOIN statuses s ON s.id=m.status_id
         WHERE m.stage=$1 AND m.status=$2 FOR SHARE OF m,s",
        vec![stage.into(), status.into()],
    ))
    .await
    .map_err(map_db)?
    .ok_or_else(|| {
        AppError::conflict(format!(
            "lifecycle_issue_status_mapping_missing:{stage}:{status}"
        ))
    })?
    .try_get("", "status_id")
    .map_err(map_db)
}

pub(super) async fn project_issue_status(
    tx: &DatabaseTransaction,
    task: Uuid,
    actor: &Principal,
    status_id: Uuid,
) -> Result<(), AppError> {
    exec(tx,"INSERT INTO issue_status_history(id,issue_id,from_status_id,to_status_id,changed_by_id,created_at)
        SELECT $1,i.id,i.status_id,s.id,u.id,clock_timestamp() FROM issues i
        JOIN statuses s ON s.id=$3
        JOIN users u ON u.central_sub=$4 AND u.is_active
        WHERE i.id=$2 AND i.status_id<>s.id",
        vec![Uuid::now_v7().into(),task.into(),status_id.into(),actor.subject.clone().into()]).await?;
    exec(
        tx,
        "UPDATE issues SET status_id=$2,sprint_id=NULL,updated_at=clock_timestamp() WHERE id=$1",
        vec![task.into(), status_id.into()],
    )
    .await
}

pub(super) async fn authorize_read(
    tx: &DatabaseTransaction,
    state: &TaskState,
    actor: &Principal,
) -> Result<(), AppError> {
    let Some(grant) = app::sdlc_lifecycle::delivery_grant(actor)? else {
        return Ok(());
    };
    if !actor.scopes.contains("task-tracker:read") {
        return Err(AppError::Forbidden);
    }
    let receipt = current(tx, state.task_id)
        .await?
        .ok_or(AppError::Forbidden)?;
    let assignment = receipt.assignment.as_ref().ok_or(AppError::Forbidden)?;
    app::sdlc_lifecycle::authorize_delivery_assignment(&grant, assignment)?;
    if receipt
        .binding
        .as_ref()
        .is_none_or(|binding| binding.machine_subject != actor.subject)
    {
        return Err(AppError::Forbidden);
    }
    exact_revision(state, &receipt)
}

async fn authorize_proposal_grant(
    tx: &DatabaseTransaction,
    grant: &DeliveryGrant,
    receipt: &DeliveryProposalReceipt,
    actor: &Principal,
) -> Result<(), AppError> {
    if receipt.producer_subject != actor.subject || receipt.task_id != grant.task_id {
        return Err(AppError::Forbidden);
    }
    let row = tx
        .query_one(statement(
            "SELECT payload FROM sdlc_delivery_assignments WHERE assignment_id=$1 AND task_id=$2",
            vec![receipt.assignment_id.into(), receipt.task_id.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    let assignment: DeliveryAssignment =
        serde_json::from_value(row.try_get::<Json>("", "payload").map_err(map_db)?)
            .map_err(|_| corrupt())?;
    app::sdlc_lifecycle::authorize_delivery_assignment(grant, &assignment)
}

async fn now(tx: &DatabaseTransaction) -> Result<DateTime<Utc>, AppError> {
    tx.query_one(statement("SELECT clock_timestamp() AS observed_at", vec![]))
        .await
        .map_err(map_db)?
        .ok_or_else(corrupt)?
        .try_get("", "observed_at")
        .map_err(map_db)
}

async fn current(
    tx: &DatabaseTransaction,
    task: Uuid,
) -> Result<Option<LifecycleReceipt>, AppError> {
    let row = tx
        .query_one(statement(
            "SELECT d.receipt,i.status_id FROM sdlc_delivery_tasks d JOIN issues i ON i.id=d.task_id
             WHERE d.task_id=$1 FOR UPDATE",
            vec![task.into()],
        ))
        .await
        .map_err(map_db)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let receipt: LifecycleReceipt =
        serde_json::from_value(row.try_get::<Json>("", "receipt").map_err(map_db)?)
            .map_err(|_| corrupt())?;
    let mapped =
        mapped_issue_status(tx, receipt.task.stage.canonical(), receipt.task.status).await?;
    if row.try_get::<Uuid>("", "status_id").map_err(map_db)? != mapped {
        return Err(AppError::conflict(
            "lifecycle_issue_status_projection_inconsistent",
        ));
    }
    Ok(Some(receipt))
}

async fn operation(
    tx: &DatabaseTransaction,
    task: Uuid,
    actor: &Principal,
    key: &str,
) -> Result<Option<(String, LifecycleReceipt)>, AppError> {
    let row = tx
        .query_one(statement(
            "SELECT payload_hash,result FROM sdlc_delivery_operations
         WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![task.into(), actor.subject.clone().into(), key.into()],
        ))
        .await
        .map_err(map_db)?;
    row.map(|row| {
        Ok((
            row.try_get("", "payload_hash").map_err(map_db)?,
            serde_json::from_value(row.try_get::<Json>("", "result").map_err(map_db)?)
                .map_err(|_| corrupt())?,
        ))
    })
    .transpose()
}

pub(super) fn exact_revision(
    state: &TaskState,
    receipt: &LifecycleReceipt,
) -> Result<(), AppError> {
    if receipt.task.task_id != state.task_id || receipt.task.owner_subject != state.owner_subject {
        return Err(corrupt());
    }
    exact_snapshot(state, &receipt.task.routing)
}

pub(super) fn exact_snapshot(
    state: &TaskState,
    snapshot: &TaskRoutingSnapshot,
) -> Result<(), AppError> {
    let revision = state.revisions.last().ok_or_else(corrupt)?;
    if snapshot.task_id != state.task_id
        || snapshot.root_task_id != state.task_id
        || snapshot.project_id != state.project_id
        || snapshot.tracker_instance_id != state.tracker_instance_id
        || snapshot.requirement_revision != revision.revision
        || snapshot.content_hash != revision.content_hash
        || snapshot.policy.project_id != state.project_id
        || snapshot.policy.tracker_instance_id != state.tracker_instance_id
        || snapshot.contract_version != snapshot.policy.routes.contract_version()
        || snapshot.policy.contract_version != snapshot.contract_version
        || snapshot.policy.routing_hash != app::sdlc::canonical_hash(&snapshot.policy.routes)?
        || !state.confirmations.iter().any(|c| {
            c.id == snapshot.confirmation_id
                && c.owner_subject == state.owner_subject
                && c.revision == revision.revision
                && c.content_hash == revision.content_hash
        })
    {
        return Err(corrupt());
    }
    Ok(())
}

fn fence(
    receipt: &LifecycleReceipt,
    expected: &DeliveryFence,
    actor: &Principal,
    observed_at: DateTime<Utc>,
    renewing: bool,
) -> Result<(), AppError> {
    let lease = receipt.lease.as_ref().ok_or_else(corrupt)?;
    if lease.holder_subject != actor.subject {
        return Err(AppError::Forbidden);
    }
    exact_fence(receipt, expected, observed_at, renewing)
}

fn exact_fence(
    receipt: &LifecycleReceipt,
    expected: &DeliveryFence,
    observed_at: DateTime<Utc>,
    renewing: bool,
) -> Result<(), AppError> {
    let assignment = receipt.assignment.as_ref().ok_or_else(corrupt)?;
    let lease = receipt.lease.as_ref().ok_or_else(corrupt)?;
    if lease.expires_at <= observed_at
        || expected.expected_lease_version > lease.version
        || expected.expected_lease_version < 1
        || (renewing && lease.version != expected.expected_lease_version)
        || assignment.assignment_id != expected.assignment_id
        || assignment.execution_id != expected.execution_id
        || assignment.fencing_token != expected.fencing_token
        || assignment.lease_id != expected.lease_id
        || lease.lease_id != expected.lease_id
        || assignment.task_id != receipt.task.task_id
        || assignment.owner_subject != receipt.task.owner_subject
        || assignment.stage != receipt.task.stage
        || assignment.cycle != receipt.task.cycle
        || assignment.routing != receipt.task.routing
        || assignment.assignment_hash != app::sdlc_lifecycle::assignment_hash(assignment)?
    {
        return Err(AppError::conflict("stale or expired delivery fence"));
    }
    Ok(())
}

impl PostgresSdlcRepository {
    async fn save_delivery_projection(
        tx: &DatabaseTransaction,
        actor: &Principal,
        receipt: &LifecycleReceipt,
        insert: bool,
    ) -> Result<(), AppError> {
        let task = receipt.task.task_id;
        let status_id =
            mapped_issue_status(tx, receipt.task.stage.canonical(), receipt.task.status).await?;
        let row = tx
            .query_one(statement(
                "SELECT state,pm_execution_id FROM sdlc_tasks WHERE task_id=$1 FOR UPDATE",
                vec![task.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(corrupt)?;
        let raw = row.try_get::<Json>("", "state").map_err(map_db)?;
        let next = app::sdlc_lifecycle::canonical_projection(&raw, &receipt.task)?;
        let mut execution = serde_json::to_value(receipt).map_err(AppError::internal)?;
        execution
            .as_object_mut()
            .ok_or_else(corrupt)?
            .remove("task");
        exec(tx,"INSERT INTO sdlc_delivery_projection_permits(task_id,transaction_id,next_state,next_execution)
            VALUES($1,pg_current_xact_id(),$2,$3)",vec![task.into(),next.clone().into(),execution.clone().into()]).await?;
        if row
            .try_get::<Option<Uuid>>("", "pm_execution_id")
            .map_err(map_db)?
            .is_some()
        {
            exec(
                tx,
                "INSERT INTO sdlc_pm_state_write_permits(task_id,transaction_id,next_state)
                SELECT $1,pg_current_xact_id(),$2 FROM sdlc_tasks
                WHERE task_id=$1 AND state IS DISTINCT FROM $2",
                vec![task.into(), next.clone().into()],
            )
            .await?;
        }
        exec(
            tx,
            "UPDATE sdlc_tasks SET state=$2 WHERE task_id=$1",
            vec![task.into(), next.into()],
        )
        .await?;
        let sql = if insert {
            "INSERT INTO sdlc_delivery_heads(task_id,execution) VALUES($1,$2)"
        } else {
            "UPDATE sdlc_delivery_heads SET execution=$2 WHERE task_id=$1"
        };
        exec(tx, sql, vec![task.into(), execution.into()]).await?;
        project_issue_status(tx, task, actor, status_id).await?;
        Ok(())
    }

    async fn delivery_seed(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
    ) -> Result<LifecycleReceipt, AppError> {
        let stage = LifecycleStage::from_legacy(state.stage);
        if !matches!(
            stage,
            LifecycleStage::Analysis | LifecycleStage::Architecture
        ) {
            return Err(AppError::conflict(
                "exact owner confirmation required before delivery",
            ));
        }
        let row = tx.query_one(statement(
            "SELECT s.payload FROM sdlc_task_routing_snapshots s
             JOIN sdlc_confirmations c ON c.id=s.confirmation_id AND c.task_id=s.task_id
             JOIN sdlc_analysis_intents i ON i.routing_snapshot_id=s.snapshot_id AND i.task_id=s.task_id
             WHERE s.task_id=$1", vec![state.task_id.into()],
        )).await.map_err(map_db)?.ok_or_else(|| AppError::conflict("confirmed routing snapshot absent"))?;
        let routing: TaskRoutingSnapshot =
            serde_json::from_value(row.try_get::<Json>("", "payload").map_err(map_db)?)
                .map_err(|_| corrupt())?;
        let previous_result_id = if stage == LifecycleStage::Architecture {
            let row = tx
                .query_one(statement(
                    "SELECT result_id FROM sdlc_analysis_results WHERE task_id=$1 AND
                    payload->>'requirement_revision'=$2 AND payload->>'content_hash'=$3",
                    vec![
                        state.task_id.into(),
                        routing.requirement_revision.to_string().into(),
                        routing.content_hash.clone().into(),
                    ],
                ))
                .await
                .map_err(map_db)?
                .ok_or_else(|| AppError::conflict("accepted Analysis result absent"))?;
            Some(row.try_get("", "result_id").map_err(map_db)?)
        } else {
            None
        };
        let receipt = LifecycleReceipt {
            task: LifecycleTask {
                task_id: state.task_id,
                owner_subject: state.owner_subject.clone(),
                version: 1,
                stage,
                status: LifecycleStatus::Ready,
                cycle: 0,
                transient_retries: 0,
                waiting_reason: Some("queued_for_assignment".into()),
                runtime: RuntimeState::Queued,
                routing,
                previous_result_id,
                resume_chat_id: None,
            },
            assignment: None,
            lease: None,
            binding: None,
            result: None,
        };
        exact_revision(state, &receipt)?;
        // Existing producers share this mutex. Never overlap a live legacy execution.
        exec(
            tx,
            "SELECT singleton FROM sdlc_reservation_capacity WHERE singleton FOR UPDATE",
            vec![],
        )
        .await?;
        let row = tx.query_one(statement(
            "SELECT EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_tasks t WHERE t.task_id=$1
                AND (t.pm_execution_id IS NOT NULL OR t.state->'assignment' IS DISTINCT FROM 'null'::jsonb)
                AND NOT sdlc_current_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state)) AS busy",
            vec![state.task_id.into()],
        )).await.map_err(map_db)?.ok_or_else(corrupt)?;
        if row.try_get::<bool>("", "busy").map_err(map_db)? {
            return Err(AppError::conflict("previous execution quiescence unknown"));
        }
        Self::save_delivery_projection(tx, actor, &receipt, true).await?;
        Ok(receipt)
    }

    async fn delivery_reserve(
        &self,
        tx: &DatabaseTransaction,
        actor: &Principal,
        receipt: &mut LifecycleReceipt,
        expected_version: i64,
    ) -> Result<(), AppError> {
        if receipt.task.version != expected_version
            || receipt.task.status != LifecycleStatus::Ready
            || receipt.task.runtime != RuntimeState::Queued
            || receipt.assignment.is_some()
        {
            return Err(AppError::conflict("queued delivery CAS required"));
        }
        let role = receipt.task.stage.role().ok_or_else(corrupt)?;
        let route = receipt
            .task
            .routing
            .policy
            .routes
            .entries()
            .into_iter()
            .find(|(key, _)| *key == role.key())
            .map(|(_, r)| r)
            .ok_or_else(corrupt)?;
        if route.agent_id.is_nil() {
            return Err(corrupt());
        }
        exec(
            tx,
            "SELECT singleton FROM sdlc_reservation_capacity WHERE singleton FOR UPDATE",
            vec![],
        )
        .await?;
        let row = tx.query_one(statement(
            "SELECT EXISTS(SELECT 1 FROM sdlc_delivery_holds WHERE task_id=$1 OR agent_id=$2)
             OR EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE root_task_id=$1 OR agent_id=$2)
             OR EXISTS(SELECT 1 FROM sdlc_assignments a JOIN sdlc_tasks t ON t.task_id=a.task_id
                AND t.pm_execution_id=(a.payload->>'execution_id')::uuid
                WHERE a.payload->>'agent_id'=$2::text
                AND NOT sdlc_current_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state)) AS busy",
            vec![receipt.task.task_id.into(), route.agent_id.into()],
        )).await.map_err(map_db)?.ok_or_else(corrupt)?;
        if row.try_get::<bool>("", "busy").map_err(map_db)? {
            return Err(AppError::conflict("task or agent capacity held"));
        }
        let slot = tx
            .query_one(statement(
                "SELECT s::smallint AS slot FROM generate_series(1,2) s
             WHERE NOT EXISTS(SELECT 1 FROM sdlc_delivery_holds WHERE pool_slot=s)
               AND NOT EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds WHERE pool_slot=s)
             ORDER BY s LIMIT 1",
                vec![],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::conflict("technical pool capacity held"))?
            .try_get::<i16>("", "slot")
            .map_err(map_db)?;
        let token = tx
            .query_one(statement(
                "SELECT nextval('sdlc_analysis_reservation_fence') AS n",
                vec![],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(corrupt)?
            .try_get::<i64>("", "n")
            .map_err(map_db)?;
        let ordinal = tx
            .query_one(statement(
                "SELECT nextval(pg_get_serial_sequence('sdlc_pm_executions','ordinal')) AS ordinal",
                vec![],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(corrupt)?
            .try_get::<i64>("", "ordinal")
            .map_err(map_db)?;
        let execution = DeliveryExecution::from_ordinal(ordinal)?;
        let observed_at = now(tx).await?;
        let mut assignment = DeliveryAssignment {
            assignment_id: Uuid::now_v7(),
            execution_id: Uuid::now_v7(),
            execution: Some(execution),
            task_id: receipt.task.task_id,
            owner_subject: receipt.task.owner_subject.clone(),
            role,
            stage: receipt.task.stage,
            cycle: receipt.task.cycle,
            attempt: receipt.task.transient_retries + 1,
            agent_id: route.agent_id,
            routing: receipt.task.routing.clone(),
            previous_result_id: receipt.task.previous_result_id,
            fencing_token: token,
            lease_id: Uuid::now_v7(),
            assignment_hash: String::new(),
        };
        assignment.assignment_hash = app::sdlc_lifecycle::assignment_hash(&assignment)?;
        let lease = ExecutionLease {
            lease_id: assignment.lease_id,
            version: 1,
            holder_subject: actor.subject.clone(),
            claimed_at: observed_at,
            heartbeat_at: observed_at,
            expires_at: observed_at + Duration::seconds(LEASE_TTL_SECONDS),
        };
        exec(tx, "INSERT INTO sdlc_delivery_assignments(assignment_id,execution_id,task_id,agent_id,payload)
            VALUES($1,$2,$3,$4,$5)", vec![assignment.assignment_id.into(),assignment.execution_id.into(),
                assignment.task_id.into(),assignment.agent_id.into(),json_value(&assignment)?]).await?;
        exec(
            tx,
            "INSERT INTO sdlc_delivery_holds(assignment_id,task_id,agent_id,pool_slot)
            VALUES($1,$2,$3,$4)",
            vec![
                assignment.assignment_id.into(),
                assignment.task_id.into(),
                assignment.agent_id.into(),
                slot.into(),
            ],
        )
        .await?;
        receipt.assignment = Some(assignment);
        receipt.lease = Some(lease);
        receipt.binding = None;
        receipt.result = None;
        receipt.task.version += 1;
        receipt.task.runtime = RuntimeState::Reserved;
        receipt.task.waiting_reason = Some("awaiting_run_binding".into());
        Ok(())
    }
}

#[async_trait]
impl LifecycleRepository for PostgresSdlcRepository {
    async fn delivery_candidates(
        &self,
        actor: &Principal,
        query: domain::sdlc_candidates::DeliveryCandidatesQuery,
    ) -> Result<domain::sdlc_candidates::DeliveryCandidates, AppError> {
        self.read_delivery_candidates(actor, query).await
    }

    async fn submit_pm_feedback(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SubmitPmFeedback,
    ) -> Result<(PmFeedbackReceipt, bool), AppError> {
        app::sdlc::validate_key(&command.idempotency_key)?;
        if command.message_id.is_nil()
            || command.text.trim().is_empty()
            || command.text.len() > 100_000
            || command.text.contains('\0')
        {
            return Err(AppError::validation("invalid PM feedback message"));
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        if !state.owner(actor) {
            return Err(AppError::Forbidden);
        }
        let assignment = state
            .assignment
            .as_ref()
            .ok_or_else(|| AppError::conflict("bound PM assignment required"))?;
        if command.fence.assignment_id != assignment.assignment_id
            || command.fence.execution_id != assignment.execution_id
            || command.fence.agent_id != assignment.agent_id
            || command.fence.assignment_version != assignment.version
        {
            return Err(AppError::conflict("stale PM feedback assignment"));
        }
        let row = tx
            .query_one(statement(
                "SELECT payload FROM sdlc_assignments WHERE task_id=$1 AND version=$2",
                vec![task.into(), assignment.version.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(corrupt)?;
        if row.try_get::<Json>("", "payload").map_err(map_db)?
            != serde_json::to_value(assignment).map_err(AppError::internal)?
        {
            return Err(corrupt());
        }
        let hash =
            app::sdlc::canonical_hash(&json!({"operation":"pm_feedback","payload":&command}))?;
        if let Some(saved) = Self::replay(&tx, task, actor, &command.idempotency_key, &hash).await?
        {
            let receipt = serde_json::from_value(saved).map_err(|_| corrupt())?;
            tx.commit().await.map_err(map_db)?;
            return Ok((receipt, true));
        }
        app::sdlc_lifecycle::validate_new_pm_feedback(&state)?;
        let duplicate = tx
            .query_one(statement(
                "SELECT message_id FROM sdlc_pm_feedback WHERE message_id=$1",
                vec![command.message_id.into()],
            ))
            .await
            .map_err(map_db)?;
        if duplicate.is_some() {
            return Err(AppError::conflict("PM feedback message already recorded"));
        }
        let receipt = PmFeedbackReceipt {
            message_id: command.message_id,
            tracker_instance_id: state.tracker_instance_id.clone(),
            project_id: state.project_id,
            task_id: task,
            root_task_id: task,
            owner_subject: state.owner_subject.clone(),
            fence: command.fence.clone(),
            text: command.text.clone(),
            content_hash: app::sdlc::canonical_hash(&json!({"text":command.text}))?,
            requirement_revision: state.current_revision(),
            created_at: now(&tx).await?,
        };
        exec(&tx, "INSERT INTO sdlc_pm_feedback(message_id,task_id,actor_subject,payload) VALUES($1,$2,$3,$4)",
            vec![receipt.message_id.into(),task.into(),actor.subject.clone().into(),json_value(&receipt)?]).await?;
        exec(&tx, "INSERT INTO sdlc_idempotency(task_id,actor_subject,idempotency_key,payload_hash,result)
            VALUES($1,$2,$3,$4,$5)", vec![task.into(),actor.subject.clone().into(),command.idempotency_key.into(),hash.into(),json_value(&receipt)?]).await?;
        let payload = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
            "project_id":state.project_id,"task_id":task,"root_task_id":task,"owner_subject":state.owner_subject,
            "stage":state.stage,"requirement_revision":state.current_revision(),"result":&receipt});
        exec(&tx, "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,'pm.feedback_recorded',$3)",
            vec![receipt.message_id.into(),task.into(),payload.into()]).await?;
        // Feedback never mutates questions, requirements, owner consent or the PM lease.
        tx.commit().await.map_err(map_db)?;
        Ok((receipt, false))
    }

    async fn pm_feedback_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<PmFeedbackReceipt, AppError> {
        app::sdlc::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        if !state.owner(actor) {
            return Err(AppError::Forbidden);
        }
        let row = tx.query_one(statement("SELECT result FROM sdlc_idempotency WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![task.into(),actor.subject.clone().into(),key.into()])).await.map_err(map_db)?
            .ok_or_else(|| AppError::not_found("PM feedback operation", key))?;
        let receipt = serde_json::from_value(row.try_get::<Json>("", "result").map_err(map_db)?)
            .map_err(|_| corrupt())?;
        tx.commit().await.map_err(map_db)?;
        Ok(receipt)
    }

    async fn pm_feedback_message(
        &self,
        task: Uuid,
        actor: &Principal,
        message_id: Uuid,
    ) -> Result<PmFeedbackReceipt, AppError> {
        app::sdlc_lifecycle::authorize_pm_feedback_message_read(task, actor)?;
        if message_id.is_nil() {
            return Err(AppError::validation("invalid PM feedback message ID"));
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load_read(&tx, task, actor).await?;
        let row = tx.query_one(statement(
            "SELECT actor_subject,payload FROM sdlc_pm_feedback WHERE task_id=$1 AND message_id=$2",
            vec![task.into(), message_id.into()],
        )).await.map_err(map_db)?.ok_or_else(|| AppError::not_found("PM feedback message", message_id))?;
        let payload: Json = row.try_get("", "payload").map_err(map_db)?;
        let receipt: PmFeedbackReceipt = serde_json::from_value(payload.clone())
            .map_err(|_| AppError::conflict("pm_feedback_history_inconsistent"))?;
        if serde_json::to_value(&receipt).map_err(AppError::internal)? != payload {
            return Err(AppError::conflict("pm_feedback_history_inconsistent"));
        }
        let receipt = app::sdlc_lifecycle::exact_pm_feedback_message(
            &state,
            message_id,
            &row.try_get::<String>("", "actor_subject").map_err(map_db)?,
            &receipt,
        )?;
        tx.commit().await.map_err(map_db)?;
        Ok(receipt)
    }

    async fn submit_delivery_proposal(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SubmitDeliveryProposal,
    ) -> Result<(DeliveryProposalReceipt, bool), AppError> {
        let grant = app::sdlc_lifecycle::producer(actor, true)?;
        app::sdlc::validate_key(&command.idempotency_key)?;
        app::sdlc_lifecycle::validate_fence(&command.fence)?;
        app::sdlc_lifecycle::validate_result(&command.result)?;
        app::sdlc_lifecycle::validate_artifact(&command.workflow_receipt)?;
        let hash = app::sdlc::canonical_hash(
            &json!({"operation":"submit_delivery_proposal","payload":command}),
        )?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        let retained = tx
            .query_one(statement(
                "SELECT payload_hash,result FROM sdlc_delivery_proposal_operations
             WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
                vec![
                    task.into(),
                    actor.subject.clone().into(),
                    command.idempotency_key.clone().into(),
                ],
            ))
            .await
            .map_err(map_db)?;
        if let Some(row) = retained {
            if row.try_get::<String>("", "payload_hash").map_err(map_db)? != hash {
                return Err(AppError::conflict("proposal key payload conflict"));
            }
            let receipt: DeliveryProposalReceipt =
                serde_json::from_value(row.try_get::<Json>("", "result").map_err(map_db)?)
                    .map_err(|_| corrupt())?;
            authorize_proposal_grant(&tx, &grant, &receipt, actor).await?;
            tx.commit().await.map_err(map_db)?;
            return Ok((receipt, true));
        }
        let receipt = current(&tx, task).await?.ok_or_else(corrupt)?;
        exact_revision(&state, &receipt)?;
        let binding = receipt
            .binding
            .as_ref()
            .ok_or_else(|| AppError::conflict("running producer binding required"))?;
        if binding.machine_subject != actor.subject {
            return Err(AppError::Forbidden);
        }
        let observed_at = now(&tx).await?;
        exact_fence(&receipt, &command.fence, observed_at, false)?;
        if receipt.task.runtime != RuntimeState::Running
            || receipt.task.status != LifecycleStatus::InProgress
            || receipt.task.stage.role() != Some(command.result.role())
        {
            return Err(AppError::conflict("proposal does not match running role"));
        }
        let assignment = receipt.assignment.as_ref().ok_or_else(corrupt)?;
        app::sdlc_lifecycle::authorize_delivery_assignment(&grant, assignment)?;
        let duplicate = tx
            .query_one(statement(
                "SELECT proposal_id FROM sdlc_delivery_proposals WHERE assignment_id=$1",
                vec![assignment.assignment_id.into()],
            ))
            .await
            .map_err(map_db)?;
        if duplicate.is_some() {
            return Err(AppError::conflict("assignment proposal already submitted"));
        }
        let proposal = DeliveryProposalReceipt {
            proposal_id: Uuid::now_v7(),
            task_id: task,
            assignment_id: assignment.assignment_id,
            execution_id: assignment.execution_id,
            agent_id: assignment.agent_id,
            producer_subject: actor.subject.clone(),
            workflow_receipt: command.workflow_receipt.clone(),
            result: command.result.clone(),
            output_sha256: app::sdlc::canonical_hash(&command.result)?,
            created_at: observed_at,
        };
        exec(
            &tx,
            "INSERT INTO sdlc_delivery_proposals(proposal_id,assignment_id,task_id,payload)
            VALUES($1,$2,$3,$4)",
            vec![
                proposal.proposal_id.into(),
                proposal.assignment_id.into(),
                task.into(),
                json_value(&proposal)?,
            ],
        )
        .await?;
        exec(&tx, "INSERT INTO sdlc_delivery_proposal_operations(task_id,actor_subject,idempotency_key,payload_hash,result)
            VALUES($1,$2,$3,$4,$5)", vec![task.into(),actor.subject.clone().into(),command.idempotency_key.into(),hash.into(),json_value(&proposal)?]).await?;
        let payload = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
            "project_id":state.project_id,"task_id":task,"root_task_id":task,"owner_subject":state.owner_subject,
            "stage":receipt.task.stage,"requirement_revision":receipt.task.routing.requirement_revision,"result":&proposal});
        exec(&tx, "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,'delivery.proposal_submitted',$3)",
            vec![proposal.proposal_id.into(),task.into(),payload.into()]).await?;
        tx.commit().await.map_err(map_db)?;
        Ok((proposal, false))
    }

    async fn delivery_proposal_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<DeliveryProposalReceipt, AppError> {
        let grant = app::sdlc_lifecycle::producer(actor, false)?;
        app::sdlc::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        self.load(&tx, task, actor).await?;
        let row = tx.query_one(statement(
            "SELECT result FROM sdlc_delivery_proposal_operations WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![task.into(),actor.subject.clone().into(),key.into()],
        )).await.map_err(map_db)?.ok_or_else(|| AppError::not_found("delivery proposal operation", key))?;
        let receipt: DeliveryProposalReceipt =
            serde_json::from_value(row.try_get::<Json>("", "result").map_err(map_db)?)
                .map_err(|_| corrupt())?;
        authorize_proposal_grant(&tx, &grant, &receipt, actor).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(receipt)
    }

    async fn lifecycle_result(
        &self,
        task: Uuid,
        actor: &Principal,
        result_id: Uuid,
    ) -> Result<DeliveryInput, AppError> {
        if app::sdlc_lifecycle::has_delivery_grant(&actor.scopes) {
            app::sdlc_lifecycle::producer(actor, false)?;
        } else {
            app::sdlc_reservation::reader(actor)?;
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        authorize_read(&tx, &state, actor).await?;
        let row = tx.query_one(statement(
            "SELECT payload,false AS legacy FROM sdlc_delivery_results WHERE task_id=$1 AND result_id=$2
             UNION ALL SELECT payload,true AS legacy FROM sdlc_analysis_results WHERE task_id=$1 AND result_id=$2",
            vec![task.into(),result_id.into()],
        )).await.map_err(map_db)?.ok_or_else(|| AppError::not_found("delivery result", result_id))?;
        let payload = row.try_get::<Json>("", "payload").map_err(map_db)?;
        let result = if row.try_get::<bool>("", "legacy").map_err(map_db)? {
            DeliveryInput::LegacyAnalysis(serde_json::from_value(payload).map_err(|_| corrupt())?)
        } else {
            DeliveryInput::Delivery(serde_json::from_value(payload).map_err(|_| corrupt())?)
        };
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }

    async fn lifecycle_command(
        &self,
        task: Uuid,
        actor: &Principal,
        command: LifecycleCommand,
    ) -> Result<(LifecycleReceipt, bool), AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        app::sdlc_lifecycle::validate_command(&command)?;
        let hash = app::sdlc::canonical_hash(&command)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        if let Some((original_hash, result)) = operation(&tx, task, actor, command.key()).await? {
            if original_hash != hash {
                return Err(AppError::conflict("delivery key payload conflict"));
            }
            tx.commit().await.map_err(map_db)?;
            return Ok((result, true));
        }
        if matches!(
            command,
            LifecycleCommand::EnsureQueued { .. } | LifecycleCommand::Reserve { .. }
        ) && !self
            .config
            .single_task_project_ids
            .contains(&state.project_id)
        {
            return Err(AppError::Unavailable(
                "single_task_lifecycle_project_not_enabled".into(),
            ));
        }
        let before = current(&tx, task).await?;
        let mut receipt = match before.clone() {
            Some(receipt) => receipt,
            None if matches!(command, LifecycleCommand::EnsureQueued { .. }) => {
                self.delivery_seed(&tx, &state, actor).await?
            }
            None => return Err(AppError::conflict("delivery queue absent")),
        };
        exact_revision(&state, &receipt)?;
        match &command {
            LifecycleCommand::Reconcile {
                fence: expected,
                terminal,
                recovery,
                ..
            } => {
                let assignment = receipt.assignment.as_ref().ok_or_else(corrupt)?;
                let lease = receipt.lease.as_ref().ok_or_else(corrupt)?;
                if lease.holder_subject != actor.subject {
                    return Err(AppError::Forbidden);
                }
                if assignment.assignment_id != expected.assignment_id
                    || assignment.execution_id != expected.execution_id
                    || assignment.fencing_token != expected.fencing_token
                    || assignment.lease_id != expected.lease_id
                    || lease.lease_id != expected.lease_id
                    || expected.expected_lease_version > lease.version
                    || assignment.assignment_hash != terminal.assignment_hash
                    || assignment.assignment_hash
                        != app::sdlc_lifecycle::assignment_hash(assignment)?
                {
                    return Err(AppError::conflict(
                        "reconciliation immutable identity mismatch",
                    ));
                }
                if receipt
                    .binding
                    .as_ref()
                    .is_some_and(|b| b.run_id != terminal.run_id || b.chat_id != terminal.chat_id)
                {
                    return Err(AppError::conflict("actual terminal differs from bound run"));
                }
                let old_assignment = assignment.assignment_id;
                exec(&tx,"INSERT INTO sdlc_delivery_reconciliations(assignment_id,task_id,payload) VALUES($1,$2,$3)",
                    vec![old_assignment.into(),task.into(),json_value(&command)?]).await?;
                app::sdlc_lifecycle::reconcile_terminal(&mut receipt.task, terminal, recovery)?;
                exec(
                    &tx,
                    "DELETE FROM sdlc_delivery_holds WHERE assignment_id=$1",
                    vec![old_assignment.into()],
                )
                .await?;
                receipt.assignment = None;
                receipt.lease = None;
                receipt.binding = None;
                receipt.result = None;
            }
            LifecycleCommand::EnsureQueued { .. } => {}
            LifecycleCommand::Reserve {
                expected_task_version,
                ..
            } => {
                self.delivery_reserve(&tx, actor, &mut receipt, *expected_task_version)
                    .await?;
            }
            LifecycleCommand::Heartbeat {
                fence: expected, ..
            }
            | LifecycleCommand::BindRun {
                fence: expected, ..
            }
            | LifecycleCommand::Complete {
                fence: expected, ..
            } => {
                let hold = tx.query_one(statement(
                    "SELECT assignment_id FROM sdlc_delivery_holds WHERE assignment_id=$1 FOR UPDATE",
                    vec![expected.assignment_id.into()],
                )).await.map_err(map_db)?;
                if hold.is_none() {
                    return Err(AppError::conflict("delivery capacity released"));
                }
                let observed_at = now(&tx).await?;
                fence(
                    &receipt,
                    expected,
                    actor,
                    observed_at,
                    matches!(command, LifecycleCommand::Heartbeat { .. }),
                )?;
                match &command {
                    LifecycleCommand::Heartbeat { .. } => {
                        let lease = receipt.lease.as_mut().ok_or_else(corrupt)?;
                        if lease.version == MAX_SAFE_VERSION {
                            return Err(AppError::conflict("lease version exhausted"));
                        }
                        lease.version += 1;
                        lease.heartbeat_at = observed_at;
                        lease.expires_at = observed_at + Duration::seconds(LEASE_TTL_SECONDS);
                    }
                    LifecycleCommand::BindRun { binding, .. } => {
                        if receipt
                            .task
                            .resume_chat_id
                            .is_some_and(|chat| chat != binding.chat_id)
                        {
                            return Err(AppError::conflict("retry must reuse the previous chat"));
                        }
                        if binding.machine_subject == self.config.reservation_scheduler_subject
                            || binding.machine_subject == receipt.task.owner_subject
                            || binding.machine_subject == self.config.verifier_subject
                        {
                            return Err(AppError::Forbidden);
                        }
                        if receipt.binding.is_some()
                            || receipt.task.runtime != RuntimeState::Reserved
                        {
                            return Err(AppError::conflict(
                                "run already bound or execution unknown",
                            ));
                        }
                        exec(&tx, "INSERT INTO sdlc_delivery_run_bindings(assignment_id,run_id,chat_id,payload)
                            VALUES($1,$2,$3,$4)", vec![expected.assignment_id.into(), binding.run_id.into(),
                                binding.chat_id.into(), json_value(binding)?]).await?;
                        receipt.binding = Some(binding.clone());
                        receipt.task.status = LifecycleStatus::InProgress;
                        receipt.task.runtime = RuntimeState::Running;
                        receipt.task.waiting_reason = None;
                        receipt.task.version += 1;
                    }
                    LifecycleCommand::Complete {
                        terminal, result, ..
                    } => {
                        let row = tx.query_one(statement(
                            "SELECT payload FROM sdlc_delivery_proposals WHERE assignment_id=$1 AND task_id=$2",
                            vec![expected.assignment_id.into(),task.into()],
                        )).await.map_err(map_db)?.ok_or_else(|| AppError::conflict("agent proposal required before terminal handoff"))?;
                        let proposal: DeliveryProposalReceipt = serde_json::from_value(
                            row.try_get::<Json>("", "payload").map_err(map_db)?,
                        )
                        .map_err(|_| corrupt())?;
                        if proposal.result != *result
                            || proposal.output_sha256 != terminal.output_sha256
                            || proposal.workflow_receipt != terminal.workflow_receipt
                        {
                            return Err(AppError::conflict(
                                "terminal differs from retained agent proposal",
                            ));
                        }
                        let binding = receipt
                            .binding
                            .as_ref()
                            .ok_or_else(|| AppError::conflict("actual run binding absent"))?;
                        if binding.run_id != terminal.run_id || binding.chat_id != terminal.chat_id
                        {
                            return Err(AppError::conflict("terminal run does not match binding"));
                        }
                        let previous_stage = receipt.task.stage;
                        app::sdlc_lifecycle::accept_result(&mut receipt.task, result)?;
                        let accepted = AcceptedDeliveryResult {
                            result_id: Uuid::now_v7(),
                            assignment_id: expected.assignment_id,
                            task_id: task,
                            terminal: terminal.clone(),
                            output: result.clone(),
                            previous_stage,
                            next_stage: receipt.task.stage,
                            next_status: receipt.task.status,
                            cycle: receipt.task.cycle,
                            created_at: observed_at,
                        };
                        exec(&tx, "INSERT INTO sdlc_delivery_results(result_id,assignment_id,task_id,payload)
                            VALUES($1,$2,$3,$4)", vec![accepted.result_id.into(),expected.assignment_id.into(),
                                task.into(),json_value(&accepted)?]).await?;
                        // A terminal receipt is the only path that releases a capacity hold.
                        exec(
                            &tx,
                            "DELETE FROM sdlc_delivery_holds WHERE assignment_id=$1",
                            vec![expected.assignment_id.into()],
                        )
                        .await?;
                        receipt.task.previous_result_id = Some(accepted.result_id);
                        receipt.result = Some(accepted);
                        receipt.assignment = None;
                        receipt.lease = None;
                        receipt.binding = None;
                    }
                    _ => return Err(corrupt()),
                }
            }
        }
        if receipt.task.version > MAX_SAFE_VERSION {
            return Err(AppError::conflict("task version exhausted"));
        }
        if before.as_ref().is_some_and(|previous| previous != &receipt) {
            Self::save_delivery_projection(&tx, actor, &receipt, false).await?;
        }
        let event = match &command {
            LifecycleCommand::Reconcile { .. } => Some("delivery.reconciled"),
            LifecycleCommand::EnsureQueued { .. } => Some("delivery.queued"),
            LifecycleCommand::Reserve { .. } => Some("delivery.assignment_reserved"),
            // Lease renewals are audited in operations, not mirrored into business chat.
            LifecycleCommand::Heartbeat { .. } => None,
            LifecycleCommand::BindRun { .. } => Some("delivery.run_bound"),
            LifecycleCommand::Complete { .. } => Some("delivery.result_accepted"),
        };
        let event_id = event
            .filter(|_| before.as_ref() != Some(&receipt))
            .map(|_| Uuid::now_v7());
        exec(&tx, "INSERT INTO sdlc_delivery_operations(task_id,actor_subject,idempotency_key,payload_hash,result,event_id)
            VALUES($1,$2,$3,$4,$5,$6)", vec![task.into(),actor.subject.clone().into(),command.key().into(),
                hash.into(),json_value(&receipt)?,event_id.into()]).await?;
        if let (Some(event_id), Some(event)) = (event_id, event) {
            let payload = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
                "project_id":state.project_id,"task_id":task,"root_task_id":task,
                "owner_subject":state.owner_subject,"stage":receipt.task.stage,
                "requirement_revision":receipt.task.routing.requirement_revision,"result":&receipt});
            exec(
                &tx,
                "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,$3,$4)",
                vec![event_id.into(), task.into(), event.into(), payload.into()],
            )
            .await?;
        }
        tx.commit().await.map_err(map_db)?;
        Ok((receipt, false))
    }

    async fn lifecycle_read(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<LifecycleReadback, AppError> {
        if app::sdlc_lifecycle::has_delivery_grant(&actor.scopes) {
            app::sdlc_lifecycle::producer(actor, false)?;
        } else {
            app::sdlc_reservation::reader(actor)?;
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        let receipt = current(&tx, task)
            .await?
            .ok_or_else(|| AppError::not_found("delivery queue", task))?;
        exact_revision(&state, &receipt)?;
        authorize_read(&tx, &state, actor).await?;
        let observed_at = now(&tx).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(LifecycleReadback {
            observed_at,
            runtime_state: if receipt
                .lease
                .as_ref()
                .is_some_and(|l| l.expires_at <= observed_at)
            {
                RuntimeState::Unknown
            } else {
                receipt.task.runtime
            },
            receipt,
        })
    }

    async fn lifecycle_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<LifecycleReceipt, AppError> {
        app::sdlc_lifecycle::scheduler_read(actor, &self.config)?;
        app::sdlc::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        self.load(&tx, task, actor).await?;
        let (_, result) = operation(&tx, task, actor, key)
            .await?
            .ok_or_else(|| AppError::not_found("delivery operation", key))?;
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }
}
