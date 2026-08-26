use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::ToSchema;

use app::auth::UserClaims;
use app::context::AppContext;
use shared::{AppError, IssueId, UserId};

#[derive(Debug, Deserialize, ToSchema)]
pub struct WatchRequest {
    pub user_id: Option<String>,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct WatcherResponse {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct WatcherListResponse {
    pub watchers: Vec<WatcherResponse>,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct VoteResponse {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub voted_at: String,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct VoteListResponse {
    pub votes: Vec<VoteResponse>,
    pub count: u64,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct VoteCountResponse {
    pub count: u64,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct WatchStatusResponse {
    pub is_watching: bool,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct VoteStatusResponse {
    pub has_voted: bool,
    pub count: u64,
}

#[utoipa::path(
    post,
    path = "/api/v1/issues/{issue_id}/watch",
    tag = "issue-watchers",
    params(("issue_id" = String, Path, description = "Issue ID")),
    request_body = WatchRequest,
    responses(
        (status = 204, description = "Now watching the issue"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Issue not found"),
    ),
    security(("bearer" = []))
)]
pub async fn watch_issue(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
    body: Option<Json<WatchRequest>>,
) -> Result<StatusCode, AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let requester = claims
        .sub
        .parse()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    // Always watch as the authenticated requester. Accepting a body-supplied
    // user_id would let any member add someone else as a watcher.
    let _ = body;
    ctx.services.watcher.watch(issue_id, requester).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/v1/issues/{issue_id}/watch",
    tag = "issue-watchers",
    params(("issue_id" = String, Path, description = "Issue ID")),
    responses(
        (status = 204, description = "Stopped watching the issue"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []))
)]
pub async fn unwatch_issue(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let requester = claims
        .sub
        .parse()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    ctx.services.watcher.unwatch(issue_id, requester).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/issues/{issue_id}/watchers",
    tag = "issue-watchers",
    params(("issue_id" = String, Path, description = "Issue ID")),
    responses(
        (status = 200, description = "Watchers listed", body = WatcherListResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []))
)]
pub async fn list_watchers(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
) -> Result<Json<WatcherListResponse>, AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let requester = parse_user_id(&claims)?;
    let watchers = ctx
        .services
        .watcher
        .list_watchers(issue_id, requester)
        .await?;
    Ok(Json(WatcherListResponse {
        watchers: watchers
            .into_iter()
            .map(|w| WatcherResponse {
                user_id: w.user_id,
                username: w.username,
                display_name: w.display_name,
            })
            .collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/issues/{issue_id}/vote",
    tag = "issue-votes",
    params(("issue_id" = String, Path, description = "Issue ID")),
    responses(
        (status = 201, description = "Vote added", body = VoteResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Issue not found"),
    ),
    security(("bearer" = []))
)]
pub async fn vote_issue(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
) -> Result<(StatusCode, Json<VoteResponse>), AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let requester = claims
        .sub
        .parse()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    let vote = ctx.services.vote.vote(issue_id, requester).await?;
    Ok((
        StatusCode::CREATED,
        Json(VoteResponse {
            user_id: vote.user_id,
            username: vote.username,
            display_name: vote.display_name,
            voted_at: vote.voted_at,
        }),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/issues/{issue_id}/vote",
    tag = "issue-votes",
    params(("issue_id" = String, Path, description = "Issue ID")),
    responses(
        (status = 204, description = "Vote removed"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []))
)]
pub async fn unvote_issue(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let requester = claims
        .sub
        .parse()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    ctx.services.vote.unvote(issue_id, requester).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/issues/{issue_id}/votes",
    tag = "issue-votes",
    params(("issue_id" = String, Path, description = "Issue ID")),
    responses(
        (status = 200, description = "Votes listed", body = VoteListResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []))
)]
pub async fn list_votes(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
) -> Result<Json<VoteListResponse>, AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let requester = parse_user_id(&claims)?;
    let votes = ctx.services.vote.list_votes(issue_id, requester).await?;
    let count = votes.len() as u64;
    Ok(Json(VoteListResponse {
        votes: votes
            .into_iter()
            .map(|v| VoteResponse {
                user_id: v.user_id,
                username: v.username,
                display_name: v.display_name,
                voted_at: v.voted_at,
            })
            .collect(),
        count,
    }))
}

fn parse_user_id(claims: &UserClaims) -> Result<UserId, AppError> {
    claims
        .sub
        .parse()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id in token"))
}
