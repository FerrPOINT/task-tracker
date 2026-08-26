use axum::{
    Extension, Json,
    extract::{Path, State},
};
use std::sync::Arc;

use crate::dto::{AddProjectMemberRequest, ProjectMemberListResponse, ProjectMemberResponse};
use app::auth::UserClaims;
use app::context::AppContext;
use shared::{AppError, ProjectId, ProjectKey, UserId};
use std::str::FromStr;

async fn resolve_project_id(ctx: &AppContext, project_key: String) -> Result<ProjectId, AppError> {
    let project_key = ProjectKey::from_str(&project_key)
        .map_err(|error| AppError::invalid_input(error.to_string()))?;
    let project = ctx.services.project.get_by_key(&project_key).await?;
    project
        .id
        .parse::<ProjectId>()
        .map_err(|_| AppError::invalid_input("project_id"))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_key}/members",
    tag = "projects",
    params(("project_key" = String, Path, description = "Project key")),
    responses(
        (status = 200, description = "Members listed", body = ProjectMemberListResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []))
)]
pub async fn list_members(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
) -> Result<Json<ProjectMemberListResponse>, AppError> {
    let project_id = resolve_project_id(&ctx, project_key).await?;
    let requester = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let items = ctx.services.member.list(project_id, requester).await?;
    Ok(Json(ProjectMemberListResponse {
        members: items
            .into_iter()
            .map(|m| ProjectMemberResponse {
                project_id: m.project_id,
                user_id: m.user_id,
                role: m.role,
                joined_at: m.joined_at,
            })
            .collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_key}/members",
    tag = "projects",
    params(("project_key" = String, Path, description = "Project key")),
    request_body = AddProjectMemberRequest,
    responses(
        (status = 201, description = "Member added", body = ProjectMemberResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "User not found"),
    ),
    security(("bearer" = []))
)]
pub async fn add_member(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
    Json(body): Json<AddProjectMemberRequest>,
) -> Result<(axum::http::StatusCode, Json<ProjectMemberResponse>), AppError> {
    let _actor_id = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let project_id = resolve_project_id(&ctx, project_key).await?;
    let user_id = body
        .user_id
        .parse::<UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    let requester = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let cmd = app::commands::AddProjectMemberCommand {
        project_id,
        user_id,
        role: body.role,
    };
    let m = ctx.services.member.add(cmd, requester).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(ProjectMemberResponse {
            project_id: m.project_id,
            user_id: m.user_id,
            role: m.role,
            joined_at: m.joined_at,
        }),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_key}/members/{user_id}",
    tag = "projects",
    params(
        ("project_key" = String, Path, description = "Project key"),
        ("user_id" = String, Path, description = "User ID"),
    ),
    responses(
        (status = 204, description = "Member removed"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Member not found"),
    ),
    security(("bearer" = []))
)]
pub async fn remove_member(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path((project_key, user_id)): Path<(String, String)>,
) -> Result<axum::http::StatusCode, AppError> {
    let requester = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let project_id = resolve_project_id(&ctx, project_key).await?;
    let user_id = user_id
        .parse::<UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    ctx.services
        .member
        .remove(project_id, user_id, requester)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
