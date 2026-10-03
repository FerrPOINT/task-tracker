use super::sdlc::service;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    routing::{get, post},
};
use domain::{sdlc::Principal, sdlc_reservation::*};
use shared::AppError;
use std::sync::Arc;
use uuid::Uuid;

pub(super) fn router() -> Router<Arc<app::AppContext>> {
    Router::new()
        .route(
            "/issues/{id}/sdlc/analysis-reservation",
            get(current).post(reserve),
        )
        .route(
            "/issues/{id}/sdlc/analysis-reservation/heartbeat",
            post(heartbeat),
        )
        .route(
            "/issues/{id}/sdlc/analysis-reservation/operations/{idempotency_key}",
            get(operation),
        )
}
fn no_store() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h
}
#[utoipa::path(post, path="/api/v1/issues/{id}/sdlc/analysis-reservation",operation_id="reserve_analysis_reservation",tag="sdlc",params(("id"=Uuid,Path)),request_body=ReserveAnalysis,
    responses((status=201,body=AnalysisReservationReceipt),(status=200,body=AnalysisReservationReceipt,description="Original durable replay; no renewal"),(status=401),(status=403),(status=404),(status=409,description="PM quiescence unknown, stale intent/CAS or held capacity"),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn reserve(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    Json(command): Json<ReserveAnalysis>,
) -> Result<(StatusCode, HeaderMap, Json<AnalysisReservationReceipt>), AppError> {
    let (receipt, replay) = service(&ctx)?
        .reserve_analysis(task, &actor, command)
        .await?;
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
#[utoipa::path(post,path="/api/v1/issues/{id}/sdlc/analysis-reservation/heartbeat",operation_id="heartbeat_analysis_reservation",tag="sdlc",params(("id"=Uuid,Path)),request_body=HeartbeatAnalysis,
    responses((status=201,body=AnalysisReservationReceipt),(status=200,body=AnalysisReservationReceipt),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn heartbeat(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    Json(command): Json<HeartbeatAnalysis>,
) -> Result<(StatusCode, HeaderMap, Json<AnalysisReservationReceipt>), AppError> {
    let (receipt, replay) = service(&ctx)?
        .heartbeat_analysis(task, &actor, command)
        .await?;
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
#[cfg(test)]
mod tests {
    use utoipa::OpenApi;
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
            "ReserveAnalysis",
            "HeartbeatAnalysis",
            "PreparedAnalysisAssignment",
            "AnalysisReservationReceipt",
            "AnalysisReservationReadback",
        ] {
            assert_eq!(
                s["components"]["schemas"][name]["additionalProperties"], false,
                "{name}"
            );
        }
        for path in [
            "/api/v1/issues/{id}/sdlc/analysis-reservation",
            "/api/v1/issues/{id}/sdlc/analysis-reservation/operations/{idempotency_key}",
        ] {
            assert!(s["paths"][path]["get"]["security"].is_array());
        }
        assert!(
            s["components"]["schemas"]["PreparedAnalysisAssignment"]["properties"]
                .get("run_id")
                .is_none()
        );
        assert!(s["components"]["schemas"]["AnalysisReservationReadback"]["properties"]["reconciliation_needed"].is_object());
    }
}
