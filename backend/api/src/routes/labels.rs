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
use shared::{AppError, IssueId, LabelId, ProjectKey, UserId};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateLabelRequest {
    pub name: String,
    #[serde(default = "default_color")]
    pub color: String,
}

fn default_color() -> String {
    "#6b7280".to_string()
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateLabelRequest {
    pub name: String,
    pub color: String,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct LabelResponse {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct LabelListResponse {
    pub labels: Vec<LabelResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AttachLabelRequest {
    pub label_id: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_key}/labels",
    tag = "labels",
    params(("project_key" = String, Path, description = "Project key")),
    responses(
        (status = 200, description = "Labels listed", body = LabelListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Project not found"),
    ),
    security(("bearer" = []))
)]
pub async fn list_labels(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
) -> Result<Json<LabelListResponse>, AppError> {
    let key = ProjectKey::new(project_key.as_str());
    let requester = parse_user_id(&claims)?;
    let items = ctx.services.label.list_by_project(&key, requester).await?;
    Ok(Json(LabelListResponse {
        labels: items
            .into_iter()
            .map(|l| LabelResponse {
                id: l.id,
                project_id: l.project_id,
                name: l.name,
                color: l.color,
            })
            .collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_key}/labels",
    tag = "labels",
    params(("project_key" = String, Path, description = "Project key")),
    request_body = CreateLabelRequest,
    responses(
        (status = 201, description = "Label created", body = LabelResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Project not found"),
    ),
    security(("bearer" = []))
)]
pub async fn create_label(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
    Json(body): Json<CreateLabelRequest>,
) -> Result<(StatusCode, Json<LabelResponse>), AppError> {
    let key = ProjectKey::new(project_key.as_str());
    let requester = parse_user(&claims)?;
    let l = ctx
        .services
        .label
        .create(&key, &body.name, &body.color, requester)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(LabelResponse {
            id: l.id,
            project_id: l.project_id,
            name: l.name,
            color: l.color,
        }),
    ))
}

#[utoipa::path(
    put,
    path = "/api/v1/labels/{id}",
    tag = "labels",
    params(("id" = String, Path, description = "Label ID")),
    request_body = UpdateLabelRequest,
    responses(
        (status = 200, description = "Label updated", body = LabelResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Label not found"),
    ),
    security(("bearer" = []))
)]
pub async fn update_label(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
    Json(body): Json<UpdateLabelRequest>,
) -> Result<Json<LabelResponse>, AppError> {
    let label_id = id
        .parse::<LabelId>()
        .map_err(|_| AppError::invalid_input("invalid label id"))?;
    let requester = parse_user(&claims)?;
    let l = ctx
        .services
        .label
        .update(label_id, &body.name, &body.color, requester)
        .await?;
    Ok(Json(LabelResponse {
        id: l.id,
        project_id: l.project_id,
        name: l.name,
        color: l.color,
    }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/labels/{id}",
    tag = "labels",
    params(("id" = String, Path, description = "Label ID")),
    responses(
        (status = 204, description = "Label deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Label not found"),
    ),
    security(("bearer" = []))
)]
pub async fn delete_label(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let label_id = id
        .parse::<LabelId>()
        .map_err(|_| AppError::invalid_input("invalid label id"))?;
    let requester = parse_user(&claims)?;
    ctx.services.label.delete(label_id, requester).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/issues/{issue_id}/labels",
    tag = "labels",
    params(("issue_id" = String, Path, description = "Issue ID")),
    responses(
        (status = 200, description = "Issue labels listed", body = LabelListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Issue not found"),
    ),
    security(("bearer" = []))
)]
pub async fn list_issue_labels(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
) -> Result<Json<LabelListResponse>, AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let requester = parse_user_id(&claims)?;
    let items = ctx
        .services
        .label
        .list_for_issue(issue_id, requester)
        .await?;
    Ok(Json(LabelListResponse {
        labels: items
            .into_iter()
            .map(|l| LabelResponse {
                id: l.id,
                project_id: l.project_id,
                name: l.name,
                color: l.color,
            })
            .collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/issues/{issue_id}/labels",
    tag = "labels",
    params(("issue_id" = String, Path, description = "Issue ID")),
    request_body = AttachLabelRequest,
    responses(
        (status = 204, description = "Label attached"),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Issue or label not found"),
    ),
    security(("bearer" = []))
)]
pub async fn attach_label(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
    Json(body): Json<AttachLabelRequest>,
) -> Result<StatusCode, AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let label_id = body
        .label_id
        .parse::<LabelId>()
        .map_err(|_| AppError::invalid_input("invalid label id"))?;
    let requester = parse_user(&claims)?;
    ctx.services
        .label
        .attach(issue_id, label_id, requester)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/v1/issues/{issue_id}/labels/{label_id}",
    tag = "labels",
    params(
        ("issue_id" = String, Path, description = "Issue ID"),
        ("label_id" = String, Path, description = "Label ID"),
    ),
    responses(
        (status = 204, description = "Label detached"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Issue or label not found"),
    ),
    security(("bearer" = []))
)]
pub async fn detach_label(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path((issue_id, label_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let label_id = label_id
        .parse::<LabelId>()
        .map_err(|_| AppError::invalid_input("invalid label id"))?;
    let requester = parse_user(&claims)?;
    ctx.services
        .label
        .detach(issue_id, label_id, requester)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn parse_user(claims: &UserClaims) -> Result<UserId, AppError> {
    claims
        .sub
        .parse()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))
}

fn parse_user_id(claims: &UserClaims) -> Result<UserId, AppError> {
    claims
        .sub
        .parse()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id in token"))
}
