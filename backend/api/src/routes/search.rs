use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use std::sync::Arc;

use crate::dto::{IssueListResponse, IssueResponse, SearchQuery};

pub async fn search(
    State(ctx): State<Arc<app::AppContext>>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<IssueListResponse>, StatusCode> {
    match ctx.services.search.search(&q.q).await {
        Ok(items) => Ok(Json(IssueListResponse {
            issues: items.into_iter().map(map_issue).collect(),
        })),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn map_issue(i: app::dto::IssueDto) -> IssueResponse {
    IssueResponse {
        id: i.id,
        key: i.key,
        summary: i.summary,
        issue_type: i.issue_type,
        status: i.status,
        priority: i.priority,
        assignee_id: i.assignee_id,
        reporter_id: i.reporter_id,
        project_name: i.project_name,
    }
}
