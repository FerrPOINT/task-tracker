use super::sdlc::service;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
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
    Json(input): Json<StartStage>,
) -> Result<(StatusCode, HeaderMap, Json<StageStartReceipt>), AppError> {
    Ok(written(
        service(&ctx)?.start_stage(task, &actor, input).await?,
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
