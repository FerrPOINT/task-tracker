use axum::{
    Json,
    extract::{Request, State},
    http::StatusCode,
};
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
) -> Result<Json<DashboardResponse>, StatusCode> {
    let claims = req
        .extensions()
        .get::<UserClaims>()
        .expect("dashboard is protected by auth middleware");
    let user_id = shared::UserId::from_str(&claims.sub).map_err(|_| StatusCode::UNAUTHORIZED)?;
    match ctx.services.dashboard.get_dashboard(user_id).await {
        Ok(dto) => {
            let issues: Vec<crate::dto::IssueResponse> =
                dto.assigned_issues.into_iter().map(map_issue).collect();
            Ok(Json(DashboardResponse {
                assigned_issues: issues,
            }))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
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
