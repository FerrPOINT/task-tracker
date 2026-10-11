//! Owner/readers are classified before human projection and never inherit human admin access.
use axum::{
    Extension, Json,
    extract::{Path, Query, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use shared::{AppError, IssueId, ProjectId, resource_context::*};
use std::sync::Arc;
use uuid::Uuid;

fn subjects(key: &str) -> Vec<String> {
    std::env::var(key)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .collect()
}
pub fn registered_machine(subject: &str) -> bool {
    [
        "TT_NAMESPACE__OWNER_SUBJECTS",
        "TT_NAMESPACE__READER_SUBJECTS",
    ]
    .iter()
    .any(|key| subjects(key).iter().any(|s| s == subject))
}
pub fn human_route_machine(subject: &str) -> bool {
    registered_machine(subject)
        || [
            "TASKTRACKER_SDLC__ORCHESTRATOR_SUBJECT",
            "TASKTRACKER_SDLC__VERIFIER_SUBJECT",
            "TASKTRACKER_SDLC__RESERVATION_SCHEDULER_SUBJECT",
        ]
        .iter()
        .any(|key| std::env::var(key).is_ok_and(|value| !value.is_empty() && value == subject))
}

pub async fn owner_auth(req: Request, next: Next) -> Result<Response, StatusCode> {
    machine_auth(req, next, "TT_NAMESPACE__OWNER_SUBJECTS").await
}
pub async fn reader_auth(req: Request, next: Next) -> Result<Response, StatusCode> {
    machine_auth(req, next, "TT_NAMESPACE__READER_SUBJECTS").await
}
async fn machine_auth(mut req: Request, next: Next, key: &str) -> Result<Response, StatusCode> {
    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let ctx = match super::super::middleware::central_auth::check_token(token).await {
        super::super::middleware::central_auth::CentralCheck::Validated(ctx, _) => ctx,
        super::super::middleware::central_auth::CentralCheck::Unavailable => {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        _ => return Err(StatusCode::UNAUTHORIZED),
    };
    if ctx.session_id.is_some()
        || !ctx.allows_service("task-tracker", req.method().as_str())
        || !subjects(key).contains(&ctx.user_id)
    {
        return Err(StatusCode::FORBIDDEN);
    }
    req.extensions_mut().insert(ctx);
    Ok(next.run(req).await)
}

fn configured_uuid(key: &str) -> Result<Uuid, AppError> {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v: &Uuid| !v.is_nil())
        .ok_or_else(|| AppError::Unavailable("namespace_owner_not_configured".into()))
}

#[utoipa::path(put,operation_id="tracker_namespace_apply",path="/api/v1/namespace-resources/tracker_project/{id}",tag="namespaces",params(("id"=Uuid,Path)),request_body=OwnerCommand,responses((status=200,body=OwnerReadback),(status=409,description="Binding/fence conflict")))]
pub async fn apply(
    State(ctx): State<Arc<app::AppContext>>,
    Path(id): Path<Uuid>,
    Json(command): Json<OwnerCommand>,
) -> Result<Json<OwnerReadback>, AppError> {
    if id != command.resource.resource_id
        || !command.valid_for(
            ResourceKind::TrackerProject,
            configured_uuid("TT_NAMESPACE__INSTANCE_ID")?,
            configured_uuid("TT_NAMESPACE__REGISTRY_INSTANCE_ID")?,
        )
    {
        return Err(AppError::invalid_input("invalid_namespace_owner_command"));
    }
    Ok(Json(ctx.repos.projects.apply_namespace(&command).await?))
}

#[utoipa::path(get,operation_id="tracker_namespace_readback",path="/api/v1/namespace-resources/tracker_project/{id}",tag="namespaces",params(("id"=Uuid,Path)),responses((status=200,body=OwnerReadback),(status=404,description="Binding not found")))]
pub async fn readback(
    State(ctx): State<Arc<app::AppContext>>,
    Path(id): Path<Uuid>,
) -> Result<Json<OwnerReadback>, AppError> {
    Ok(Json(
        ctx.repos
            .projects
            .namespace_binding(ProjectId::from_uuid(id))
            .await?
            .ok_or_else(|| AppError::not_found("namespace_binding", id))?,
    ))
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
pub struct TaskContextQuery {
    pub registry_instance_id: Uuid,
    pub namespace_id: Uuid,
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
pub struct TaskCatalogQuery {
    pub registry_instance_id: Uuid,
    pub namespace_id: Uuid,
    pub offset: Option<u32>,
}
#[utoipa::path(get,operation_id="tracker_namespace_task_catalog",path="/api/v1/namespace-tasks",tag="namespaces",params(TaskCatalogQuery),responses((status=200,body=Vec<TaskCatalogItem>)))]
pub async fn task_catalog(
    State(ctx): State<Arc<app::AppContext>>,
    Query(query): Query<TaskCatalogQuery>,
) -> Result<Json<Vec<TaskCatalogItem>>, AppError> {
    Ok(Json(
        ctx.repos
            .projects
            .namespace_task_catalog(
                &NamespaceRef {
                    registry_instance_id: query.registry_instance_id,
                    namespace_id: query.namespace_id,
                },
                i64::from(query.offset.unwrap_or(0)),
            )
            .await?,
    ))
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
pub struct Page {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
#[utoipa::path(get,operation_id="tracker_namespace_available_resources",path="/api/v1/namespace-available-resources",tag="namespaces",params(Page),responses((status=200,body=Vec<ResourceCatalogItem>)))]
pub async fn available_resources(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Query(page): Query<Page>,
) -> Result<Json<Vec<ResourceCatalogItem>>, AppError> {
    let user = shared::UserId::from_uuid(claims.sub.parse().map_err(|_| AppError::Unauthorized)?);
    let mut result = Vec::new();
    for item in ctx
        .repos
        .projects
        .namespace_available_resources(page.limit.unwrap_or(50), page.offset.unwrap_or(0))
        .await?
    {
        match ctx
            .authz
            .require_project_access(ProjectId::from_uuid(item.resource.resource_id), user)
            .await
        {
            Ok(()) => result.push(item),
            Err(AppError::Forbidden) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(Json(result))
}
#[utoipa::path(get,operation_id="tracker_namespace_stats",path="/api/v1/namespace-stats/{registry}/{namespace}",tag="namespaces",params(("registry"=Uuid,Path),("namespace"=Uuid,Path)),responses((status=200,body=ResourceStats)))]
pub async fn stats(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Path((registry, namespace)): Path<(Uuid, Uuid)>,
) -> Result<Json<ResourceStats>, AppError> {
    let resources = ctx
        .repos
        .projects
        .namespace_contexts(
            Some(&NamespaceRef {
                registry_instance_id: registry,
                namespace_id: namespace,
            }),
            1,
            0,
        )
        .await?;
    let resource = resources
        .first()
        .ok_or_else(|| AppError::not_found("namespace_binding", namespace))?;
    let user = shared::UserId::from_uuid(claims.sub.parse().map_err(|_| AppError::Unauthorized)?);
    let project = ProjectId::from_uuid(resource.binding.resource.resource_id);
    ctx.authz.require_project_access(project, user).await?;
    Ok(Json(ctx.repos.projects.namespace_stats(project).await?))
}
#[utoipa::path(get,operation_id="tracker_namespace_contexts",path="/api/v1/namespace-contexts",tag="namespaces",params(Page),responses((status=200,body=Vec<ResourceContextSummary>)))]
pub async fn contexts(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Query(page): Query<Page>,
) -> Result<Json<Vec<ResourceContextSummary>>, AppError> {
    let user = shared::UserId::from_uuid(claims.sub.parse().map_err(|_| AppError::Unauthorized)?);
    let resources = ctx
        .repos
        .projects
        .namespace_contexts(None, page.limit.unwrap_or(50), page.offset.unwrap_or(0))
        .await?;
    let mut visible = Vec::new();
    for resource in resources {
        match ctx
            .authz
            .require_project_access(
                ProjectId::from_uuid(resource.binding.resource.resource_id),
                user,
            )
            .await
        {
            Ok(()) => visible.push(resource),
            Err(AppError::Forbidden) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(Json(visible))
}
/// Machine reader exposes verified navigation metadata, never human mutations.
#[utoipa::path(get,operation_id="tracker_namespace_project_catalog",path="/api/v1/namespace-projects",tag="namespaces",params(Page),responses((status=200,body=Vec<ResourceContextSummary>)))]
pub async fn project_catalog(
    State(ctx): State<Arc<app::AppContext>>,
    Query(page): Query<Page>,
) -> Result<Json<Vec<ResourceContextSummary>>, AppError> {
    Ok(Json(
        ctx.repos
            .projects
            .namespace_contexts(
                None,
                page.limit.unwrap_or(50).clamp(1, 100),
                page.offset.unwrap_or(0).max(0),
            )
            .await?,
    ))
}

#[utoipa::path(get,operation_id="tracker_namespace_context",path="/api/v1/namespace-contexts/{registry}/{namespace}",tag="namespaces",params(("registry"=Uuid,Path),("namespace"=Uuid,Path)),responses((status=200,body=ResourceContextSummary),(status=404,description="No confirmed local binding")))]
pub async fn context(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Path((registry, namespace)): Path<(Uuid, Uuid)>,
) -> Result<Json<ResourceContextSummary>, AppError> {
    let mut resources = ctx
        .repos
        .projects
        .namespace_contexts(
            Some(&NamespaceRef {
                registry_instance_id: registry,
                namespace_id: namespace,
            }),
            1,
            0,
        )
        .await?;
    let resource = resources
        .pop()
        .ok_or_else(|| AppError::not_found("namespace_binding", namespace))?;
    let user = shared::UserId::from_uuid(claims.sub.parse().map_err(|_| AppError::Unauthorized)?);
    ctx.authz
        .require_project_access(
            ProjectId::from_uuid(resource.binding.resource.resource_id),
            user,
        )
        .await?;
    Ok(Json(resource))
}

#[utoipa::path(get,operation_id="tracker_namespace_task_context",path="/api/v1/namespace-tasks/{id}",tag="namespaces",params(("id"=Uuid,Path),TaskContextQuery),responses((status=200,description="Verified immutable Task UUID/project/namespace metadata"),(status=403,description="Foreign namespace")))]
pub async fn task_context(
    State(ctx): State<Arc<app::AppContext>>,
    Path(id): Path<Uuid>,
    Query(query): Query<TaskContextQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let task = ctx.repos.issues.get_by_id(IssueId::from_uuid(id)).await?;
    let binding = ctx
        .repos
        .projects
        .namespace_binding(task.project_id)
        .await?
        .ok_or_else(|| AppError::conflict("unmanaged_task"))?;
    if binding.namespace.registry_instance_id != query.registry_instance_id
        || binding.namespace.namespace_id != query.namespace_id
    {
        return Err(AppError::Forbidden);
    }
    Ok(Json(
        serde_json::json!({"schema_version":1,"namespace":binding.namespace,"tracker_instance_id":binding.resource.instance_id,"project_id":task.project_id.as_uuid(),"task_id":id,"task_key":task.key.to_string(),"state":binding.state,"generation":binding.generation}),
    ))
}

#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TaskDocumentLink {
    pub namespace: NamespaceRef,
    pub document_id: Uuid,
    pub revision_id: Uuid,
    pub title: String,
    pub version: i32,
    pub space_key: String,
    pub task_key: String,
}
#[utoipa::path(get,operation_id="tracker_namespace_documents",path="/api/v1/tasks/{id}/documents",tag="namespaces",params(("id"=Uuid,Path)),responses((status=200,body=Vec<TaskDocumentLink>),(status=503,description="Wiki unavailable, not a zero count")))]
pub async fn documents(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TaskDocumentLink>>, AppError> {
    let task = ctx
        .repos
        .issues
        .get_by_id_include_deleted(IssueId::from_uuid(id))
        .await?;
    let user = shared::UserId::from_uuid(claims.sub.parse().map_err(|_| AppError::Unauthorized)?);
    ctx.authz
        .require_project_access(task.project_id, user)
        .await?;
    let binding = ctx
        .repos
        .projects
        .namespace_binding(task.project_id)
        .await?
        .ok_or_else(|| AppError::conflict("unmanaged_task"))?;
    let links = ctx
        .task_documents
        .read(&binding.namespace, binding.resource.instance_id, id)
        .await?
        .into_iter()
        .map(|value| {
            serde_json::from_value(value)
                .map_err(|_| AppError::Unavailable("wiki_reader_invalid_response".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(links))
}

#[utoipa::path(get,operation_id="tracker_task_repository_links",path="/api/v1/tasks/{id}/repositories",tag="namespaces",params(("id"=Uuid,Path)),responses((status=200,body=Vec<domain::task_repositories::TaskRepositoryLink>)))]
pub async fn repositories(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<domain::task_repositories::TaskRepositoryLink>>, AppError> {
    let task = ctx
        .repos
        .issues
        .get_by_id_include_deleted(IssueId::from_uuid(id))
        .await?;
    let user = shared::UserId::from_uuid(claims.sub.parse().map_err(|_| AppError::Unauthorized)?);
    ctx.authz
        .require_project_access(task.project_id, user)
        .await?;
    let binding = ctx
        .repos
        .projects
        .namespace_binding(task.project_id)
        .await?
        .ok_or_else(|| AppError::conflict("unmanaged_task"))?;
    let values = ctx.repos.issues.repository_links(task.id).await?;
    let links = values
        .into_iter()
        .map(|value| {
            serde_json::from_value::<domain::task_repositories::TaskRepositoryLink>(value)
                .map_err(|_| AppError::Unavailable("invalid_repository_link_projection".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if links.iter().any(|link| link.namespace != binding.namespace) {
        return Err(AppError::Unavailable(
            "invalid_repository_link_projection".into(),
        ));
    }
    Ok(Json(links))
}
#[derive(serde::Deserialize, utoipa::IntoParams)]
pub struct RepositoryPage {
    pub offset: Option<u32>,
}
#[utoipa::path(get,operation_id="tracker_task_delivery_evidence",path="/api/v1/tasks/{id}/delivery-evidence",tag="namespaces",params(("id"=Uuid,Path),RepositoryPage),responses((status=200,body=Vec<TaskPullEvidence>),(status=503,description="Forge unavailable")))]
pub async fn delivery_evidence(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Path(id): Path<Uuid>,
    Query(page): Query<RepositoryPage>,
) -> Result<Json<Vec<TaskPullEvidence>>, AppError> {
    let task = ctx
        .repos
        .issues
        .get_by_id_include_deleted(IssueId::from_uuid(id))
        .await?;
    let user = shared::UserId::from_uuid(claims.sub.parse().map_err(|_| AppError::Unauthorized)?);
    ctx.authz
        .require_project_access(task.project_id, user)
        .await?;
    let binding = ctx
        .repos
        .projects
        .namespace_binding(task.project_id)
        .await?
        .ok_or_else(|| AppError::conflict("unmanaged_task"))?;
    Ok(Json(
        ctx.task_repositories
            .evidence(
                &binding.namespace,
                binding.resource.instance_id,
                id,
                page.offset.unwrap_or(0),
            )
            .await?,
    ))
}
#[utoipa::path(get,operation_id="tracker_available_task_repositories",path="/api/v1/tasks/{id}/available-repositories",tag="namespaces",params(("id"=Uuid,Path),RepositoryPage),responses((status=200,body=Vec<domain::task_repositories::TaskRepositoryLink>),(status=503,description="Forge unavailable, not an empty catalog")))]
pub async fn available_repositories(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Path(id): Path<Uuid>,
    Query(page): Query<RepositoryPage>,
) -> Result<Json<Vec<domain::task_repositories::TaskRepositoryLink>>, AppError> {
    let task = ctx.repos.issues.get_by_id(IssueId::from_uuid(id)).await?;
    let user = shared::UserId::from_uuid(claims.sub.parse().map_err(|_| AppError::Unauthorized)?);
    ctx.authz
        .require_project_access(task.project_id, user)
        .await?;
    let binding = ctx
        .repos
        .projects
        .namespace_binding(task.project_id)
        .await?
        .ok_or_else(|| AppError::conflict("unmanaged_task"))?;
    let links = ctx
        .task_repositories
        .list(&binding.namespace, page.offset.unwrap_or(0))
        .await?
        .into_iter()
        .map(|value| {
            serde_json::from_value(value)
                .map_err(|_| AppError::Unavailable("forge_reader_invalid_response".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(links))
}
#[utoipa::path(post,operation_id="tracker_link_task_repository",path="/api/v1/tasks/{id}/repositories",tag="namespaces",params(("id"=Uuid,Path)),request_body=domain::task_repositories::RepositoryRef,responses((status=200,body=domain::task_repositories::TaskRepositoryLink),(status=503,description="Forge reader unavailable")))]
pub async fn link_repository(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Path(id): Path<Uuid>,
    Json(repository): Json<domain::task_repositories::RepositoryRef>,
) -> Result<Json<domain::task_repositories::TaskRepositoryLink>, AppError> {
    let task = ctx.repos.issues.get_by_id(IssueId::from_uuid(id)).await?;
    let user = shared::UserId::from_uuid(claims.sub.parse().map_err(|_| AppError::Unauthorized)?);
    ctx.authz
        .require_project_edit(task.project_id, user)
        .await?;
    let binding = ctx
        .repos
        .projects
        .namespace_binding(task.project_id)
        .await?
        .ok_or_else(|| AppError::conflict("unmanaged_task"))?;
    let saved_links = ctx
        .repos
        .issues
        .repository_links(task.id)
        .await?
        .into_iter()
        .map(|value| {
            serde_json::from_value::<domain::task_repositories::TaskRepositoryLink>(value)
                .map_err(|_| AppError::Unavailable("invalid_repository_link_projection".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(saved) = saved_links.into_iter().find(|link| {
        link.repository_id == repository.repository_id
            && link.forge_instance_id == repository.forge_instance_id
    }) {
        if saved.namespace != binding.namespace {
            return Err(AppError::Unavailable(
                "invalid_repository_link_projection".into(),
            ));
        }
        return Ok(Json(saved));
    }
    if binding.state != "active" {
        return Err(AppError::conflict("namespace_resource_read_only"));
    }
    let snapshot = ctx
        .task_repositories
        .verify(&binding.namespace, &repository)
        .await?;
    let link = serde_json::from_value(snapshot.clone())
        .map_err(|_| AppError::Unavailable("forge_reader_invalid_response".into()))?;
    ctx.repos
        .issues
        .link_repository(task.id, user, &repository, &snapshot)
        .await?;
    Ok(Json(link))
}
