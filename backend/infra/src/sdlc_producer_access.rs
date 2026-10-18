//! Shared current delivery-producer authorization for task-owned contributions.
use super::*;
use domain::sdlc_lifecycle::*;

fn decode<T: serde::de::DeserializeOwned>(row: &QueryResult, field: &str) -> Result<T, AppError> {
    serde_json::from_value(row.try_get::<Json>("", field).map_err(map_db)?)
        .map_err(|_| AppError::conflict("SDLC producer history invalid"))
}

pub(super) async fn local_access(
    tx: &DatabaseTransaction,
    project: Uuid,
    user: Uuid,
) -> Result<(), AppError> {
    tx.query_one(statement(
        "SELECT id FROM users WHERE id=$1 AND is_active FOR SHARE",
        vec![user.into()],
    ))
    .await
    .map_err(map_db)?
    .ok_or(AppError::Forbidden)?;
    let row = tx
        .query_one(statement(
            "SELECT owner_id FROM projects WHERE id=$1 FOR SHARE",
            vec![project.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    if row.try_get::<Uuid>("", "owner_id").map_err(map_db)? != user {
        tx.query_one(statement(
            "SELECT user_id FROM project_members WHERE project_id=$1 AND user_id=$2 FOR SHARE",
            vec![project.into(), user.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    }
    Ok(())
}

pub(super) struct Authority {
    pub(super) user: Uuid,
    pub(super) state: TaskState,
    pub(super) head: LifecycleReceipt,
    pub(super) scheduler: bool,
}

pub(super) enum ProjectGate {
    EnabledProject,
    CurrentAssignment,
}

pub(super) async fn authority(
    tx: &DatabaseTransaction,
    config: &SdlcConfig,
    task: Uuid,
    actor: &Principal,
    write: bool,
    project_gate: ProjectGate,
) -> Result<Authority, AppError> {
    let grant = if write {
        Some(app::sdlc_lifecycle::producer(actor, true)?)
    } else if let Some(grant) = app::sdlc_lifecycle::delivery_grant(actor)? {
        app::sdlc_lifecycle::producer(actor, false)?;
        Some(grant)
    } else {
        app::sdlc_lifecycle::scheduler_read(actor, config)?;
        None
    };
    if grant.as_ref().is_some_and(|grant| grant.task_id != task) {
        return Err(AppError::Forbidden);
    }
    let issue = tx
        .query_one(statement(
            "SELECT project_id,status_id FROM issues WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",
            vec![task.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    let project: Uuid = issue.try_get("", "project_id").map_err(map_db)?;
    let user = tx
        .query_one(statement(
            "SELECT id FROM users WHERE central_sub=$1 AND is_active FOR SHARE",
            vec![actor.subject.clone().into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?
        .try_get::<Uuid>("", "id")
        .map_err(map_db)?;
    local_access(tx, project, user).await?;
    if matches!(project_gate, ProjectGate::EnabledProject)
        && !config.single_task_project_ids.contains(&project)
    {
        return Err(AppError::Unavailable(
            "single-task artifact policy disabled".into(),
        ));
    }
    let state: TaskState = decode(
        &tx.query_one(statement(
            "SELECT state FROM sdlc_tasks WHERE task_id=$1 FOR UPDATE",
            vec![task.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?,
        "state",
    )?;
    state.require_root()?;
    if state.task_id != task
        || state.project_id != project
        || state.tracker_instance_id != config.instance_id
        || state.draft_flow()? != Some(DraftFlow::SingleTaskV1)
    {
        return Err(AppError::Forbidden);
    }
    let head: LifecycleReceipt = decode(
        &tx.query_one(statement(
            "SELECT receipt FROM sdlc_delivery_tasks WHERE task_id=$1 FOR UPDATE",
            vec![task.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?,
        "receipt",
    )?;
    super::lifecycle::exact_revision(&state, &head)?;
    if head.task.routing.contract_version != 2 {
        return Err(AppError::conflict("single-task artifact routing required"));
    }
    app::sdlc_routing::validate_routes(&head.task.routing.policy.routes)
        .map_err(|_| AppError::conflict("artifact routing history invalid"))?;
    if issue.try_get::<Uuid>("", "status_id").map_err(map_db)?
        != super::lifecycle::mapped_issue_status(tx, head.task.stage.canonical(), head.task.status)
            .await?
    {
        return Err(AppError::conflict("artifact lifecycle projection drift"));
    }
    let assignment = head.assignment.as_ref().ok_or(AppError::Forbidden)?;
    let binding = head.binding.as_ref().ok_or(AppError::Forbidden)?;
    if assignment.task_id != task
        || assignment.owner_subject != head.task.owner_subject
        || assignment.stage != head.task.stage
        || assignment.cycle != head.task.cycle
        || assignment.routing != head.task.routing
        || assignment.stage.role() != Some(assignment.role)
        || assignment.assignment_hash != app::sdlc_lifecycle::assignment_hash(assignment)?
    {
        return Err(AppError::conflict("artifact assignment drift"));
    }
    let saved: DeliveryAssignment = decode(
        &tx.query_one(statement(
            "SELECT payload FROM sdlc_delivery_assignments WHERE assignment_id=$1 AND task_id=$2",
            vec![assignment.assignment_id.into(), task.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?,
        "payload",
    )?;
    let run: RunBinding = decode(
        &tx.query_one(statement(
            "SELECT payload FROM sdlc_delivery_run_bindings WHERE assignment_id=$1 AND run_id=$2",
            vec![assignment.assignment_id.into(), binding.run_id.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?,
        "payload",
    )?;
    if &saved != assignment || &run != binding {
        return Err(AppError::conflict("artifact binding history drift"));
    }
    if let Some(grant) = &grant {
        app::sdlc_lifecycle::authorize_delivery_assignment(grant, assignment)?;
        if binding.machine_subject != actor.subject {
            return Err(AppError::Forbidden);
        }
    }
    if write {
        if head
            .lease
            .as_ref()
            .is_none_or(|lease| lease.holder_subject != config.reservation_scheduler_subject)
        {
            return Err(AppError::Forbidden);
        }
        ensure_live(tx, &head).await?;
    }
    Ok(Authority {
        user,
        state,
        head,
        scheduler: grant.is_none(),
    })
}

pub(super) async fn ensure_live(
    tx: &DatabaseTransaction,
    head: &LifecycleReceipt,
) -> Result<(), AppError> {
    let assignment = head.assignment.as_ref().ok_or(AppError::Forbidden)?;
    let lease = head.lease.as_ref().ok_or(AppError::Forbidden)?;
    let now: chrono::DateTime<chrono::Utc> = tx
        .query_one(statement("SELECT clock_timestamp() AS now", vec![]))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?
        .try_get("", "now")
        .map_err(map_db)?;
    if head.task.runtime != RuntimeState::Running
        || head.task.status != LifecycleStatus::InProgress
        || lease.lease_id != assignment.lease_id
        || lease.version < 1
        || lease.expires_at <= now
    {
        return Err(AppError::conflict(
            "artifact publisher not running or lease expired",
        ));
    }
    // The scheduler owns this lease. Agent identity is the run binding, never lease.holder_subject.
    Ok(())
}
