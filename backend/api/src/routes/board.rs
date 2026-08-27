use app::auth::UserClaims;
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use shared::AppError;
use std::sync::Arc;

use crate::dto::{BoardResponse, MoveIssueRequest};
use std::str::FromStr;

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_key}/board",
    params(("project_key" = String, Path, description = "Project key")),
    responses((status = 200, body = BoardResponse))
)]
pub async fn get_board(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
) -> Result<Json<BoardResponse>, AppError> {
    let key = shared::ProjectKey::from_str(&project_key)
        .map_err(|e| AppError::invalid_input(e.to_string()))?;
    let requester = parse_user_id(&claims)?;
    let b = ctx.services.board.get_board(&key, requester).await?;
    Ok(Json(map_board(b)))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_key}/backlog",
    params(("project_key" = String, Path, description = "Project key")),
    responses((status = 200, body = crate::dto::BacklogResponse))
)]
pub async fn get_backlog(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
) -> Result<Json<crate::dto::BacklogResponse>, AppError> {
    let key = shared::ProjectKey::from_str(&project_key)
        .map_err(|e| AppError::invalid_input(e.to_string()))?;
    let requester = parse_user_id(&claims)?;
    let b = ctx.services.board.get_backlog(&key, requester).await?;
    Ok(Json(map_backlog(b)))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_key}/board/move",
    params(("project_key" = String, Path, description = "Project key")),
    request_body = MoveIssueRequest,
    responses((status = 200, body = BoardResponse))
)]
pub async fn move_issue(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
    Json(req): Json<MoveIssueRequest>,
) -> Result<Json<BoardResponse>, AppError> {
    let key = shared::ProjectKey::from_str(&project_key)
        .map_err(|e| AppError::invalid_input(e.to_string()))?;
    let issue_id = req
        .issue_id
        .parse()
        .ok()
        .map(shared::IssueId::from_uuid)
        .ok_or(AppError::invalid_input("issue_id"))?;
    let status_id = req
        .status_id
        .parse()
        .ok()
        .map(shared::StatusId::from_uuid)
        .ok_or(AppError::invalid_input("status_id"))?;
    let b = ctx
        .services
        .board
        .move_issue(&key, issue_id, status_id, parse_user_id(&claims)?)
        .await?;
    Ok(Json(map_board(b)))
}

fn map_board(b: app::dto::BoardDto) -> BoardResponse {
    BoardResponse {
        project_id: b.project_id,
        project_key: b.project_key,
        columns: b
            .columns
            .into_iter()
            .map(|c| crate::dto::BoardColumnResponse {
                id: c.id,
                name: c.name,
                wip_limit: c.wip_limit.map(|v| v as u32),
                issue_ids: c.issue_ids,
            })
            .collect(),
        issues: b.issues.into_iter().map(map_issue).collect(),
        sprint: crate::dto::SprintResponse {
            id: b.sprint.id,
            name: b.sprint.name,
            goal: b.sprint.goal,
            state: b.sprint.state,
            velocity: b.sprint.velocity,
            remaining_days: b.sprint.remaining_days,
            issue_ids: b.sprint.issue_ids,
            start_date: b.sprint.start_date,
            end_date: b.sprint.end_date,
        },
    }
}

fn map_backlog(b: app::dto::BacklogDto) -> crate::dto::BacklogResponse {
    crate::dto::BacklogResponse {
        project_id: b.project_id,
        project_key: b.project_key,
        backlog_total: b.backlog_total,
        sprint: crate::dto::SprintResponse {
            id: b.sprint.id,
            name: b.sprint.name,
            goal: b.sprint.goal,
            state: b.sprint.state,
            velocity: b.sprint.velocity,
            remaining_days: b.sprint.remaining_days,
            issue_ids: b.sprint.issue_ids,
            start_date: b.sprint.start_date,
            end_date: b.sprint.end_date,
        },
        sprint_issues: b.sprint_issues.into_iter().map(map_issue).collect(),
        backlog_issues: b.backlog_issues.into_iter().map(map_issue).collect(),
    }
}

fn map_issue(i: app::dto::IssueDto) -> crate::dto::IssueResponse {
    crate::dto::IssueResponse {
        id: i.id,
        key: i.key,
        summary: i.summary,
        description: i.description,
        issue_type: i.issue_type,
        project_key: i.project_key.clone(),
        status: i.status,
        status_id: i.status_id.clone(),
        priority: i.priority,
        labels: i.labels,
        assignee_id: i.assignee_id,
        assignee_name: i.assignee_name,
        reporter_id: i.reporter_id,
        reporter_name: i.reporter_name,
        project_name: i.project_name,
        sprint_id: i.sprint_id,
        original_estimate_seconds: i.original_estimate_seconds,
        remaining_estimate_seconds: i.remaining_estimate_seconds,
    }
}

fn parse_user_id(claims: &UserClaims) -> Result<shared::UserId, AppError> {
    claims
        .sub
        .parse()
        .map(shared::UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))
}
