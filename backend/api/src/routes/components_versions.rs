use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use app::{
    auth::UserClaims,
    context::{AppContext, ComponentDto, VersionDto},
};
use shared::{AppError, ProjectComponentId, ProjectKey, ProjectVersionId};

#[derive(Debug, Deserialize, ToSchema)]
pub struct ComponentRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ComponentResponse {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ComponentListResponse {
    pub components: Vec<ComponentResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VersionRequest {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub released: bool,
    pub release_date: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VersionResponse {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: Option<String>,
    pub released: bool,
    pub release_date: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VersionListResponse {
    pub versions: Vec<VersionResponse>,
}

fn component_response(item: ComponentDto) -> ComponentResponse {
    ComponentResponse {
        id: item.id,
        project_id: item.project_id,
        name: item.name,
        description: item.description,
        created_at: item.created_at,
    }
}

fn version_response(item: VersionDto) -> VersionResponse {
    VersionResponse {
        id: item.id,
        project_id: item.project_id,
        name: item.name,
        description: item.description,
        released: item.released,
        release_date: item.release_date,
        created_at: item.created_at,
    }
}

#[utoipa::path(get, path = "/api/v1/projects/{project_key}/components", tag = "components", params(("project_key" = String, Path)), responses((status = 200, body = ComponentListResponse)), security(("bearer" = [])))]
pub async fn list_components(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
) -> Result<Json<ComponentListResponse>, AppError> {
    let key = ProjectKey::new(project_key.as_str());
    let requester = parse_user_id(&claims)?;
    let components = ctx
        .services
        .component
        .list_by_project(&key, requester)
        .await?;
    Ok(Json(ComponentListResponse {
        components: components.into_iter().map(component_response).collect(),
    }))
}

#[utoipa::path(post, path = "/api/v1/projects/{project_key}/components", tag = "components", params(("project_key" = String, Path)), request_body = ComponentRequest, responses((status = 201, body = ComponentResponse)), security(("bearer" = [])))]
pub async fn create_component(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
    Json(body): Json<ComponentRequest>,
) -> Result<(StatusCode, Json<ComponentResponse>), AppError> {
    let key = ProjectKey::new(project_key.as_str());
    let requester = parse_user_id(&claims)?;
    let component = ctx
        .services
        .component
        .create(&key, &body.name, body.description.as_deref(), requester)
        .await?;
    Ok((StatusCode::CREATED, Json(component_response(component))))
}

#[utoipa::path(put, path = "/api/v1/projects/{project_key}/components/{component_id}", tag = "components", params(("project_key" = String, Path), ("component_id" = String, Path)), request_body = ComponentRequest, responses((status = 200, body = ComponentResponse)), security(("bearer" = [])))]
pub async fn update_component(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path((_project_key, component_id)): Path<(String, String)>,
    Json(body): Json<ComponentRequest>,
) -> Result<Json<ComponentResponse>, AppError> {
    let id: ProjectComponentId = component_id
        .parse()
        .map_err(|_| AppError::invalid_input("invalid component id"))?;
    let requester = parse_user_id(&claims)?;
    let component = ctx
        .services
        .component
        .update(id, &body.name, body.description.as_deref(), requester)
        .await?;
    Ok(Json(component_response(component)))
}

#[utoipa::path(delete, path = "/api/v1/projects/{project_key}/components/{component_id}", tag = "components", params(("project_key" = String, Path), ("component_id" = String, Path)), responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_component(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path((_project_key, component_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let id: ProjectComponentId = component_id
        .parse()
        .map_err(|_| AppError::invalid_input("invalid component id"))?;
    let requester = parse_user_id(&claims)?;
    ctx.services.component.delete(id, requester).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/v1/projects/{project_key}/versions", tag = "versions", params(("project_key" = String, Path)), responses((status = 200, body = VersionListResponse)), security(("bearer" = [])))]
pub async fn list_versions(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
) -> Result<Json<VersionListResponse>, AppError> {
    let key = ProjectKey::new(project_key.as_str());
    let requester = parse_user_id(&claims)?;
    let versions = ctx
        .services
        .version
        .list_by_project(&key, requester)
        .await?;
    Ok(Json(VersionListResponse {
        versions: versions.into_iter().map(version_response).collect(),
    }))
}

#[utoipa::path(post, path = "/api/v1/projects/{project_key}/versions", tag = "versions", params(("project_key" = String, Path)), request_body = VersionRequest, responses((status = 201, body = VersionResponse)), security(("bearer" = [])))]
pub async fn create_version(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(project_key): Path<String>,
    Json(body): Json<VersionRequest>,
) -> Result<(StatusCode, Json<VersionResponse>), AppError> {
    let key = ProjectKey::new(project_key.as_str());
    let requester = parse_user_id(&claims)?;
    let version = ctx
        .services
        .version
        .create(
            &key,
            &body.name,
            body.description.as_deref(),
            body.released,
            body.release_date,
            requester,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(version_response(version))))
}

#[utoipa::path(put, path = "/api/v1/projects/{project_key}/versions/{version_id}", tag = "versions", params(("project_key" = String, Path), ("version_id" = String, Path)), request_body = VersionRequest, responses((status = 200, body = VersionResponse)), security(("bearer" = [])))]
pub async fn update_version(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path((_project_key, version_id)): Path<(String, String)>,
    Json(body): Json<VersionRequest>,
) -> Result<Json<VersionResponse>, AppError> {
    let id: ProjectVersionId = version_id
        .parse()
        .map_err(|_| AppError::invalid_input("invalid version id"))?;
    let requester = parse_user_id(&claims)?;
    let version = ctx
        .services
        .version
        .update(
            id,
            &body.name,
            body.description.as_deref(),
            body.released,
            Some(body.release_date),
            requester,
        )
        .await?;
    Ok(Json(version_response(version)))
}

#[utoipa::path(delete, path = "/api/v1/projects/{project_key}/versions/{version_id}", tag = "versions", params(("project_key" = String, Path), ("version_id" = String, Path)), responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_version(
    State(ctx): State<Arc<AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path((_project_key, version_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let id: ProjectVersionId = version_id
        .parse()
        .map_err(|_| AppError::invalid_input("invalid version id"))?;
    let requester = parse_user_id(&claims)?;
    ctx.services.version.delete(id, requester).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn parse_user_id(claims: &UserClaims) -> Result<shared::UserId, AppError> {
    claims
        .sub
        .parse()
        .map(shared::UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id in token"))
}
