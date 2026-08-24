use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use std::sync::Arc;

use crate::dto::{
    CreateSavedFilterRequest, IssueListResponse, IssueResponse, SavedFilterListResponse,
    SavedFilterResponse,
};
use shared::AppError;

#[utoipa::path(
    get,
    path = "/api/v1/filters",
    responses((status = 200, body = SavedFilterListResponse))
)]
pub async fn list_filters(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
) -> Result<Json<SavedFilterListResponse>, AppError> {
    let user_id = uuid::Uuid::parse_str(&claims.0.sub)
        .map(shared::UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    let filters = ctx.services.saved_filter.list_filters(user_id).await?;
    Ok(Json(SavedFilterListResponse {
        filters: filters.into_iter().map(map_filter).collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/filters",
    request_body = CreateSavedFilterRequest,
    responses((status = 200, body = SavedFilterResponse))
)]
pub async fn create_filter(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    Json(req): Json<CreateSavedFilterRequest>,
) -> Result<Json<SavedFilterResponse>, AppError> {
    let user_id = uuid::Uuid::parse_str(&claims.0.sub)
        .map(shared::UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    let filter = ctx
        .services
        .saved_filter
        .create_filter(user_id, req.name, req.jql, req.is_public.unwrap_or(false))
        .await?;
    Ok(Json(map_filter(filter)))
}

#[utoipa::path(
    get,
    path = "/api/v1/filters/{id}",
    params(("id" = String, Path, description = "Filter id")),
    responses((status = 200, body = SavedFilterResponse))
)]
pub async fn get_filter(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    Path(id): Path<String>,
) -> Result<Json<SavedFilterResponse>, AppError> {
    let user_id = uuid::Uuid::parse_str(&claims.0.sub)
        .map(shared::UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    let filter = ctx.services.saved_filter.get_filter(id, user_id).await?;
    Ok(Json(map_filter(filter)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/filters/{id}",
    params(("id" = String, Path, description = "Filter id")),
    responses((status = 204), (status = 404))
)]
pub async fn delete_filter(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let user_id = uuid::Uuid::parse_str(&claims.0.sub)
        .map(shared::UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    ctx.services.saved_filter.delete_filter(id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/filters/{id}/execute",
    params(("id" = String, Path, description = "Filter id")),
    responses((status = 200, body = IssueListResponse))
)]
pub async fn execute_filter(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    Path(id): Path<String>,
) -> Result<Json<IssueListResponse>, AppError> {
    let user_id = uuid::Uuid::parse_str(&claims.0.sub)
        .map(shared::UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    let items = ctx
        .services
        .saved_filter
        .execute_filter(id, user_id)
        .await?;
    Ok(Json(IssueListResponse {
        issues: items.into_iter().map(map_issue).collect(),
    }))
}

fn map_filter(f: app::context::SavedFilterDto) -> SavedFilterResponse {
    use chrono::DateTime;
    SavedFilterResponse {
        id: f.id,
        name: f.name,
        jql: f.jql,
        owner_id: f.owner_id,
        is_public: f.is_public,
        created_at: DateTime::parse_from_rfc3339(&f.created_at)
            .unwrap_or_else(|_| chrono::Utc::now().fixed_offset()),
        updated_at: DateTime::parse_from_rfc3339(&f.updated_at)
            .unwrap_or_else(|_| chrono::Utc::now().fixed_offset()),
    }
}

fn map_issue(i: app::dto::IssueDto) -> IssueResponse {
    IssueResponse {
        id: i.id,
        key: i.key,
        summary: i.summary,
        description: i.description,
        issue_type: i.issue_type,
        project_key: i.project_key,
        status: i.status,
        status_id: i.status_id,
        priority: i.priority,
        labels: i.labels,
        assignee_id: i.assignee_id,
        assignee_name: i.assignee_name,
        reporter_id: i.reporter_id,
        reporter_name: i.reporter_name,
        project_name: i.project_name,
        sprint_id: i.sprint_id,
    }
}
