use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use shared::{AppError, ProjectKey, UserId};
use std::sync::Arc;
use uuid::Uuid;

use crate::routes::namespace::RepositoryPage;

async fn project_for_access(
    ctx: &app::AppContext,
    key: &str,
    requester: UserId,
    edit: bool,
) -> Result<(domain::Project, shared::resource_context::OwnerReadback), AppError> {
    let key = ProjectKey::new(key);
    if !key.is_valid() {
        return Err(AppError::invalid_input("project_key"));
    }
    let project = ctx.repos.projects.get_by_key(&key).await?;
    if edit {
        ctx.authz
            .require_project_edit(project.id, requester)
            .await?;
    } else {
        ctx.authz
            .require_project_access(project.id, requester)
            .await?;
    }
    let binding = ctx
        .repos
        .projects
        .namespace_binding(project.id)
        .await?
        .ok_or_else(|| AppError::conflict("unmanaged_project"))?;
    Ok((project, binding))
}

fn requester(claims: &app::auth::UserClaims) -> Result<UserId, AppError> {
    claims
        .sub
        .parse::<Uuid>()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::Unauthorized)
}

fn parse_links(
    values: Vec<serde_json::Value>,
) -> Result<Vec<domain::task_repositories::TaskRepositoryLink>, AppError> {
    values
        .into_iter()
        .map(|value| {
            serde_json::from_value(value)
                .map_err(|_| AppError::Unavailable("invalid_repository_link_projection".into()))
        })
        .collect()
}

#[utoipa::path(get, operation_id="tracker_project_repository_links", path="/api/v1/projects/{project_key}/repositories", tag="projects",
    params(("project_key"=String,Path)), responses((status=200,body=Vec<domain::task_repositories::TaskRepositoryLink>)), security(("bearer" = [])))]
pub async fn list(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Path(key): Path<String>,
) -> Result<Json<Vec<domain::task_repositories::TaskRepositoryLink>>, AppError> {
    let (project, binding) = project_for_access(&ctx, &key, requester(&claims)?, false).await?;
    let links = parse_links(ctx.repos.projects.repository_links(project.id).await?)?;
    if links.iter().any(|link| link.namespace != binding.namespace) {
        return Err(AppError::Unavailable(
            "invalid_repository_link_projection".into(),
        ));
    }
    Ok(Json(links))
}

#[utoipa::path(get, operation_id="tracker_available_project_repositories", path="/api/v1/projects/{project_key}/available-repositories", tag="projects",
    params(("project_key"=String,Path),RepositoryPage), responses((status=200,body=Vec<domain::task_repositories::TaskRepositoryLink>),(status=503,description="Forge unavailable, not an empty catalog")), security(("bearer" = [])))]
pub async fn available(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Path(key): Path<String>,
    Query(page): Query<RepositoryPage>,
) -> Result<Json<Vec<domain::task_repositories::TaskRepositoryLink>>, AppError> {
    let (_, binding) = project_for_access(&ctx, &key, requester(&claims)?, false).await?;
    let links = ctx
        .task_repositories
        .list(&binding.namespace, page.offset.unwrap_or(0))
        .await?
        .into_iter()
        .map(|value| {
            serde_json::from_value(value)
                .map_err(|_| AppError::Unavailable("forge_reader_invalid_response".into()))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok(Json(links))
}

#[utoipa::path(post, operation_id="tracker_link_project_repository", path="/api/v1/projects/{project_key}/repositories", tag="projects",
    params(("project_key"=String,Path)), request_body=domain::task_repositories::RepositoryRef, responses((status=204,description="Repository linked; repeated link is a no-op"),(status=409,description="Archived project"),(status=503,description="Forge unavailable")), security(("bearer" = [])))]
pub async fn link(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Path(key): Path<String>,
    Json(repository): Json<domain::task_repositories::RepositoryRef>,
) -> Result<StatusCode, AppError> {
    let requester = requester(&claims)?;
    let (project, binding) = project_for_access(&ctx, &key, requester, true).await?;
    if binding.state != "active" {
        return Err(AppError::conflict("namespace_resource_read_only"));
    }
    let snapshot = ctx
        .task_repositories
        .verify(&binding.namespace, &repository)
        .await?;
    let link: domain::task_repositories::TaskRepositoryLink =
        serde_json::from_value(snapshot.clone())
            .map_err(|_| AppError::Unavailable("forge_reader_invalid_response".into()))?;
    if link.namespace != binding.namespace
        || link.forge_instance_id != repository.forge_instance_id
        || link.repository_id != repository.repository_id
    {
        return Err(AppError::Unavailable(
            "forge_reader_invalid_response".into(),
        ));
    }
    ctx.repos
        .projects
        .link_repository(project.id, requester, &repository, &snapshot)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, operation_id="tracker_unlink_project_repository", path="/api/v1/projects/{project_key}/repositories/{forge_instance_id}/{repository_id}", tag="projects",
    params(("project_key"=String,Path),("forge_instance_id"=Uuid,Path),("repository_id"=Uuid,Path)), responses((status=204,description="Repository link removed"),(status=409,description="Archived project")), security(("bearer" = [])))]
pub async fn unlink(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Path((key, forge_instance_id, repository_id)): Path<(String, Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let requester = requester(&claims)?;
    let (project, _) = project_for_access(&ctx, &key, requester, true).await?;
    ctx.repos
        .projects
        .unlink_repository(
            project.id,
            &domain::task_repositories::RepositoryRef {
                forge_instance_id,
                repository_id,
            },
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
