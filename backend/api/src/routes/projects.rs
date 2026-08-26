use axum::{Extension, Json, extract::State, http::StatusCode};
use shared::{AppError, ProjectKey, UserId};
use std::sync::Arc;

use crate::dto::{
    CreateProjectRequest, ProjectListResponse, ProjectResponse, UpdateProjectRequest,
};
use app::commands::ProjectQueryDto;

#[utoipa::path(
    get,
    path = "/api/v1/projects",
    responses((status = 200, body = ProjectListResponse))
)]
pub async fn list_projects(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
) -> Result<Json<ProjectListResponse>, AppError> {
    let requester = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let query = ProjectQueryDto {
        limit: 100,
        offset: 0,
    };
    let items = ctx.services.project.list(query, requester).await?;
    Ok(Json(ProjectListResponse {
        projects: items.into_iter().map(map_project_response).collect(),
    }))
}

fn map_project_response(dto: app::dto::ProjectDto) -> ProjectResponse {
    ProjectResponse {
        id: dto.id,
        key: dto.key,
        name: dto.name,
        description: if dto.description.is_empty() {
            None
        } else {
            Some(dto.description)
        },
        owner_id: dto.owner_id,
        todo_count: dto.todo_count as u32,
        in_progress_count: dto.in_progress_count as u32,
        done_count: dto.done_count as u32,
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/projects",
    request_body = CreateProjectRequest,
    responses((status = 201, body = ProjectResponse))
)]
pub async fn create_project(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), AppError> {
    let key = ProjectKey::new(req.key.as_str());
    if !key.is_valid() {
        return Err(AppError::invalid_input("key"));
    }
    let cmd = app::commands::CreateProjectCommand {
        key,
        name: req.name,
        description: req.description,
        owner_id: claims
            .sub
            .parse::<uuid::Uuid>()
            .map(UserId::from_uuid)
            .map_err(|_| AppError::invalid_input("owner_id"))?,
    };
    let dto = ctx.services.project.create(cmd).await?;
    Ok((StatusCode::CREATED, Json(map_project_response(dto))))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_key}",
    params(("project_key" = String, Path, description = "Project key")),
    responses((status = 200, body = ProjectResponse))
)]
pub async fn get_project(
    State(ctx): State<Arc<app::AppContext>>,
    axum::extract::Path(key): axum::extract::Path<String>,
) -> Result<Json<ProjectResponse>, AppError> {
    let key = ProjectKey::new(key.as_str());
    if !key.is_valid() {
        return Err(AppError::invalid_input("project_key"));
    }
    let p = ctx.services.project.get_by_key(&key).await?;
    Ok(Json(map_project_response(p)))
}

#[utoipa::path(
    patch,
    path = "/api/v1/projects/{project_key}",
    params(("project_key" = String, Path, description = "Project key")),
    request_body = UpdateProjectRequest,
    responses((status = 200, body = ProjectResponse))
)]
pub async fn update_project(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
    axum::extract::Path(key): axum::extract::Path<String>,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectResponse>, AppError> {
    let key = ProjectKey::new(key.as_str());
    if !key.is_valid() {
        return Err(AppError::invalid_input("project_key"));
    }
    let requester_id = claims
        .sub
        .parse::<uuid::Uuid>()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("owner_id"))?;
    let cmd = app::commands::UpdateProjectCommand {
        name: req.name,
        description: req.description,
    };
    let dto = ctx.services.project.update(&key, cmd, requester_id).await?;
    Ok(Json(map_project_response(dto)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_key}",
    params(("project_key" = String, Path, description = "Project key")),
    responses((status = 204))
)]
pub async fn delete_project(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
    axum::extract::Path(key): axum::extract::Path<String>,
) -> Result<StatusCode, AppError> {
    let key = ProjectKey::new(key.as_str());
    if !key.is_valid() {
        return Err(AppError::invalid_input("project_key"));
    }
    let requester_id = claims
        .sub
        .parse::<uuid::Uuid>()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("owner_id"))?;
    ctx.services.project.delete(&key, requester_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
