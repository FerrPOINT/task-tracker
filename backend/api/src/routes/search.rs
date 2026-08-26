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
    claims: axum::Extension<app::auth::UserClaims>,
) -> Result<Json<IssueListResponse>, AppError> {
    let requester = claims
        .0
        .sub
        .parse()
        .map(shared::UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    let user_id = Some(requester);
    let items = ctx
        .services
        .search
        .search(
            app::context::SearchFilters {
                q: q.q,
                project_key: q.project_key,
                priority: q.priority,
                status: q.status,
                assignee_id: q.assignee_id,
                sort_by: q.sort_by,
                sort_order: q.sort_order,
                limit: q.limit,
                offset: q.offset,
                jql: q.jql,
                user_id: user_id.map(|u| u.to_string()),
            },
            requester,
        )
        .await?;
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
        sprint_id: i.sprint_id,
        original_estimate_seconds: i.original_estimate_seconds,
        remaining_estimate_seconds: i.remaining_estimate_seconds,
    }
}
