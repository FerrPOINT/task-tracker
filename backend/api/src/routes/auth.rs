use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

use crate::dto::{AuthResponse, LoginRequest, RegisterRequest};
use app::commands::{LoginCommand, RegisterCommand};

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    request_body = RegisterRequest,
    responses((status = 200, body = AuthResponse))
)]
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
            user_id: dto.user.id,
            email: dto.user.email,
        })),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses((status = 200, body = AuthResponse))
)]
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
            user_id: dto.user.id,
            email: dto.user.email,
        })),
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}
