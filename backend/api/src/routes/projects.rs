use axum::{Extension, Json, extract::State, http::StatusCode};
use std::sync::Arc;

use crate::dto::{CreateProjectRequest, ProjectListResponse, ProjectResponse};
use app::commands::ProjectQueryDto;

pub async fn list_projects(
    State(ctx): State<Arc<app::AppContext>>,
) -> Result<Json<ProjectListResponse>, StatusCode> {
    let query = ProjectQueryDto {
        limit: 100,
        offset: 0,
    };
    match ctx.services.project.list(query).await {
        Ok(items) => Ok(Json(ProjectListResponse {
            projects: items
                .into_iter()
                .map(|p| ProjectResponse {
                    id: p.id,
                    key: p.key,
                    name: p.name,
                    description: if p.description.is_empty() {
                        None
                    } else {
                        Some(p.description)
                    },
                    owner_id: p.owner_id,
                    todo_count: p.todo_count as u32,
                    in_progress_count: p.in_progress_count as u32,
                    done_count: p.done_count as u32,
                })
                .collect(),
        })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn create_project(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), StatusCode> {
    let cmd = app::commands::CreateProjectCommand {
        key: shared::ProjectKey::new(req.key.as_str()),
        name: req.name,
        description: req.description,
        owner_id: claims.sub.parse().map_err(|_| StatusCode::BAD_REQUEST)?,
    };
    let dto = ctx.services.project.create(cmd).await.map_err(|e| {
        tracing::warn!("create project failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok((
        StatusCode::CREATED,
        Json(ProjectResponse {
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
        }),
    ))
}

pub async fn get_project(
    State(ctx): State<Arc<app::AppContext>>,
    axum::extract::Path(key): axum::extract::Path<String>,
) -> Result<Json<ProjectResponse>, StatusCode> {
    let key = shared::ProjectKey::new(key.as_str());
    match ctx.services.project.get_by_key(&key).await {
        Ok(p) => Ok(Json(ProjectResponse {
            id: p.id,
            key: p.key,
            name: p.name,
            description: if p.description.is_empty() {
                None
            } else {
                Some(p.description)
            },
            owner_id: p.owner_id,
            todo_count: p.todo_count as u32,
            in_progress_count: p.in_progress_count as u32,
            done_count: p.done_count as u32,
        })),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}
