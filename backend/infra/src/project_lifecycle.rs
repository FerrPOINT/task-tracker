//! Independent project metadata. Canonical links and Space catalog survive every transition.
use domain::project_spaces::{ChangeProjectLifecycle, SpaceWritePermit};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait, Value};
use shared::{AppError, ProjectId, project_spaces::ProjectLifecycle, space_access::ProjectRef};
fn sql(query: &str, args: impl IntoIterator<Item = Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, query, args)
}
fn project_ref(project: ProjectId) -> Result<ProjectRef, AppError> {
    let instance = std::env::var("TT_NAMESPACE__INSTANCE_ID")
        .ok()
        .and_then(|s| s.parse::<uuid::Uuid>().ok())
        .filter(|id| !id.is_nil())
        .ok_or_else(|| AppError::Unavailable("project_owner_not_configured".into()))?;
    Ok(ProjectRef {
        tracker_instance_id: instance,
        project_id: project.as_uuid(),
    })
}
pub async fn get<C: ConnectionTrait>(
    db: &C,
    project: ProjectId,
) -> Result<ProjectLifecycle, AppError> {
    let row=db.query_one(sql("SELECT project_lifecycle_revision AS revision,(archived_at IS NOT NULL) AS archived FROM projects WHERE id=$1",[project.as_uuid().into()])).await.map_err(crate::db_error::map)?.ok_or_else(||AppError::not_found("project",project))?;
    let view = ProjectLifecycle {
        schema_version: 1,
        project: project_ref(project)?,
        revision: row.try_get("", "revision").map_err(crate::db_error::map)?,
        archived: row.try_get("", "archived").map_err(crate::db_error::map)?,
    };
    if !view.valid() {
        return Err(AppError::Unavailable("invalid_project_lifecycle".into()));
    }
    Ok(view)
}
pub async fn change(
    db: &DatabaseConnection,
    project: ProjectId,
    command: &ChangeProjectLifecycle,
    permit: &SpaceWritePermit,
) -> Result<ProjectLifecycle, AppError> {
    command.validate()?;
    if !permit.is_current() {
        return Err(AppError::Forbidden);
    }
    let tx = db.begin().await.map_err(crate::db_error::map)?;
    tx.query_one(sql(
        "SELECT singleton FROM tracker_space_admission_state WHERE singleton FOR SHARE",
        [],
    ))
    .await
    .map_err(crate::db_error::map)?;
    tx.execute(sql(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        [format!("project-lifecycle:{}", command.operation_id).into()],
    ))
    .await
    .map_err(crate::db_error::map)?;
    let row = tx
        .query_one(sql(
            "SELECT owner_id FROM projects WHERE id=$1 FOR UPDATE",
            [project.as_uuid().into()],
        ))
        .await
        .map_err(crate::db_error::map)?
        .ok_or_else(|| AppError::not_found("project", project))?;
    if row
        .try_get::<uuid::Uuid>("", "owner_id")
        .map_err(crate::db_error::map)?
        != permit.actor().as_uuid()
    {
        return Err(AppError::Forbidden);
    }
    let user = tx
        .query_one(sql(
            "SELECT is_active,central_sub FROM users WHERE id=$1 FOR SHARE",
            [permit.actor().as_uuid().into()],
        ))
        .await
        .map_err(crate::db_error::map)?;
    if !user.is_some_and(|u| {
        u.try_get::<bool>("", "is_active").unwrap_or(false)
            && u.try_get::<Option<String>>("", "central_sub")
                .ok()
                .flatten()
                .as_deref()
                == Some(permit.identity().subject.as_str())
    }) {
        return Err(AppError::Forbidden);
    }
    tx.query_one(sql(
        "SELECT revision FROM tracker_project_space_sets WHERE project_id=$1 FOR UPDATE",
        [project.as_uuid().into()],
    ))
    .await
    .map_err(crate::db_error::map)?
    .ok_or_else(|| AppError::Unavailable("project_spaces_missing".into()))?;
    let links = crate::project_spaces::get(&tx, project).await?;
    let active = crate::project_spaces::lock_catalog(&tx, &links.spaces).await?;
    if !links
        .spaces
        .iter()
        .any(|s| active.contains(s) && permit.allows(s))
        || !permit.is_current()
    {
        return Err(AppError::Forbidden);
    }
    let identity =
        serde_json::to_value(permit.identity()).map_err(|e| AppError::Internal(e.to_string()))?;
    let payload = serde_json::to_value(command).map_err(|e| AppError::Internal(e.to_string()))?;
    let current = get(&tx, project).await?;
    if let Some(previous)=tx.query_one(sql("SELECT project_id,actor_identity,command FROM tracker_project_lifecycle_operations WHERE operation_id=$1",[command.operation_id.into()])).await.map_err(crate::db_error::map)?{
  if previous.try_get::<uuid::Uuid>("","project_id").map_err(crate::db_error::map)?!=project.as_uuid() || previous.try_get::<serde_json::Value>("","actor_identity").map_err(crate::db_error::map)?!=identity || previous.try_get::<serde_json::Value>("","command").map_err(crate::db_error::map)?!=payload{return Err(AppError::conflict("project_lifecycle_operation_conflict"));}
  if !permit.is_current(){return Err(AppError::Forbidden);}tx.commit().await.map_err(crate::db_error::map)?;return Ok(current);
 }
    if current.revision != command.expected_revision {
        return Err(AppError::conflict("project_lifecycle_revision_conflict"));
    }
    tx.execute(sql("INSERT INTO tracker_project_lifecycle_operations(operation_id,project_id,actor_identity,command,authority_valid_until) VALUES($1,$2,$3,$4,$5)",[command.operation_id.into(),project.as_uuid().into(),identity.into(),payload.into(),permit.valid_until().into()])).await.map_err(crate::db_error::map)?;
    if current.archived != command.archived {
        tx.execute(sql(
            "SELECT set_config('pdlc.tracker.project_lifecycle_operation',$1,true)",
            [command.operation_id.to_string().into()],
        ))
        .await
        .map_err(crate::db_error::map)?;
        tx.execute(sql("UPDATE projects SET archived_at=CASE WHEN $2 THEN now() ELSE NULL END,project_lifecycle_revision=project_lifecycle_revision+1 WHERE id=$1",[project.as_uuid().into(),command.archived.into()])).await.map_err(crate::db_error::map)?;
    }
    let result = get(&tx, project).await?;
    if !permit.is_current() {
        return Err(AppError::Forbidden);
    }
    tx.commit().await.map_err(crate::db_error::map)?;
    Ok(result)
}
