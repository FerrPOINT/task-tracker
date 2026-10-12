use super::sdlc::service;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    routing::get,
};
use domain::{
    sdlc::Principal,
    sdlc_routing::{RoutingPolicy, SetRoutingPolicy, TaskRoutingSnapshot},
};
use shared::AppError;
use std::sync::Arc;
use uuid::Uuid;

pub(super) fn router() -> Router<Arc<app::AppContext>> {
    Router::new()
        .route(
            "/projects/{project_id}/sdlc/routing-policy",
            get(current).post(set),
        )
        .route(
            "/projects/{project_id}/sdlc/routing-policy/versions/{version}",
            get(version),
        )
        .route(
            "/projects/{project_id}/sdlc/routing-policy/operations/{idempotency_key}",
            get(operation),
        )
        .route("/issues/{id}/sdlc/routing-snapshot", get(snapshot))
}

fn no_store() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers
}

#[utoipa::path(post, path="/api/v1/projects/{project_id}/sdlc/routing-policy", tag="sdlc",
    params(("project_id"=Uuid, Path)), request_body=SetRoutingPolicy,
    responses((status=201, body=RoutingPolicy), (status=200, body=RoutingPolicy, description="Exact durable replay"),
        (status=401), (status=403, description="Central human session and active project owner required"),
        (status=404), (status=409, description="Stale CAS or changed idempotency payload"), (status=422), (status=503)), security(("bearer"=[])))]
pub async fn set(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(project): Path<Uuid>,
    Json(command): Json<SetRoutingPolicy>,
) -> Result<(StatusCode, HeaderMap, Json<RoutingPolicy>), AppError> {
    let (policy, replay) = service(&ctx)?
        .set_routing_policy(project, &actor, command)
        .await?;
    Ok((
        if replay {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        no_store(),
        Json(policy),
    ))
}

#[utoipa::path(get, path="/api/v1/projects/{project_id}/sdlc/routing-policy", tag="sdlc",
    params(("project_id"=Uuid, Path)), responses((status=200, body=RoutingPolicy), (status=401), (status=403), (status=404), (status=503)), security(("bearer"=[])))]
pub async fn current(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(project): Path<Uuid>,
) -> Result<(HeaderMap, Json<RoutingPolicy>), AppError> {
    Ok((
        no_store(),
        Json(service(&ctx)?.routing_policy(project, &actor, None).await?),
    ))
}

#[utoipa::path(get, path="/api/v1/projects/{project_id}/sdlc/routing-policy/versions/{version}", tag="sdlc",
    params(("project_id"=Uuid, Path), ("version"=i64, Path)), responses((status=200, body=RoutingPolicy), (status=401), (status=403), (status=404), (status=422), (status=503)), security(("bearer"=[])))]
pub async fn version(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((project, version)): Path<(Uuid, i64)>,
) -> Result<(HeaderMap, Json<RoutingPolicy>), AppError> {
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .routing_policy(project, &actor, Some(version))
                .await?,
        ),
    ))
}

#[utoipa::path(get, path="/api/v1/projects/{project_id}/sdlc/routing-policy/operations/{idempotency_key}", tag="sdlc",
    params(("project_id"=Uuid, Path), ("idempotency_key"=String, Path)), responses((status=200, body=RoutingPolicy), (status=401), (status=403), (status=404), (status=422), (status=503)), security(("bearer"=[])))]
pub async fn operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((project, key)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Json<RoutingPolicy>), AppError> {
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .routing_policy_operation(project, &actor, &key)
                .await?,
        ),
    ))
}

#[utoipa::path(get, path="/api/v1/issues/{id}/sdlc/routing-snapshot", tag="sdlc",
    params(("id"=Uuid, Path)), responses((status=200, body=TaskRoutingSnapshot), (status=401), (status=403),
        (status=404, description="No explicit routed publication; legacy remains untouched"), (status=409), (status=503)), security(("bearer"=[])))]
pub async fn snapshot(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
) -> Result<(HeaderMap, Json<TaskRoutingSnapshot>), AppError> {
    Ok((
        no_store(),
        Json(service(&ctx)?.task_routing_snapshot(task, &actor).await?),
    ))
}

#[cfg(test)]
mod tests {
    use utoipa::OpenApi;
    #[test]
    fn routing_owner_schema_has_strict_cas_readback_and_seven_roles() {
        let schema = serde_json::to_value(crate::ApiDoc::openapi()).unwrap();
        let components = &schema["components"]["schemas"];
        assert_eq!(components["RoleRoutes"]["additionalProperties"], false);
        assert_eq!(
            components["RoleRoutes"]["required"]
                .as_array()
                .unwrap()
                .len(),
            7
        );
        assert_eq!(components["RoleRoute"]["additionalProperties"], false);
        assert!(
            components["RoleRoute"]["properties"]
                .get("runtime_ready")
                .is_none()
        );
        assert_eq!(
            components["SetRoutingPolicy"]["additionalProperties"],
            false
        );
        for path in [
            "/api/v1/projects/{project_id}/sdlc/routing-policy",
            "/api/v1/projects/{project_id}/sdlc/routing-policy/versions/{version}",
            "/api/v1/projects/{project_id}/sdlc/routing-policy/operations/{idempotency_key}",
            "/api/v1/issues/{id}/sdlc/routing-snapshot",
        ] {
            assert!(
                schema["paths"][path]["get"]["security"].is_array(),
                "{path}"
            );
        }
        assert!(schema["paths"]["/api/v1/projects/{project_id}/sdlc/routing-policy"]["post"]["responses"]["409"].is_object());
        let legacy: domain::sdlc::ConfirmCommand = serde_json::from_value(
            serde_json::json!({"content_hash":"a".repeat(64),"idempotency_key":"legacy"}),
        )
        .unwrap();
        assert!(legacy.expected_routing_policy_version.is_none());
        assert!(
            serde_json::from_value::<domain::sdlc_routing::SetRoutingPolicy>(
                serde_json::json!({"routes":{},"idempotency_key":"missing-cas"})
            )
            .is_err()
        );
    }
}
