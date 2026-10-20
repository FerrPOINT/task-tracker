//! Versioned project archive metadata; independent of Space owner transitions.
use super::project_spaces::{actor, key, no_store};
use crate::middleware::auth::UserClaims;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::HeaderMap,
    response::Response,
};
use domain::project_spaces::ChangeProjectLifecycle;
use shared::{AppError, project_spaces::ProjectLifecycle};
use std::sync::Arc;
#[utoipa::path(operation_id="get_project_lifecycle_v2",get,path="/api/v2/projects/{project_key}/lifecycle",params(("project_key"=String,Path)),responses((status=200,body=ProjectLifecycle),(status=403,description="Space admission denied"),(status=400,description="Invalid identifier or command"),(status=401,description="Fresh central token required"),(status=404,description="Requested resource unavailable"),(status=503,description="Fresh owner confirmation unavailable")),security(("bearer"=[])))]
pub async fn get(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    headers: HeaderMap,
    Path(value): Path<String>,
) -> Result<Response, AppError> {
    let key = key(&value)?;
    let project = ctx.repos.projects.get_by_key(&key).await?;
    let links = ctx.repos.projects.project_spaces(project.id).await?;
    let actor = actor(&ctx, &headers, &claims, "GET", &links.spaces).await?;
    no_store(ctx.services.project.lifecycle(&key, &actor).await?, &actor)
}
#[utoipa::path(operation_id="change_project_lifecycle_v2",put,path="/api/v2/projects/{project_key}/lifecycle",params(("project_key"=String,Path)),request_body=ChangeProjectLifecycle,responses((status=200,body=ProjectLifecycle),(status=403,description="Project management denied"),(status=409,description="Revision or original operation conflict"),(status=400,description="Invalid identifier or command"),(status=401,description="Fresh central token required"),(status=404,description="Requested resource unavailable"),(status=422,description="Invalid strict request schema"),(status=503,description="Fresh owner confirmation unavailable")),security(("bearer"=[])))]
pub async fn change(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    headers: HeaderMap,
    Path(value): Path<String>,
    Json(command): Json<ChangeProjectLifecycle>,
) -> Result<Response, AppError> {
    command.validate()?;
    let key = key(&value)?;
    let project = ctx.repos.projects.get_by_key(&key).await?;
    let links = ctx.repos.projects.project_spaces(project.id).await?;
    let actor = actor(&ctx, &headers, &claims, "PUT", &links.spaces).await?;
    no_store(
        ctx.services
            .project
            .change_lifecycle(&key, command, &actor)
            .await?,
        &actor,
    )
}
