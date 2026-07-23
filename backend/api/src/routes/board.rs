use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use std::sync::Arc;

use crate::dto::{BoardResponse, MoveIssueRequest};

pub async fn get_board(
    State(ctx): State<Arc<app::AppContext>>,
    Path(project_key): Path<String>,
) -> Result<Json<BoardResponse>, StatusCode> {
    let key = shared::ProjectKey::from_str(&project_key).map_err(|_| StatusCode::BAD_REQUEST)?;
    match ctx.services.board.get_board(&key).await {
        Ok(b) => Ok(Json(map_board(b))),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn get_backlog(
    State(ctx): State<Arc<app::AppContext>>,
    Path(project_key): Path<String>,
) -> Result<Json<crate::dto::BacklogResponse>, StatusCode> {
    let key = shared::ProjectKey::from_str(&project_key).map_err(|_| StatusCode::BAD_REQUEST)?;
    match ctx.services.board.get_backlog(&key).await {
        Ok(b) => Ok(Json(map_backlog(b))),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn move_issue(
    State(ctx): State<Arc<app::AppContext>>,
    Path(project_key): Path<String>,
    Json(req): Json<MoveIssueRequest>,
) -> Result<Json<BoardResponse>, StatusCode> {
    let key = shared::ProjectKey::from_str(&project_key).map_err(|_| StatusCode::BAD_REQUEST)?;
    let issue_id = req
        .issue_id
        .parse()
        .ok()
        .map(shared::IssueId::from_uuid)
        .ok_or(StatusCode::BAD_REQUEST)?;
    let status_id = req
        .status_id
        .parse()
        .ok()
        .map(shared::StatusId::from_uuid)
        .ok_or(StatusCode::BAD_REQUEST)?;
    match ctx.services.board.move_issue(&key, issue_id, status_id).await {
        Ok(b) => Ok(Json(map_board(b))),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

fn map_board(b: app::dto::BoardDto) -> BoardResponse {
    BoardResponse {
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
        },
    }
}

fn map_backlog(b: app::dto::BacklogDto) -> crate::dto::BacklogResponse {
    crate::dto::BacklogResponse {
        sprint: crate::dto::SprintResponse {
            id: b.sprint.id,
            name: b.sprint.name,
            goal: b.sprint.goal,
            state: b.sprint.state,
            velocity: b.sprint.velocity,
            remaining_days: b.sprint.remaining_days,
            issue_ids: b.sprint.issue_ids,
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
