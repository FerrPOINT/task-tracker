use axum::{
    Extension, Json,
    extract::{Path, State},
};
use std::sync::Arc;

use crate::dto::{
    CommentListResponse, CommentResponse, CreateCommentRequest, UpdateCommentRequest,
};
use app::auth::UserClaims;
use app::context::AppContext;
use shared::{AppError, CommentId, IssueId, UserId};

#[utoipa::path(
    get,
    path = "/api/v1/issues/{issue_id}/comments",
    tag = "comments",
    params(("issue_id" = String, Path, description = "Issue ID")),
    responses(
        (status = 200, description = "Comments listed", body = CommentListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Issue not found"),
    ),
    security(("bearer" = []))
)]
pub async fn list_comments(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
    axum::extract::RawQuery(raw): axum::extract::RawQuery,
) -> Result<Json<CommentListResponse>, AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let (limit, offset) = parse_page_params(raw.as_deref());
    let items = ctx
        .services
        .comment
        .list(
            issue_id,
            UserId::from_uuid(
                claims
                    .sub
                    .parse()
                    .map_err(|_| AppError::invalid_input("invalid user id"))?,
            ),
            limit,
            offset,
        )
        .await?;
    Ok(Json(CommentListResponse {
        comments: items
            .into_iter()
            .map(|c| CommentResponse {
                id: c.id,
                issue_id: c.issue_id,
                author_id: c.author_id,
                author_name: c.author_name,
                body: c.body,
                created_at: c.created_at,
                updated_at: c.updated_at,
            })
            .collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/issues/{issue_id}/comments",
    tag = "comments",
    params(("issue_id" = String, Path, description = "Issue ID")),
    request_body = CreateCommentRequest,
    responses(
        (status = 201, description = "Comment created", body = CommentResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Issue not found"),
    ),
    security(("bearer" = []))
)]
pub async fn create_comment(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
    Json(body): Json<CreateCommentRequest>,
) -> Result<(axum::http::StatusCode, Json<CommentResponse>), AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let cmd = app::commands::CreateCommentCommand {
        issue_id,
        author_id: UserId::from_uuid(
            claims
                .sub
                .parse()
                .map_err(|_| AppError::invalid_input("invalid user id"))?,
        ),
        body: body.body,
        actor_id: UserId::from_uuid(
            claims
                .sub
                .parse()
                .map_err(|_| AppError::invalid_input("invalid user id"))?,
        ),
    };
    let requester = UserId::from_uuid(
        claims
            .sub
            .parse()
            .map_err(|_| AppError::invalid_input("invalid user id"))?,
    );
    let c = ctx.services.comment.create(cmd, requester).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(CommentResponse {
            id: c.id,
            issue_id: c.issue_id,
            author_id: c.author_id,
            author_name: c.author_name,
            body: c.body,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }),
    ))
}

#[utoipa::path(
    patch,
    path = "/api/v1/comments/{id}",
    tag = "comments",
    params(("id" = String, Path, description = "Comment ID")),
    request_body = UpdateCommentRequest,
    responses(
        (status = 200, description = "Comment updated", body = CommentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Comment not found"),
    ),
    security(("bearer" = []))
)]
pub async fn update_comment(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
    Json(body): Json<UpdateCommentRequest>,
) -> Result<Json<CommentResponse>, AppError> {
    let id = id
        .parse::<CommentId>()
        .map_err(|_| AppError::invalid_input("invalid comment id"))?;
    let cmd = app::commands::UpdateCommentCommand {
        body: Some(body.body),
    };
    let c = ctx
        .services
        .comment
        .update(
            id,
            cmd,
            UserId::from_uuid(
                claims
                    .sub
                    .parse()
                    .map_err(|_| AppError::invalid_input("invalid user id"))?,
            ),
        )
        .await?;
    Ok(Json(CommentResponse {
        id: c.id,
        issue_id: c.issue_id,
        author_id: c.author_id,
        author_name: c.author_name,
        body: c.body,
        created_at: c.created_at,
        updated_at: c.updated_at,
    }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/comments/{id}",
    tag = "comments",
    params(("id" = String, Path, description = "Comment ID")),
    responses(
        (status = 204, description = "Comment deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Comment not found"),
    ),
    security(("bearer" = []))
)]
pub async fn delete_comment(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
) -> Result<axum::http::StatusCode, AppError> {
    let id = id
        .parse::<CommentId>()
        .map_err(|_| AppError::invalid_input("invalid comment id"))?;
    ctx.services
        .comment
        .delete(
            id,
            UserId::from_uuid(
                claims
                    .sub
                    .parse()
                    .map_err(|_| AppError::invalid_input("invalid user id"))?,
            ),
        )
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// Parse `limit`/`offset` from a raw query string with strict validation.
pub(crate) fn parse_page_params(raw: Option<&str>) -> (Option<u64>, u64) {
    let mut limit = None;
    let mut offset = 0u64;
    if let Some(q) = raw {
        for pair in q.split('&') {
            let mut it = pair.splitn(2, '=');
            match (it.next(), it.next()) {
                (Some("limit"), Some(v)) => {
                    if let Ok(n) = v.parse::<u64>() {
                        limit = Some(n);
                    }
                }
                (Some("offset"), Some(v)) => {
                    if let Ok(n) = v.parse::<u64>() {
                        offset = n;
                    }
                }
                _ => {}
            }
        }
    }
    (limit, offset)
}
