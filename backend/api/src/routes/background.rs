//! Human-only projection. Service/agent grants never impersonate a project member.
use super::super::middleware::auth::UserClaims;
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use shared::{AppError, ProjectId, UserId};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

#[derive(Serialize, utoipa::ToSchema)]
pub struct BackgroundProject {
    pub id: Uuid,
    pub name: String,
    pub can_manage: bool,
    pub can_triage: bool,
    pub can_create_tasks: bool,
}
#[derive(Serialize, utoipa::ToSchema)]
pub struct ProjectPage {
    pub items: Vec<BackgroundProject>,
    pub next_cursor: Option<String>,
}
#[derive(Default, Deserialize, utoipa::IntoParams)]
#[serde(deny_unknown_fields)]
pub struct PageQuery {
    pub after: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Deserialize, utoipa::IntoParams)]
#[serde(deny_unknown_fields)]
pub struct TasksQuery {
    pub tasks: String,
}
pub fn task_ids(value: &str) -> Result<Vec<Uuid>, AppError> {
    let ids = value
        .split(',')
        .map(str::parse::<Uuid>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AppError::validation("invalid linked task identifier"))?;
    if ids.is_empty()
        || ids.len() > 100
        || ids.iter().any(Uuid::is_nil)
        || ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len()
    {
        return Err(AppError::validation(
            "expected 1..100 distinct task identifiers",
        ));
    }
    Ok(ids)
}

#[utoipa::path(get,operation_id="background_project_task_states",path="/api/v1/background/projects/{project_id}/task-states",tag="background",
    params(("project_id"=Uuid,Path),TasksQuery),responses((status=200,body=domain::sdlc_source::BackgroundTaskStates)),security(("bearer"=[])))]
pub async fn task_states(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<Uuid>,
    Query(query): Query<TasksQuery>,
) -> Result<Json<domain::sdlc_source::BackgroundTaskStates>, AppError> {
    let actor: UserId = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    ctx.authz
        .require_project_access(ProjectId::from_uuid(id), actor)
        .await?;
    let ids = task_ids(&query.tasks)?;
    let service = ctx
        .sdlc
        .as_ref()
        .ok_or_else(|| AppError::Unavailable("task reader unavailable".into()))?;
    Ok(Json(
        service
            .source_repository()?
            .background_task_states(id, &ids)
            .await?,
    ))
}

fn permitted(result: Result<(), AppError>) -> Result<bool, AppError> {
    match result {
        Ok(()) => Ok(true),
        Err(AppError::Forbidden) => Ok(false),
        Err(AppError::Conflict(reason)) if reason == "project_archived_read_only" => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archived_projects_remain_readable_without_hiding_storage_failures() {
        assert!(!permitted(Err(AppError::conflict("project_archived_read_only"))).unwrap());
        assert!(!permitted(Err(AppError::Forbidden)).unwrap());
        assert!(permitted(Err(AppError::conflict("unexpected_conflict"))).is_err());
        assert!(permitted(Err(AppError::Unavailable("storage".into()))).is_err());
    }
}

async fn project(
    ctx: &app::AppContext,
    actor: UserId,
    value: domain::Project,
    membership: Option<&domain::ProjectRole>,
    administrator: bool,
) -> Result<BackgroundProject, AppError> {
    ctx.authz.require_project_access(value.id, actor).await?;
    let writable = permitted(ctx.repos.projects.require_writable(value.id).await)?;
    let (can_manage, can_triage) =
        app::background::permissions(value.owner_id == actor, membership, administrator, writable);
    Ok(BackgroundProject {
        id: value.id.as_uuid(),
        name: value.name.to_string(),
        can_manage,
        can_triage,
        can_create_tasks: can_triage,
    })
}

#[utoipa::path(get,operation_id="background_project_list",path="/api/v1/background/projects",tag="background",params(PageQuery),
    responses((status=200,body=ProjectPage)),security(("bearer"=[])))]
pub async fn list(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Query(page): Query<PageQuery>,
) -> Result<Json<ProjectPage>, AppError> {
    let actor: UserId = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let limit = page.limit.unwrap_or(50);
    let offset = page.after.unwrap_or(0);
    if !(1..=100).contains(&limit) || offset > i64::MAX as u64 {
        return Err(AppError::invalid_input("invalid background page"));
    }
    let administrator = app::authz::central_administrator(actor)?;
    let memberships: HashMap<_, _> = ctx
        .repos
        .members
        .list_by_user(actor)
        .await?
        .into_iter()
        .map(|member| (member.project_id, member.role))
        .collect();
    let values = ctx
        .services
        .project
        .list(app::commands::ProjectQueryDto { limit, offset }, actor)
        .await?;
    let next_cursor = (values.len() as u64 == limit).then(|| (offset + limit).to_string());
    let mut items = Vec::with_capacity(values.len());
    for item in values {
        let id: ProjectId = item
            .id
            .parse()
            .map_err(|_| AppError::Unavailable("invalid project identity".into()))?;
        let value = ctx.repos.projects.get_by_id(id).await?;
        items.push(project(&ctx, actor, value, memberships.get(&id), administrator).await?);
    }
    Ok(Json(ProjectPage { items, next_cursor }))
}

#[utoipa::path(get,operation_id="background_project_get",path="/api/v1/background/projects/{project_id}",tag="background",
    params(("project_id"=Uuid,Path)),responses((status=200,body=BackgroundProject)),security(("bearer"=[])))]
pub async fn get(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<Uuid>,
) -> Result<Json<BackgroundProject>, AppError> {
    let actor: UserId = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let id = ProjectId::from_uuid(id);
    let value = ctx.repos.projects.get_by_id(id).await?;
    ctx.authz.require_project_access(id, actor).await?;
    let membership = match ctx.repos.members.get(id, actor).await {
        Ok(member) => Some(member.role),
        Err(AppError::NotFound(_)) => None,
        Err(error) => return Err(error),
    };
    Ok(Json(
        project(
            &ctx,
            actor,
            value,
            membership.as_ref(),
            app::authz::central_administrator(actor)?,
        )
        .await?,
    ))
}

#[utoipa::path(get,operation_id="background_project_intake_fields",path="/api/v1/background/projects/{project_id}/intake-fields",tag="background",
    params(("project_id"=Uuid,Path)),responses((status=200,body=super::custom_fields::CustomFieldListResponse)),security(("bearer"=[])))]
pub async fn intake_fields(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<Uuid>,
) -> Result<Json<super::custom_fields::CustomFieldListResponse>, AppError> {
    let actor: UserId = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let project = ctx
        .repos
        .projects
        .get_by_id(ProjectId::from_uuid(id))
        .await?;
    ctx.authz.require_project_edit(project.id, actor).await?;
    let items = ctx
        .services
        .custom_field
        .list_fields(&project.key, actor)
        .await?;
    Ok(Json(super::custom_fields::CustomFieldListResponse {
        fields: items
            .into_iter()
            .map(|f| super::custom_fields::CustomFieldResponse {
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
