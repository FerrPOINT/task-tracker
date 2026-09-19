use axum::{Extension, Json, extract::State, http::HeaderMap};
use std::sync::Arc;

use crate::dto::{DirectoryUserResponse, UserListResponse, UserResponse};
use shared::UserId;

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    tag = "auth",
    responses((status = 200, body = UserResponse)),
    security(("bearer" = []))
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
        is_system_admin: user.is_system_admin,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/users/me",
    responses((status = 200, body = UserResponse)),
    security(("bearer" = []))
)]
pub async fn get_users_me(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
) -> Result<Json<UserResponse>, shared::AppError> {
    get_me(State(ctx), Extension(claims)).await
}

#[utoipa::path(
    get,
    path = "/api/v1/users",
    responses((status = 200, body = UserListResponse)),
    security(("bearer" = []))
)]
pub async fn list_users(
    State(ctx): State<Arc<app::AppContext>>,
    headers: HeaderMap,
) -> Result<Json<UserListResponse>, shared::AppError> {
    if let Ok(jwks_uri) = std::env::var("TT_AUTH__CENTRAL_JWKS_URI") {
        return Ok(Json(UserListResponse {
            users: central_directory(&ctx, &headers, &jwks_uri).await?,
        }));
    }
    let users = ctx.services.auth.list_active_users().await?;
    Ok(Json(UserListResponse {
        users: users
            .into_iter()
            .map(|u| DirectoryUserResponse {
                id: u.id,
                username: u.username,
                display_name: u.display_name,
            })
            .collect(),
    }))
}

#[derive(serde::Deserialize)]
struct CentralDirectoryUser {
    id: String,
    email: String,
    display_name: String,
    status: String,
}

async fn central_directory(
    ctx: &Arc<app::AppContext>,
    headers: &HeaderMap,
    jwks_uri: &str,
) -> Result<Vec<DirectoryUserResponse>, shared::AppError> {
    let authorization = headers
        .get(axum::http::header::AUTHORIZATION)
        .ok_or(shared::AppError::Unauthorized)?;
    let mut url = reqwest::Url::parse(jwks_uri)
        .map_err(|_| shared::AppError::Unavailable("Central Auth URL is invalid".into()))?;
    url.set_path("/auth/users");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|_| shared::AppError::Unavailable("Central Auth is unavailable".into()))?;
    let mut users = Vec::new();
    for page in 0..100 {
        let response = client
            .get(url.clone())
            .query(&[("offset", page * 100)])
            .header(axum::http::header::AUTHORIZATION, authorization.clone())
            .send()
            .await
            .map_err(|_| shared::AppError::Unavailable("Central Auth is unavailable".into()))?;
        if !response.status().is_success() {
            return Err(shared::AppError::Unavailable(
                "Central Auth directory is unavailable".into(),
            ));
        }
        let batch = response
            .json::<Vec<CentralDirectoryUser>>()
            .await
            .map_err(|_| {
                shared::AppError::Unavailable("Central Auth directory is invalid".into())
            })?;
        let count = batch.len();
        for entry in batch {
            if entry.status == "disabled" {
                continue;
            }
            let local = ctx
                .repos
                .users
                .find_or_create_central_user(&entry.id, &entry.email, &entry.display_name)
                .await?;
            users.push(DirectoryUserResponse {
                id: local.id.to_string(),
                username: local.username.to_string(),
                display_name: entry.display_name,
            });
        }
        if count < 100 {
            return Ok(users);
        }
    }
    Err(shared::AppError::Unavailable(
        "Central Auth directory is too large".into(),
    ))
}
