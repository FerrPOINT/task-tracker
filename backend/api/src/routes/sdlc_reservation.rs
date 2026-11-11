use super::sdlc::service;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    routing::get,
};
use domain::sdlc_configured_cli::*;
use domain::{sdlc::Principal, sdlc_reservation::*};
use shared::AppError;
use std::sync::Arc;
use uuid::Uuid;

pub(super) fn router() -> Router<Arc<app::AppContext>> {
    Router::new()
        .route("/issues/{id}/sdlc/analysis-reservation", get(current))
        .route(
            "/issues/{id}/sdlc/analysis-reservation/configuration-preflight",
            get(configuration_preflight),
        )
        .route(
            "/issues/{id}/sdlc/analysis-reservation/operations/{idempotency_key}",
            get(operation),
        )
        .route(
            "/issues/{id}/sdlc/analysis-result/operations/{idempotency_key}",
            get(result_operation),
        )
        .route(
            "/issues/{id}/sdlc/pm-draft-completion",
            get(pm_completion).post(complete_pm_draft),
        )
        .route(
            "/issues/{id}/sdlc/pm-draft-completion/operations/{idempotency_key}",
            get(pm_completion_operation),
        )
}
fn no_store() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h
}
#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/analysis-reservation/configuration-preflight",operation_id="read_analysis_configuration_preflight",tag="sdlc",params(("id"=Uuid,Path)),responses((status=200,body=domain::sdlc_configuration::AnalysisConfigurationPreflight),(status=401),(status=403),(status=404),(status=409),(status=503)),security(("bearer"=[])))]
pub async fn configuration_preflight(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
) -> Result<
    (
        HeaderMap,
        Json<domain::sdlc_configuration::AnalysisConfigurationPreflight>,
    ),
    AppError,
> {
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .analysis_configuration_preflight(task, &actor)
                .await?,
        ),
    ))
}
#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/analysis-reservation",operation_id="read_analysis_reservation",tag="sdlc",params(("id"=Uuid,Path)),responses((status=200,body=AnalysisReservationReadback),(status=401),(status=403),(status=404),(status=409),(status=503)),security(("bearer"=[])))]
pub async fn current(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
) -> Result<(HeaderMap, Json<AnalysisReservationReadback>), AppError> {
    Ok((
        no_store(),
        Json(service(&ctx)?.analysis_reservation(task, &actor).await?),
    ))
}
#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/analysis-reservation/operations/{idempotency_key}",operation_id="read_analysis_reservation_operation",tag="sdlc",params(("id"=Uuid,Path),("idempotency_key"=String,Path)),responses((status=200,body=AnalysisReservationOperation),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, key)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Json<AnalysisReservationOperation>), AppError> {
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .analysis_reservation_operation(task, &actor, &key)
                .await?,
        ),
    ))
}
#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/analysis-result/operations/{idempotency_key}",operation_id="read_analysis_result_operation",tag="sdlc",params(("id"=Uuid,Path),("idempotency_key"=String,Path)),responses((status=200,body=AnalysisResultOperation),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn result_operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, key)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Json<AnalysisResultOperation>), AppError> {
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .analysis_result_operation(task, &actor, &key)
                .await?,
        ),
    ))
}

#[utoipa::path(post,path="/api/v1/issues/{id}/sdlc/pm-draft-completion",operation_id="complete_pm_draft",tag="sdlc",params(("id"=Uuid,Path)),request_body=CompleteConfiguredPm,
    responses((status=201,body=ConfiguredPmCompletionReceipt),(status=200,body=ConfiguredPmCompletionReceipt,description="Original durable replay; no duplicate completion"),(status=401),(status=403),(status=404),(status=409,description="Stale business fence, terminal receipt or payload conflict"),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn complete_pm_draft(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    Json(command): Json<CompleteConfiguredPm>,
) -> Result<(StatusCode, HeaderMap, Json<ConfiguredPmCompletionReceipt>), AppError> {
    let svc = service(&ctx)?;
    let (receipt, replay) = svc.complete_configured_pm(task, &actor, command).await?;
    Ok((
        if replay {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        no_store(),
        Json(receipt),
    ))
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/pm-draft-completion",operation_id="read_pm_draft_completion",tag="sdlc",params(("id"=Uuid,Path)),responses((status=200,body=PmCompletionReadbackResponse),(status=401),(status=403),(status=404),(status=409),(status=503)),security(("bearer"=[])))]
pub async fn pm_completion(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
) -> Result<(HeaderMap, Json<PmCompletionReadbackResponse>), AppError> {
    let svc = service(&ctx)?;
    let state = svc.repository.pm_completion_state(task, &actor).await?;
    let result = if state.configured_cli() {
        let saved = svc
            .repository
            .configured_pm_completion(task, &actor, None)
            .await?;
        PmCompletionReadbackResponse::Configured(Box::new(ConfiguredPmCompletionReadback {
            interaction_protocol: domain::sdlc::InteractionProtocol::ConfiguredCliV1,
            current: saved.map(|s| s.result),
        }))
    } else {
        PmCompletionReadbackResponse::Legacy(Box::new(svc.pm_draft_completion(task, &actor).await?))
    };
    Ok((no_store(), Json(result)))
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/pm-draft-completion/operations/{idempotency_key}",operation_id="read_pm_draft_completion_operation",tag="sdlc",params(("id"=Uuid,Path),("idempotency_key"=String,Path)),responses((status=200,body=PmCompletionOperationResponse),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn pm_completion_operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, key)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Json<PmCompletionOperationResponse>), AppError> {
    let svc = service(&ctx)?;
    let state = svc.repository.pm_completion_state(task, &actor).await?;
    let result = if state.configured_cli() {
        let saved = svc
            .repository
            .configured_pm_completion(task, &actor, Some(&key))
            .await?
            .ok_or_else(|| AppError::not_found("PM completion operation", &key))?;
        PmCompletionOperationResponse::Configured(Box::new(saved))
    } else {
        PmCompletionOperationResponse::Legacy(Box::new(
            svc.pm_draft_completion_operation(task, &actor, &key)
                .await?,
        ))
    };
    Ok((no_store(), Json(result)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    use utoipa::OpenApi;

    #[tokio::test]
    async fn retired_analysis_writers_are_not_registered_and_current_routes_remain() {
        let mut config = shared::AppConfig::default();
        config.auth.jwt_secret = "synthetic-routing-test".into();
        let ctx = Arc::new(app::AppContext::new(
            Arc::new(config),
            Arc::new(domain::Repositories::default()),
            Arc::new(domain::InMemoryStorage::default()),
        ));
        // No SDLC service: retained GETs must reach its fail-closed unavailable boundary.
        assert!(ctx.sdlc.is_none());
        let routes = router()
            .merge(super::super::sdlc_delivery::router())
            .layer(Extension(Principal {
                subject: Uuid::new_v4().to_string(),
                human_session: false,
                trusted_human: false,
                scopes: Default::default(),
            }))
            .with_state(ctx);
        let task = Uuid::new_v4();
        for (method, suffix, expected) in [
            (
                "POST",
                "analysis-reservation",
                StatusCode::METHOD_NOT_ALLOWED,
            ),
            (
                "POST",
                "analysis-reservation/heartbeat",
                StatusCode::NOT_FOUND,
            ),
            ("POST", "analysis-result", StatusCode::NOT_FOUND),
            (
                "GET",
                "analysis-reservation",
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                "GET",
                "analysis-reservation/configuration-preflight",
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                "GET",
                "analysis-reservation/operations/original-key",
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                "GET",
                "analysis-result/operations/original-key",
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                "GET",
                "pm-draft-completion",
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                "GET",
                "pm-draft-completion/operations/original-key",
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            ("GET", "lifecycle", StatusCode::SERVICE_UNAVAILABLE),
            (
                "GET",
                "lifecycle/operations/original-key",
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                "POST",
                "pm-draft-completion",
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
            ("POST", "lifecycle", StatusCode::UNPROCESSABLE_ENTITY),
        ] {
            let response = routes
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(format!("/issues/{task}/sdlc/{suffix}"))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected, "{method} {suffix}");
        }
    }

    #[test]
    fn reservation_schema_is_prepared_only_with_strict_commands_and_readback() {
        let s = serde_json::to_value(crate::ApiDoc::openapi()).unwrap();
        let mut ids = std::collections::HashSet::new();
        for path in s["paths"].as_object().unwrap().values() {
            for method in path.as_object().unwrap().values() {
                if let Some(id) = method["operationId"].as_str() {
                    assert!(ids.insert(id), "duplicate operationId {id}");
                }
            }
        }
        for name in [
            "AnalysisConfigurationPreflight",
            "PreparedAnalysisAssignment",
            "AnalysisReservationReceipt",
            "AnalysisReservationReadback",
            "AnalysisResultReceipt",
            "AnalysisResultOperation",
            "CompleteConfiguredPm",
            "PmDraftCompletionReceipt",
            "PmDraftCompletionOperation",
            "PmDraftCompletionReadback",
        ] {
            assert_eq!(
                s["components"]["schemas"][name]["additionalProperties"], false,
                "{name}"
            );
        }
        for path in [
            "/api/v1/issues/{id}/sdlc/analysis-reservation",
            "/api/v1/issues/{id}/sdlc/analysis-reservation/operations/{idempotency_key}",
            "/api/v1/issues/{id}/sdlc/analysis-result/operations/{idempotency_key}",
            "/api/v1/issues/{id}/sdlc/pm-draft-completion",
            "/api/v1/issues/{id}/sdlc/pm-draft-completion/operations/{idempotency_key}",
        ] {
            assert!(s["paths"][path]["get"]["security"].is_array());
        }
        for path in [
            "/api/v1/issues/{id}/sdlc/analysis-reservation",
            "/api/v1/issues/{id}/sdlc/analysis-reservation/heartbeat",
            "/api/v1/issues/{id}/sdlc/analysis-result",
        ] {
            assert!(s["paths"][path].get("post").is_none(), "{path}");
        }
        assert!(s["paths"]["/api/v1/issues/{id}/sdlc/lifecycle"]["post"]["security"].is_array());
        assert!(
            s["paths"]["/api/v1/issues/{id}/sdlc/pm-draft-completion"]["post"]["security"]
                .is_array()
        );
        assert_eq!(
            s["components"]["schemas"]["AnalysisResultReceipt"]["properties"]["next_stage"]["$ref"],
            "#/components/schemas/Stage"
        );
        assert!(
            s["components"]["schemas"]["PreparedAnalysisAssignment"]["properties"]
                .get("run_id")
                .is_none()
        );
        let preflight = &s["components"]["schemas"]["AnalysisConfigurationPreflight"]["properties"];
        assert_eq!(
            preflight["runtime_ready"]["enum"],
            serde_json::json!([false])
        );
        assert_eq!(
            preflight["dispatch_allowed"]["enum"],
            serde_json::json!([false])
        );
        assert!(s["paths"]["/api/v1/issues/{id}/sdlc/analysis-reservation/configuration-preflight"]["get"]["security"].is_array());
        assert!(s["components"]["schemas"]["AnalysisReservationReadback"]["properties"]["reconciliation_needed"].is_object());
        assert_eq!(
            s["components"]["schemas"]["CompleteConfiguredPm"]["properties"]["interaction_protocol"]
                ["$ref"],
            serde_json::json!("#/components/schemas/InteractionProtocol")
        );
    }
}
