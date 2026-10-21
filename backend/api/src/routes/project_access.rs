//! Additive bounded contract; the legacy SDLC scope response remains unchanged.
use crate::middleware::auth::UserClaims;
use axum::{Extension, extract::State, http::HeaderMap, response::Response};
use pdlc_shared::project_access::ProjectAccessV2;
use shared::{AppError, UserId};
use std::sync::Arc;
#[utoipa::path(operation_id="get_project_access_v2",get,path="/api/v2/sdlc/project-access",
responses((status=200,body=ProjectAccessV2),(status=403,description="Human project admission required"),(status=503,description="Fresh confirmation unavailable")),security(("bearer"=[])))]
pub async fn get(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let user = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| AppError::Unauthorized)?;
    let frame = app::space_access::current_request_for(user)?;
    let actor = if let Some(actor) = frame.as_ref().and_then(|f| f.spaces()) {
        actor.clone()
    } else {
        let refs = ctx.repos.projects.space_catalog_refs().await?;
        super::project_spaces::actor(&ctx, &headers, &claims, "GET", &refs).await?
    };
    let readable = actor.readable_spaces()?;
    let active = if actor.permit().is_ok() {
        actor.active_spaces()?
    } else {
        Vec::new()
    };
    let (ids, writable) = ctx
        .repos
        .projects
        .admitted_project_ids(&readable, &active)
        .await?;
    let value = ProjectAccessV2 {
        schema_version: 2,
        identity: actor.identity().clone(),
        tracker_instance_id: std::env::var("TASKTRACKER_SDLC__INSTANCE_ID")
            .map_err(|_| AppError::Unavailable("sdlc_instance_not_configured".into()))?,
        canonical_tracker_instance_id: super::project_contexts::instance()?,
        policy_revision: actor.policy_revision(),
        validity: actor.read_validity()?,
        readable_spaces: readable,
        writable_spaces: active,
        project_ids: ids,
        writable_project_ids: writable,
    };
    if !value.valid() {
        return Err(AppError::Unavailable("invalid_project_access".into()));
    }
    super::project_spaces::no_store(value, &actor)
}
