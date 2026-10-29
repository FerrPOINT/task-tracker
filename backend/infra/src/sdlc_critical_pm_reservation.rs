//! Child of sdlc: durable PM review reservation only, never runtime admission or another queue.
use super::*;
use app::sdlc_critical_pm_reservation::{
    CompletedPmReceipt, CriticalPmReservationAllocation, CriticalPmReservationSource,
    prepare_critical_pm_reservation,
};
use domain::sdlc_critical_blockers::{
    CriticalBlockerReceipt, CriticalBlockerState, valid_operation_key,
};
use domain::sdlc_critical_pm_reservation::*;
use domain::sdlc_reservation::PmDraftCompletionReceipt;

fn inconsistent() -> AppError {
    AppError::conflict("critical PM review reservation history inconsistent")
}

fn decode<T: serde::de::DeserializeOwned>(row: &QueryResult, field: &str) -> Result<T, AppError> {
    serde_json::from_value(row.try_get::<Json>("", field).map_err(map_db)?)
        .map_err(|_| inconsistent())
}

fn key(value: &str) -> Result<(), AppError> {
    if !valid_operation_key(value) {
        return Err(AppError::validation("invalid critical operation key"));
    }
    Ok(())
}

async fn owner_access(tx: &DatabaseTransaction, state: &TaskState) -> Result<(), AppError> {
    let row = tx
        .query_one(statement(
            "SELECT u.id,p.owner_id FROM users u JOIN projects p ON p.id=$1
         WHERE u.central_sub=$2 AND u.is_active FOR SHARE OF u,p",
            vec![state.project_id.into(), state.owner_subject.clone().into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    let owner: Uuid = row.try_get("", "id").map_err(map_db)?;
    super::spaces::require_owner_project(tx, state.project_id, owner).await?;
    if owner != row.try_get::<Uuid>("", "owner_id").map_err(map_db)? {
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

async fn original_pm(tx: &DatabaseTransaction, task: Uuid) -> Result<PmAssignment, AppError> {
    decode(&tx.query_one(statement(
        "SELECT payload FROM sdlc_assignments WHERE task_id=$1 ORDER BY version LIMIT 1 FOR SHARE",
        vec![task.into()],
    )).await.map_err(map_db)?.ok_or_else(inconsistent)?, "payload")
}

async fn saved_result(
    tx: &DatabaseTransaction,
    state: &TaskState,
    row: &QueryResult,
) -> Result<CriticalPmReservationResult, AppError> {
    let result: CriticalPmReservationResult = decode(row, "result")?;
    let blocker: CriticalBlockerReceipt = decode(row, "after_blocker")?;
    let input = PostgresSdlcRepository::original_input(tx, state).await?;
    let original = original_pm(tx, state.task_id).await?;
    blocker.proposal.validate(blocker.role)?;
    let reservation = &result.reservation;
    if reservation.binding != PostgresSdlcRepository::draft_binding(state)
        || reservation.assignment.agent_id != original.agent_id
        || reservation.input.snapshot_ref != input.snapshot_ref
        || reservation.input.sha256 != input.sha256
        || reservation.dispatch_allowed
        || blocker.state != CriticalBlockerState::InPmReview
        || result.context.blocker_id != blocker.blocker_id
        || result.context.blocker_version != blocker.version
        || result.context.blocker_sha256 != blocker.blocker_sha256
        || result.context.requirements.revision != blocker.proposal.requirement_revision
        || result.context.requirements.content_hash != blocker.proposal.content_hash
        || blocker.pm_review.as_ref().is_none_or(|pm| {
            pm.assignment_id != reservation.assignment.assignment_id
                || pm.execution_id != reservation.assignment.execution_id
        })
        || blocker.task_id != state.task_id
        || blocker.root_task_id != state.task_id
        || blocker.project_id != state.project_id
        || blocker.tracker_instance_id != state.tracker_instance_id
        || blocker.owner_subject != state.owner_subject
        || blocker.blocker_sha256 != app::sdlc::canonical_hash(&blocker.proposal)?
    {
        return Err(inconsistent());
    }
    Ok(result)
}

impl PostgresSdlcRepository {
    pub async fn reserve_critical_pm_review(
        &self,
        task: Uuid,
        actor: &Principal,
        command: ReserveCriticalPmReview,
    ) -> Result<(CriticalPmReservationResult, bool), AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        command.validate()?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        owner_access(&tx, &state).await?;
        let hash = app::sdlc::canonical_hash(&command)?;
        if let Some(row) = tx
            .query_one(statement(
                "SELECT command,request_sha256,result,after_blocker FROM sdlc_critical_pm_reviews
             WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
                vec![
                    task.into(),
                    actor.subject.clone().into(),
                    command.idempotency_key.clone().into(),
                ],
            ))
            .await
            .map_err(map_db)?
        {
            let original: ReserveCriticalPmReview = decode(&row, "command")?;
            if original != command
                || row
                    .try_get::<String>("", "request_sha256")
                    .map_err(map_db)?
                    != hash
            {
                return Err(AppError::conflict(
                    "critical PM review key payload mismatch",
                ));
            }
            let result = saved_result(&tx, &state, &row).await?;
            tx.commit().await.map_err(map_db)?;
            return Ok((result, true));
        }
        let current = super::lifecycle::current(&tx, task)
            .await?
            .ok_or_else(inconsistent)?;
        super::lifecycle::exact_revision(&state, &current)?;
        let cursor = tx.query_one(statement(
            "SELECT state,pm_owner_version,pm_execution_id,pm_admission_state FROM sdlc_tasks WHERE task_id=$1",
            vec![task.into()],
        )).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        let owner_version: i64 = cursor.try_get("", "pm_owner_version").map_err(map_db)?;
        let execution: Option<Uuid> = cursor.try_get("", "pm_execution_id").map_err(map_db)?;
        let admission: Option<String> = cursor.try_get("", "pm_admission_state").map_err(map_db)?;
        let previous = Self::reservation(&tx, &state, execution.ok_or_else(inconsistent)?).await?;
        let configured;
        let historical;
        let completion = if state.configured_cli() {
            configured = Self::configured_saved(&tx, &state, None)
                .await?
                .ok_or_else(inconsistent)?
                .result;
            CompletedPmReceipt::Configured(&configured)
        } else {
            historical = decode::<PmDraftCompletionReceipt>(&tx.query_one(statement(
                "SELECT result FROM sdlc_pm_completions WHERE task_id=$1 AND execution_id=$2 FOR SHARE",
                vec![task.into(), previous.assignment.execution_id.into()],
            )).await.map_err(map_db)?.ok_or_else(inconsistent)?, "result")?;
            CompletedPmReceipt::Historical(&historical)
        };
        let original = original_pm(&tx, task).await?;
        let input = PmDraftInputResponse {
            contract_version: 1,
            tracker_instance_id: state.tracker_instance_id.clone(),
            project_id: state.project_id,
            task_id: task,
            root_task_id: task,
            owner_subject: state.owner_subject.clone(),
            input: Self::original_input(&tx, &state).await?,
        };
        let blocker: CriticalBlockerReceipt = decode(&tx.query_one(statement(
            "SELECT sdlc_critical_pm_detail(h.blocker_id) AS payload FROM sdlc_critical_blocker_heads h
             JOIN sdlc_critical_pm_returns r ON r.task_id=h.task_id AND r.blocker_id=h.blocker_id
             WHERE h.task_id=$1 AND h.blocker_id=$2 FOR SHARE OF h,r",
            vec![task.into(), command.blocker_id.into()],
        )).await.map_err(map_db)?.ok_or_else(inconsistent)?, "payload")?;
        // Replay above precedes all fresh CAS and server allocation.
        if command.expected_task_version != current.task.version
            || command.expected_blocker_version != blocker.version
            || command.blocker_sha256 != blocker.blocker_sha256
            || command.expected_pm_owner_version != owner_version
            || command.expected_pm_assignment_version != previous.assignment.version
        {
            return Err(AppError::conflict("critical PM review stale CAS"));
        }
        let ordinal: i64 = tx
            .query_one(statement(
                "SELECT nextval(pg_get_serial_sequence('sdlc_pm_executions','ordinal')) AS ordinal",
                vec![],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(inconsistent)?
            .try_get("", "ordinal")
            .map_err(map_db)?;
        let plan = prepare_critical_pm_reservation(
            CriticalPmReservationSource {
                state: &state,
                lifecycle: &current,
                blocker: &blocker,
                previous_reservation: &previous,
                current_completed: completion,
                original_pm_assignment: &original,
                original_input: &input,
                pm_owner_version: owner_version,
                pm_admission_state: admission.as_deref().unwrap_or(""),
            },
            actor,
            &self.config,
            &command,
            CriticalPmReservationAllocation {
                assignment_id: Uuid::now_v7(),
                execution_id: Uuid::now_v7(),
                ordinal,
            },
        )?;
        let before_state: Json = cursor.try_get("", "state").map_err(map_db)?;
        let before_execution: Json = tx
            .query_one(statement(
                "SELECT execution FROM sdlc_delivery_heads WHERE task_id=$1",
                vec![task.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(inconsistent)?
            .try_get("", "execution")
            .map_err(map_db)?;
        let after_state = serde_json::to_value(&plan.state).map_err(AppError::internal)?;
        let mut after_execution =
            serde_json::to_value(&plan.lifecycle).map_err(AppError::internal)?;
        after_execution
            .as_object_mut()
            .ok_or_else(inconsistent)?
            .remove("task");
        let review_id = Uuid::now_v7();
        let reservation = &plan.result.reservation;
        let assignment = &reservation.assignment;
        exec(
            &tx,
            "INSERT INTO sdlc_critical_pm_reviews(review_id,task_id,blocker_id,actor_subject,
            idempotency_key,request_sha256,command,previous_execution_id,previous_owner_version,
            execution_id,assignment_id,assignment_version,owner_version,
            before_state,after_state,before_execution,after_execution,after_blocker,result)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)",
            vec![
                review_id.into(),
                task.into(),
                blocker.blocker_id.into(),
                actor.subject.clone().into(),
                command.idempotency_key.clone().into(),
                hash.into(),
                json_value(&command)?,
                previous.assignment.execution_id.into(),
                owner_version.into(),
                assignment.execution_id.into(),
                assignment.assignment_id.into(),
                assignment.version.into(),
                reservation.owner_cas.version.into(),
                before_state.into(),
                after_state.clone().into(),
                before_execution.into(),
                after_execution.clone().into(),
                json_value(&plan.blocker)?,
                json_value(&plan.result)?,
            ],
        )
        .await?;
        exec(
            &tx,
            "INSERT INTO sdlc_assignments(task_id,version,payload) VALUES($1,$2,$3)",
            vec![
                task.into(),
                assignment.version.into(),
                json_value(assignment)?,
            ],
        )
        .await?;
        exec(&tx, "INSERT INTO sdlc_pm_executions(execution_id,ordinal,task_id,assignment_id,assignment_version,
            expected_owner_version,owner_version,input_snapshot_ref,input_sha256,assignment_operation_key,result)
            OVERRIDING SYSTEM VALUE VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)", vec![
                assignment.execution_id.into(), ordinal.into(), task.into(), assignment.assignment_id.into(),
                assignment.version.into(), owner_version.into(), reservation.owner_cas.version.into(),
                reservation.input.snapshot_ref.into(), reservation.input.sha256.clone().into(),
                reservation.assignment_operation_key.clone().into(), json_value(reservation)?,
            ]).await?;
        exec(&tx, "INSERT INTO sdlc_delivery_projection_permits(task_id,transaction_id,next_state,next_execution)
            VALUES($1,pg_current_xact_id(),$2,$3)", vec![task.into(), after_state.clone().into(), after_execution.clone().into()]).await?;
        exec(
            &tx,
            "INSERT INTO sdlc_pm_state_write_permits(task_id,transaction_id,next_state)
            VALUES($1,pg_current_xact_id(),$2)",
            vec![task.into(), after_state.clone().into()],
        )
        .await?;
        exec(&tx, "UPDATE sdlc_tasks SET state=$2,pm_owner_version=$3,pm_execution_id=$4,pm_admission_state='reserved'
            WHERE task_id=$1 AND pm_owner_version=$5 AND pm_execution_id=$6 AND pm_admission_state='completed'", vec![
                task.into(), after_state.into(), reservation.owner_cas.version.into(),
                assignment.execution_id.into(), owner_version.into(), previous.assignment.execution_id.into(),
            ]).await?;
        exec(
            &tx,
            "UPDATE sdlc_delivery_heads SET execution=$2 WHERE task_id=$1",
            vec![task.into(), after_execution.into()],
        )
        .await?;
        let status = super::lifecycle::mapped_issue_status(
            &tx,
            plan.lifecycle.task.stage.canonical(),
            plan.lifecycle.task.status,
        )
        .await?;
        super::lifecycle::project_issue_status(&tx, task, actor, status).await?;
        exec(
            &tx,
            "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload)
            VALUES($1,$2,'critical.pm_review_reserved',$3)",
            vec![
                review_id.into(),
                task.into(),
                json!({
                    "contract_version":1,"tracker_instance_id":state.tracker_instance_id,
                    "project_id":state.project_id,"task_id":task,"root_task_id":task,
                    "owner_subject":state.owner_subject,"stage":"Draft","result":plan.result,
                })
                .into(),
            ],
        )
        .await?;
        tx.commit().await.map_err(map_db)?;
        Ok((plan.result, false))
    }

    pub async fn critical_pm_review_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        original_key: &str,
    ) -> Result<CriticalPmReservationResult, AppError> {
        app::sdlc_lifecycle::scheduler_read(actor, &self.config)?;
        key(original_key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        owner_access(&tx, &state).await?;
        let row = tx
            .query_one(statement(
                "SELECT result,after_blocker FROM sdlc_critical_pm_reviews WHERE task_id=$1
             AND actor_subject=$2 AND idempotency_key=$3",
                vec![
                    task.into(),
                    actor.subject.clone().into(),
                    original_key.into(),
                ],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::not_found("critical PM review operation", original_key))?;
        let result = saved_result(&tx, &state, &row).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }
}
