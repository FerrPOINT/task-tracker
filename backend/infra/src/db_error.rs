//! PostgreSQL authorization and concurrency errors retain their public meaning.
//! Unknown storage failures stay errors; no mutation is automatically retried.
use sea_orm::{DbErr, RuntimeErr, SqlxError};
use shared::AppError;
pub fn map(error: DbErr) -> AppError {
    let code = match &error {
        DbErr::Exec(RuntimeErr::SqlxError(SqlxError::Database(db)))
        | DbErr::Query(RuntimeErr::SqlxError(SqlxError::Database(db))) => db.code(),
        _ => None,
    };
    match code.as_deref() {
        Some("42501") => return AppError::Forbidden,
        Some("40001" | "40P01") => return AppError::Conflict("concurrent_project_change".into()),
        _ => {}
    }
    if code.as_deref() == Some("23514") {
        let message = error.to_string();
        for rule in [
            "task_lifecycle_transition_required",
            "task_lifecycle_exact_manual_transition_required",
            "task_lifecycle_revision_immutable",
            "task_lifecycle_identity_immutable",
            "task_lifecycle_wip_limit",
            "task_lifecycle_handoff_required",
            "task_lifecycle_execution_history_retained",
            "task_lifecycle_trashed_issue_required",
        ] {
            if message.contains(rule) {
                return AppError::conflict(rule);
            }
        }
    }
    if matches!(&error,DbErr::Custom(code) if code==crate::guarded_db::EXPIRED) {
        return AppError::Forbidden;
    }
    AppError::database(error)
}
