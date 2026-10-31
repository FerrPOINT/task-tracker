//! Stable model identity is independent from optional execution configuration.
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement, TransactionTrait};
use shared::AppError;

/// Only a genuinely new database may activate the empty model during bootstrap.
/// An existing empty installation still needs the coordinated writer upgrade.
pub async fn is_pristine(database_url: &str) -> Result<bool, AppError> {
    let db = Database::connect(database_url)
        .await
        .map_err(crate::db_error::map)?;
    db.query_one(Statement::from_string(DatabaseBackend::Postgres,
        "SELECT to_regclass('public.seaql_migrations') IS NULL AND to_regclass('public.issues') IS NULL AS pristine".to_owned()))
        .await.map_err(crate::db_error::map)?.ok_or_else(||AppError::Unavailable("task_model_database_unavailable".into()))?
        .try_get("","pristine").map_err(crate::db_error::map)
}

pub async fn ensure(
    database_url: &str,
    configured: Option<&str>,
    pristine: bool,
) -> Result<(), AppError> {
    let configured = configured.filter(|id| !id.trim().is_empty());
    if configured.is_some_and(|id| id.len() > 128) {
        return Err(AppError::validation(
            "stable task model instance ID is too long",
        ));
    }
    let db = Database::connect(database_url)
        .await
        .map_err(crate::db_error::map)?;
    let tx = db.begin().await.map_err(crate::db_error::map)?;
    tx.execute(Statement::from_string(
        DatabaseBackend::Postgres,
        "SELECT pg_advisory_xact_lock(hashtextextended('task-lifecycle/bootstrap',0))".to_owned(),
    ))
    .await
    .map_err(crate::db_error::map)?;
    let proposed = configured
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
    tx.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "INSERT INTO sdlc_instance(singleton,instance_id) VALUES(true,$1) ON CONFLICT(singleton) DO NOTHING",[proposed.into()]))
        .await.map_err(crate::db_error::map)?;
    let row = tx
        .query_one(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT instance_id FROM sdlc_instance WHERE singleton FOR SHARE".to_owned(),
        ))
        .await
        .map_err(crate::db_error::map)?
        .ok_or_else(|| AppError::Unavailable("task_model_instance_identity_required".into()))?;
    let persisted: String = row
        .try_get("", "instance_id")
        .map_err(crate::db_error::map)?;
    if configured.is_some_and(|id| id != persisted) {
        return Err(AppError::conflict("task_model_instance_identity_mismatch"));
    }
    if pristine {
        tx.execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT task_lifecycle_bootstrap_empty()".to_owned(),
        ))
        .await
        .map_err(crate::db_error::map)?;
    }
    tx.commit().await.map_err(crate::db_error::map)
}
