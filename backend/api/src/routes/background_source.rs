//! Backend-only project source projection for registered Fleet, never an agent grant.
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, header},
    routing::get,
};
use pdlc_auth_core::{application_access::HumanRole, service_bridge::VerifiedBridgeError};
use serde::{Deserialize, Serialize};
use shared::{AppError, ProjectId};
use std::sync::Arc;
use uuid::Uuid;

pub fn router() -> Router<Arc<app::AppContext>> {
    Router::new()
        .route(
            "/internal/background/v1/projects/{project}/task-states",
            get(task_states),
        )
        .route(
            "/internal/background/v1/projects/{project}/source-access",
            get(read),
        )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskQuery {
    owner_subject: Uuid,
    owner_issuer: String,
    tasks: String,
}
pub async fn task_states(
    State(ctx): State<Arc<app::AppContext>>,
    Path(project): Path<Uuid>,
    Query(query): Query<TaskQuery>,
    headers: HeaderMap,
) -> Result<Json<domain::sdlc_source::BackgroundTaskStates>, AppError> {
    let ids = super::background::task_ids(&query.tasks)?;
    let access = read(
        State(ctx.clone()),
        Path(project),
        Query(OwnerQuery {
            owner_subject: query.owner_subject,
            owner_issuer: query.owner_issuer,
            sources: false,
        }),
        headers,
    )
    .await?
    .0;
    let service = ctx
        .sdlc
        .as_ref()
        .ok_or_else(|| AppError::Unavailable("task reader unavailable".into()))?;
    let value = service
        .source_repository()?
        .background_task_states(project, &ids)
        .await?;
    if access.valid_until <= chrono::Utc::now() {
        return Err(AppError::Forbidden);
    }
    Ok(Json(value))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerQuery {
    owner_subject: Uuid,
    owner_issuer: String,
    sources: bool,
}
#[derive(Serialize)]
pub struct SourceGrant {
    repository: Uuid,
    forge_instance_id: Uuid,
    branch: String,
    namespace: shared::resource_context::NamespaceRef,
}
#[derive(Serialize)]
pub struct SourceAccess {
    schema: &'static str,
    tracker_instance_id: Uuid,
    project_id: Uuid,
    project_name: String,
    owner_subject: Uuid,
    owner_issuer: String,
    sources_included: bool,
    source_revision: Option<i64>,
    repositories: Vec<SourceGrant>,
    valid_until: chrono::DateTime<chrono::Utc>,
}

pub async fn read(
    State(ctx): State<Arc<app::AppContext>>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<OwnerQuery>,
    headers: HeaderMap,
) -> Result<Json<SourceAccess>, AppError> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;
    let verified = app::space_access::CENTRAL_BRIDGE
        .try_verified_token(token)
        .await
        .map_err(|error| match error {
            VerifiedBridgeError::Unavailable | VerifiedBridgeError::NotConfigured => {
                AppError::Unavailable("background project reader identity unavailable".into())
            }
            _ => AppError::Unauthorized,
        })?;
    let registered = std::env::var("TT_BACKGROUND__FLEET_READER_SUBJECTS")
        .unwrap_or_default()
        .split(',')
        .any(|subject| subject == verified.context.user_id);
    if !registered
        || verified.subject_role.as_deref() != Some("service_account")
        || verified.context.session_id.is_some()
        || !verified.context.has_scope("task-tracker:background:read")
        || query.owner_issuer != verified.issuer
    {
        return Err(AppError::Forbidden);
    }
    if !verified.lifetime.is_current() {
        return Err(AppError::Unauthorized);
    }
    let owner =
        app::native_space_access::background_owner_access(query.owner_subject, &query.owner_issuer)
            .await?;
    let profile = ctx
        .repos
        .users
        .central_profiles(&[query.owner_subject.to_string()])
        .await?
        .remove(&query.owner_subject.to_string())
        .filter(|profile| profile.is_active)
        .ok_or(AppError::Forbidden)?;
    let project = ctx
        .repos
        .projects
        .get_by_id(ProjectId::from_uuid(project_id))
        .await?;
    let member = match ctx.repos.members.get(project.id, profile.id).await {
        Ok(value) => Some(value.role),
        Err(AppError::NotFound(_)) => None,
        Err(error) => return Err(error),
    };
    ctx.repos.projects.require_writable(project.id).await?;
    let (manage, _) = app::background::permissions(
        project.owner_id == profile.id,
        member.as_ref(),
        owner.decision.role == HumanRole::Admin,
        true,
    );
    if !manage {
        return Err(AppError::Forbidden);
    }
    let tracker_instance_id = std::env::var("TT_NAMESPACE__INSTANCE_ID")
        .ok()
        .and_then(|value| value.parse::<Uuid>().ok())
        .filter(|id| !id.is_nil())
        .ok_or_else(|| AppError::Unavailable("background project origin not configured".into()))?;
    let mut repositories = vec![];
    let mut source_revision = None;
    if query.sources {
        let service = ctx.sdlc.as_ref().ok_or_else(|| {
            AppError::Unavailable("project source policy reader not configured".into())
        })?;
        let policy = service
            .source_repository()?
            .background_source_policy(project_id)
            .await?;
        let binding = ctx
            .repos
            .projects
            .namespace_binding(project.id)
            .await?
            .ok_or_else(|| AppError::conflict("unmanaged_project"))?;
        if policy.tracker_instance_id != tracker_instance_id.to_string()
            || policy.verification.namespace != binding.namespace
            || binding.state != "active"
        {
            return Err(AppError::Forbidden);
        }
        source_revision = Some(policy.source_revision);
        repositories.push(SourceGrant {
            repository: policy.source.repository_id,
            forge_instance_id: policy.source.forge_instance_id,
            branch: policy.source.base_ref,
            namespace: binding.namespace,
        });
    }
    if !owner.is_current() || !verified.lifetime.is_current() {
        return Err(AppError::Forbidden);
    }
    let until = chrono::DateTime::from_timestamp(verified.lifetime.local_valid_until(), 0)
        .ok_or(AppError::Unauthorized)?;
    Ok(Json(SourceAccess {
        schema: "fleet/background-source-access/v1",
        tracker_instance_id,
        project_id,
        project_name: project.name.to_string(),
        owner_subject: query.owner_subject,
        owner_issuer: query.owner_issuer,
        sources_included: query.sources,
        source_revision,
        repositories,
        valid_until: owner.decision.expires_at.min(until),
    }))
}
