use axum::{Extension, Json, extract::State};
use shared::{AppError, UserId};
use std::sync::Arc;

use crate::dto::UserResponse;
use crate::middleware::auth::UserClaims;

#[utoipa::path(
    get,
    path = "/api/v1/users/me",
    responses((status = 200, body = UserResponse))
)]
pub async fn get_me(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
) -> Result<Json<UserResponse>, AppError> {
    let user_id = claims
        .sub
        .parse::<uuid::Uuid>()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("user_id"))?;
    let user = ctx.services.auth.me(user_id).await?;
    Ok(Json(UserResponse {
        id: user.id,
        email: user.email,
        username: user.name.clone(),
        display_name: user.name,
    }))
}
