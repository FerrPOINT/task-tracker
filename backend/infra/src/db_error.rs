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
    if matches!(&error,DbErr::Custom(code) if code==crate::guarded_db::EXPIRED) {
        return AppError::Forbidden;
    }
    AppError::database(error)
}
