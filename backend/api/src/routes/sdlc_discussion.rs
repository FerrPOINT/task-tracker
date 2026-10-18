use super::sdlc::service;
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    routing::get,
};
use domain::{sdlc::Principal, sdlc_discussion::*};
use shared::AppError;
use std::sync::Arc;
use uuid::Uuid;

pub(super) fn router() -> Router<Arc<app::AppContext>> {
    Router::new()
        .route("/issues/{id}/sdlc/discussion", get(read).post(publish))
        .route(
            "/issues/{id}/sdlc/discussion/operations/{key}",
            get(operation),
        )
}

fn headers() -> HeaderMap {
    let mut result = HeaderMap::new();
    result.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    result
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/discussion",operation_id="read_task_discussion",tag="sdlc",
    params(("id"=Uuid,Path),DiscussionQuery),
    responses((status=200,body=TaskDiscussion),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn read(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    Query(query): Query<DiscussionQuery>,
) -> Result<(HeaderMap, Json<TaskDiscussion>), AppError> {
    Ok((
        headers(),
        Json(service(&ctx)?.discussion(task, &actor, query).await?),
    ))
}

#[utoipa::path(post,path="/api/v1/issues/{id}/sdlc/discussion",operation_id="publish_stage_comment",tag="sdlc",
    params(("id"=Uuid,Path)),request_body=PublishStageComment,
    responses((status=201,body=StageCommentReceipt),(status=200,body=StageCommentReceipt,description="Original committed comment replay"),
        (status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn publish(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    Json(command): Json<PublishStageComment>,
) -> Result<(StatusCode, HeaderMap, Json<StageCommentReceipt>), AppError> {
    let (receipt, replayed) = service(&ctx)?
        .publish_stage_comment(task, &actor, command)
        .await?;
    if !replayed {
        ctx.services
            .comment
            .notify_committed(
                shared::CommentId::from_uuid(receipt.comment_id),
                shared::UserId::from_uuid(receipt.author_id),
            )
            .await;
    }
    Ok((
        if replayed {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        headers(),
        Json(receipt),
    ))
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/discussion/operations/{key}",operation_id="read_stage_comment_operation",tag="sdlc",
    params(("id"=Uuid,Path),("key"=String,Path)),
    responses((status=200,body=StageCommentReceipt),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((task, key)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Json<StageCommentReceipt>), AppError> {
    Ok((
        headers(),
        Json(
            service(&ctx)?
                .stage_comment_operation(task, &actor, &key)
                .await?,
        ),
    ))
}
