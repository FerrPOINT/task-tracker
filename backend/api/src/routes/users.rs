use axum::{Extension, Json, extract::State, http::HeaderMap};
use std::collections::{HashMap, HashSet};
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
    let entries = fetch_central_directory(headers, jwks_uri).await?;
    let subjects = entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    let profiles = ctx.repos.users.central_profiles(&subjects).await?;
    project_directory(&entries, &profiles)
}

fn project_directory(
    entries: &[CentralDirectoryUser],
    profiles: &HashMap<String, domain::User>,
) -> Result<Vec<DirectoryUserResponse>, shared::AppError> {
    let mut ids = HashSet::new();
    entries
        .iter()
        .map(|entry| {
            let local = profiles.get(&entry.id);
            let id = local.map_or_else(|| UserId::for_central_subject(&entry.id), |user| user.id);
            if !ids.insert(id) {
                return Err(shared::AppError::Unavailable(
                    "Central Auth directory identities conflict".into(),
                ));
            }
            Ok(DirectoryUserResponse {
                id: id.to_string(),
                username: local.map_or_else(
                    || format!("central-{}", id.as_uuid().simple()),
                    |user| user.username.to_string(),
                ),
                display_name: entry.display_name.clone(),
            })
        })
        .collect()
}

/// Called only for explicit user references in mutations, never by directory reads.
pub(crate) async fn resolve_directory_references(
    ctx: &Arc<app::AppContext>,
    headers: &HeaderMap,
    ids: &[UserId],
) -> Result<HashMap<UserId, UserId>, shared::AppError> {
    let Ok(jwks_uri) = std::env::var("TT_AUTH__CENTRAL_JWKS_URI") else {
        return Ok(ids.iter().map(|id| (*id, *id)).collect());
    };
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let entries = fetch_central_directory(headers, &jwks_uri).await?;
    let subjects = entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    let profiles = ctx.repos.users.central_profiles(&subjects).await?;
    let directory = project_directory(&entries, &profiles)?;
    let candidates = ids
        .iter()
        .map(|id| {
            let index = directory
                .iter()
                .position(|entry| entry.id == id.to_string())
                .ok_or_else(|| {
                    shared::AppError::invalid_input(
                        "user reference is not in the central directory",
                    )
                })?;
            if profiles
                .get(&entries[index].id)
                .is_some_and(|profile| !profile.is_active)
            {
                return Err(shared::AppError::invalid_input(
                    "user reference is inactive",
                ));
            }
            Ok((*id, index))
        })
        .collect::<Result<Vec<_>, shared::AppError>>()?;
    let mut resolved = HashMap::new();
    for (id, index) in candidates {
        if resolved.contains_key(&id) {
            continue;
        }
        let entry = &entries[index];
        let profile = ctx
            .repos
            .users
            .find_or_create_central_user(&entry.id, &entry.email, &entry.display_name)
            .await
            .map_err(|error| match error {
                shared::AppError::Unauthorized => {
                    shared::AppError::invalid_input("user reference is inactive")
                }
                error => error,
            })?;
        resolved.insert(id, profile.id);
    }
    Ok(resolved)
}

async fn fetch_central_directory(
    headers: &HeaderMap,
    jwks_uri: &str,
) -> Result<Vec<CentralDirectoryUser>, shared::AppError> {
    let authorization = headers
        .get(axum::http::header::AUTHORIZATION)
        .ok_or(shared::AppError::Unauthorized)?;
    let mut url = reqwest::Url::parse(jwks_uri)
        .map_err(|_| shared::AppError::Unavailable("Central Auth URL is invalid".into()))?;
    url.set_path("/auth/users");
    url.set_query(None);
    url.set_fragment(None);
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
            if entry.id.trim().is_empty()
                || entry.email.trim().is_empty()
                || entry.display_name.trim().is_empty()
                || !matches!(entry.status.as_str(), "active" | "pending" | "disabled")
            {
                return Err(shared::AppError::Unavailable(
                    "Central Auth directory is invalid".into(),
                ));
            }
            if entry.status == "disabled" {
                continue;
            }
            users.push(entry);
        }
        if count < 100 {
            return Ok(users);
        }
    }
    Err(shared::AppError::Unavailable(
        "Central Auth directory is too large".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(subject: &str) -> CentralDirectoryUser {
        CentralDirectoryUser {
            id: subject.into(),
            email: "same@example.test".into(),
            display_name: "Central name".into(),
            status: "pending".into(),
        }
    }

    fn historical_profile(id: UserId) -> domain::User {
        domain::User {
            id,
            email: "same@example.test".into(),
            username: "historical-central".into(),
            display_name: "Historical name".into(),
            password_hash: "!".into(),
            refresh_token_hash: None,
            is_system_admin: false,
            is_active: false,
            created_at: shared::now(),
            updated_at: shared::now(),
        }
    }

    #[test]
    fn pending_directory_reference_is_stable_without_a_profile() {
        let profiles = HashMap::new();
        let first = project_directory(&[entry("new-subject")], &profiles).unwrap();
        let second = project_directory(&[entry("new-subject")], &profiles).unwrap();
        assert!(profiles.is_empty());
        assert_eq!(first[0].id, second[0].id);
        assert_eq!(
            first[0].id,
            UserId::for_central_subject("new-subject").to_string()
        );
        assert_eq!(first[0].display_name, "Central name");
    }

    #[test]
    fn inactive_foreign_profile_preserves_its_historical_reference() {
        let id = UserId::new();
        let profile = historical_profile(id);
        let updated_at = profile.updated_at;
        let profiles = HashMap::from([("existing-subject".into(), profile)]);
        let directory = project_directory(
            &[entry("existing-subject"), entry("new-subject")],
            &profiles,
        )
        .unwrap();
        assert_eq!(directory[0].id, id.to_string());
        assert_eq!(directory[0].username, "historical-central");
        assert_eq!(directory[0].display_name, "Central name");
        assert_eq!(
            profiles["existing-subject"].display_name.as_ref(),
            "Historical name"
        );
        assert_eq!(profiles["existing-subject"].updated_at, updated_at);
        assert!(!profiles["existing-subject"].is_active);
        assert_eq!(directory.len(), 2);
    }

    #[test]
    fn conflicting_projected_and_historical_ids_are_not_aliased() {
        let profile = historical_profile(UserId::for_central_subject("new-subject"));
        let profiles = HashMap::from([("existing-subject".into(), profile)]);
        assert!(matches!(
            project_directory(
                &[entry("existing-subject"), entry("new-subject")],
                &profiles
            ),
            Err(shared::AppError::Unavailable(_))
        ));
    }
}
