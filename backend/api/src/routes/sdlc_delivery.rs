use super::sdlc::service;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    routing::{get, post},
};
use domain::{sdlc::Principal, sdlc_critical_blockers::*, sdlc_lifecycle::*};
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
            "/issues/{id}/sdlc/critical-blockers",
            post(critical_propose),
        )
        .route(
            "/issues/{id}/sdlc/critical-blockers/{blocker_id}",
            get(critical_detail),
        )
        .route(
            "/issues/{id}/sdlc/critical-blockers/operations/{idempotency_key}",
            get(critical_operation),
        )
        .route(
            "/issues/{id}/sdlc/delivery-proposals/operations/{idempotency_key}",
            get(proposal_operation),
        )
        .route("/issues/{id}/sdlc/pm-feedback", post(feedback))
        .route(
            "/issues/{id}/sdlc/pm-feedback/operations/{idempotency_key}",
            get(feedback_operation),
        )
        .route(
            "/issues/{id}/sdlc/pm-feedback/messages/{message_id}",
            get(feedback_message),
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

#[utoipa::path(post,path="/api/v1/issues/{id}/sdlc/critical-blockers",operation_id="submit_critical_business_blocker",tag="sdlc",
    params(("id"=Uuid,Path)),request_body=SubmitCriticalBlocker,
    responses((status=201,body=CriticalBlockerReceipt),(status=200,body=CriticalBlockerReceipt,description="Original durable replay"),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn critical_propose(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    Json(input): Json<SubmitCriticalBlocker>,
) -> Result<(StatusCode, HeaderMap, Json<CriticalBlockerReceipt>), AppError> {
    Ok(written(
        service(&ctx)?
            .submit_critical_blocker(task, &actor, input)
            .await?,
    ))
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/critical-blockers/{blocker_id}",operation_id="read_critical_business_blocker",tag="sdlc",
    params(("id"=Uuid,Path),("blocker_id"=Uuid,Path)),responses((status=200,body=CriticalBlockerReceipt),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn critical_detail(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, blocker)): Path<(Uuid, Uuid)>,
) -> Result<(HeaderMap, Json<CriticalBlockerReceipt>), AppError> {
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .critical_blocker(task, &actor, blocker)
                .await?,
        ),
    ))
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/critical-blockers/operations/{idempotency_key}",operation_id="read_critical_business_blocker_operation",tag="sdlc",
    params(("id"=Uuid,Path),("idempotency_key"=String,Path)),responses((status=200,body=CriticalBlockerReceipt),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn critical_operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, key)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Json<CriticalBlockerReceipt>), AppError> {
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .critical_blocker_operation(task, &actor, &key)
                .await?,
        ),
    ))
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

// Parent registers GET /issues/{id}/sdlc/pm-feedback/messages/{message_id}
// with get(feedback_message) here and adds this handler to ApiDoc in lib.rs.
#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/pm-feedback/messages/{message_id}",operation_id="read_single_task_pm_feedback_message",tag="sdlc",
    params(("id"=Uuid,Path),("message_id"=Uuid,Path)),responses((status=200,body=PmFeedbackReceipt),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn feedback_message(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, message_id)): Path<(Uuid, Uuid)>,
) -> Result<(HeaderMap, Json<PmFeedbackReceipt>), AppError> {
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .pm_feedback_message(task, &actor, message_id)
                .await?,
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::OpenApi;

    #[test]
    fn saved_feedback_handler_contract_is_scoped_get_and_original_receipt() {
        #[derive(utoipa::OpenApi)]
        #[openapi(paths(feedback_message))]
        struct FeedbackMessageContract;
        let spec = serde_json::to_value(FeedbackMessageContract::openapi()).unwrap();
        let path = &spec["paths"]["/api/v1/issues/{id}/sdlc/pm-feedback/messages/{message_id}"];
        assert!(path["get"]["security"].is_array());
        assert!(path.get("post").is_none());
        assert_eq!(
            path["get"]["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/PmFeedbackReceipt"
        );
        for status in ["401", "403", "404", "409"] {
            assert!(path["get"]["responses"][status].is_object());
        }
        assert_eq!(no_store()[header::CACHE_CONTROL], "no-store");
    }

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
            ("/api/v1/issues/{id}/sdlc/critical-blockers", "post"),
            (
                "/api/v1/issues/{id}/sdlc/critical-blockers/{blocker_id}",
                "get",
            ),
            (
                "/api/v1/issues/{id}/sdlc/critical-blockers/operations/{idempotency_key}",
                "get",
            ),
            (
                "/api/v1/issues/{id}/sdlc/delivery-proposals/operations/{idempotency_key}",
                "get",
            ),
            ("/api/v1/issues/{id}/sdlc/pm-feedback", "post"),
            (
                "/api/v1/issues/{id}/sdlc/pm-feedback/operations/{idempotency_key}",
                "get",
            ),
            (
                "/api/v1/issues/{id}/sdlc/requirements/{revision}/confirmation",
                "get",
            ),
            (
                "/api/v1/issues/{id}/sdlc/pm-draft-execution-lease/resume",
                "post",
            ),
            (
                "/api/v1/issues/{id}/sdlc/pm-draft-execution-lease/resume/operations/{idempotency_key}",
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
        assert_eq!(
            schemas["ResumePmExecutionLease"]["additionalProperties"],
            false
        );
        for field in [
            "input",
            "checkpoint",
            "previous_run_id",
            "previous_run_terminal",
            "expected_lease_version",
        ] {
            assert!(
                schemas["ResumePmExecutionLease"]["required"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!(field))
            );
        }
        for field in ["observed_at", "receipt", "runtime_state"] {
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
