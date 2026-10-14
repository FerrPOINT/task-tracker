//! Parent mounts this handler on the existing SDLC-authenticated v1 router.
use super::sdlc::service;
use axum::{
    Extension, Json,
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, header},
};
use domain::{
    sdlc::Principal,
    sdlc_candidates::{DeliveryCandidates, DeliveryCandidatesQuery},
};
use shared::AppError;
use std::sync::Arc;

#[utoipa::path(get, path="/api/v1/sdlc/delivery-candidates", operation_id="read_single_task_delivery_candidates", tag="sdlc",
    params(DeliveryCandidatesQuery), responses((status=200,body=DeliveryCandidates),(status=400),(status=401),
        (status=403),(status=404),(status=409),(status=422),(status=503)), security(("bearer"=[])))]
pub async fn delivery_candidates(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Query(query): Query<DeliveryCandidatesQuery>,
) -> Result<(HeaderMap, Json<DeliveryCandidates>), AppError> {
    let result = service(&ctx)?.delivery_candidates(&actor, query).await?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok((headers, Json(result)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::OpenApi;

    #[test]
    fn candidate_handler_schema_is_bounded_authenticated_get_only() {
        #[derive(utoipa::OpenApi)]
        #[openapi(paths(delivery_candidates))]
        struct Contract;
        let schema = serde_json::to_value(Contract::openapi()).unwrap();
        let path = &schema["paths"]["/api/v1/sdlc/delivery-candidates"];
        assert!(path.get("post").is_none());
        assert!(path["get"]["security"].is_array());
        let parameters = path["get"]["parameters"].as_array().unwrap();
        assert_eq!(parameters.len(), 3);
        let limit = parameters.iter().find(|p| p["name"] == "limit").unwrap();
        assert_eq!(limit["schema"]["default"], 50);
        assert_eq!(limit["schema"]["minimum"], 1);
        assert_eq!(limit["schema"]["maximum"], 100);
        assert_eq!(limit["required"], false);
    }

    #[test]
    fn actual_query_extractor_defaults_rejects_unknowns_and_validates_bounds() {
        let project = uuid::Uuid::new_v4();
        let uri = format!("/api/v1/sdlc/delivery-candidates?project_id={project}")
            .parse()
            .unwrap();
        let Query(query) = Query::<DeliveryCandidatesQuery>::try_from_uri(&uri).unwrap();
        assert_eq!(query.limit, 50);
        query.validate().unwrap();
        for suffix in [
            "&limit=0",
            "&limit=101",
            "&after=00000000-0000-0000-0000-000000000000",
        ] {
            let uri = format!("/api/v1/sdlc/delivery-candidates?project_id={project}{suffix}")
                .parse()
                .unwrap();
            let Query(query) = Query::<DeliveryCandidatesQuery>::try_from_uri(&uri).unwrap();
            assert!(query.validate().is_err());
        }
        for suffix in [
            "&claim=true",
            "&limit=-1",
            "&limit=abc",
            "&after=invalid",
            "&limit=1&limit=2",
        ] {
            let uri = format!("/api/v1/sdlc/delivery-candidates?project_id={project}{suffix}")
                .parse()
                .unwrap();
            assert!(Query::<DeliveryCandidatesQuery>::try_from_uri(&uri).is_err());
        }
    }
}
