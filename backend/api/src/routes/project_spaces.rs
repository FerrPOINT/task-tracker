//! V2 project/space operations require central admission; legacy defaults are never guessed.
use super::projects::map_project_response;
use crate::{
    dto::ProjectResponse,
    middleware::{auth::UserClaims, central_auth::BRIDGE},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use domain::project_spaces::ReplaceProjectSpaces;
use pdlc_auth_core::service_bridge::SpaceAdmissionClient;
use shared::{AppError, ProjectKey, UserId, space_access::*};
use std::sync::Arc;

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateProjectInSpaces {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    #[schema(min_items = 1, max_items = 1000)]
    pub spaces: Vec<SpaceRef>,
}

pub(crate) async fn client() -> Result<&'static SpaceAdmissionClient, AppError> {
    app::space_access::admission_client().await
}

pub(crate) fn key(value: &str) -> Result<ProjectKey, AppError> {
    let key = ProjectKey::new(value);
    if key.is_valid() {
        Ok(key)
    } else {
        Err(AppError::invalid_input("project_key"))
    }
}
fn bounded_set(spaces: &[SpaceRef]) -> Result<(), AppError> {
    domain::project_spaces::validate_space_set(spaces)?;
    if spaces.len() > SpaceAdmissionRequest::MAX_SPACES {
        return Err(AppError::invalid_input("too_many_project_spaces"));
    }
    Ok(())
}
pub(crate) async fn actor(
    ctx: &app::AppContext,
    headers: &HeaderMap,
    claims: &UserClaims,
    method: &str,
    spaces: &[SpaceRef],
) -> Result<app::space_access::SpaceActor, AppError> {
    let user = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| AppError::Unauthorized)?;
    if let Some(frame) = app::space_access::current_request_for(user)? {
        if let Some(actor) = frame.spaces() {
            return Ok(actor.clone());
        }
    }
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| {
            h.strip_prefix("Bearer ")
                .or_else(|| h.strip_prefix("bearer "))
        })
        .ok_or(AppError::Unauthorized)?;
    let verified = BRIDGE
        .try_verified_token(token)
        .await
        .map_err(|error| match error {
            pdlc_auth_core::service_bridge::VerifiedBridgeError::Unavailable
            | pdlc_auth_core::service_bridge::VerifiedBridgeError::NotConfigured => {
                AppError::Unavailable("central_confirmation_unavailable".into())
            }
            _ => AppError::Unauthorized,
        })?;
    if verified.subject_role.as_deref() == Some("service_account")
        || crate::routes::namespace::human_route_machine(&verified.context.user_id)
        || !verified.context.allows_service("task-tracker", method)
    {
        return Err(AppError::Forbidden);
    }
    let user = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| AppError::Unauthorized)?;
    let mut profiles = ctx
        .repos
        .users
        .central_profiles(&[verified.context.user_id.clone()])
        .await?;
    if !profiles
        .remove(&verified.context.user_id)
        .is_some_and(|profile| profile.id == user && profile.is_active)
    {
        return Err(AppError::Unauthorized);
    }
    let admission = client()
        .await?
        .confirm_many(&BRIDGE, &verified, spaces)
        .await
        .map_err(|_| AppError::Unavailable("space_admission_unavailable".into()))?;
    app::space_access::SpaceActor::verified(user, &verified, admission)
}
pub(crate) fn no_store<T: serde::Serialize>(
    value: T,
) -> ([(axum::http::header::HeaderName, &'static str); 1], Json<T>) {
    (
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(value),
    )
}

#[utoipa::path(operation_id="create_project_in_spaces_v2",post,path="/api/v2/projects",request_body=CreateProjectInSpaces,responses((status=201,body=ProjectResponse),(status=400,description="Invalid project or space set"),(status=401,description="Central human required"),(status=403,description="Project write admission denied"),(status=409,description="Project key or lifecycle conflict"),(status=422,description="Invalid strict request"),(status=503,description="Fresh owner confirmation unavailable")),security(("bearer"=[])))]
pub async fn create(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    headers: HeaderMap,
    Json(request): Json<CreateProjectInSpaces>,
) -> Result<
    (
        StatusCode,
        [(axum::http::header::HeaderName, &'static str); 1],
        Json<ProjectResponse>,
    ),
    AppError,
> {
    bounded_set(&request.spaces)?;
    let key = key(&request.key)?;
    let actor = actor(&ctx, &headers, &claims, "POST", &request.spaces).await?;
    let command = app::commands::CreateProjectCommand {
        key,
        name: request.name,
        description: request.description,
        owner_id: actor.user(),
    };
    let result = ctx
        .services
        .project
        .create_in_spaces(command, request.spaces, actor)
        .await?;
    let (headers, value) = no_store(map_project_response(result));
    Ok((StatusCode::CREATED, headers, value))
}

#[utoipa::path(operation_id="get_project_spaces_v2",get,path="/api/v2/projects/{project_key}/spaces",params(("project_key"=String,Path)),responses((status=200,body=ConfirmedProjectSpaces),(status=400,description="Invalid project key"),(status=401,description="Central human required"),(status=403,description="Project read admission denied"),(status=404,description="Project unavailable"),(status=503,description="Fresh owner confirmation unavailable")),security(("bearer"=[])))]
pub async fn get(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    headers: HeaderMap,
    Path(value): Path<String>,
) -> Result<
    (
        [(axum::http::header::HeaderName, &'static str); 1],
        Json<ConfirmedProjectSpaces>,
    ),
    AppError,
> {
    let key = key(&value)?;
    let project = ctx.repos.projects.get_by_key(&key).await?;
    let links = ctx.repos.projects.project_spaces(project.id).await?;
    let actor = actor(&ctx, &headers, &claims, "GET", &links.spaces).await?;
    Ok(no_store(ctx.services.project.spaces(&key, &actor).await?))
}

#[utoipa::path(operation_id="replace_project_spaces_v2",put,path="/api/v2/projects/{project_key}/spaces",params(("project_key"=String,Path)),request_body=ReplaceProjectSpaces,responses((status=200,body=ConfirmedProjectSpaces),(status=409,description="Link revision or lifecycle conflict")),security(("bearer"=[])))]
pub async fn replace(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    headers: HeaderMap,
    Path(value): Path<String>,
    Json(change): Json<ReplaceProjectSpaces>,
) -> Result<
    (
        [(axum::http::header::HeaderName, &'static str); 1],
        Json<ConfirmedProjectSpaces>,
    ),
    AppError,
> {
    change.validate()?;
    bounded_set(&change.spaces)?;
    let key = key(&value)?;
    let project = ctx.repos.projects.get_by_key(&key).await?;
    let mut spaces = ctx.repos.projects.project_spaces(project.id).await?.spaces;
    spaces.extend(change.spaces.iter().cloned());
    spaces.sort_by_key(|space| (space.registry_instance_id, space.space_id));
    spaces.dedup();
    let actor = actor(&ctx, &headers, &claims, "PUT", &spaces).await?;
    Ok(no_store(
        ctx.services
            .project
            .replace_spaces(&key, change, &actor)
            .await?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn create_contract_has_no_implicit_or_primary_space() {
        assert!(
            serde_json::from_value::<CreateProjectInSpaces>(
                serde_json::json!({"key":"QA","name":"QA"})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<CreateProjectInSpaces>(
                serde_json::json!({"key":"QA","name":"QA","spaces":[],"primary_space":"guess"})
            )
            .is_err()
        );
        let empty: CreateProjectInSpaces =
            serde_json::from_value(serde_json::json!({"key":"QA","name":"QA","spaces":[]}))
                .unwrap();
        assert!(bounded_set(&empty.spaces).is_err());
    }
}

#[utoipa::path(operation_id="get_project_space_projection_v2",get,path="/api/v2/projects/{project_key}/spaces/projection",params(("project_key"=String,Path)),responses((status=200,body=domain::project_spaces::ProjectSpaceProjectionStatus)),security(("bearer"=[])))]
pub async fn projection_status(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    headers: HeaderMap,
    Path(value): Path<String>,
) -> Result<
    (
        [(axum::http::header::HeaderName, &'static str); 1],
        Json<domain::project_spaces::ProjectSpaceProjectionStatus>,
    ),
    AppError,
> {
    let key = key(&value)?;
    let project = ctx.repos.projects.get_by_key(&key).await?;
    let links = ctx.repos.projects.project_spaces(project.id).await?;
    let actor = actor(&ctx, &headers, &claims, "GET", &links.spaces).await?;
    actor.require_read(&links)?;
    let status = ctx
        .repos
        .projects
        .project_space_projection_status(project.id)
        .await?;
    actor.require_read(&links)?;
    Ok(no_store(status))
}
#[utoipa::path(operation_id="reconcile_project_space_projection_v2",post,path="/api/v2/projects/{project_key}/spaces/projection/reconcile",params(("project_key"=String,Path)),responses((status=200,body=domain::project_spaces::ProjectSpaceProjectionStatus),(status=202,body=domain::project_spaces::ProjectSpaceProjectionStatus,description="Durable delivery remains pending")),security(("bearer"=[])))]
pub async fn reconcile_projection(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    headers: HeaderMap,
    Path(value): Path<String>,
) -> Result<
    (
        StatusCode,
        [(axum::http::header::HeaderName, &'static str); 1],
        Json<domain::project_spaces::ProjectSpaceProjectionStatus>,
    ),
    AppError,
> {
    let key = key(&value)?;
    let project = ctx.repos.projects.get_by_key(&key).await?;
    let links = ctx.repos.projects.project_spaces(project.id).await?;
    let actor = actor(&ctx, &headers, &claims, "POST", &links.spaces).await?;
    actor.require_read(&links)?;
    if project.owner_id != actor.user() {
        return Err(AppError::Forbidden);
    }
    let status = ctx
        .repos
        .projects
        .reconcile_project_spaces(project.id, &actor.permit()?)
        .await?;
    actor.require_read(&links)?;
    let (headers, body) = no_store(status.clone());
    Ok((
        if status.confirmed {
            StatusCode::OK
        } else {
            StatusCode::ACCEPTED
        },
        headers,
        body,
    ))
}
