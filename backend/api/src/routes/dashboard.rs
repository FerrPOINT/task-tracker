use axum::{
    Json,
    extract::{Request, State},
    http::StatusCode,
};
use std::str::FromStr;
use std::sync::Arc;

use crate::dto::DashboardResponse;

use app::auth::UserClaims;

pub async fn get_dashboard(
    State(ctx): State<Arc<app::AppContext>>,
    req: Request,
) -> Result<Json<DashboardResponse>, StatusCode> {
    let claims = req.extensions().get::<UserClaims>().cloned();
    let user_id = match claims {
        Some(c) => shared::UserId::from_str(&c.sub).map_err(|_| StatusCode::UNAUTHORIZED)?,
        None => shared::UserId::from_str("a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11").unwrap(),
    };
    match ctx.services.dashboard.get_dashboard(user_id).await {
        Ok(dto) => {
            let issues: Vec<crate::dto::IssueResponse> = dto
                .assigned_issues
                .into_iter()
                .map(|i| crate::dto::IssueResponse {
                    id: i.id,
                    key: i.key,
                    summary: i.summary,
                    issue_type: i.issue_type,
                    status: i.status,
                    priority: i.priority,
                    assignee_id: i.assignee_id,
                    reporter_id: i.reporter_id,
                    project_name: i.project_name,
                })
                .collect();
            Ok(Json(DashboardResponse {
                assigned_issues: issues,
            }))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn _unused() {}
