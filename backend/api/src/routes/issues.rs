use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use std::sync::Arc;

use crate::dto::{
    CreateIssueRequest, IssueListResponse, IssueResponse, SearchQuery, UpdateIssueRequest,
};
use app::auth::UserClaims;
use app::commands::{CreateIssueCommand, UpdateIssueCommand};
use axum::Extension;
use shared::{AppError, IssueId, ProjectKey, UserId};
use std::str::FromStr;

#[utoipa::path(
    post,
    path = "/api/v1/issues",
    request_body = CreateIssueRequest,
    responses((status = 201, description = "Issue created", body = IssueResponse))
)]
pub async fn create_issue(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Json(req): Json<CreateIssueRequest>,
) -> Result<(StatusCode, Json<IssueResponse>), AppError> {
    let actor_id = shared::UserId::from_uuid(
        uuid::Uuid::parse_str(&claims.sub).map_err(|_| AppError::invalid_input("invalid token"))?,
    );
    let project_key = ProjectKey::from_str(&req.project_key)
        .map_err(|e| AppError::invalid_input(e.to_string()))?;
    let status_id = match req.status_id {
        Some(status_id) => status_id,
        None => ctx
            .services
            .board
            .get_board(&project_key, actor_id)
            .await?
            .columns
            .into_iter()
            .next()
            .map(|column| column.id)
            .ok_or_else(|| AppError::invalid_input("project board has no columns"))?,
    };
    let reporter_id = req
        .reporter_id
        .map(|reporter_id| {
            reporter_id
                .parse()
                .map(shared::UserId::from_uuid)
                .map_err(|_| AppError::invalid_input("reporter_id"))
        })
        .transpose()?
        .unwrap_or(actor_id);
    let cmd = CreateIssueCommand {
        project_key,
        issue_type: shared::IssueType::from_str(&req.issue_type).unwrap_or(shared::IssueType::Task),
        summary: req.summary,
        description: req.description,
        priority: shared::Priority::from_str(&req.priority).unwrap_or(shared::Priority::Medium),
        status_id,
        assignee_id: req
            .assignee_id
            .and_then(|s| s.parse().ok().map(shared::UserId::from_uuid)),
        reporter_id,
        actor_id,
    };
    let i = ctx.services.issue.create(cmd, actor_id).await?;
    Ok((StatusCode::CREATED, Json(map_issue(i))))
}

#[utoipa::path(
    patch,
    path = "/api/v1/issues/{id}",
    params(("id" = String, Path, description = "Issue id")),
    request_body = UpdateIssueRequest,
    responses((status = 200, body = IssueResponse))
)]
pub async fn update_issue(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
    Json(req): Json<UpdateIssueRequest>,
) -> Result<Json<IssueResponse>, AppError> {
    let actor_id = shared::UserId::from_uuid(
        uuid::Uuid::parse_str(&claims.sub).map_err(|_| AppError::invalid_input("invalid token"))?,
    );
    let issue_id = id
        .parse()
        .ok()
        .map(shared::IssueId::from_uuid)
        .ok_or(AppError::invalid_input("id"))?;
    let cmd = UpdateIssueCommand {
        summary: req.summary,
        description: req.description,
        priority: req
            .priority
            .and_then(|s| shared::Priority::from_str(s.as_str()).ok()),
        status_id: req.status_id,
        assignee_id: match req.assignee_id.as_deref() {
            None | Some("") => None,
            Some(s) => {
                let uuid = s
                    .parse()
                    .map_err(|_| AppError::invalid_input("assignee_id"))?;
                Some(Some(shared::UserId::from_uuid(uuid)))
            }
        },
        sprint_id: match req.sprint_id {
            None => None,
            Some(None) => Some(None),
            Some(Some(s)) => {
                let uuid = s
                    .parse()
                    .map_err(|_| AppError::invalid_input("sprint_id"))?;
                Some(Some(shared::SprintId::from_uuid(uuid)))
            }
        },
        component_id: parse_optional_uuid(req.component_id, "component_id")?
            .map(|value| value.map(shared::ProjectComponentId::from_uuid)),
        affected_version_id: parse_optional_uuid(req.affected_version_id, "affected_version_id")?
            .map(|value| value.map(shared::ProjectVersionId::from_uuid)),
        fix_version_id: parse_optional_uuid(req.fix_version_id, "fix_version_id")?
            .map(|value| value.map(shared::ProjectVersionId::from_uuid)),
        actor_id,
    };
    let i = ctx.services.issue.update(issue_id, cmd, actor_id).await?;
    Ok(Json(map_issue(i)))
}

fn parse_optional_uuid(
    value: Option<Option<String>>,
    field: &str,
) -> Result<Option<Option<uuid::Uuid>>, AppError> {
    value
        .map(|inner| {
            inner
                .map(|raw| raw.parse().map_err(|_| AppError::invalid_input(field)))
                .transpose()
        })
        .transpose()
}

#[utoipa::path(
    get,
    path = "/api/v1/issues/{id}",
    params(("id" = String, Path, description = "Issue id")),
    responses((status = 200, body = IssueResponse))
)]
pub async fn get_issue(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
) -> Result<Json<IssueResponse>, AppError> {
    let issue_id = id
        .parse()
        .ok()
        .map(shared::IssueId::from_uuid)
        .ok_or(AppError::invalid_input("id"))?;
    let requester = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let i = ctx.services.issue.get_by_id(issue_id, requester).await?;
    Ok(Json(map_issue(i)))
}

#[utoipa::path(
    get,
    path = "/api/v1/issues",
    params(SearchQuery),
    responses((status = 200, body = IssueListResponse))
)]
pub async fn search_issues(
    State(ctx): State<Arc<app::AppContext>>,
    Query(q): Query<SearchQuery>,
    claims: axum::Extension<app::auth::UserClaims>,
) -> Result<Json<IssueListResponse>, AppError> {
    let requester = claims
        .0
        .sub
        .parse::<UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let user_id = Some(requester);
    let items = ctx
        .services
        .issue
        .search(
            app::context::SearchFilters {
                q: q.q,
                project_key: q.project_key,
                priority: q.priority,
                status: None,
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
        project_key: i.project_key,
        status: i.status,
        status_id: i.status_id,
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

#[utoipa::path(
    delete,
    path = "/api/v1/issues/{id}",
    responses((status = 204), (status = 404))
)]
pub async fn delete_issue(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
) -> Result<StatusCode, shared::AppError> {
    let issue_id = id
        .parse::<IssueId>()
        .map_err(|_| shared::AppError::invalid_input("id"))?;
    let actor_id = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| shared::AppError::invalid_input("invalid user id in token"))?;
    ctx.services.issue.delete(issue_id, actor_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/v1/issues/{id}/restore",
    params(("id" = String, Path, description = "Issue id")),
    responses((status = 200, body = IssueResponse), (status = 404))
)]
pub async fn restore_issue(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
) -> Result<Json<IssueResponse>, AppError> {
    let issue_id = id
        .parse::<IssueId>()
        .map_err(|_| shared::AppError::invalid_input("id"))?;
    let actor_id = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| shared::AppError::invalid_input("invalid user id in token"))?;
    let i = ctx.services.issue.restore(issue_id, actor_id).await?;
    Ok(Json(map_issue(i)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/issues/{id}/trash",
    params(("id" = String, Path, description = "Issue id")),
    responses((status = 204), (status = 404))
)]
pub async fn purge_issue(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
) -> Result<StatusCode, shared::AppError> {
    let issue_id = id
        .parse::<IssueId>()
        .map_err(|_| shared::AppError::invalid_input("id"))?;
    let actor_id = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| shared::AppError::invalid_input("invalid user id in token"))?;
    ctx.services.issue.purge(issue_id, actor_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{key}/trash",
    params(("key" = String, Path, description = "Project key")),
    responses((status = 200, body = IssueListResponse))
)]
pub async fn list_trash(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(key): Path<String>,
) -> Result<Json<IssueListResponse>, AppError> {
    let requester = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let project_key =
        ProjectKey::from_str(&key).map_err(|e| AppError::invalid_input(e.to_string()))?;
    let items = ctx
        .services
        .issue
        .list_trash(&project_key, requester)
        .await?;
    Ok(Json(IssueListResponse {
        issues: items.into_iter().map(map_issue).collect(),
    }))
}
