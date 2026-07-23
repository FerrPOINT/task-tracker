use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use std::sync::Arc;

use crate::dto::{
    BacklogResponse, BoardColumnResponse, BoardResponse, IssueResponse, SprintResponse,
};

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
) -> Result<Json<BacklogResponse>, StatusCode> {
    let key = shared::ProjectKey::from_str(&project_key).map_err(|_| StatusCode::BAD_REQUEST)?;
    match ctx.services.board.get_backlog(&key).await {
        Ok(b) => Ok(Json(map_backlog(b))),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

fn map_board(b: app::dto::BoardDto) -> BoardResponse {
    BoardResponse {
        columns: b
            .columns
            .into_iter()
            .map(|c| BoardColumnResponse {
                id: c.id,
                name: c.name,
                wip_limit: c.wip_limit.map(|v| v as u32),
                issue_ids: c.issue_ids,
            })
            .collect(),
        issues: b.issues.into_iter().map(map_issue).collect(),
        sprint: map_sprint(b.sprint),
    }
}

fn map_backlog(b: app::dto::BacklogDto) -> BacklogResponse {
    BacklogResponse {
        sprint: map_sprint(b.sprint),
        sprint_issues: b.sprint_issues.into_iter().map(map_issue).collect(),
        backlog_issues: b.backlog_issues.into_iter().map(map_issue).collect(),
    }
}

fn map_sprint(s: app::dto::SprintDto) -> SprintResponse {
    SprintResponse {
        id: s.id,
        name: s.name,
        goal: s.goal,
        state: s.state,
        velocity: s.velocity,
        remaining_days: s.remaining_days,
        issue_ids: s.issue_ids,
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
