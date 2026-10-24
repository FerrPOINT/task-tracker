//! Owner metadata endpoint with its own live human authorization; no JIT user write.
use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
};
use pdlc_auth_core::service_bridge::VerifiedBridgeError;
use serde::Deserialize;
use shared::AppError;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Deserialize, utoipa::IntoParams)]
#[serde(deny_unknown_fields)]
pub struct PageQuery {
    #[serde(default = "default_limit")]
    pub limit: u64,
    #[serde(default)]
    pub offset: u64,
}
fn default_limit() -> u64 {
    25
}

#[utoipa::path(get, path="/api/v2/identity-users/{subject}/memberships", tag="identity-administration", params(("subject"=Uuid,Path), PageQuery), responses((status=200,body=domain::identity_memberships::IdentityMembershipPage),(status=403,description="Fresh human platform administrator required"),(status=503,description="Owner projection unavailable")),security(("bearer"=[])))]
pub async fn get(
    State(ctx): State<Arc<app::AppContext>>,
    headers: HeaderMap,
    Path(subject): Path<Uuid>,
    Query(page): Query<PageQuery>,
) -> Result<Response, AppError> {
    domain::identity_memberships::validate_page(subject, page.limit, page.offset)?;
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(pdlc_auth_core::bearer_from_header)
        .ok_or(AppError::Unauthorized)?;
    let actor = app::space_access::CENTRAL_BRIDGE
        .try_verified_token(token)
        .await
        .map_err(|error| match error {
            VerifiedBridgeError::Unavailable | VerifiedBridgeError::NotConfigured => {
                AppError::Unavailable("central_identity_unavailable".into())
            }
            _ => AppError::Unauthorized,
        })?;
    let config = app::identity_memberships::IdentityManagementConfig::from_env()?;
    let result = app::identity_memberships::page(
        &ctx.repos,
        &actor,
        &config,
        subject,
        page.limit,
        page.offset,
    )
    .await?;
    let mut response = Json(result).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
