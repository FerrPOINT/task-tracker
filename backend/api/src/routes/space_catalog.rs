//! Product-scoped human catalog read. Frontend never receives a machine credential.
use crate::middleware::auth::UserClaims;
use axum::{
    Extension, Json,
    extract::{Query, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
};
use pdlc_shared::space_catalog::{SpaceCatalogQuery, VisibleSpaceCatalogV2};
use shared::{AppError, UserId};
use std::sync::Arc;

#[utoipa::path(operation_id="get_visible_spaces_v2",get,path="/api/v2/spaces/visible",
params(
    ("limit"=Option<i64>,Query,minimum=1,maximum=10,example=10),
    ("offset"=Option<i64>,Query,minimum=0,example=0),
    ("q"=Option<String>,Query,max_length=200)
),
responses((status=200,body=VisibleSpaceCatalogV2),(status=401,description="Central human identity required"),
(status=403,description="Product read/original context denied"),(status=400,description="Malformed catalog query or bounds"),
(status=503,description="Fresh owner confirmation unavailable")),
security(("bearer"=[])))]
pub async fn visible(
    State(_ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Query(query): Query<SpaceCatalogQuery>,
) -> Result<Response, AppError> {
    if !query.valid() {
        return Err(AppError::invalid_input("invalid_space_catalog_query"));
    }
    let user = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| AppError::Unauthorized)?;
    let frame = app::space_access::current_request_for(user)?.ok_or(AppError::Unauthorized)?;
    let origin = frame.catalog_origin()?;
    let read = app::space_access::catalog_client()
        .await?
        .query_origin(&app::space_access::CENTRAL_BRIDGE, origin, &query)
        .await
        .map_err(|error| match error {
            pdlc_auth_core::service_bridge::SpaceAdmissionError::Expired => AppError::Forbidden,
            _ => AppError::Unavailable("space_catalog_unavailable".into()),
        })?;
    if !frame.is_current() {
        return Err(AppError::Forbidden);
    }
    let deadline = read.monotonic_deadline();
    let value = read.into_catalog().map_err(|_| AppError::Forbidden)?;
    let until = value.validity.valid_until;
    let mut response = Json(value).into_response();
    let headers: &mut HeaderMap = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, "private, no-store".parse().unwrap());
    headers.insert(
        "x-pdlc-admission-valid-until",
        until.to_string().parse().unwrap(),
    );
    crate::middleware::auth::restrict_response(&mut response, until, deadline);
    Ok(response)
}
