use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

use crate::dto::{CreateProjectRequest, ProjectListResponse, ProjectResponse};
use app::commands::ProjectQueryDto;

pub async fn list_projects(
    State(ctx): State<Arc<app::AppContext>>,
) -> Result<Json<ProjectListResponse>, StatusCode> {
    let query = ProjectQueryDto { limit: 100, offset: 0 };
    match ctx.services.project.list(query).await {
        Ok(items) => Ok(Json(ProjectListResponse {
            projects: items
                .into_iter()
                .map(|p| ProjectResponse {
                    id: p.id,
                    key: p.key,
                    name: p.name,
                    description: if p.description.is_empty() { None } else { Some(p.description) },
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
    State(_ctx): State<Arc<app::AppContext>>,
    Json(_req): Json<CreateProjectRequest>,
) -> Result<StatusCode, StatusCode> {
    Ok(StatusCode::CREATED)
}
