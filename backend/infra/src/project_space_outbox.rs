//! Transactional metadata outbox. A durable command never carries an expiring admission.
use domain::project_spaces::ProjectSpaceProjectionStatus;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement, Value,
};
use shared::{AppError, ProjectId, project_spaces::*, space_access::*};
use uuid::Uuid;
fn sql(query: &str, values: impl IntoIterator<Item = Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, query, values)
}
fn wire<T: serde::Serialize>(value: &T) -> Result<serde_json::Value, AppError> {
    serde_json::to_value(value).map_err(|e| AppError::Internal(e.to_string()))
}
/// Called only with the project header locked, inside the canonical link transaction.
pub async fn enqueue(
    tx: &DatabaseTransaction,
    links: &ConfirmedProjectSpaces,
    identity: &CentralIdentity,
    operation_id: Uuid,
) -> Result<Uuid, AppError> {
    if !links.valid() || !identity.valid() || operation_id.is_nil() {
        return Err(AppError::invalid_input("invalid_project_projection_intent"));
    }
    let legacy=tx.query_one(sql("SELECT registry_instance_id,namespace_id FROM tracker_namespace_bindings WHERE resource_id=$1",[links.project.project_id.into()])).await.map_err(AppError::database)?;
    let legacy_project = legacy
        .map(
            |row| -> Result<shared::resource_context::NamespaceRef, AppError> {
                Ok(shared::resource_context::NamespaceRef {
                    registry_instance_id: row
                        .try_get("", "registry_instance_id")
                        .map_err(AppError::database)?,
                    namespace_id: row
                        .try_get("", "namespace_id")
                        .map_err(AppError::database)?,
                })
            },
        )
        .transpose()?;
    let mut spaces = links.spaces.clone();
    spaces.sort_by_key(|s| (s.registry_instance_id, s.space_id));
    let command = ProjectSpacesProjection {
        schema_version: 1,
        operation_id,
        project: links.project.clone(),
        revision: links.revision,
        spaces,
        legacy_project,
    };
    if !command.valid() {
        return Err(AppError::Unavailable(
            "invalid_project_space_projection".into(),
        ));
    }
    if let Some(row) = tx
        .query_one(sql(
            "SELECT command FROM tracker_project_space_outbox WHERE project_id=$1 AND revision=$2",
            [links.project.project_id.into(), links.revision.into()],
        ))
        .await
        .map_err(AppError::database)?
    {
        let old: ProjectSpacesProjection =
            serde_json::from_value(row.try_get("", "command").map_err(AppError::database)?)
                .map_err(|_| AppError::Unavailable("invalid_project_projection_outbox".into()))?;
        if !old.valid()
            || old.project != command.project
            || old.spaces != command.spaces
            || old.legacy_project != command.legacy_project
        {
            return Err(AppError::conflict("project_projection_revision_conflict"));
        }
        return Ok(old.operation_id);
    }
    tx.execute(sql("INSERT INTO tracker_project_space_outbox(operation_id,project_id,revision,command,actor_identity) VALUES($1,$2,$3,$4,$5)",[operation_id.into(),links.project.project_id.into(),links.revision.into(),wire(&command)?.into(),wire(identity)?.into()])).await.map_err(AppError::database)?;
    Ok(operation_id)
}
pub async fn current_status<C: ConnectionTrait>(
    db: &C,
    project: ProjectId,
) -> Result<ProjectSpaceProjectionStatus, AppError> {
    let row=db.query_one(sql("SELECT o.operation_id,o.command,o.readback,coalesce(o.last_error,(SELECT e.last_error FROM tracker_project_space_outbox e WHERE e.project_id=o.project_id AND e.readback IS NULL ORDER BY revision LIMIT 1)) AS last_error,NOT EXISTS(SELECT 1 FROM tracker_project_space_outbox older WHERE older.project_id=o.project_id AND older.revision<o.revision AND older.readback IS NULL) AS prior_confirmed FROM tracker_project_space_outbox o JOIN tracker_project_space_sets h ON h.project_id=o.project_id AND h.revision=o.revision WHERE o.project_id=$1",[project.as_uuid().into()])).await.map_err(AppError::database)?.ok_or_else(||AppError::Unavailable("project_projection_not_queued".into()))?;
    let command: ProjectSpacesProjection =
        serde_json::from_value(row.try_get("", "command").map_err(AppError::database)?)
            .map_err(|_| AppError::Unavailable("invalid_project_projection_outbox".into()))?;
    let readback: Option<serde_json::Value> =
        row.try_get("", "readback").map_err(AppError::database)?;
    let confirmed = match readback {
        Some(value) => serde_json::from_value::<ProjectSpacesProjectionReadback>(value)
            .ok()
            .is_some_and(|a| a.matches(&command)),
        None => false,
    };
    if !command.valid() {
        return Err(AppError::Unavailable(
            "invalid_project_projection_outbox".into(),
        ));
    }
    Ok(ProjectSpaceProjectionStatus {
        operation_id: command.operation_id,
        project: command.project,
        revision: command.revision,
        confirmed: confirmed
            && row
                .try_get::<bool>("", "prior_confirmed")
                .map_err(AppError::database)?,
        last_error: row.try_get("", "last_error").map_err(AppError::database)?,
    })
}
/// Only an exact owner ACK completes delivery. HTTP/body/timeout errors leave it pending.
pub async fn accept_ack(
    db: &DatabaseConnection,
    command: &ProjectSpacesProjection,
    ack: &ProjectSpacesProjectionReadback,
) -> Result<(), AppError> {
    if !ack.matches(command) {
        return Err(AppError::conflict("project_projection_ack_mismatch"));
    }
    let result=db.execute(sql("UPDATE tracker_project_space_outbox SET readback=$2,confirmed_at=coalesce(confirmed_at,now()),last_error=NULL WHERE operation_id=$1 AND command=$3",[command.operation_id.into(),wire(ack)?.into(),wire(command)?.into()])).await.map_err(AppError::database)?;
    if result.rows_affected() != 1 {
        return Err(AppError::conflict("project_projection_command_mismatch"));
    }
    Ok(())
}

static HTTP: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
fn client() -> Result<&'static reqwest::Client, AppError> {
    if let Some(client) = HTTP.get() {
        return Ok(client);
    }
    let built = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|_| AppError::Unavailable("project_projection_client_unavailable".into()))?;
    let _ = HTTP.set(built);
    HTTP.get()
        .ok_or_else(|| AppError::Unavailable("project_projection_client_unavailable".into()))
}
fn owner_destination() -> Result<(reqwest::Url, String), AppError> {
    let origin = std::env::var("TT_SPACES__WIKI_ORIGIN")
        .map_err(|_| AppError::Unavailable("wiki_project_projection_not_configured".into()))?;
    let url = reqwest::Url::parse(&origin).map_err(|_| {
        AppError::Unavailable("wiki_project_projection_configuration_invalid".into())
    })?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::Unavailable(
            "wiki_project_projection_configuration_invalid".into(),
        ));
    }
    let file = std::env::var("TT_SPACES__WIKI_TOKEN_FILE")
        .map_err(|_| AppError::Unavailable("wiki_project_projection_not_configured".into()))?;
    let bytes = std::fs::read(file).map_err(|_| {
        AppError::Unavailable("wiki_project_projection_credential_unavailable".into())
    })?;
    if bytes.len() > 8192 {
        return Err(AppError::Unavailable(
            "wiki_project_projection_credential_invalid".into(),
        ));
    }
    let token = std::str::from_utf8(&bytes)
        .map_err(|_| AppError::Unavailable("wiki_project_projection_credential_invalid".into()))?
        .trim();
    if token.is_empty()
        || token
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
    {
        return Err(AppError::Unavailable(
            "wiki_project_projection_credential_invalid".into(),
        ));
    }
    Ok((url, token.to_owned()))
}
async fn deliver(
    command: &ProjectSpacesProjection,
) -> Result<ProjectSpacesProjectionReadback, AppError> {
    let (mut url, token) = owner_destination()?;
    url.set_path(&format!(
        "/api/v2/projects/{}/{}/spaces/projection",
        command.project.tracker_instance_id, command.project.project_id
    ));
    let mut response = client()?
        .put(url)
        .bearer_auth(token)
        .json(command)
        .send()
        .await
        .map_err(|_| AppError::Unavailable("wiki_project_projection_transport_failed".into()))?;
    if !response.status().is_success() {
        return Err(AppError::Unavailable(
            "wiki_project_projection_owner_rejected".into(),
        ));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AppError::Unavailable("wiki_project_projection_read_failed".into()))?
    {
        if body.len().saturating_add(chunk.len()) > 1024 * 1024 {
            return Err(AppError::Unavailable(
                "wiki_project_projection_response_too_large".into(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    let ack: ProjectSpacesProjectionReadback = serde_json::from_slice(&body)
        .map_err(|_| AppError::Unavailable("wiki_project_projection_response_invalid".into()))?;
    if !ack.matches(command) {
        return Err(AppError::conflict("wiki_project_projection_ack_mismatch"));
    }
    Ok(ack)
}
/// Reconcile in revision order. Concurrent retries can send only the same first
/// pending intent; Wiki's durable receipt makes this safe without a fake completion lease.
pub async fn reconcile(
    db: &DatabaseConnection,
    project: ProjectId,
    permit: &domain::project_spaces::SpaceWritePermit,
) -> Result<ProjectSpaceProjectionStatus, AppError> {
    let owner = db
        .query_one(sql(
            "SELECT owner_id FROM projects WHERE id=$1",
            [project.as_uuid().into()],
        ))
        .await
        .map_err(AppError::database)?
        .ok_or_else(|| AppError::not_found("project", project))?;
    if !permit.is_current()
        || owner
            .try_get::<Uuid>("", "owner_id")
            .map_err(AppError::database)?
            != permit.actor().as_uuid()
    {
        return Err(AppError::Forbidden);
    }
    // Bounded request work. Pending status is explicit if a larger backlog remains.
    for _ in 0..8 {
        if !permit.is_current() {
            return Err(AppError::Forbidden);
        }
        let row=db.query_one(sql("SELECT command FROM tracker_project_space_outbox WHERE project_id=$1 AND readback IS NULL ORDER BY revision LIMIT 1",[project.as_uuid().into()])).await.map_err(AppError::database)?;
        let Some(row) = row else { break };
        let command: ProjectSpacesProjection =
            serde_json::from_value(row.try_get("", "command").map_err(AppError::database)?)
                .map_err(|_| AppError::Unavailable("invalid_project_projection_outbox".into()))?;
        if !command.valid() || command.project.project_id != project.as_uuid() {
            return Err(AppError::Unavailable(
                "invalid_project_projection_outbox".into(),
            ));
        }
        let ack = match deliver(&command).await {
            Ok(ack) => ack,
            Err(_) => {
                // Fixed safe code only: transport errors can contain endpoint or credential context.
                db.execute(sql("UPDATE tracker_project_space_outbox SET last_error='wiki_projection_not_confirmed' WHERE operation_id=$1 AND readback IS NULL",[command.operation_id.into()])).await.map_err(AppError::database)?;
                break;
            }
        };
        if !permit.is_current() {
            return Err(AppError::Forbidden);
        }
        accept_ack(db, &command, &ack).await?;
    }
    if !permit.is_current() {
        return Err(AppError::Forbidden);
    }
    current_status(db, project).await
}
