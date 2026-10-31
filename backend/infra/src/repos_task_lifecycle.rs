//! Common task commands use the same guarded request transaction as ordinary content.
use crate::guarded_db::GuardedDb;
use domain::task_lifecycle::{self as model, Readback, State, TransitionCommand};
use sea_orm::{ConnectionTrait, DatabaseBackend, QueryResult, Statement, TransactionTrait};
use serde_json::{Value, json};
use shared::{AppError, IssueId, UserId};

fn sql(text: &str, values: Vec<sea_orm::Value>) -> Statement {
    Statement::from_sql_and_values(DatabaseBackend::Postgres, text, values)
}
fn view(row: &QueryResult, task: IssueId, may_edit: bool) -> Result<Readback, AppError> {
    let state: Value = row.try_get("", "state").map_err(crate::db_error::map)?;
    let revision = row
        .try_get("", "core_revision")
        .map_err(crate::db_error::map)?;
    let busy = row
        .try_get("", "execution_busy")
        .map_err(crate::db_error::map)?;
    let projected = model::effective_state(&state, revision)?;
    let draft_submode = if state.get("stage").and_then(Value::as_str) == Some("Clarification") {
        Some(domain::sdlc::DraftSubmode::Clarification)
    } else {
        state
            .get("draft_submode")
            .filter(|v| !v.is_null())
            .map(|v| serde_json::from_value(v.clone()).map_err(AppError::internal))
            .transpose()?
    };
    let state = projected;
    Ok(Readback {
        task_id: task.as_uuid(),
        state,
        draft_submode,
        allowed_actions: model::transition_options(state, may_edit, busy),
        execution_busy: busy,
    })
}
const READ: &str = "SELECT t.state,t.core_revision,i.description,task_lifecycle_execution_busy(t.task_id) AS execution_busy
    FROM sdlc_tasks t JOIN issues i ON i.id=t.task_id WHERE t.task_id=$1 AND i.deleted_at IS NULL";

pub(crate) async fn read(
    db: &GuardedDb,
    task: IssueId,
    may_edit: bool,
) -> Result<Readback, AppError> {
    let row = db
        .query_one(sql(READ, vec![task.as_uuid().into()]))
        .await
        .map_err(crate::db_error::map)?
        .ok_or_else(|| AppError::Unavailable("task_lifecycle_import_required".into()))?;
    view(&row, task, may_edit)
}

pub(crate) async fn change(
    db: &GuardedDb,
    task: IssueId,
    actor: UserId,
    command: &TransitionCommand,
) -> Result<Readback, AppError> {
    command.validate()?;
    let hash = app::sdlc::canonical_hash(&json!(command))?;
    let tx = db.begin().await.map_err(crate::db_error::map)?;
    let row = tx
        .query_one(sql(
            &format!("{READ} FOR UPDATE OF t,i"),
            vec![task.as_uuid().into()],
        ))
        .await
        .map_err(crate::db_error::map)?
        .ok_or_else(|| AppError::Unavailable("task_lifecycle_import_required".into()))?;
    let current = view(&row, task, true)?;
    let previous = tx
        .query_one(sql(
            "SELECT command_hash,next_state,next_submode FROM task_lifecycle_history
        WHERE task_id=$1 AND actor_id=$2 AND idempotency_key=$3",
            vec![
                task.as_uuid().into(),
                actor.as_uuid().into(),
                command.idempotency_key.clone().into(),
            ],
        ))
        .await
        .map_err(crate::db_error::map)?;
    if let Some(previous) = previous {
        let saved_hash: String = previous
            .try_get("", "command_hash")
            .map_err(crate::db_error::map)?;
        if saved_hash != hash {
            return Err(AppError::conflict("task_lifecycle_idempotency_conflict"));
        }
        let saved: State = serde_json::from_value(
            previous
                .try_get("", "next_state")
                .map_err(crate::db_error::map)?,
        )
        .map_err(AppError::internal)?;
        tx.commit().await.map_err(crate::db_error::map)?;
        return Ok(Readback {
            task_id: task.as_uuid(),
            state: saved,
            draft_submode: previous
                .try_get::<Option<String>>("", "next_submode")
                .map_err(crate::db_error::map)?
                .map(|mode| serde_json::from_value(json!(mode)).map_err(AppError::internal))
                .transpose()?,
            execution_busy: current.execution_busy,
            allowed_actions: if saved == current.state {
                current.allowed_actions
            } else {
                Vec::new()
            },
        });
    }
    if current.state.revision != command.expected_revision {
        return Err(AppError::conflict("task_lifecycle_revision_conflict"));
    }
    if !current
        .allowed_actions
        .iter()
        .any(|option| option.action == command.action)
    {
        return Err(AppError::conflict(
            "task_lifecycle_execution_busy_or_transition_denied",
        ));
    }
    if command.action == model::Action::Complete {
        let description: Option<String> = row
            .try_get("", "description")
            .map_err(crate::db_error::map)?;
        if description
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(AppError::validation("task_lifecycle_description_required"));
        }
    }
    let next = model::transition(current.state, command.action)?;
    let mut state: Value = row.try_get("", "state").map_err(crate::db_error::map)?;
    for key in ["lifecycle", "runtime_state", "waiting_reason"] {
        state
            .as_object_mut()
            .ok_or_else(|| AppError::conflict("invalid task state"))?
            .remove(key);
    }
    let next_submode = if next.stage == domain::sdlc_lifecycle::LifecycleStage::Draft {
        current.draft_submode
    } else {
        None
    };
    if let Some(mode) = &next_submode {
        state["draft_submode"] = json!(mode);
    } else {
        state.as_object_mut().unwrap().remove("draft_submode");
    }
    state["stage"] = json!(next.stage);
    state["status"] = json!(next.status);
    let execute = |text: &str, values| tx.execute(sql(text, values));
    execute(
        "INSERT INTO task_lifecycle_write_permits(task_id,transaction_id,next_state)
        VALUES($1,pg_current_xact_id(),$2)",
        vec![task.as_uuid().into(), state.clone().into()],
    )
    .await
    .map_err(crate::db_error::map)?;
    execute(
        "UPDATE sdlc_tasks SET state=$2,manual_control=true WHERE task_id=$1",
        vec![task.as_uuid().into(), state.into()],
    )
    .await
    .map_err(crate::db_error::map)?;
    crate::task_lifecycle_projection::project_status(
        &tx,
        task.as_uuid(),
        actor.as_uuid(),
        model::projection_status_id(next.stage, next.status).as_uuid(),
    )
    .await?;
    execute("INSERT INTO task_lifecycle_history(task_id,actor_id,action,previous_state,next_state,idempotency_key,command_hash,next_submode)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8)",vec![task.as_uuid().into(),actor.as_uuid().into(),
            serde_json::to_value(command.action).map_err(AppError::internal)?.as_str().unwrap().into(),
            json!(current.state).into(),json!(next).into(),command.idempotency_key.clone().into(),hash.into(),
            next_submode.as_ref().map(|value|serde_json::to_value(value).unwrap().as_str().unwrap().to_owned()).into()])
        .await.map_err(crate::db_error::map)?;
    execute("DELETE FROM task_lifecycle_write_permits WHERE task_id=$1 AND transaction_id=pg_current_xact_id()",
        vec![task.as_uuid().into()]).await.map_err(crate::db_error::map)?;
    tx.commit().await.map_err(crate::db_error::map)?;
    Ok(Readback {
        task_id: task.as_uuid(),
        state: next,
        draft_submode: next_submode,
        allowed_actions: model::transition_options(next, true, false),
        execution_busy: false,
    })
}

pub(crate) async fn active(db: &GuardedDb) -> Result<bool, AppError> {
    db.query_one(sql(
        "SELECT enabled FROM task_lifecycle_control WHERE singleton",
        vec![],
    ))
    .await
    .map_err(crate::db_error::map)?
    .ok_or_else(|| AppError::Unavailable("task_lifecycle_not_configured".into()))?
    .try_get("", "enabled")
    .map_err(crate::db_error::map)
}
pub(crate) async fn initial_status(db: &GuardedDb) -> Result<Option<shared::StatusId>, AppError> {
    if !active(db).await? {
        return Ok(None);
    }
    let row = db
        .query_one(sql(
            "SELECT task_lifecycle_status_id('Draft','Ready') AS id",
            vec![],
        ))
        .await
        .map_err(crate::db_error::map)?
        .ok_or_else(|| AppError::Unavailable("task_lifecycle_not_configured".into()))?;
    Ok(Some(shared::StatusId::from_uuid(
        row.try_get("", "id").map_err(crate::db_error::map)?,
    )))
}
pub(crate) async fn project(
    db: &GuardedDb,
    project: shared::ProjectId,
    may_edit: bool,
) -> Result<Vec<Readback>, AppError> {
    let rows=db.query_all(sql("SELECT t.task_id,t.state,t.core_revision,task_lifecycle_execution_busy(t.task_id) AS execution_busy
        FROM sdlc_tasks t JOIN issues i ON i.id=t.task_id WHERE t.project_id=$1 AND i.deleted_at IS NULL ORDER BY i.position,i.id",
        vec![project.as_uuid().into()])).await.map_err(crate::db_error::map)?;
    rows.iter()
        .map(|row| {
            let id = row.try_get("", "task_id").map_err(crate::db_error::map)?;
            view(row, IssueId::from_uuid(id), may_edit)
        })
        .collect()
}

pub(crate) async fn columns(
    db: &impl ConnectionTrait,
    project: shared::ProjectId,
) -> Result<Vec<model::StageProjection>, AppError> {
    let rows=db.query_all(sql("SELECT m.stage,m.status_id,b.wip_limit::bigint AS wip_limit FROM sdlc_issue_status_mapping m
        LEFT JOIN task_lifecycle_board_limits b ON b.project_id=$1 AND b.stage=m.stage WHERE m.status='Ready'",
        vec![project.as_uuid().into()])).await.map_err(crate::db_error::map)?;
    let mut columns = Vec::new();
    for stage in model::STAGES {
        let name = serde_json::to_value(stage).map_err(AppError::internal)?;
        let row = rows
            .iter()
            .find(|row| row.try_get::<String>("", "stage").ok().as_deref() == name.as_str())
            .ok_or_else(|| {
                AppError::Unavailable("task_lifecycle_column_projection_missing".into())
            })?;
        columns.push(model::StageProjection {
            stage,
            projection_status_id: shared::StatusId::from_uuid(
                row.try_get("", "status_id").map_err(crate::db_error::map)?,
            ),
            wip_limit: row.try_get("", "wip_limit").map_err(crate::db_error::map)?,
        });
    }
    Ok(columns)
}

pub(crate) async fn states(
    db: &GuardedDb,
    ids: &[IssueId],
) -> Result<std::collections::HashMap<IssueId, State>, AppError> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let values = sea_orm::Value::Array(
        sea_orm::sea_query::ArrayType::Uuid,
        Some(Box::new(
            ids.iter()
                .map(|id| sea_orm::Value::from(id.as_uuid()))
                .collect(),
        )),
    );
    let rows=db.query_all(sql("SELECT task_id,state,core_revision,false AS execution_busy FROM sdlc_tasks WHERE task_id=ANY($1::uuid[])",vec![values]))
        .await.map_err(crate::db_error::map)?;
    rows.iter()
        .map(|row| {
            let id = IssueId::from_uuid(row.try_get("", "task_id").map_err(crate::db_error::map)?);
            Ok((id, view(row, id, false)?.state))
        })
        .collect()
}

pub(crate) async fn board_snapshot(
    db: &GuardedDb,
    project: shared::ProjectId,
    may_edit: bool,
) -> Result<model::StageBoardSnapshot, AppError> {
    use sea_orm::{AccessMode, FromQueryResult, IsolationLevel};
    let tx = db
        .begin_with_config(
            Some(IsolationLevel::RepeatableRead),
            Some(AccessMode::ReadOnly),
        )
        .await
        .map_err(crate::db_error::map)?;
    let control = tx
        .query_one(sql(
            "SELECT enabled FROM task_lifecycle_control WHERE singleton",
            vec![],
        ))
        .await
        .map_err(crate::db_error::map)?
        .ok_or_else(|| AppError::Unavailable("task_lifecycle_not_configured".into()))?;
    if !control
        .try_get::<bool>("", "enabled")
        .map_err(crate::db_error::map)?
    {
        return Err(AppError::conflict("task_lifecycle_not_active"));
    }
    let rows=tx.query_all(sql("SELECT i.*,t.state,t.core_revision,
        task_lifecycle_execution_busy(t.task_id) AS execution_busy,
        assignee.display_name AS assignee_name,reporter.display_name AS reporter_name,
        COALESCE((SELECT jsonb_agg(l.name ORDER BY l.name,l.id)
            FROM issue_labels link JOIN labels l ON l.id=link.label_id WHERE link.issue_id=i.id),'[]'::jsonb) AS label_names
        FROM issues i LEFT JOIN sdlc_tasks t ON t.task_id=i.id
        LEFT JOIN users assignee ON assignee.id=i.assignee_id LEFT JOIN users reporter ON reporter.id=i.reporter_id
        WHERE i.project_id=$1 AND i.deleted_at IS NULL ORDER BY i.position,i.id",vec![project.as_uuid().into()]))
        .await.map_err(crate::db_error::map)?;
    let mut cards = Vec::with_capacity(rows.len());
    for row in rows {
        if row
            .try_get::<Option<i64>>("", "core_revision")
            .map_err(crate::db_error::map)?
            .is_none()
        {
            return Err(AppError::conflict("task_lifecycle_board_import_incomplete"));
        }
        let issue = crate::repos::map_issue(
            crate::entities::issue::Model::from_query_result(&row, "")
                .map_err(crate::db_error::map)?,
        );
        let lifecycle = view(&row, issue.id, may_edit)?;
        let labels: Value = row
            .try_get("", "label_names")
            .map_err(crate::db_error::map)?;
        cards.push(model::StageBoardIssue {
            issue,
            lifecycle,
            assignee_name: row
                .try_get("", "assignee_name")
                .map_err(crate::db_error::map)?,
            reporter_name: row
                .try_get("", "reporter_name")
                .map_err(crate::db_error::map)?,
            labels: serde_json::from_value(labels).map_err(AppError::internal)?,
        });
    }
    let columns = columns(&tx, project).await?;
    let sprint = tx
        .query_one(sql(
            "SELECT * FROM sprints WHERE project_id=$1 AND state='active' ORDER BY id LIMIT 1",
            vec![project.as_uuid().into()],
        ))
        .await
        .map_err(crate::db_error::map)?
        .map(|row| {
            crate::entities::sprint::Model::from_query_result(&row, "")
                .map(crate::repos::map_sprint)
                .map_err(crate::db_error::map)
        })
        .transpose()?;
    tx.commit().await.map_err(crate::db_error::map)?;
    Ok(model::StageBoardSnapshot {
        cards,
        columns,
        sprint,
    })
}
