use axum::{
    Json,
    extract::{Path, Query, State},
};
use std::sync::Arc;

use crate::dto::{
    CreateIssueRequest, IssueListResponse, IssueResponse, SearchQuery, UpdateIssueRequest,
};
use app::commands::{CreateIssueCommand, UpdateIssueCommand};
use shared::{AppError, ProjectKey};
use std::str::FromStr;

#[utoipa::path(
    post,
    path = "/api/v1/issues",
    request_body = CreateIssueRequest,
    responses((status = 200, body = IssueResponse))
)]
pub async fn create_issue(
    State(ctx): State<Arc<app::AppContext>>,
    Json(req): Json<CreateIssueRequest>,
) -> Result<Json<IssueResponse>, AppError> {
    let cmd = CreateIssueCommand {
        project_key: ProjectKey::from_str(&req.project_key)
            .map_err(|e| AppError::invalid_input(e.to_string()))?,
        issue_type: shared::IssueType::from_str(&req.issue_type).unwrap_or(shared::IssueType::Task),
        summary: req.summary,
        description: req.description,
        priority: shared::Priority::from_str(&req.priority).unwrap_or(shared::Priority::Medium),
        status_id: req.status_id,
        assignee_id: req
            .assignee_id
            .and_then(|s| s.parse().ok().map(shared::UserId::from_uuid)),
        reporter_id: req
            .reporter_id
            .parse()
            .ok()
            .map(shared::UserId::from_uuid)
            .ok_or(AppError::invalid_input("reporter_id"))?,
    };
    let i = ctx.services.issue.create(cmd).await?;
    Ok(Json(map_issue(i)))
}

#[utoipa::path(
    patch,
    path = "/api/v1/issues/{id}",
    params(("id" = String, Path, description = "Issue id")),
    request_body = UpdateIssueRequest,
    responses((status = 200, body = IssueResponse))
)]
pub async fn update_issue(
    State(ctx): State<Arc<app::AppContext>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateIssueRequest>,
) -> Result<Json<IssueResponse>, AppError> {
    let issue_id = id
        .parse()
        .ok()
        .map(shared::IssueId::from_uuid)
        .ok_or(AppError::invalid_input("id"))?;
    let cmd = UpdateIssueCommand {
        summary: req.summary,
        description: req.description,
        priority: req
            .priority
            .and_then(|s| shared::Priority::from_str(s.as_str()).ok()),
        status_id: req.status_id,
        assignee_id: match req.assignee_id.as_deref() {
            None | Some("") => None,
            Some(s) => {
                let uuid = s
                    .parse()
                    .map_err(|_| AppError::invalid_input("assignee_id"))?;
                Some(Some(shared::UserId::from_uuid(uuid)))
            }
        },
    };
    let i = ctx.services.issue.update(issue_id, cmd).await?;
    Ok(Json(map_issue(i)))
}

#[utoipa::path(
    get,
    path = "/api/v1/issues/{id}",
    params(("id" = String, Path, description = "Issue id")),
    responses((status = 200, body = IssueResponse))
)]
pub async fn get_issue(
    State(ctx): State<Arc<app::AppContext>>,
    Path(id): Path<String>,
) -> Result<Json<IssueResponse>, AppError> {
    let issue_id = id
        .parse()
        .ok()
        .map(shared::IssueId::from_uuid)
        .ok_or(AppError::invalid_input("id"))?;
    let i = ctx.services.issue.get_by_id(issue_id).await?;
    Ok(Json(map_issue(i)))
}

#[utoipa::path(
    get,
    path = "/api/v1/issues",
    params(SearchQuery),
    responses((status = 200, body = IssueListResponse))
)]
pub async fn search_issues(
    State(ctx): State<Arc<app::AppContext>>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<IssueListResponse>, AppError> {
    let items = ctx.services.issue.search(&q.q).await?;
    Ok(Json(IssueListResponse {
        issues: items.into_iter().map(map_issue).collect(),
    }))
}

fn map_issue(i: app::dto::IssueDto) -> IssueResponse {
    IssueResponse {
        id: i.id,
        key: i.key,
        summary: i.summary,
        description: i.description,
        issue_type: i.issue_type,
        status: i.status,
        priority: i.priority,
        labels: i.labels,
        assignee_id: i.assignee_id,
        assignee_name: i.assignee_name,
        reporter_id: i.reporter_id,
        reporter_name: i.reporter_name,
        project_name: i.project_name,
    }
}
