use app::services::board::backlog;
use app::{auth::UserClaims, dto::SdlcBoard};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use shared::AppError;
use std::sync::Arc;

use crate::dto::{BoardResponse, SdlcBoardResponse};
use domain::sdlc_lifecycle::LifecycleStage;
use std::str::FromStr;

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_key}/board",
    params(("project_key" = String, Path, description = "Project key")),
    responses((status = 200, body = BoardResponse)),
    security(("bearer" = []))
)]
pub async fn get_board(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
) -> Result<Json<BoardResponse>, AppError> {
    let board = read_board(&ctx, &claims, &project_key).await?;
    let backlog_total = board
        .cards
        .iter()
        .filter(|(issue, task)| SdlcBoard::backlog(issue, task))
        .count();
    let (issues, tasks) = board
        .cards
        .into_iter()
        .map(|(issue, task)| (map_issue(issue), task))
        .unzip();
    Ok(Json(BoardResponse {
        project_id: board.project_id,
        project_key: board.project_key,
        issues,
        sprint: map_sprint(board.sprint),
        backlog_total,
        sdlc: SdlcBoardResponse {
            stages: LifecycleStage::BOARD_ORDER.to_vec(),
            tasks,
        },
    }))
}

/// Query parameters for backlog paging. Defaults: offset 0, limit 100
/// (server clamps limit to 200).
#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct BacklogQuery {
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_key}/backlog",
    params(
        BacklogQuery,
        ("project_key" = String, Path, description = "Project key")
    ),
    responses((status = 200, body = crate::dto::BacklogResponse)),
    security(("bearer" = []))
)]
pub async fn get_backlog(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
    axum::extract::Query(pagination): axum::extract::Query<BacklogQuery>,
) -> Result<Json<crate::dto::BacklogResponse>, AppError> {
    let board = read_board(&ctx, &claims, &project_key).await?;
    let mut sprint_issues = Vec::new();
    let mut backlog_issues = Vec::new();
    for (issue, task) in board.cards {
        if issue.sprint_id.is_some() {
            sprint_issues.push(map_issue(issue));
        } else if SdlcBoard::backlog(&issue, &task) {
            backlog_issues.push(map_issue(issue));
        }
    }
    backlog_issues.sort_unstable_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    let backlog_total = backlog_issues.len();
    let backlog_offset = pagination.offset.unwrap_or(0) as usize;
    let backlog_limit = (pagination
        .limit
        .unwrap_or(backlog::BACKLOG_PAGE_LIMIT as u32) as usize)
        .clamp(1, backlog::BACKLOG_MAX_PAGE_SIZE);
    Ok(Json(crate::dto::BacklogResponse {
        project_id: board.project_id,
        project_key: board.project_key,
        backlog_total,
        backlog_offset,
        backlog_limit,
        sprint: map_sprint(board.sprint),
        sprint_issues,
        backlog_issues: backlog_issues
            .into_iter()
            .skip(backlog_offset)
            .take(backlog_limit)
            .collect(),
    }))
}

async fn read_board(
    ctx: &app::AppContext,
    claims: &UserClaims,
    project_key: &str,
) -> Result<SdlcBoard, AppError> {
    let key = shared::ProjectKey::from_str(project_key)
        .map_err(|e| AppError::invalid_input(e.to_string()))?;
    ctx.services
        .board
        .get_board(&key, parse_user_id(claims)?)
        .await
}

fn map_sprint(sprint: app::dto::SprintDto) -> crate::dto::SprintResponse {
    crate::dto::SprintResponse {
        id: sprint.id,
        name: sprint.name,
        goal: sprint.goal,
        state: sprint.state,
        velocity: sprint.velocity,
        remaining_days: sprint.remaining_days,
        issue_ids: sprint.issue_ids,
        start_date: sprint.start_date,
        end_date: sprint.end_date,
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
        time_spent_seconds: i.time_spent_seconds,
        created_at: i.created_at,
        updated_at: i.updated_at,
    }
}

fn parse_user_id(claims: &UserClaims) -> Result<shared::UserId, AppError> {
    claims
        .sub
        .parse()
        .map(shared::UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))
}
