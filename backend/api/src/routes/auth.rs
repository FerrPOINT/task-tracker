use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

use crate::dto::{AuthResponse, LoginRequest, RegisterRequest};
use app::commands::{LoginCommand, RegisterCommand};

pub async fn register(
    State(ctx): State<Arc<app::AppContext>>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    let cmd = RegisterCommand {
        email: req.email,
        username: req.username.clone(),
        name: req.username,
        password: req.password,
    };
    match ctx.services.auth.register(cmd).await {
        Ok(dto) => Ok(Json(AuthResponse {
            access_token: dto.token,
            token_type: "Bearer".to_string(),
        })),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

pub async fn login(
    State(ctx): State<Arc<app::AppContext>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    let cmd = LoginCommand {
        email: req.email,
        password: req.password,
    };
    match ctx.services.auth.login(cmd).await {
        Ok(dto) => Ok(Json(AuthResponse {
            access_token: dto.token,
            token_type: "Bearer".to_string(),
        })),
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}
