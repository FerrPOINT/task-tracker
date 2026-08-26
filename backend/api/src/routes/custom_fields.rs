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
use shared::{AppError, CustomFieldId, IssueId, ProjectKey, UserId};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCustomFieldRequest {
    pub name: String,
    pub field_type: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub is_required: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCustomFieldRequest {
    pub name: String,
    pub field_type: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub is_required: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetCustomFieldValueRequest {
    pub value: serde_json::Value,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct CustomFieldResponse {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub field_type: String,
    pub options: Vec<String>,
    pub is_required: bool,
    pub created_at: String,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct CustomFieldListResponse {
    pub fields: Vec<CustomFieldResponse>,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct CustomFieldValueListResponse {
    pub values: Vec<CustomFieldValueResponse>,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct CustomFieldValueResponse {
    pub field_id: String,
    pub value: serde_json::Value,
}

fn parse_user(claims: &UserClaims) -> Result<UserId, AppError> {
    claims
        .sub
        .parse()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_key}/custom-fields",
    tag = "custom-fields",
    params(("project_key" = String, Path, description = "Project key")),
    responses(
        (status = 200, description = "Custom fields listed", body = CustomFieldListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Project not found"),
    ),
    security(("bearer" = []))
)]
pub async fn list_custom_fields(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
) -> Result<Json<CustomFieldListResponse>, AppError> {
    let key = ProjectKey::new(project_key.as_str());
    let requester = parse_user_id(&claims)?;
    let items = ctx
        .services
        .custom_field
        .list_fields(&key, requester)
        .await?;
    Ok(Json(CustomFieldListResponse {
        fields: items
            .into_iter()
            .map(|f| CustomFieldResponse {
                id: f.id,
                project_id: f.project_id,
                name: f.name,
                field_type: f.field_type,
                options: f.options,
                is_required: f.is_required,
                created_at: f.created_at,
            })
            .collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_key}/custom-fields",
    tag = "custom-fields",
    params(("project_key" = String, Path, description = "Project key")),
    request_body = CreateCustomFieldRequest,
    responses(
        (status = 201, description = "Custom field created", body = CustomFieldResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Project not found"),
    ),
    security(("bearer" = []))
)]
pub async fn create_custom_field(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
    Json(body): Json<CreateCustomFieldRequest>,
) -> Result<(StatusCode, Json<CustomFieldResponse>), AppError> {
    let key = ProjectKey::new(project_key.as_str());
    let requester = parse_user(&claims)?;
    let f = ctx
        .services
        .custom_field
        .create_field(
            &key,
            &body.name,
            &body.field_type,
            &body.options,
            body.is_required,
            requester,
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(CustomFieldResponse {
            id: f.id,
            project_id: f.project_id,
            name: f.name,
            field_type: f.field_type,
            options: f.options,
            is_required: f.is_required,
            created_at: f.created_at,
        }),
    ))
}

#[utoipa::path(
    put,
    path = "/api/v1/custom-fields/{id}",
    tag = "custom-fields",
    params(("id" = String, Path, description = "Custom field ID")),
    request_body = UpdateCustomFieldRequest,
    responses(
        (status = 200, description = "Custom field updated", body = CustomFieldResponse),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Custom field not found"),
    ),
    security(("bearer" = []))
)]
pub async fn update_custom_field(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
    Json(body): Json<UpdateCustomFieldRequest>,
) -> Result<Json<CustomFieldResponse>, AppError> {
    let field_id = id
        .parse::<CustomFieldId>()
        .map_err(|_| AppError::invalid_input("invalid custom field id"))?;
    let requester = parse_user(&claims)?;
    let f = ctx
        .services
        .custom_field
        .update_field(
            field_id,
            &body.name,
            &body.field_type,
            &body.options,
            body.is_required,
            requester,
        )
        .await?;
    Ok(Json(CustomFieldResponse {
        id: f.id,
        project_id: f.project_id,
        name: f.name,
        field_type: f.field_type,
        options: f.options,
        is_required: f.is_required,
        created_at: f.created_at,
    }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/custom-fields/{id}",
    tag = "custom-fields",
    params(("id" = String, Path, description = "Custom field ID")),
    responses(
        (status = 204, description = "Custom field deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Custom field not found"),
    ),
    security(("bearer" = []))
)]
pub async fn delete_custom_field(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let field_id = id
        .parse::<CustomFieldId>()
        .map_err(|_| AppError::invalid_input("invalid custom field id"))?;
    let requester = parse_user(&claims)?;
    ctx.services
        .custom_field
        .delete_field(field_id, requester)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/issues/{issue_id}/custom-fields",
    tag = "custom-fields",
    params(("issue_id" = String, Path, description = "Issue ID")),
    responses(
        (status = 200, description = "Custom field values listed", body = CustomFieldValueListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Issue not found"),
    ),
    security(("bearer" = []))
)]
pub async fn list_issue_custom_field_values(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(issue_id): Path<String>,
) -> Result<Json<CustomFieldValueListResponse>, AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let requester = parse_user_id(&claims)?;
    let values = ctx
        .services
        .custom_field
        .get_values_for_issue(issue_id, requester)
        .await?;
    Ok(Json(CustomFieldValueListResponse {
        values: values
            .into_iter()
            .map(|v| CustomFieldValueResponse {
                field_id: v.field_id,
                value: v.value,
            })
            .collect(),
    }))
}

#[utoipa::path(
    put,
    path = "/api/v1/issues/{issue_id}/custom-fields/{field_id}/value",
    tag = "custom-fields",
    params(
        ("issue_id" = String, Path, description = "Issue ID"),
        ("field_id" = String, Path, description = "Custom field ID"),
    ),
    request_body = SetCustomFieldValueRequest,
    responses(
        (status = 204, description = "Value set"),
        (status = 400, description = "Bad request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Issue or field not found"),
    ),
    security(("bearer" = []))
)]
pub async fn set_custom_field_value(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path((issue_id, field_id)): Path<(String, String)>,
    Json(body): Json<SetCustomFieldValueRequest>,
) -> Result<StatusCode, AppError> {
    let issue_id = issue_id
        .parse::<IssueId>()
        .map_err(|_| AppError::invalid_input("invalid issue id"))?;
    let field_id = field_id
        .parse::<CustomFieldId>()
        .map_err(|_| AppError::invalid_input("invalid custom field id"))?;
    let requester = parse_user(&claims)?;
    ctx.services
        .custom_field
        .set_value(issue_id, field_id, body.value, requester)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn parse_user_id(claims: &UserClaims) -> Result<UserId, AppError> {
    claims
        .sub
        .parse()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id in token"))
}
