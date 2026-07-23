use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

use crate::dto::DashboardResponse;

pub async fn get_dashboard(
    State(_ctx): State<Arc<app::AppContext>>,
) -> Result<Json<DashboardResponse>, StatusCode> {
    Ok(Json(DashboardResponse { assigned_issues: vec![] }))
}
