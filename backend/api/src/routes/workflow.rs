use axum::{Json, extract::State};
use shared::AppError;
use std::sync::Arc;

use crate::dto::{IssueTypeResponse, StatusResponse, TransitionResponse};

#[utoipa::path(
    get,
    path = "/api/v1/statuses",
    responses(
        (status = 200, description = "List statuses", body = Vec<StatusResponse>)
    )
)]
pub async fn list_statuses(
    State(ctx): State<Arc<app::AppContext>>,
) -> Result<Json<Vec<StatusResponse>>, AppError> {
    let statuses = ctx.services.status.list_statuses().await?;
    Ok(Json(
        statuses
            .into_iter()
            .map(|s| StatusResponse {
                id: s.id.to_string(),
                name: s.name.to_string(),
                category: format!("{:?}", s.category).to_lowercase(),
                position: s.position,
                is_default: s.is_default,
                is_closed: s.is_closed,
            })
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/transitions",
    responses(
        (status = 200, description = "List workflow transitions", body = Vec<TransitionResponse>)
    )
)]
pub async fn list_transitions(
    State(ctx): State<Arc<app::AppContext>>,
) -> Result<Json<Vec<TransitionResponse>>, AppError> {
    let transitions = ctx.services.workflow.list_transitions().await?;
    Ok(Json(
        transitions
            .into_iter()
            .map(|t| TransitionResponse {
                id: t.id.to_string(),
                name: t.name.map(|n| n.to_string()).unwrap_or_default(),
                from_status_id: t.from_status_id.to_string(),
                to_status_id: t.to_status_id.to_string(),
            })
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/issue-types",
    responses(
        (status = 200, description = "List issue types", body = Vec<IssueTypeResponse>)
    )
)]
pub async fn list_issue_types(
    State(ctx): State<Arc<app::AppContext>>,
) -> Result<Json<Vec<IssueTypeResponse>>, AppError> {
    let types = ctx.services.issue_type.list_issue_types().await?;
    Ok(Json(
        types
            .into_iter()
            .map(|t| IssueTypeResponse {
                id: t.id.to_string(),
                name: t.name.to_string(),
                description: t.description.map(|d| d.to_string()),
                icon: t.icon.map(|i| i.to_string()),
                color: t.color.map(|c| c.to_string()),
                is_subtask: t.is_subtask,
                hierarchy_level: t.hierarchy_level,
            })
            .collect(),
    ))
}
