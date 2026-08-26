use axum::{
    Extension,
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use std::sync::Arc;

use crate::dto::{AttachmentListResponse, AttachmentResponse};
use app::auth::UserClaims;
use app::context::AppContext;
use shared::{AppError, AttachmentId, IssueId, UserId};

#[utoipa::path(
    get,
    path = "/api/v1/issues/{issue_id}/attachments",
    tag = "attachments",
    params(("issue_id" = String, Path, description = "Issue ID")),
    responses(
        (status = 200, description = "Attachments listed", body = AttachmentListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Issue not found"),
    ),
    security(("bearer" = []))
)]
pub async fn list_attachments(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
) -> Result<axum::Json<AttachmentListResponse>, AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let requester = parse_user_id(&claims)?;
    let items = ctx
        .services
        .attachment
        .list_by_issue(issue_id, requester)
        .await?;
    Ok(axum::Json(AttachmentListResponse {
        attachments: items
            .into_iter()
            .map(|a| AttachmentResponse {
                id: a.id,
                issue_id: a.issue_id,
                author_id: a.author_id,
                file_name: a.file_name,
                content_type: a.content_type,
                size_bytes: a.size_bytes,
                created_at: a.created_at,
            })
            .collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/issues/{issue_id}/attachments",
    tag = "attachments",
    params(("issue_id" = String, Path, description = "Issue ID")),
    request_body(description = "Multipart form with a `file` field"),
    responses(
        (status = 201, description = "Attachment uploaded", body = AttachmentResponse),
        (status = 400, description = "Bad request (missing file, empty file, too large)"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Issue not found"),
    ),
    security(("bearer" = []))
)]
pub async fn upload_attachment(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, axum::Json<AttachmentResponse>), AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let author_id = UserId::from_uuid(
        claims
            .sub
            .parse()
            .map_err(|_| AppError::invalid_input("invalid user id"))?,
    );

    let mut file_name: Option<String> = None;
    let mut content_type: Option<String> = None;
    let mut bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::invalid_input(format!("multipart error: {e}")))?
    {
        if field.name() != Some("file") {
            continue;
        }
        file_name = field.file_name().map(|s| s.to_string());
        content_type = field.content_type().map(|s| s.to_string());
        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::invalid_input(format!("read error: {e}")))?;
        bytes = Some(data.to_vec());
        break;
    }

    let bytes = bytes.ok_or_else(|| AppError::invalid_input("file field is required"))?;
    let file_name = file_name.unwrap_or_else(|| "upload.bin".to_string());
    let content_type = content_type.unwrap_or_else(|| "application/octet-stream".to_string());

    let a = ctx
        .services
        .attachment
        .upload(issue_id, author_id, &file_name, &content_type, bytes)
        .await?;
    Ok((
        StatusCode::CREATED,
        axum::Json(AttachmentResponse {
            id: a.id,
            issue_id: a.issue_id,
            author_id: a.author_id,
            file_name: a.file_name,
            content_type: a.content_type,
            size_bytes: a.size_bytes,
            created_at: a.created_at,
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/attachments/{id}/download",
    tag = "attachments",
    params(("id" = String, Path, description = "Attachment ID")),
    responses(
        (status = 200, description = "File bytes", content_type = "application/octet-stream"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Attachment not found"),
    ),
    security(("bearer" = []))
)]
pub async fn download_attachment(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let id = id
        .parse::<AttachmentId>()
        .map_err(|_| AppError::invalid_input("invalid attachment id"))?;
    let requester = parse_user_id(&claims)?;
    let (meta, bytes) = ctx.services.attachment.download(id, requester).await?;

    let mut resp = Response::new(Body::from(bytes));
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&meta.content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    resp.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"{}\"",
            meta.file_name.replace('"', "_")
        ))
        .unwrap_or(HeaderValue::from_static("attachment")),
    );
    Ok(resp)
}

#[utoipa::path(
    delete,
    path = "/api/v1/attachments/{id}",
    tag = "attachments",
    params(("id" = String, Path, description = "Attachment ID")),
    responses(
        (status = 204, description = "Attachment deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Attachment not found"),
    ),
    security(("bearer" = []))
)]
pub async fn delete_attachment(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let id = id
        .parse::<AttachmentId>()
        .map_err(|_| AppError::invalid_input("invalid attachment id"))?;
    let requester = UserId::from_uuid(
        claims
            .sub
            .parse()
            .map_err(|_| AppError::invalid_input("invalid user id"))?,
    );
    ctx.services.attachment.delete(id, requester).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn parse_user_id(claims: &UserClaims) -> Result<UserId, AppError> {
    claims
        .sub
        .parse()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id in token"))
}
