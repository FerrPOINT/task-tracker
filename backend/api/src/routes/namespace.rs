//! Owner/readers are classified before human projection and never inherit human admin access.
use axum::{
    Extension, Json,
    extract::{Path, Query, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use shared::{AppError, IssueId, ProjectId, resource_context::*};
use std::{collections::HashSet, sync::Arc};
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
const NAMESPACE_CONTEXT_RAW_PAGE_SIZE: i64 = 100;

struct NamespaceContextPager {
    // API offsets count authorized entries; this cursor only counts raw bindings.
    raw_offset: i64,
    visible_offset: usize,
    limit: usize,
    items: Vec<ResourceContextSummary>,
}

impl NamespaceContextPager {
    fn new(limit: i64, visible_offset: i64) -> Self {
        Self {
            raw_offset: 0,
            visible_offset: visible_offset as usize,
            limit: limit as usize,
            items: Vec::with_capacity(limit as usize + 1),
        }
    }

    fn push_authorized(&mut self, item: ResourceContextSummary) {
        if self.visible_offset > 0 {
            self.visible_offset -= 1;
        } else if self.items.len() <= self.limit {
            self.items.push(item);
        }
    }

    fn push_if_accessible(
        &mut self,
        item: ResourceContextSummary,
        accessible_projects: &HashSet<ProjectId>,
    ) {
        let project = ProjectId::from_uuid(item.binding.resource.resource_id);
        if accessible_projects.contains(&project) {
            self.push_authorized(item);
        }
    }

    fn has_lookahead(&self) -> bool {
        self.items.len() > self.limit
    }

    fn advance_raw_page(&mut self, raw_count: usize) -> bool {
        if self.has_lookahead() || raw_count < NAMESPACE_CONTEXT_RAW_PAGE_SIZE as usize {
            return false;
        }
        self.raw_offset += raw_count as i64;
        true
    }

    fn finish(mut self) -> Vec<ResourceContextSummary> {
        self.items.truncate(self.limit);
        self.items
    }
}

const NAMESPACE_AVAILABLE_RESOURCES_RAW_PAGE_SIZE: i64 = 100;

struct NamespaceAvailableResourcesPager {
    // Public offsets count visible projects; this cursor counts raw catalog rows.
    raw_offset: i64,
    visible_offset: usize,
    limit: usize,
    items: Vec<ResourceCatalogItem>,
}

impl NamespaceAvailableResourcesPager {
    fn new(limit: i64, visible_offset: i64) -> Self {
        Self {
            raw_offset: 0,
            visible_offset: visible_offset as usize,
            limit: limit as usize,
            items: Vec::with_capacity(limit as usize + 1),
        }
    }

    fn push_if_accessible(
        &mut self,
        item: ResourceCatalogItem,
        accessible_projects: &HashSet<ProjectId>,
    ) {
        let project = ProjectId::from_uuid(item.resource.resource_id);
        if !accessible_projects.contains(&project) {
            return;
        }
        if self.visible_offset > 0 {
            self.visible_offset -= 1;
        } else if self.items.len() <= self.limit {
            self.items.push(item);
        }
    }

    fn has_lookahead(&self) -> bool {
        self.items.len() > self.limit
    }

    fn advance_raw_page(&mut self, raw_count: usize) -> bool {
        if self.has_lookahead() || raw_count < NAMESPACE_AVAILABLE_RESOURCES_RAW_PAGE_SIZE as usize
        {
            return false;
        }
        self.raw_offset += raw_count as i64;
        true
    }

    fn finish(mut self) -> Vec<ResourceCatalogItem> {
        self.items.truncate(self.limit);
        self.items
    }
}

#[utoipa::path(get,operation_id="tracker_namespace_available_resources",path="/api/v1/namespace-available-resources",tag="namespaces",params(Page),responses((status=200,body=Vec<ResourceCatalogItem>)))]
pub async fn available_resources(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Query(page): Query<Page>,
) -> Result<Json<Vec<ResourceCatalogItem>>, AppError> {
    let user = shared::UserId::from_uuid(claims.sub.parse().map_err(|_| AppError::Unauthorized)?);
    let limit = page.limit.unwrap_or(50).clamp(1, 100);
    let offset = page.offset.unwrap_or(0).max(0);
    let accessible_projects: HashSet<_> = ctx
        .authz
        .accessible_project_ids(user)
        .await?
        .into_iter()
        .collect();
    let mut pager = NamespaceAvailableResourcesPager::new(limit, offset);
    loop {
        let resources = ctx
            .repos
            .projects
            .namespace_available_resources(
                NAMESPACE_AVAILABLE_RESOURCES_RAW_PAGE_SIZE,
                pager.raw_offset,
            )
            .await?;
        let raw_count = resources.len();
        for resource in resources {
            pager.push_if_accessible(resource, &accessible_projects);
            if pager.has_lookahead() {
                break;
            }
        }
        if !pager.advance_raw_page(raw_count) {
            break;
        }
    }
    Ok(Json(pager.finish()))
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
    let limit = page.limit.unwrap_or(50).clamp(1, 100);
    let offset = page.offset.unwrap_or(0).max(0);
    let accessible_projects: HashSet<_> = ctx
        .authz
        .accessible_project_ids(user)
        .await?
        .into_iter()
        .collect();
    let mut pager = NamespaceContextPager::new(limit, offset);
    loop {
        let resources = ctx
            .repos
            .projects
            .namespace_contexts(None, NAMESPACE_CONTEXT_RAW_PAGE_SIZE, pager.raw_offset)
            .await?;
        let raw_count = resources.len();
        for resource in resources {
            pager.push_if_accessible(resource, &accessible_projects);
            if pager.has_lookahead() {
                break;
            }
        }
        if !pager.advance_raw_page(raw_count) {
            break;
        }
    }
    Ok(Json(pager.finish()))
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

#[cfg(test)]
mod namespace_context_page_tests {
    use super::*;

    fn catalog_item(id: Uuid) -> ResourceCatalogItem {
        ResourceCatalogItem {
            resource: ResourceRef {
                kind: ResourceKind::TrackerProject,
                instance_id: Uuid::from_u128(1),
                resource_id: id,
            },
            label: format!("Project {id}"),
            resource_key: format!("P{}", &id.to_string()[..8]),
        }
    }

    fn item() -> ResourceContextSummary {
        let id = Uuid::new_v4();
        ResourceContextSummary {
            binding: OwnerReadback {
                schema_version: 1,
                namespace: NamespaceRef {
                    registry_instance_id: Uuid::new_v4(),
                    namespace_id: Uuid::new_v4(),
                },
                resource: ResourceRef {
                    kind: ResourceKind::TrackerProject,
                    instance_id: Uuid::new_v4(),
                    resource_id: id,
                },
                operation_id: Uuid::new_v4(),
                generation: 1,
                state: "active".into(),
                drained: true,
            },
            label: format!("Project {id}"),
            resource_key: format!("P{}", &id.to_string()[..8]),
        }
    }

    #[test]
    fn namespace_page_includes_only_projects_in_the_prefetched_access_set() {
        let accessible = item();
        let inaccessible = item();
        let accessible_ids = HashSet::from([ProjectId::from_uuid(
            accessible.binding.resource.resource_id,
        )]);
        let mut pager = NamespaceContextPager::new(2, 0);

        pager.push_if_accessible(inaccessible, &accessible_ids);
        pager.push_if_accessible(accessible.clone(), &accessible_ids);

        let page = pager.finish();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].resource_key, accessible.resource_key);
    }

    #[test]
    fn sparse_authorized_projects_fill_visible_pages_across_raw_pages() {
        let one = item();
        let two = item();
        let three = item();
        let four = item();
        let mut first = NamespaceContextPager::new(2, 0);
        first.push_authorized(one.clone());
        assert!(first.advance_raw_page(100));
        first.push_authorized(two.clone());
        first.push_authorized(three.clone());
        first.push_authorized(four.clone());
        assert!(first.has_lookahead());
        let page = first.finish();
        assert_eq!(
            page.iter()
                .map(|item| item.resource_key.as_str())
                .collect::<Vec<_>>(),
            [one.resource_key.as_str(), two.resource_key.as_str()]
        );

        let mut second = NamespaceContextPager::new(2, 2);
        second.push_authorized(one);
        assert!(second.advance_raw_page(100));
        second.push_authorized(two);
        second.push_authorized(three.clone());
        second.push_authorized(four.clone());
        assert!(!second.has_lookahead());
        assert!(!second.advance_raw_page(38));
        let page = second.finish();
        assert_eq!(
            page.iter()
                .map(|item| item.resource_key.as_str())
                .collect::<Vec<_>>(),
            [three.resource_key.as_str(), four.resource_key.as_str()]
        );
    }

    #[test]
    fn fully_accessible_page_uses_visible_offset_and_limit_plus_one_lookahead() {
        let one = item();
        let two = item();
        let three = item();
        let four = item();
        let five = item();
        let mut first = NamespaceContextPager::new(2, 0);
        first.push_authorized(one.clone());
        first.push_authorized(two.clone());
        first.push_authorized(three.clone());
        assert!(first.has_lookahead());
        assert!(!first.advance_raw_page(100));
        let page = first.finish();
        assert_eq!(
            page.iter()
                .map(|item| item.resource_key.as_str())
                .collect::<Vec<_>>(),
            [one.resource_key.as_str(), two.resource_key.as_str()]
        );

        let mut second = NamespaceContextPager::new(2, 2);
        second.push_authorized(one);
        second.push_authorized(two);
        second.push_authorized(three.clone());
        second.push_authorized(four.clone());
        second.push_authorized(five);
        assert!(second.has_lookahead());
        let page = second.finish();
        assert_eq!(
            page.iter()
                .map(|item| item.resource_key.as_str())
                .collect::<Vec<_>>(),
            [three.resource_key.as_str(), four.resource_key.as_str()]
        );
    }

    #[test]
    fn available_resource_pages_fill_from_later_raw_pages_without_skipping_visible_items() {
        let raw: Vec<_> = (0..160)
            .map(|index| catalog_item(Uuid::from_u128(index + 1)))
            .collect();
        let accessible: HashSet<_> = raw
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 2 == 0)
            .map(|(_, item)| ProjectId::from_uuid(item.resource.resource_id))
            .collect();

        let load_page = |offset| {
            let mut pager = NamespaceAvailableResourcesPager::new(50, offset);
            loop {
                let start = pager.raw_offset as usize;
                let batch: Vec<_> = raw
                    .iter()
                    .skip(start)
                    .take(NAMESPACE_AVAILABLE_RESOURCES_RAW_PAGE_SIZE as usize)
                    .cloned()
                    .collect();
                let raw_count = batch.len();
                for item in batch {
                    pager.push_if_accessible(item, &accessible);
                    if pager.has_lookahead() {
                        break;
                    }
                }
                if !pager.advance_raw_page(raw_count) {
                    break;
                }
            }
            pager.finish()
        };

        let first = load_page(0);
        let second = load_page(50);
        assert_eq!(first.len(), 50);
        assert_eq!(second.len(), 30);
        assert_eq!(first[0].resource.resource_id, Uuid::from_u128(1));
        assert_eq!(first[49].resource.resource_id, Uuid::from_u128(99));
        assert_eq!(second[0].resource.resource_id, Uuid::from_u128(101));
        assert_eq!(second[29].resource.resource_id, Uuid::from_u128(159));
    }
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
