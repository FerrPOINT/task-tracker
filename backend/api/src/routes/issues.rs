use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use std::sync::Arc;

use crate::dto::{
    CreateIssueRequest, IssueListResponse, IssueResponse, SearchQuery, UpdateIssueRequest,
};
use app::commands::{CreateIssueCommand, UpdateIssueCommand};

pub async fn create_issue(
    State(ctx): State<Arc<app::AppContext>>,
    Json(req): Json<CreateIssueRequest>,
) -> Result<Json<IssueResponse>, StatusCode> {
    let cmd = CreateIssueCommand {
        project_key: shared::ProjectKey::from_str(&req.project_key)
            .map_err(|_| StatusCode::BAD_REQUEST)?,
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
            .unwrap_or_default(),
    };
    match ctx.services.issue.create(cmd).await {
        Ok(i) => Ok(Json(map_issue(i))),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

pub async fn update_issue(
    State(ctx): State<Arc<app::AppContext>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateIssueRequest>,
) -> Result<Json<IssueResponse>, StatusCode> {
    let issue_id = id
        .parse()
        .ok()
        .map(shared::IssueId::from_uuid)
        .ok_or(StatusCode::BAD_REQUEST)?;
    let cmd = UpdateIssueCommand {
        summary: req.summary,
        description: req.description,
        priority: req
            .priority
            .and_then(|s| shared::Priority::from_str(s.as_str()).ok()),
        status_id: req.status_id,
        assignee_id: req.assignee_id.map(|s| {
            if s.is_empty() {
                None
            } else {
                s.parse().ok().map(shared::UserId::from_uuid)
            }
        }),
    };
    match ctx.services.issue.update(issue_id, cmd).await {
        Ok(i) => Ok(Json(map_issue(i))),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn get_issue(
    State(ctx): State<Arc<app::AppContext>>,
    Path(id): Path<String>,
) -> Result<Json<IssueResponse>, StatusCode> {
    let issue_id = id
        .parse()
        .ok()
        .map(shared::IssueId::from_uuid)
        .unwrap_or_default();
    match ctx.services.issue.get_by_id(issue_id).await {
        Ok(i) => Ok(Json(map_issue(i))),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn search(
    State(ctx): State<Arc<app::AppContext>>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<IssueListResponse>, StatusCode> {
    match ctx.services.issue.search(&q.q).await {
        Ok(items) => Ok(Json(IssueListResponse {
            issues: items.into_iter().map(map_issue).collect(),
        })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
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

use std::str::FromStr;
