use axum::{Extension, Json, extract::State, http::StatusCode};
use shared::{AppError, UserId};
use std::sync::Arc;

use crate::dto::{AuthResponse, LoginRequest, RefreshRequest, RegisterRequest};
use app::auth::UserClaims;
use app::commands::{LoginCommand, RegisterCommand};

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    tag = "auth",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered", body = AuthResponse),
        (status = 400, description = "Bad request"),
    )
)]
pub async fn register(
    State(ctx): State<Arc<app::AppContext>>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), AppError> {
    let cmd = RegisterCommand {
        email: body.email,
        username: body.username.clone(),
        name: body.username,
        password: body.password,
    };
    let dto = ctx.services.auth.register(cmd).await?;
    Ok((StatusCode::CREATED, Json(map_auth(dto))))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials"),
    )
)]
pub async fn login(
    State(ctx): State<Arc<app::AppContext>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let cmd = LoginCommand {
        email: body.email,
        password: body.password,
    };
    let dto = ctx.services.auth.login(cmd).await?;
    Ok(Json(map_auth(dto)))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    tag = "auth",
    request_body = RefreshRequest,
    responses(
        (status = 200, description = "Tokens refreshed", body = AuthResponse),
        (status = 401, description = "Invalid refresh token"),
    )
)]
pub async fn refresh(
    State(ctx): State<Arc<app::AppContext>>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let dto = ctx.services.auth.refresh(&body.refresh_token).await?;
    Ok(Json(map_auth(dto)))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    tag = "auth",
    responses(
        (status = 204, description = "Logged out"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []))
)]
pub async fn logout(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
) -> Result<StatusCode, AppError> {
    let user_id = UserId::from_uuid(
        claims
            .sub
            .parse()
            .map_err(|_| AppError::invalid_input("invalid user id"))?,
    );
    ctx.services.auth.logout(user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn map_auth(dto: app::dto::AuthDto) -> AuthResponse {
    AuthResponse {
        access_token: dto.access_token,
        refresh_token: dto.refresh_token,
        token_type: "Bearer".to_string(),
        user_id: dto.user.id,
        email: dto.user.email,
        expires_in: dto.expires_in,
    }
}
