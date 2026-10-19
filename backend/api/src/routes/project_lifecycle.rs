//! Versioned project archive metadata; independent of Space owner transitions.
use super::project_spaces::{actor, key, no_store};
use crate::middleware::auth::UserClaims;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderMap, header::HeaderName},
};
use domain::project_spaces::ChangeProjectLifecycle;
use shared::{AppError, project_spaces::ProjectLifecycle};
use std::sync::Arc;
#[utoipa::path(operation_id="get_project_lifecycle_v2",get,path="/api/v2/projects/{project_key}/lifecycle",params(("project_key"=String,Path)),responses((status=200,body=ProjectLifecycle),(status=403,description="Space admission denied")),security(("bearer"=[])))]
pub async fn get(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    headers: HeaderMap,
    Path(value): Path<String>,
) -> Result<([(HeaderName, &'static str); 1], Json<ProjectLifecycle>), AppError> {
    let key = key(&value)?;
    let project = ctx.repos.projects.get_by_key(&key).await?;
    let links = ctx.repos.projects.project_spaces(project.id).await?;
    let actor = actor(&ctx, &headers, &claims, "GET", &links.spaces).await?;
    Ok(no_store(
        ctx.services.project.lifecycle(&key, &actor).await?,
    ))
}
#[utoipa::path(operation_id="change_project_lifecycle_v2",put,path="/api/v2/projects/{project_key}/lifecycle",params(("project_key"=String,Path)),request_body=ChangeProjectLifecycle,responses((status=200,body=ProjectLifecycle),(status=403,description="Project management denied"),(status=409,description="Revision or original operation conflict")),security(("bearer"=[])))]
pub async fn change(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    headers: HeaderMap,
    Path(value): Path<String>,
    Json(command): Json<ChangeProjectLifecycle>,
) -> Result<([(HeaderName, &'static str); 1], Json<ProjectLifecycle>), AppError> {
    command.validate()?;
    let key = key(&value)?;
    let project = ctx.repos.projects.get_by_key(&key).await?;
    let links = ctx.repos.projects.project_spaces(project.id).await?;
    let actor = actor(&ctx, &headers, &claims, "PUT", &links.spaces).await?;
    Ok(no_store(
        ctx.services
            .project
            .change_lifecycle(&key, command, &actor)
            .await?,
    ))
}
