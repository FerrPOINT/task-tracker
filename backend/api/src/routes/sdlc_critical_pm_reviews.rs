use super::sdlc::service;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use domain::{
    sdlc::Principal,
    sdlc_critical_pm_reservation::{CriticalPmReservationResult, ReserveCriticalPmReview},
};
use std::sync::Arc;
use uuid::Uuid;

// Mount beside the existing SDLC router with the SAME strict auth; no-store wraps auth failures too.
pub(super) fn router() -> Router<Arc<app::AppContext>> {
    Router::new()
        .route(
            "/issues/{id}/sdlc/critical-blockers/pm-reviews",
            post(reserve),
        )
        .route(
            "/issues/{id}/sdlc/critical-blockers/pm-reviews/operations/{idempotency_key}",
            get(operation),
        )
        .route_layer(axum::middleware::from_fn(
            crate::middleware::sdlc_auth::strict_central_auth,
        ))
        .layer(axum::middleware::map_response(uncached))
}

async fn uncached(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn no_store() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers
}

fn failure(error: impl IntoResponse) -> (HeaderMap, Response) {
    (no_store(), error.into_response())
}

fn written<T>((result, replay): (T, bool)) -> (StatusCode, HeaderMap, Json<T>) {
    (
        if replay {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        no_store(),
        Json(result),
    )
}

#[utoipa::path(post,path="/api/v1/issues/{id}/sdlc/critical-blockers/pm-reviews",operation_id="reserve_critical_business_pm_review",tag="sdlc",
    params(("id"=Uuid,Path)),request_body=ReserveCriticalPmReview,
    responses((status=201,body=CriticalPmReservationResult),(status=200,body=CriticalPmReservationResult,description="Original durable reservation replay"),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn reserve(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    input: Result<Json<ReserveCriticalPmReview>, axum::extract::rejection::JsonRejection>,
) -> Result<(StatusCode, HeaderMap, Json<CriticalPmReservationResult>), (HeaderMap, Response)> {
    let Json(input) = input.map_err(failure)?;
    let result = service(&ctx)
        .map_err(failure)?
        .reserve_critical_pm_review(task, &actor, input)
        .await
        .map_err(failure)?;
    Ok(written(result))
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/critical-blockers/pm-reviews/operations/{idempotency_key}",operation_id="read_critical_business_pm_review_operation",tag="sdlc",
    params(("id"=Uuid,Path),("idempotency_key"=String,Path)),
    responses((status=200,body=CriticalPmReservationResult),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, key)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Json<CriticalPmReservationResult>), (HeaderMap, Response)> {
    let result = service(&ctx)
        .map_err(failure)?
        .critical_pm_review_operation(task, &actor, &key)
        .await
        .map_err(failure)?;
    Ok((no_store(), Json(result)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::FromRequest;
    use serde_json::json;
    use utoipa::OpenApi;

    #[test]
    fn contract_is_closed_typed_separate_authenticated_and_not_initial_draft() {
        #[derive(utoipa::OpenApi)]
        #[openapi(
            paths(reserve, operation),
            components(schemas(ReserveCriticalPmReview, CriticalPmReservationResult))
        )]
        struct Contract;
        let spec = serde_json::to_value(Contract::openapi()).unwrap();
        for (path, method, statuses) in [
            (
                "/api/v1/issues/{id}/sdlc/critical-blockers/pm-reviews",
                "post",
                vec!["200", "201"],
            ),
            (
                "/api/v1/issues/{id}/sdlc/critical-blockers/pm-reviews/operations/{idempotency_key}",
                "get",
                vec!["200"],
            ),
        ] {
            let op = &spec["paths"][path][method];
            assert_eq!(op["security"], json!([{"bearer":[]}]));
            for status in statuses {
                assert_eq!(
                    op["responses"][status]["content"]["application/json"]["schema"]["$ref"],
                    "#/components/schemas/CriticalPmReservationResult"
                );
            }
            for status in ["401", "403", "404", "409", "422", "503"] {
                assert!(op["responses"][status].is_object());
            }
        }
        let schemas = &spec["components"]["schemas"];
        for name in [
            "ReserveCriticalPmReview",
            "CriticalPmReservationResult",
            "CriticalPmReviewContext",
            "CriticalPmRequirementsRef",
        ] {
            assert_eq!(schemas[name]["additionalProperties"], false, "{name}");
        }
        assert_eq!(
            schemas["ReserveCriticalPmReview"]["properties"]
                .as_object()
                .unwrap()
                .len(),
            7
        );
        assert_eq!(
            schemas["CriticalPmReservationResult"]["properties"]
                .as_object()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            schemas["CriticalPmReservationPurpose"]["enum"],
            json!(["critical_business_review"])
        );
        assert_eq!(spec["paths"].as_object().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn actual_json_extractor_rejects_open_fields_and_unsafe_versions() {
        let body = json!({"blocker_id":Uuid::new_v4(),"blocker_sha256":"a".repeat(64),
            "expected_blocker_version":2,"expected_task_version":9,"expected_pm_owner_version":17,
            "expected_pm_assignment_version":17,"idempotency_key":"pm-review:original"});
        for mismatch in 0..8 {
            let mut value = body.clone();
            match mismatch {
                0 => {}
                1 => value["requested_agent_id"] = json!(Uuid::new_v4()),
                2 => value["terminal"] = json!({}),
                3 => value["dispatch_allowed"] = json!(true),
                4 => value["expected_pm_owner_version"] = json!(null),
                5 => value["expected_task_version"] = json!(0),
                6 => value["expected_pm_assignment_version"] = json!(9_007_199_254_740_992_i64),
                _ => {
                    value.as_object_mut().unwrap().remove("blocker_id");
                }
            }
            let request = axum::http::Request::builder()
                .method("POST")
                .header(header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(value.to_string()))
                .unwrap();
            let parsed = Json::<ReserveCriticalPmReview>::from_request(request, &()).await;
            if mismatch == 0 {
                assert_eq!(serde_json::to_value(parsed.unwrap().0).unwrap(), body);
            } else {
                let response = failure(parsed.unwrap_err()).into_response();
                assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
                assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            }
        }
    }

    #[tokio::test]
    async fn replay_creation_and_failures_never_cache() {
        assert_eq!(written(((), false)).0, StatusCode::CREATED);
        assert_eq!(written(((), true)).0, StatusCode::OK);
        for error in [
            shared::AppError::Forbidden,
            shared::AppError::not_found("review", Uuid::nil()),
            shared::AppError::conflict("fixture conflict"),
            shared::AppError::validation("fixture invalid"),
            shared::AppError::Unavailable("fixture unavailable".into()),
        ] {
            let response = failure(error).into_response();
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        }
        let response = uncached(StatusCode::BAD_REQUEST.into_response()).await;
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn same_strict_central_middleware_rejects_missing_bearer_without_cache() {
        use tower::ServiceExt;
        let app = Router::new()
            .route(
                "/issues/{id}/sdlc/critical-blockers/pm-reviews",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .route(
                "/issues/{id}/sdlc/critical-blockers/pm-reviews/operations/{idempotency_key}",
                get(|| async { StatusCode::NO_CONTENT }),
            )
            .route_layer(axum::middleware::from_fn(
                crate::middleware::sdlc_auth::strict_central_auth,
            ))
            .layer(axum::middleware::map_response(uncached));
        for (method, suffix) in [("POST", ""), ("GET", "/operations/original")] {
            let request = axum::http::Request::builder()
                .method(method)
                .uri(format!(
                    "/issues/{}/sdlc/critical-blockers/pm-reviews{suffix}",
                    Uuid::new_v4()
                ))
                .body(axum::body::Body::empty())
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        }
    }
}
