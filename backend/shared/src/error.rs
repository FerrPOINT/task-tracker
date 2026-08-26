use axum::response::{IntoResponse, Response};
use http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("database error: {0}")]
    Database(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, public_msg) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden".to_string()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::Database(msg) => {
                tracing::error!(error = %msg, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
            AppError::Internal(msg) => {
                tracing::error!(error = %msg, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_string(),
                )
            }
        };
        let body = axum::Json(serde_json::json!({ "error": public_msg }));
        (status, body).into_response()
    }
}

impl AppError {
    pub fn not_found(entity: &str, id: impl std::fmt::Display) -> Self {
        Self::NotFound(format!("{} {} not found", entity, id))
    }

    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self::InvalidInput(msg.into())
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }

    pub fn database(err: impl std::fmt::Display) -> Self {
        let msg = err.to_string();
        // PostgreSQL uses this wording for SQLSTATE 23505 unique violations.
        if msg.contains("duplicate key value violates unique constraint") {
            return Self::Conflict("duplicate entry".to_string());
        }
        Self::Database(msg)
    }

    pub fn internal(err: impl std::fmt::Display) -> Self {
        Self::Internal(err.to_string())
    }
}
