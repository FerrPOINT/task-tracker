use super::sdlc::service;
use axum::{
    Extension, Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use domain::sdlc_stage_status::{StageStartReceipt, StartStage};
use domain::{sdlc::Principal, sdlc_critical_blockers::*, sdlc_lifecycle::*};
use shared::AppError;
use std::sync::Arc;
use uuid::Uuid;

pub(super) fn router() -> Router<Arc<app::AppContext>> {
    Router::new()
        .route("/issues/{id}/sdlc/stage-starts", post(start_stage))
        .route(
            "/issues/{id}/sdlc/stage-starts/operations/{idempotency_key}",
            get(stage_start_operation),
        )
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
            "/issues/{id}/sdlc/critical-blockers/returns",
            post(critical_return),
        )
        .route(
            "/issues/{id}/sdlc/critical-blockers/returns/operations/{idempotency_key}",
            get(critical_return_operation),
        )
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

const START_SHAPE: &str = "Invalid StartStage JSON: expected fence (kind=pm or delivery, with the corresponding assignment fields) and idempotency_key. Read the current assignment before constructing the request.";
const PROPOSAL_SHAPE: &str = "Invalid SubmitDeliveryProposal JSON: expected fence, workflow_receipt, idempotency_key and result. result is a flat object with role and the corresponding fields: analyst (analysis), architect (design), developer (commit_sha, changes, checks), reviewer (verdict, report), tester (verdict, report), or devops (deployment, verification). Artifact receipts contain reference and sha256.";

fn request_json_error(error: JsonRejection, shape: &'static str) -> (HeaderMap, Response) {
    let status = error.status();
    let message = match error {
        JsonRejection::JsonDataError(_) => shape,
        JsonRejection::JsonSyntaxError(_) => {
            "Invalid JSON syntax: serialize the request with a JSON library."
        }
        JsonRejection::MissingJsonContentType(_) => "Expected Content-Type: application/json.",
        _ => "Unable to read the JSON request body.",
    };
    let mut response = AppError::validation(message).into_response();
    *response.status_mut() = status;
    (no_store(), response)
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

#[utoipa::path(post,path="/api/v1/issues/{id}/sdlc/stage-starts",operation_id="start_assigned_stage_work",tag="sdlc",
    params(("id"=Uuid,Path)),request_body=StartStage,
    responses((status=201,body=StageStartReceipt),(status=200,body=StageStartReceipt,description="Original durable replay"),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn start_stage(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    input: Result<Json<StartStage>, JsonRejection>,
) -> Result<(StatusCode, HeaderMap, Json<StageStartReceipt>), (HeaderMap, Response)> {
    let Json(input) = input.map_err(|error| request_json_error(error, START_SHAPE))?;
    Ok(written(
        service(&ctx)
            .map_err(|error| (no_store(), error.into_response()))?
            .start_stage(task, &actor, input)
            .await
            .map_err(|error| (no_store(), error.into_response()))?,
    ))
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/stage-starts/operations/{idempotency_key}",operation_id="read_assigned_stage_start_operation",tag="sdlc",
    params(("id"=Uuid,Path),("idempotency_key"=String,Path)),responses((status=200,body=StageStartReceipt),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn stage_start_operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, key)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Json<StageStartReceipt>), AppError> {
    app::sdlc::validate_key(&key)?;
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .stage_start_operation(task, &actor, &key)
                .await?,
        ),
    ))
}

// Parent registers both on the EXISTING strict-Central SDLC router (never the generic issues router).
#[utoipa::path(post,path="/api/v1/issues/{id}/sdlc/critical-blockers/returns",operation_id="return_critical_business_blocker_to_pm",tag="sdlc",
    params(("id"=Uuid,Path)),request_body=ReturnCriticalToPm,
    responses((status=201,body=CriticalPolicyChange),(status=200,body=CriticalPolicyChange,description="Original durable return replay"),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn critical_return(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    input: Result<Json<ReturnCriticalToPm>, axum::extract::rejection::JsonRejection>,
) -> Result<(StatusCode, HeaderMap, Json<CriticalPolicyChange>), (HeaderMap, Response)> {
    let Json(input) = input.map_err(|error| (no_store(), error.into_response()))?;
    let result = service(&ctx)
        .map_err(|error| (no_store(), error.into_response()))?
        .return_critical_to_pm(task, &actor, input)
        .await
        .map_err(|error| (no_store(), error.into_response()))?;
    Ok(written(result))
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/critical-blockers/returns/operations/{idempotency_key}",operation_id="read_critical_business_blocker_return_operation",tag="sdlc",
    params(("id"=Uuid,Path),("idempotency_key"=String,Path)),responses((status=200,body=CriticalPolicyChange),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn critical_return_operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, key)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Json<CriticalPolicyChange>), (HeaderMap, Response)> {
    let result = service(&ctx)
        .map_err(|error| (no_store(), error.into_response()))?
        .critical_pm_return_operation(task, &actor, &key)
        .await
        .map_err(|error| (no_store(), error.into_response()))?;
    Ok((no_store(), Json(result)))
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
    input: Result<Json<SubmitDeliveryProposal>, JsonRejection>,
) -> Result<(StatusCode, HeaderMap, Json<DeliveryProposalReceipt>), (HeaderMap, Response)> {
    let Json(input) = input.map_err(|error| request_json_error(error, PROPOSAL_SHAPE))?;
    Ok(written(
        service(&ctx)
            .map_err(|error| (no_store(), error.into_response()))?
            .submit_delivery_proposal(task, &actor, input)
            .await
            .map_err(|error| (no_store(), error.into_response()))?,
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
    fn lifecycle_schema_has_untimed_binding_and_no_heartbeat_command() {
        let spec = serde_json::to_value(crate::ApiDoc::openapi()).unwrap();
        let schemas = &spec["components"]["schemas"];
        let binding = &schemas["DeliveryBinding"];
        assert_eq!(binding["additionalProperties"], false);
        let required = binding["required"].as_array().unwrap();
        for field in ["lease_id", "version", "holder_subject", "claimed_at"] {
            assert!(required.contains(&serde_json::json!(field)));
        }
        for field in ["heartbeat_at", "expires_at"] {
            assert!(!required.contains(&serde_json::json!(field)));
        }
        let variants = schemas["LifecycleCommand"]["oneOf"].as_array().unwrap();
        let operations: std::collections::BTreeSet<_> = variants
            .iter()
            .map(|v| {
                v["properties"]["operation"]["enum"].as_array().unwrap()[0]
                    .as_str()
                    .unwrap()
            })
            .collect();
        assert_eq!(
            operations,
            [
                "start_analysis",
                "reconcile",
                "ensure_queued",
                "resume_with_current_routing",
                "reserve",
                "bind_run",
                "complete"
            ]
            .into_iter()
            .collect()
        );
        assert!(
            serde_json::from_value::<LifecycleCommand>(serde_json::json!({
                "operation":"heartbeat", "idempotency_key":"retired"
            }))
            .is_err()
        );
    }

    #[test]
    fn routing_resume_schema_and_request_require_both_explicit_cas_versions() {
        let spec = serde_json::to_value(crate::ApiDoc::openapi()).unwrap();
        let variants = spec["components"]["schemas"]["LifecycleCommand"]["oneOf"].as_array().unwrap();
        let resume_schema = variants.iter().find(|variant|
            variant["properties"]["operation"]["enum"][0] == "resume_with_current_routing").unwrap();
        for field in ["expected_task_version","expected_routing_policy_version","idempotency_key"] {
            assert!(resume_schema["required"].as_array().unwrap().contains(&serde_json::json!(field)));
        }
        let resume = serde_json::json!({"operation":"resume_with_current_routing",
            "expected_task_version":7,"expected_routing_policy_version":9,"idempotency_key":"reviewed:resume"});
        let parsed: LifecycleCommand = serde_json::from_value(resume.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(),resume);
        for field in ["expected_task_version","expected_routing_policy_version"] {
            let mut omitted=resume.clone();omitted.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<LifecycleCommand>(omitted).is_err());
            let mut zero=resume.clone();zero[field]=serde_json::json!(0);
            assert!(serde_json::from_value::<LifecycleCommand>(zero).is_err());
        }
    }

    fn stage_start_bodies() -> [serde_json::Value; 2] {
        use serde_json::json;
        [
            json!({"fence":{"kind":"pm","assignment_id":Uuid::from_u128(1),
                "execution_id":Uuid::from_u128(2),"agent_id":Uuid::from_u128(3),
                "assignment_version":1},"idempotency_key":"start:1"}),
            json!({"fence":{"kind":"delivery","assignment_id":Uuid::from_u128(1),
                "execution_id":Uuid::from_u128(2),"fencing_token":1,
                "lease_id":Uuid::from_u128(3),"expected_lease_version":1},
                "idempotency_key":"start:1"}),
        ]
    }

    #[test]
    fn stage_start_openapi_has_tagged_body_and_closed_receipt() {
        use serde_json::json;
        let spec = serde_json::to_value(crate::ApiDoc::openapi()).unwrap();
        let path = "/api/v1/issues/{id}/sdlc/stage-starts";
        assert_eq!(
            spec["paths"][path]["post"]["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/StartStage"
        );
        assert_eq!(spec["paths"][path]["post"]["requestBody"]["required"], true);
        for (path, method, statuses) in [
            (path, "post", vec!["200", "201"]),
            (
                "/api/v1/issues/{id}/sdlc/stage-starts/operations/{idempotency_key}",
                "get",
                vec!["200"],
            ),
        ] {
            let operation = &spec["paths"][path][method];
            assert_eq!(operation["security"], json!([{"bearer":[]}]));
            for status in statuses {
                assert_eq!(
                    operation["responses"][status]["content"]["application/json"]["schema"]["$ref"],
                    "#/components/schemas/StageStartReceipt"
                );
            }
            for status in ["401", "403", "404", "409", "422", "503"] {
                assert!(
                    operation["responses"][status].is_object(),
                    "{path}: {status}"
                );
            }
        }
        let schemas = &spec["components"]["schemas"];
        let request = &schemas["StartStage"];
        assert_eq!(request["additionalProperties"], false);
        assert_eq!(request["properties"].as_object().unwrap().len(), 2);
        assert_eq!(request["required"], json!(["fence", "idempotency_key"]));
        assert_eq!(
            request["properties"]["fence"]["$ref"],
            "#/components/schemas/StageStartFence"
        );
        let variants = schemas["StageStartFence"]["oneOf"].as_array().unwrap();
        assert_eq!(variants.len(), 2);
        for (variant, kind) in variants.iter().zip(["pm", "delivery"]) {
            // A tagged newtype may compose its fields through JSON Schema allOf.
            let tag = std::iter::once(variant)
                .chain(variant["allOf"].as_array().into_iter().flatten())
                .find(|node| node["properties"].get("kind").is_some())
                .expect("fence variant has an explicit kind discriminator");
            assert_eq!(tag["properties"]["kind"]["enum"], json!([kind]));
            assert!(tag["required"].as_array().unwrap().contains(&json!("kind")));
        }
        let receipt = &schemas["StageStartReceipt"];
        assert_eq!(receipt["additionalProperties"], false);
        let fields = [
            "task_id",
            "stage",
            "status",
            "execution_id",
            "idempotency_key",
            "started_at",
        ];
        assert_eq!(
            receipt["properties"].as_object().unwrap().len(),
            fields.len()
        );
        assert_eq!(receipt["required"].as_array().unwrap().len(), fields.len());
        for field in fields {
            assert!(
                receipt["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(field))
            );
        }
        assert_eq!(receipt["properties"]["task_id"]["format"], "uuid");
        assert_eq!(receipt["properties"]["execution_id"]["format"], "uuid");
        assert_eq!(receipt["properties"]["started_at"]["format"], "date-time");
        assert_eq!(
            receipt["properties"]["stage"]["$ref"],
            "#/components/schemas/Stage"
        );
        assert_eq!(
            receipt["properties"]["status"]["$ref"],
            "#/components/schemas/LifecycleStatus"
        );
        let value = json!({"task_id":Uuid::from_u128(4),"stage":"Draft","status":"InProgress",
            "execution_id":Uuid::from_u128(2),"idempotency_key":"start:1","started_at":"2026-11-07T10:00:00Z"});
        let parsed: StageStartReceipt = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);
        let mut foreign = value;
        foreign["owner_subject"] = json!("caller");
        assert!(serde_json::from_value::<StageStartReceipt>(foreign).is_err());
    }

    #[tokio::test]
    async fn stage_start_actual_json_extractor_requires_closed_typed_kind() {
        use axum::extract::FromRequest;
        use serde_json::json;
        for body in stage_start_bodies() {
            for mismatch in 0..7 {
                let mut value = body.clone();
                match mismatch {
                    0 => {}
                    1 => value["stage"] = json!("Analysis"),
                    2 => value["status"] = json!("InProgress"),
                    3 => value["fence"]["owner_subject"] = json!("caller"),
                    4 => value["fence"]["kind"] = json!("human"),
                    5 => {
                        value["fence"].as_object_mut().unwrap().remove("kind");
                    }
                    _ => {
                        value["fence"]["kind"] = if body["fence"]["kind"] == "pm" {
                            json!("delivery")
                        } else {
                            json!("pm")
                        };
                    }
                }
                let request = axum::http::Request::builder()
                    .method("POST")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(axum::body::Body::from(value.to_string()))
                    .unwrap();
                let parsed = Json::<StartStage>::from_request(request, &()).await;
                if mismatch == 0 {
                    assert_eq!(serde_json::to_value(parsed.unwrap().0).unwrap(), body);
                } else {
                    assert!(serde_json::from_value::<StartStage>(value).is_err());
                    assert_eq!(
                        parsed.unwrap_err().into_response().status(),
                        StatusCode::UNPROCESSABLE_ENTITY
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn stage_start_and_proposal_json_errors_preserve_status_and_hide_untrusted_data() {
        use axum::extract::FromRequest;
        for (body, content_type, status, expected) in [
            (
                r#"{"assignment_id":"secret-value","secret-field":"private-document"}"#,
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
                START_SHAPE,
            ),
            (
                r#"{"fence":{"kind":"secret-value"},"idempotency_key":"private-document"}"#,
                true,
                StatusCode::UNPROCESSABLE_ENTITY,
                START_SHAPE,
            ),
            (
                r#"{"fence":"#,
                true,
                StatusCode::BAD_REQUEST,
                "Invalid JSON syntax: serialize the request with a JSON library.",
            ),
            (
                "private-document",
                false,
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Expected Content-Type: application/json.",
            ),
        ] {
            let mut builder = axum::http::Request::builder().method("POST");
            if content_type {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
            }
            let request = builder.body(axum::body::Body::from(body)).unwrap();
            let error = Json::<StartStage>::from_request(request, &())
                .await
                .unwrap_err();
            let (headers, response) = request_json_error(error, START_SHAPE);
            assert_eq!(headers[header::CACHE_CONTROL], "no-store");
            assert_eq!(response.status(), status);
            assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                value,
                serde_json::json!({"error":{"code":"VALIDATION_ERROR","message":expected}})
            );
            let public = String::from_utf8(bytes.to_vec()).unwrap();
            for private in ["secret-value", "secret-field", "private-document"] {
                assert!(!public.contains(private));
            }
        }
        let request = axum::http::Request::builder()
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(r#"{"result":{"role":"analyst","artifact":{"reference":"private-document","sha256":"secret-value"}}}"#))
            .unwrap();
        let error = Json::<SubmitDeliveryProposal>::from_request(request, &())
            .await
            .unwrap_err();
        let (_, response) = request_json_error(error, PROPOSAL_SHAPE);
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["error"]["message"], PROPOSAL_SHAPE);
        assert!(PROPOSAL_SHAPE.contains("workflow_receipt"));
        assert!(PROPOSAL_SHAPE.contains("flat object with role"));
        assert!(PROPOSAL_SHAPE.contains("analyst (analysis)"));
    }

    #[test]
    fn critical_return_contract_is_separate_typed_authenticated_and_no_store() {
        #[derive(utoipa::OpenApi)]
        #[openapi(
            paths(critical_return, critical_return_operation),
            components(schemas(CriticalPolicyChange, ReturnCriticalToPm))
        )]
        struct Contract;
        let spec = serde_json::to_value(Contract::openapi()).unwrap();
        for (path, method, statuses) in [
            (
                "/api/v1/issues/{id}/sdlc/critical-blockers/returns",
                "post",
                vec!["200", "201"],
            ),
            (
                "/api/v1/issues/{id}/sdlc/critical-blockers/returns/operations/{idempotency_key}",
                "get",
                vec!["200"],
            ),
        ] {
            let operation = &spec["paths"][path][method];
            assert!(operation["security"].is_array());
            for status in statuses {
                assert_eq!(
                    operation["responses"][status]["content"]["application/json"]["schema"]["$ref"],
                    "#/components/schemas/CriticalPolicyChange"
                );
            }
            for status in ["401", "403", "404", "409", "422", "503"] {
                assert!(operation["responses"][status].is_object());
            }
        }
        let plan = &spec["components"]["schemas"]["CriticalPolicyChange"];
        assert_eq!(plan["additionalProperties"], false);
        assert_eq!(plan["properties"].as_object().unwrap().len(), 2);
        for field in ["lifecycle", "blocker"] {
            assert!(
                plan["required"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!(field))
            );
        }
        assert_eq!(
            spec["components"]["schemas"]["ReturnCriticalToPm"]["additionalProperties"],
            false
        );
        assert!(
            spec["paths"]
                .get("/api/v1/issues/{id}/sdlc/lifecycle")
                .is_none()
        );
        assert!(
            serde_json::from_value::<LifecycleCommand>(
                serde_json::json!({"operation":"return_to_pm"})
            )
            .is_err()
        );
        assert_eq!(written(((), true)).0, StatusCode::OK);
        assert_eq!(written(((), false)).0, StatusCode::CREATED);
        assert_eq!(written(((), true)).1[header::CACHE_CONTROL], "no-store");
        let failure = (no_store(), AppError::Forbidden).into_response();
        assert_eq!(failure.status(), StatusCode::FORBIDDEN);
        assert_eq!(failure.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn critical_return_actual_json_extractor_is_closed_and_requires_terminal_not_ack() {
        use axum::extract::FromRequest;
        let body = serde_json::json!({
            "review":{"blocker_id":Uuid::new_v4(),"blocker_sha256":"a".repeat(64),
                "expected_blocker_version":1,"expected_task_version":7,
                "fence":{"assignment_id":Uuid::new_v4(),"execution_id":Uuid::new_v4(),
                    "fencing_token":1,"lease_id":Uuid::new_v4(),"expected_lease_version":3},"idempotency_key":"return:original:1"},
            "terminal":{"assignment_hash":"b".repeat(64),"chat_id":Uuid::new_v4(),"run_id":Uuid::new_v4(),
                "state":"cancelled","receipt":{"reference":"fixture://SYNTHETIC-native-readback","sha256":"c".repeat(64)}}
        });
        for mismatch in 0..6 {
            let mut value = body.clone();
            match mismatch {
                0 => {}
                1 => value["owner_subject"] = serde_json::json!("owner"),
                2 => value["review"]["observed_lease_holder"] = serde_json::json!("scheduler"),
                3 => value["terminal"]["state"] = serde_json::json!("unknown"),
                4 => value["terminal"]["state"] = serde_json::json!("stop_acknowledged"),
                _ => {
                    value.as_object_mut().unwrap().remove("terminal");
                }
            }
            let request = axum::http::Request::builder()
                .method("POST")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(value.to_string()))
                .unwrap();
            let parsed = Json::<ReturnCriticalToPm>::from_request(request, &()).await;
            if mismatch == 0 {
                assert_eq!(serde_json::to_value(parsed.unwrap().0).unwrap(), body);
            } else {
                let response = (no_store(), parsed.unwrap_err()).into_response();
                assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
                assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            }
        }
    }

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
            ("/api/v1/issues/{id}/sdlc/stage-starts", "post"),
            (
                "/api/v1/issues/{id}/sdlc/stage-starts/operations/{idempotency_key}",
                "get",
            ),
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
            ("/api/v1/issues/{id}/sdlc/critical-blockers/returns", "post"),
            (
                "/api/v1/issues/{id}/sdlc/critical-blockers/pm-reviews",
                "post",
            ),
            (
                "/api/v1/issues/{id}/sdlc/critical-blockers/pm-reviews/operations/{idempotency_key}",
                "get",
            ),
            (
                "/api/v1/issues/{id}/sdlc/critical-blockers/returns/operations/{idempotency_key}",
                "get",
            ),
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
        ] {
            assert!(spec["paths"][path][method]["security"].is_array(), "{path}");
            assert!(
                spec["paths"][path][method]["responses"]["403"].is_object(),
                "{path}"
            );
        }
        let schemas = &spec["components"]["schemas"];
        assert_eq!(
            schemas["CriticalPolicyChange"]["additionalProperties"],
            false
        );
        for field in ["lifecycle", "blocker"] {
            assert!(
                schemas["CriticalPolicyChange"]["required"]
                    .as_array()
                    .unwrap()
                    .contains(&serde_json::json!(field))
            );
        }
        assert_eq!(schemas["LifecycleReadback"]["additionalProperties"], false);
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
