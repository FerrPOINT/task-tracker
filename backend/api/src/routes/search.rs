use axum::{
    Json,
    extract::{Query, State},
};
use shared::AppError;
use std::sync::Arc;

use crate::dto::{IssueListResponse, IssueResponse, SearchQuery};

#[utoipa::path(
    get,
    path = "/api/v1/search",
    params(SearchQuery),
    responses((status = 200, body = IssueListResponse))
)]
pub async fn search_global(
    State(ctx): State<Arc<app::AppContext>>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<IssueListResponse>, AppError> {
    let items = ctx.services.search.search(&q.q).await?;
    Ok(Json(IssueListResponse {
        issues: items.into_iter().map(map_issue).collect(),
    }))
}

fn map_issue(i: app::dto::IssueDto) -> IssueResponse {
    IssueResponse {
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
    }
}
