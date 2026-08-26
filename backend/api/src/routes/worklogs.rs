use axum::{
    Extension, Json,
    extract::{Path, State},
};
use std::sync::Arc;

use crate::dto::{
    CreateWorklogRequest, UpdateWorklogRequest, WorklogListResponse, WorklogResponse,
};
use app::auth::UserClaims;
use app::context::AppContext;
use shared::{AppError, IssueId, UserId, WorklogId};

#[utoipa::path(
    get,
    path = "/api/v1/issues/{issue_id}/worklogs",
    tag = "worklogs",
    params(("issue_id" = String, Path, description = "Issue ID")),
    responses(
        (status = 200, description = "Worklogs listed", body = WorklogListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Issue not found"),
    ),
    security(("bearer" = []))
)]
pub async fn list_worklogs(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
    axum::extract::RawQuery(raw): axum::extract::RawQuery,
) -> Result<Json<WorklogListResponse>, AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let (limit, offset) = crate::routes::comments::parse_page_params(raw.as_deref());
    let items = ctx
        .services
        .worklog
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
    Ok(Json(WorklogListResponse {
        worklogs: items
            .into_iter()
            .map(|w| WorklogResponse {
                id: w.id,
                issue_id: w.issue_id,
                author_id: w.author_id,
                author_name: w.author_name,
                started_at: w.started_at,
                duration_seconds: w.duration_seconds,
                description: w.description,
                created_at: w.created_at,
                updated_at: w.updated_at,
            })
            .collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/issues/{issue_id}/worklogs",
    tag = "worklogs",
    params(("issue_id" = String, Path, description = "Issue ID")),
    request_body = CreateWorklogRequest,
    responses(
        (status = 201, description = "Worklog created", body = WorklogResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Issue not found"),
    ),
    security(("bearer" = []))
)]
pub async fn create_worklog(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
    Json(body): Json<CreateWorklogRequest>,
) -> Result<(axum::http::StatusCode, Json<WorklogResponse>), AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let cmd = app::commands::CreateWorklogCommand {
        issue_id,
        author_id: UserId::from_uuid(
            claims
                .sub
                .parse()
                .map_err(|_| AppError::invalid_input("invalid user id"))?,
        ),
        started_at: body.started_at,
        duration_seconds: body.duration_seconds,
        description: body.description,
    };
    let w = ctx
        .services
        .worklog
        .create(
            cmd,
            UserId::from_uuid(
                claims
                    .sub
                    .parse()
                    .map_err(|_| AppError::invalid_input("invalid user id"))?,
            ),
        )
        .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(WorklogResponse {
            id: w.id,
            issue_id: w.issue_id,
            author_id: w.author_id,
            author_name: w.author_name,
            started_at: w.started_at,
            duration_seconds: w.duration_seconds,
            description: w.description,
            created_at: w.created_at,
            updated_at: w.updated_at,
        }),
    ))
}

#[utoipa::path(
    patch,
    path = "/api/v1/worklogs/{id}",
    tag = "worklogs",
    params(("id" = String, Path, description = "Worklog ID")),
    request_body = UpdateWorklogRequest,
    responses(
        (status = 200, description = "Worklog updated", body = WorklogResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Worklog not found"),
    ),
    security(("bearer" = []))
)]
pub async fn update_worklog(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
    Json(body): Json<UpdateWorklogRequest>,
) -> Result<Json<WorklogResponse>, AppError> {
    let id = id
        .parse::<WorklogId>()
        .map_err(|_| AppError::invalid_input("invalid worklog id"))?;
    let cmd = app::commands::UpdateWorklogCommand {
        started_at: body.started_at,
        duration_seconds: body.duration_seconds,
        description: Some(body.description),
    };
    let w = ctx
        .services
        .worklog
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
    Ok(Json(WorklogResponse {
        id: w.id,
        issue_id: w.issue_id,
        author_id: w.author_id,
        author_name: w.author_name,
        started_at: w.started_at,
        duration_seconds: w.duration_seconds,
        description: w.description,
        created_at: w.created_at,
        updated_at: w.updated_at,
    }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/worklogs/{id}",
    tag = "worklogs",
    params(("id" = String, Path, description = "Worklog ID")),
    responses(
        (status = 204, description = "Worklog deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Worklog not found"),
    ),
    security(("bearer" = []))
)]
pub async fn delete_worklog(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
) -> Result<axum::http::StatusCode, AppError> {
    let id = id
        .parse::<WorklogId>()
        .map_err(|_| AppError::invalid_input("invalid worklog id"))?;
    ctx.services
        .worklog
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
