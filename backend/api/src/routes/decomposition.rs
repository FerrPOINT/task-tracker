use super::sdlc::service;
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    routing::get,
};
use domain::{decomposition::*, sdlc::Principal};
use shared::AppError;
use std::sync::Arc;
use uuid::Uuid;

pub(super) fn router() -> Router<Arc<app::AppContext>> {
    Router::new()
        .route("/issues/{id}/decomposition", get(read).post(save))
        .route("/issues/{id}/decomposition/revisions", get(history))
        .route(
            "/issues/{id}/decomposition/revisions/{revision}",
            get(revision),
        )
        .route(
            "/issues/{id}/decomposition/operations/{key}",
            get(operation),
        )
}
fn headers() -> HeaderMap {
    let mut result = HeaderMap::new();
    result.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    result
}
#[utoipa::path(get,path="/api/v1/issues/{id}/decomposition",operation_id="read_decomposition",tag="decomposition",params(("id"=Uuid,Path)),responses((status=200,body=PlanReadback),(status=401),(status=403),(status=404),(status=503)),security(("bearer"=[]))) ]
pub async fn read(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
) -> Result<(HeaderMap, Json<PlanReadback>), AppError> {
    Ok((
        headers(),
        Json(
            service(&ctx)?
                .decomposition()?
                .read_plan(task, &actor)
                .await?,
        ),
    ))
}
#[utoipa::path(post,path="/api/v1/issues/{id}/decomposition",operation_id="save_decomposition",tag="decomposition",params(("id"=Uuid,Path)),request_body=SavePlan,responses((status=201,body=PlanOperation),(status=200,body=PlanOperation),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[]))) ]
pub async fn save(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    Json(command): Json<SavePlan>,
) -> Result<(StatusCode, HeaderMap, Json<PlanOperation>), AppError> {
    let (result, replayed) = service(&ctx)?
        .decomposition()?
        .save_plan(task, &actor, command)
        .await?;
    Ok((
        if replayed || !result.changed {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        headers(),
        Json(result),
    ))
}
#[utoipa::path(get,path="/api/v1/issues/{id}/decomposition/revisions",operation_id="decomposition_history",tag="decomposition",params(("id"=Uuid,Path),PlanHistoryQuery),responses((status=200,body=PlanHistory),(status=401),(status=403),(status=404),(status=422),(status=503)),security(("bearer"=[]))) ]
pub async fn history(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    Query(query): Query<PlanHistoryQuery>,
) -> Result<(HeaderMap, Json<PlanHistory>), AppError> {
    Ok((
        headers(),
        Json(
            service(&ctx)?
                .decomposition()?
                .plan_history(task, &actor, query)
                .await?,
        ),
    ))
}
#[utoipa::path(get,path="/api/v1/issues/{id}/decomposition/revisions/{revision}",operation_id="decomposition_revision",tag="decomposition",params(("id"=Uuid,Path),("revision"=i64,Path)),responses((status=200,body=PlanRevision),(status=401),(status=403),(status=404),(status=422),(status=503)),security(("bearer"=[]))) ]
pub async fn revision(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, revision)): Path<(Uuid, i64)>,
) -> Result<(HeaderMap, Json<PlanRevision>), AppError> {
    Ok((
        headers(),
        Json(
            service(&ctx)?
                .decomposition()?
                .plan_revision(task, &actor, revision)
                .await?,
        ),
    ))
}
#[utoipa::path(get,path="/api/v1/issues/{id}/decomposition/operations/{key}",operation_id="decomposition_operation",tag="decomposition",params(("id"=Uuid,Path),("key"=String,Path)),responses((status=200,body=PlanOperation),(status=401),(status=403),(status=404),(status=422),(status=503)),security(("bearer"=[]))) ]
pub async fn operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, key)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Json<PlanOperation>), AppError> {
    Ok((
        headers(),
        Json(
            service(&ctx)?
                .decomposition()?
                .plan_operation(task, &actor, &key)
                .await?,
        ),
    ))
}
