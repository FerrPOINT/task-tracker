use axum::{Extension, Json, extract::State};
use std::sync::Arc;

use crate::dto::{UserListResponse, UserResponse};
use shared::UserId;

#[utoipa::path(
    get,
    path = "/api/v1/users/me",
    responses((status = 200, body = UserResponse))
)]
pub async fn get_me(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
) -> Result<Json<UserResponse>, shared::AppError> {
    let user_id = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| shared::AppError::invalid_input("invalid user id"))?;
    let user = ctx.services.auth.me(user_id).await?;
    Ok(Json(UserResponse {
        id: user.id,
        email: user.email,
        username: user.username,
        display_name: user.display_name,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/users",
    responses((status = 200, body = UserListResponse))
)]
pub async fn list_users(
    State(ctx): State<Arc<app::AppContext>>,
) -> Result<Json<UserListResponse>, shared::AppError> {
    let users = ctx.services.auth.list_users().await?;
    Ok(Json(UserListResponse {
        users: users
            .into_iter()
            .map(|u| UserResponse {
                id: u.id,
                email: u.email,
                username: u.username,
                display_name: u.display_name,
            })
            .collect(),
    }))
}
