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
use shared::{AppError, IssueId, IssueLinkId, UserId};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateLinkRequest {
    pub target_key: String,
    pub link_type: String,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct IssueLinkResponse {
    pub id: String,
    pub source_id: String,
    pub source_key: String,
    pub target_id: String,
    pub target_key: String,
    pub link_type: String,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct IssueLinkListResponse {
    pub links: Vec<IssueLinkResponse>,
}

#[utoipa::path(
    get,
    path = "/api/v1/issues/{issue_id}/links",
    tag = "issue-links",
    params(("issue_id" = String, Path, description = "Issue ID")),
    responses(
        (status = 200, description = "Links listed", body = IssueLinkListResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []))
)]
pub async fn list_links(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
) -> Result<Json<IssueLinkListResponse>, AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let requester = parse_user_id(&claims)?;
    let links = ctx
        .services
        .issue_link
        .list_by_issue(issue_id, requester)
        .await?;
    Ok(Json(IssueLinkListResponse {
        links: links
            .into_iter()
            .map(|l| IssueLinkResponse {
                id: l.id,
                source_id: l.source_id,
                source_key: l.source_key,
                target_id: l.target_id,
                target_key: l.target_key,
                link_type: l.link_type,
            })
            .collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/issues/{issue_id}/links",
    tag = "issue-links",
    params(("issue_id" = String, Path, description = "Issue ID")),
    request_body = CreateLinkRequest,
    responses(
        (status = 201, description = "Link created", body = IssueLinkResponse),
        (status = 400, description = "Bad request (unknown link type, self-link)"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Source or target issue not found"),
    ),
    security(("bearer" = []))
)]
pub async fn create_link(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
    Json(body): Json<CreateLinkRequest>,
) -> Result<(StatusCode, Json<IssueLinkResponse>), AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let requester = claims
        .sub
        .parse()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    let link = ctx
        .services
        .issue_link
        .create(issue_id, &body.target_key, &body.link_type, requester)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(IssueLinkResponse {
            id: link.id,
            source_id: link.source_id,
            source_key: link.source_key,
            target_id: link.target_id,
            target_key: link.target_key,
            link_type: link.link_type,
        }),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/issue-links/{id}",
    tag = "issue-links",
    params(("id" = String, Path, description = "Link ID")),
    responses(
        (status = 204, description = "Link deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Link not found"),
    ),
    security(("bearer" = []))
)]
pub async fn delete_link(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let link_id = id
        .parse::<IssueLinkId>()
        .map_err(|_| AppError::invalid_input("invalid link id"))?;
    let requester = claims
        .sub
        .parse()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    ctx.services.issue_link.delete(link_id, requester).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn parse_user_id(claims: &UserClaims) -> Result<UserId, AppError> {
    claims
        .sub
        .parse()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id in token"))
}
