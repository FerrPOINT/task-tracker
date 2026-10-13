use super::sdlc::service;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    routing::{get, post},
};
use domain::{sdlc::Principal, sdlc_lifecycle::*};
use shared::AppError;
use std::sync::Arc;
use uuid::Uuid;

pub(super) fn router() -> Router<Arc<app::AppContext>> {
    Router::new()
        .route("/issues/{id}/sdlc/lifecycle", get(current).post(command))
        .route(
            "/issues/{id}/sdlc/lifecycle/operations/{idempotency_key}",
            get(operation),
        )
        .route(
            "/issues/{id}/sdlc/lifecycle/results/{result_id}",
            get(result),
        )
        .route("/issues/{id}/sdlc/delivery-proposals", post(propose))
        .route(
            "/issues/{id}/sdlc/delivery-proposals/operations/{idempotency_key}",
            get(proposal_operation),
        )
        .route("/issues/{id}/sdlc/pm-feedback", post(feedback))
        .route(
            "/issues/{id}/sdlc/pm-feedback/operations/{idempotency_key}",
            get(feedback_operation),
        )
}

fn no_store() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers
}

fn written<T>(result: (T, bool)) -> (StatusCode, HeaderMap, Json<T>) {
    (
        if result.1 {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        no_store(),
        Json(result.0),
    )
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/lifecycle",operation_id="read_single_task_lifecycle",tag="sdlc",
    params(("id"=Uuid,Path)),responses((status=200,body=LifecycleReadback),(status=401),(status=403),(status=404),(status=409),(status=503)),security(("bearer"=[])))]
pub async fn current(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
) -> Result<(HeaderMap, Json<LifecycleReadback>), AppError> {
    Ok((
        no_store(),
        Json(service(&ctx)?.lifecycle_read(task, &actor).await?),
    ))
}

#[utoipa::path(post,path="/api/v1/issues/{id}/sdlc/lifecycle",operation_id="single_task_lifecycle_command",tag="sdlc",
    params(("id"=Uuid,Path)),request_body=LifecycleCommand,
    responses((status=201,body=LifecycleReceipt),(status=200,body=LifecycleReceipt,description="Original durable replay"),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn command(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    Json(input): Json<LifecycleCommand>,
) -> Result<(StatusCode, HeaderMap, Json<LifecycleReceipt>), AppError> {
    Ok(written(
        service(&ctx)?
            .lifecycle_command(task, &actor, input)
            .await?,
    ))
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/lifecycle/operations/{idempotency_key}",operation_id="read_single_task_lifecycle_operation",tag="sdlc",
    params(("id"=Uuid,Path),("idempotency_key"=String,Path)),responses((status=200,body=LifecycleReceipt),(status=401),(status=403),(status=404),(status=409),(status=503)),security(("bearer"=[])))]
pub async fn operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, key)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Json<LifecycleReceipt>), AppError> {
    app::sdlc::validate_key(&key)?;
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .lifecycle_operation(task, &actor, &key)
                .await?,
        ),
    ))
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/lifecycle/results/{result_id}",operation_id="read_single_task_delivery_result",tag="sdlc",
    params(("id"=Uuid,Path),("result_id"=Uuid,Path)),responses((status=200,body=DeliveryInput),(status=401),(status=403),(status=404),(status=409),(status=503)),security(("bearer"=[])))]
pub async fn result(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, id)): Path<(Uuid, Uuid)>,
) -> Result<(HeaderMap, Json<DeliveryInput>), AppError> {
    Ok((
        no_store(),
        Json(service(&ctx)?.lifecycle_result(task, &actor, id).await?),
    ))
}

#[utoipa::path(post,path="/api/v1/issues/{id}/sdlc/delivery-proposals",operation_id="submit_single_task_delivery_proposal",tag="sdlc",
    params(("id"=Uuid,Path)),request_body=SubmitDeliveryProposal,
    responses((status=201,body=DeliveryProposalReceipt),(status=200,body=DeliveryProposalReceipt,description="Original durable replay"),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn propose(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    Json(input): Json<SubmitDeliveryProposal>,
) -> Result<(StatusCode, HeaderMap, Json<DeliveryProposalReceipt>), AppError> {
    Ok(written(
        service(&ctx)?
            .submit_delivery_proposal(task, &actor, input)
            .await?,
    ))
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/delivery-proposals/operations/{idempotency_key}",operation_id="read_single_task_delivery_proposal",tag="sdlc",
    params(("id"=Uuid,Path),("idempotency_key"=String,Path)),responses((status=200,body=DeliveryProposalReceipt),(status=401),(status=403),(status=404),(status=409),(status=503)),security(("bearer"=[])))]
pub async fn proposal_operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, key)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Json<DeliveryProposalReceipt>), AppError> {
    app::sdlc::validate_key(&key)?;
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .delivery_proposal_operation(task, &actor, &key)
                .await?,
        ),
    ))
}

#[utoipa::path(post,path="/api/v1/issues/{id}/sdlc/pm-feedback",operation_id="submit_single_task_pm_feedback",tag="sdlc",
    params(("id"=Uuid,Path)),request_body=SubmitPmFeedback,
    responses((status=201,body=PmFeedbackReceipt),(status=200,body=PmFeedbackReceipt,description="Original durable replay"),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn feedback(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    Json(input): Json<SubmitPmFeedback>,
) -> Result<(StatusCode, HeaderMap, Json<PmFeedbackReceipt>), AppError> {
    Ok(written(
        service(&ctx)?
            .submit_pm_feedback(task, &actor, input)
            .await?,
    ))
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/pm-feedback/operations/{idempotency_key}",operation_id="read_single_task_pm_feedback",tag="sdlc",
    params(("id"=Uuid,Path),("idempotency_key"=String,Path)),responses((status=200,body=PmFeedbackReceipt),(status=401),(status=403),(status=404),(status=409),(status=503)),security(("bearer"=[])))]
pub async fn feedback_operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, key)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Json<PmFeedbackReceipt>), AppError> {
    app::sdlc::validate_key(&key)?;
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .pm_feedback_operation(task, &actor, &key)
                .await?,
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::OpenApi;

    #[test]
    fn every_lifecycle_surface_is_documented_with_auth_and_typed_readback() {
        let spec = serde_json::to_value(crate::ApiDoc::openapi()).unwrap();
        for (path, method) in [
            ("/api/v1/issues/{id}/sdlc/lifecycle", "get"),
            ("/api/v1/issues/{id}/sdlc/lifecycle", "post"),
            (
                "/api/v1/issues/{id}/sdlc/lifecycle/operations/{idempotency_key}",
                "get",
            ),
            (
                "/api/v1/issues/{id}/sdlc/lifecycle/results/{result_id}",
                "get",
            ),
            ("/api/v1/issues/{id}/sdlc/delivery-proposals", "post"),
            (
                "/api/v1/issues/{id}/sdlc/delivery-proposals/operations/{idempotency_key}",
                "get",
            ),
            ("/api/v1/issues/{id}/sdlc/pm-feedback", "post"),
            (
                "/api/v1/issues/{id}/sdlc/pm-feedback/operations/{idempotency_key}",
                "get",
            ),
        ] {
            assert!(spec["paths"][path][method]["security"].is_array(), "{path}");
            assert!(
                spec["paths"][path][method]["responses"]["403"].is_object(),
                "{path}"
            );
        }
        let schemas = &spec["components"]["schemas"];
        assert_eq!(schemas["LifecycleReadback"]["additionalProperties"], false);
        for field in ["observed_at", "receipt"] {
            assert!(
                schemas["LifecycleReadback"]["required"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!(field))
            );
        }
        assert_eq!(no_store()[header::CACHE_CONTROL], "no-store");
        assert_eq!(written(((), true)).0, StatusCode::OK);
        assert_eq!(written(((), false)).0, StatusCode::CREATED);
    }
}
