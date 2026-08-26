use axum::{
    Json,
    extract::{Request, State},
};
use shared::{AppError, UserId};
use std::str::FromStr;
use std::sync::Arc;

use crate::dto::DashboardResponse;
use app::auth::UserClaims;

#[utoipa::path(
    get,
    path = "/api/v1/dashboard",
    responses((status = 200, body = DashboardResponse))
)]
pub async fn get_dashboard(
    State(ctx): State<Arc<app::AppContext>>,
    req: Request,
) -> Result<Json<DashboardResponse>, AppError> {
    let claims = req
        .extensions()
        .get::<UserClaims>()
        .expect("dashboard is protected by auth middleware");
    let user_id = UserId::from_str(&claims.sub).map_err(|_| AppError::Unauthorized)?;
    let dto = ctx.services.dashboard.get_dashboard(user_id).await?;
    let issues: Vec<crate::dto::IssueResponse> =
        dto.assigned_issues.into_iter().map(map_issue).collect();
    Ok(Json(DashboardResponse {
        assigned_issues: issues,
    }))
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
