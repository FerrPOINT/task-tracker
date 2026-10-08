//! Local owner transaction. No dependency on Admin availability after confirmation.
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, TransactionTrait, Value,
};
use shared::{AppError, ProjectId, resource_context::*};
use uuid::Uuid;

fn statement(sql: &str, values: Vec<Value>) -> Statement {
    Statement::from_sql_and_values(DatabaseBackend::Postgres, sql, values)
}
fn id(value: Uuid) -> Value {
    Value::Uuid(Some(Box::new(value)))
}

pub async fn binding(
    db: &DatabaseConnection,
    project: ProjectId,
) -> Result<Option<OwnerReadback>, AppError> {
    let Some(row) = db
        .query_one(statement(
            "SELECT command FROM tracker_namespace_bindings WHERE resource_id=$1",
            vec![id(project.as_uuid())],
        ))
        .await
        .map_err(AppError::database)?
    else {
        let managed = db
            .query_one(statement(
                "SELECT namespace_managed FROM projects WHERE id=$1",
                vec![id(project.as_uuid())],
            ))
            .await
            .map_err(AppError::database)?;
        if managed.is_some_and(|row| row.try_get::<bool>("", "namespace_managed").unwrap_or(true)) {
            return Err(AppError::Unavailable("namespace_projection_missing".into()));
        }
        return Ok(None);
    };
    let value: serde_json::Value = row.try_get("", "command").map_err(AppError::database)?;
    let command: OwnerCommand = serde_json::from_value(value)
        .map_err(|_| AppError::Unavailable("invalid_namespace_projection".into()))?;
    validate_projection(&command)?;
    let drained = drained(db, project.as_uuid()).await?;
    Ok(Some(command.readback(drained)))
}

/// Foundation v1 has no verified stop/release protocol. Reserved ownership is
/// never inferred quiescent from an expired lease or dispatch_allowed=false.
async fn drained(db: &DatabaseConnection, project: Uuid) -> Result<bool, AppError> {
    let row=db.query_one(statement("SELECT NOT EXISTS(SELECT 1 FROM sdlc_assignments a JOIN sdlc_tasks t USING(task_id) WHERE t.project_id=$1) AND NOT EXISTS(SELECT 1 FROM sdlc_analysis_reservations r JOIN sdlc_tasks t USING(task_id) WHERE t.project_id=$1) AS drained",vec![id(project)])).await.map_err(AppError::database)?.ok_or_else(|| AppError::Unavailable("namespace_drain_readback_missing".into()))?;
    row.try_get("", "drained").map_err(AppError::database)
}

pub fn validate_projection(command: &OwnerCommand) -> Result<(), AppError> {
    let instance = std::env::var("TT_NAMESPACE__INSTANCE_ID")
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| AppError::Unavailable("namespace_owner_not_configured".into()))?;
    let registry = std::env::var("TT_NAMESPACE__REGISTRY_INSTANCE_ID")
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| AppError::Unavailable("namespace_owner_not_configured".into()))?;
    if !command.valid_for(ResourceKind::TrackerProject, instance, registry) {
        return Err(AppError::Unavailable("invalid_namespace_projection".into()));
    }
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateSpec {
    key: String,
    name: String,
    description: Option<String>,
    owner_id: Option<Uuid>,
    owner_subject: Option<String>,
}

pub async fn apply(
    db: &DatabaseConnection,
    command: &OwnerCommand,
) -> Result<OwnerReadback, AppError> {
    let tx = db.begin().await.map_err(AppError::database)?;
    tx.execute(statement(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        vec![command.resource.resource_id.to_string().into()],
    ))
    .await
    .map_err(AppError::database)?;
    if let Some(row) = tx
        .query_one(statement(
            "SELECT command FROM tracker_namespace_bindings WHERE resource_id=$1 FOR UPDATE",
            vec![id(command.resource.resource_id)],
        ))
        .await
        .map_err(AppError::database)?
    {
        let old: OwnerCommand =
            serde_json::from_value(row.try_get("", "command").map_err(AppError::database)?)
                .map_err(|_| AppError::Unavailable("invalid_namespace_projection".into()))?;
        if !command.follows(&old) {
            return Err(AppError::conflict("namespace_binding_conflict"));
        }
        if command == &old {
            tx.commit().await.map_err(AppError::database)?;
            return Ok(old.readback(drained(db, command.resource.resource_id).await?));
        }
    } else {
        if command.state != "active" {
            return Err(AppError::conflict("binding_required_before_lifecycle"));
        }
        let exists = tx
            .query_one(statement(
                "SELECT id FROM projects WHERE id=$1 FOR UPDATE",
                vec![id(command.resource.resource_id)],
            ))
            .await
            .map_err(AppError::database)?
            .is_some();
        if exists && command.create_spec.is_some() {
            return Err(AppError::conflict("existing_resource_requires_attach"));
        }
        if !exists {
            let spec: CreateSpec = serde_json::from_value(
                command
                    .create_spec
                    .clone()
                    .ok_or_else(|| AppError::not_found("project", command.resource.resource_id))?,
            )
            .map_err(|_| AppError::invalid_input("invalid_project_create_spec"))?;
            if !shared::ProjectKey::new(spec.key.clone()).is_valid()
                || spec.name.trim().is_empty()
                || spec.name.chars().count() > 200
            {
                return Err(AppError::invalid_input("invalid_project_properties"));
            }
            let board_id = Uuid::new_v4();
            let owner_id = match (spec.owner_id, spec.owner_subject) {
                (Some(owner), None) => owner,
                (None, Some(subject)) => {
                    let row = tx
                        .query_one(statement(
                            "SELECT id FROM users WHERE central_sub=$1 AND is_active",
                            vec![subject.into()],
                        ))
                        .await
                        .map_err(AppError::database)?
                        .ok_or_else(|| {
                            AppError::conflict("responsible_tracker_profile_required")
                        })?;
                    row.try_get("", "id").map_err(AppError::database)?
                }
                _ => return Err(AppError::invalid_input("one_resource_owner_required")),
            };
            let active = tx
                .query_one(statement(
                    "SELECT id FROM users WHERE id=$1 AND is_active",
                    vec![id(owner_id)],
                ))
                .await
                .map_err(AppError::database)?
                .is_some();
            if !active {
                return Err(AppError::conflict(
                    "active_responsible_tracker_profile_required",
                ));
            }
            tx.execute(statement("INSERT INTO projects(id,key,name,description,owner_id,default_board_id,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,now(),now())",vec![id(command.resource.resource_id),spec.key.into(),spec.name.into(),spec.description.into(),id(owner_id),id(board_id)])).await.map_err(AppError::database)?;
            let columns = serde_json::to_value(app::services::default_board_columns().iter().map(|c| serde_json::json!({"id":c.id.as_uuid(),"name":c.name.as_ref(),"category":format!("{:?}",c.category),"wip_limit":c.wip_limit,"position":c.position})).collect::<Vec<_>>()).map_err(|e| AppError::Internal(e.to_string()))?;
            tx.execute(statement(
                "INSERT INTO boards(id,project_id,name,columns) VALUES($1,$2,'Board',$3)",
                vec![
                    id(board_id),
                    id(command.resource.resource_id),
                    columns.into(),
                ],
            ))
            .await
            .map_err(AppError::database)?;
        }
    }
    let payload = serde_json::to_value(command).map_err(|e| AppError::Internal(e.to_string()))?;
    tx.execute(statement("INSERT INTO tracker_namespace_bindings(resource_id,registry_instance_id,namespace_id,generation,state,command) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(resource_id) DO UPDATE SET generation=EXCLUDED.generation,state=EXCLUDED.state,command=EXCLUDED.command",vec![id(command.resource.resource_id),id(command.namespace.registry_instance_id),id(command.namespace.namespace_id),command.generation.into(),command.state.clone().into(),payload.into()])).await.map_err(|error| { tracing::warn!(error=%error,"namespace owner write rejected"); AppError::conflict("namespace_binding_rejected") })?;
    tx.commit().await.map_err(AppError::database)?;
    Ok(command.readback(drained(db, command.resource.resource_id).await?))
}
