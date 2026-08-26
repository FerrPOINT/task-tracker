use std::sync::Arc;

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use shared::AppError;

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct VelocityQuery {
    pub project_id: String,
    #[serde(default = "default_count")]
    pub count: u32,
}

fn default_count() -> u32 {
    6
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BurndownQuery {
    pub sprint_id: String,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CumulativeFlowQuery {
    pub project_id: String,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ControlChartQuery {
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct VelocitySprintResponse {
    pub name: String,
    pub committed: usize,
    pub completed: usize,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct VelocityResponse {
    pub sprints: Vec<VelocitySprintResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BurndownPointResponse {
    pub date: String,
    pub remaining: usize,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BurndownResponse {
    pub sprint_name: String,
    pub points: Vec<BurndownPointResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CumulativeFlowPointResponse {
    pub date: String,
    pub todo: usize,
    pub in_progress: usize,
    pub done: usize,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CumulativeFlowResponse {
    pub points: Vec<CumulativeFlowPointResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ControlChartPointResponse {
    pub issue_key: String,
    pub cycle_time_days: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ControlChartResponse {
    pub points: Vec<ControlChartPointResponse>,
}

#[utoipa::path(
    get,
    path = "/api/v1/reports/velocity",
    params(VelocityQuery),
    responses((status = 200, body = VelocityResponse))
)]
pub async fn get_velocity_report(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    axum::extract::Query(query): axum::extract::Query<VelocityQuery>,
) -> Result<Json<VelocityResponse>, AppError> {
    let requester = parse_requester(&claims.0)?;
    let project_id = parse_project_id(&query.project_id)?;
    let result = ctx
        .services
        .report
        .get_velocity(project_id, query.count, requester)
        .await?;
    Ok(Json(VelocityResponse {
        sprints: result
            .into_iter()
            .map(|s| VelocitySprintResponse {
                name: s.name,
                committed: s.committed,
                completed: s.completed,
            })
            .collect(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/reports/burndown",
    params(BurndownQuery),
    responses((status = 200, body = BurndownResponse))
)]
pub async fn get_burndown_report(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    axum::extract::Query(query): axum::extract::Query<BurndownQuery>,
) -> Result<Json<BurndownResponse>, AppError> {
    let requester = parse_requester(&claims.0)?;
    let sprint_id = parse_sprint_id(&query.sprint_id)?;
    let result = ctx
        .services
        .report
        .get_burndown(sprint_id, requester)
        .await?;
    Ok(Json(BurndownResponse {
        sprint_name: result.sprint_name,
        points: result
            .points
            .into_iter()
            .map(|p| BurndownPointResponse {
                date: p.date,
                remaining: p.remaining,
            })
            .collect(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/reports/cumulative-flow",
    params(CumulativeFlowQuery),
    responses((status = 200, body = CumulativeFlowResponse))
)]
pub async fn get_cumulative_flow_report(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    axum::extract::Query(query): axum::extract::Query<CumulativeFlowQuery>,
) -> Result<Json<CumulativeFlowResponse>, AppError> {
    let requester = parse_requester(&claims.0)?;
    let project_id = parse_project_id(&query.project_id)?;
    let result = ctx
        .services
        .report
        .get_cumulative_flow(project_id, requester)
        .await?;
    Ok(Json(CumulativeFlowResponse {
        points: result
            .into_iter()
            .map(|p| CumulativeFlowPointResponse {
                date: p.date,
                todo: p.todo,
                in_progress: p.in_progress,
                done: p.done,
            })
            .collect(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/reports/control-chart",
    params(ControlChartQuery),
    responses((status = 200, body = ControlChartResponse))
)]
pub async fn get_control_chart_report(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    axum::extract::Query(query): axum::extract::Query<ControlChartQuery>,
) -> Result<Json<ControlChartResponse>, AppError> {
    let requester = parse_requester(&claims.0)?;
    let project_id = parse_project_id(&query.project_id)?;
    let result = ctx
        .services
        .report
        .get_control_chart(project_id, requester)
        .await?;
    Ok(Json(ControlChartResponse {
        points: result
            .into_iter()
            .map(|p| ControlChartPointResponse {
                issue_key: p.issue_key,
                cycle_time_days: p.cycle_time_days,
            })
            .collect(),
    }))
}

fn parse_requester(claims: &app::auth::UserClaims) -> Result<shared::UserId, AppError> {
    claims
        .sub
        .parse()
        .map(shared::UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id in token"))
}

fn parse_project_id(s: &str) -> Result<shared::ProjectId, AppError> {
    uuid::Uuid::parse_str(s)
        .map(shared::ProjectId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid project_id"))
}

fn parse_sprint_id(s: &str) -> Result<shared::SprintId, AppError> {
    uuid::Uuid::parse_str(s)
        .map(shared::SprintId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid sprint_id"))
}
