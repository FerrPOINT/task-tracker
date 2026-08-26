use axum::{
    Extension, Json,
    extract::{Path, State},
};
use std::sync::Arc;

use crate::dto::{IssueResponse, TransitionIssueRequest};
use app::auth::UserClaims;
use app::context::AppContext;
use shared::{AppError, IssueId, StatusId};
use std::str::FromStr;

#[utoipa::path(
    post,
    path = "/api/v1/issues/{id}/transition",
    tag = "issues",
    params(("id" = String, Path, description = "Issue ID")),
    request_body = TransitionIssueRequest,
    responses(
        (status = 200, description = "Issue transitioned", body = IssueResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Issue not found"),
    ),
    security(("bearer" = []))
)]
pub async fn transition_issue(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
    Json(body): Json<TransitionIssueRequest>,
) -> Result<Json<crate::dto::IssueResponse>, AppError> {
    let issue_id = id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let target_status_id = body
        .target_status_id
        .parse::<StatusId>()
        .map_err(|_| AppError::invalid_input("invalid status id"))?;
    let actor_id = shared::UserId::from_str(&claims.sub)
        .map_err(|_| shared::AppError::invalid_input("invalid token"))?;
    let cmd = app::commands::TransitionIssueCommand {
        issue_id,
        target_status_id,
        actor_id,
    };
    let dto = ctx.services.issue.transition(cmd).await?;
    Ok(Json(crate::dto::IssueResponse {
        id: dto.id,
        key: dto.key,
        summary: dto.summary,
        description: dto.description,
        issue_type: dto.issue_type,
        project_key: dto.project_key,
        status: dto.status,
        status_id: dto.status_id,
        priority: dto.priority,
        labels: dto.labels,
        assignee_id: dto.assignee_id,
        assignee_name: dto.assignee_name,
        reporter_id: dto.reporter_id,
        reporter_name: dto.reporter_name,
        project_name: dto.project_name,
        sprint_id: dto.sprint_id,
        original_estimate_seconds: dto.original_estimate_seconds,
        remaining_estimate_seconds: dto.remaining_estimate_seconds,
    }))
}
