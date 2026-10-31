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

/// Remember a genuinely first bootstrap before migrations can make it look legacy.
/// An interrupted first run resumes this exact intention; existing empty installs
/// never acquire it merely because they have no tasks.
pub async fn prepare_bootstrap(database_url: &str) -> Result<bool, AppError> {
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
    let row = tx.query_one(Statement::from_string(DatabaseBackend::Postgres,
        "SELECT to_regclass('public.task_model_bootstrap_provenance') IS NOT NULL AS provenance,
            to_regclass('public.seaql_migrations') IS NULL AND to_regclass('public.issues') IS NULL
                AND to_regclass('public.users') IS NULL AND to_regclass('public.projects') IS NULL
                AND to_regclass('public.statuses') IS NULL AS uninitialized".to_owned()))
        .await.map_err(crate::db_error::map)?.ok_or_else(||AppError::Unavailable("task_model_database_unavailable".into()))?;
    if row
        .try_get::<bool>("", "provenance")
        .map_err(crate::db_error::map)?
    {
        if let Some(record) = tx.query_one(Statement::from_string(DatabaseBackend::Postgres,
            "SELECT pending FROM task_model_bootstrap_provenance WHERE singleton AND origin='uninitialized_tracker_database'".to_owned()))
            .await.map_err(crate::db_error::map)? {
            let pending = record.try_get("", "pending").map_err(crate::db_error::map)?;
            tx.commit().await.map_err(crate::db_error::map)?;
            return Ok(pending);
        }
    }
    if !row
        .try_get::<bool>("", "uninitialized")
        .map_err(crate::db_error::map)?
    {
        tx.commit().await.map_err(crate::db_error::map)?;
        return Ok(false);
    }
    tx.execute_unprepared(include_str!(
        "../../migration/src/task_model_bootstrap_provenance.sql"
    ))
    .await
    .map_err(crate::db_error::map)?;
    tx.execute(Statement::from_sql_and_values(DatabaseBackend::Postgres,
        "INSERT INTO task_model_bootstrap_provenance(singleton,bootstrap_id,origin) VALUES(true,$1,'uninitialized_tracker_database')",
        [uuid::Uuid::now_v7().into()])).await.map_err(crate::db_error::map)?;
    tx.commit().await.map_err(crate::db_error::map)?;
    Ok(true)
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
        let provenance = tx.query_one(Statement::from_string(DatabaseBackend::Postgres,
            "SELECT pending FROM task_model_bootstrap_provenance WHERE singleton AND origin='uninitialized_tracker_database' FOR UPDATE".to_owned()))
            .await.map_err(crate::db_error::map)?.ok_or_else(||AppError::conflict("task_model_bootstrap_provenance_required"))?;
        if !provenance
            .try_get::<bool>("", "pending")
            .map_err(crate::db_error::map)?
        {
            // Concurrent/retried startup already completed the same first bootstrap.
            let active = tx
                .query_one(Statement::from_string(
                    DatabaseBackend::Postgres,
                    "SELECT enabled FROM task_lifecycle_control WHERE singleton".to_owned(),
                ))
                .await
                .map_err(crate::db_error::map)?
                .ok_or_else(|| AppError::Unavailable("task_model_control_unavailable".into()))?;
            if !active
                .try_get::<bool>("", "enabled")
                .map_err(crate::db_error::map)?
            {
                return Err(AppError::conflict(
                    "task_model_bootstrap_provenance_inconsistent",
                ));
            }
        }
        tx.execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT task_lifecycle_bootstrap_empty()".to_owned(),
        ))
        .await
        .map_err(crate::db_error::map)?;
        tx.execute(Statement::from_string(DatabaseBackend::Postgres,
            "UPDATE task_model_bootstrap_provenance SET pending=false,completed_at=clock_timestamp() WHERE singleton AND pending".to_owned()))
            .await.map_err(crate::db_error::map)?;
    }
    tx.commit().await.map_err(crate::db_error::map)
}
