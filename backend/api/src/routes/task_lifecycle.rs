//! Human task state is independent of optional agent execution configuration.
use crate::middleware::auth::UserClaims;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use domain::task_lifecycle::{self as model, Readback, TransitionCommand};
use shared::{AppError, IssueId, UserId};
use std::sync::Arc;

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct TaskLifecycleCatalog {
    pub enabled: bool,
    pub stages: Vec<domain::sdlc_lifecycle::LifecycleStage>,
    pub statuses: Vec<domain::sdlc_lifecycle::LifecycleStatus>,
}
fn no_store(value: impl serde::Serialize) -> Response {
    ([(header::CACHE_CONTROL, "no-store")], Json(value)).into_response()
}
fn user(claims: &UserClaims) -> Result<UserId, AppError> {
    claims.sub.parse().map_err(|_| AppError::Unauthorized)
}
#[utoipa::path(get,path="/api/v1/task-lifecycle",responses((status=200,body=TaskLifecycleCatalog)),security(("bearer"=[])))]
pub async fn task_lifecycle_catalog(
    State(ctx): State<Arc<app::AppContext>>,
) -> Result<Response, AppError> {
    Ok(no_store(TaskLifecycleCatalog {
        enabled: ctx.services.task_lifecycle.active().await?,
        stages: model::STAGES.to_vec(),
        statuses: model::STATUSES.to_vec(),
    }))
}
#[utoipa::path(get,path="/api/v1/issues/{id}/lifecycle",params(("id"=uuid::Uuid,Path)),
    responses((status=200,body=Readback),(status=403),(status=404),(status=503)),security(("bearer"=[])))]
pub async fn get_task_lifecycle(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Response, AppError> {
    let actor = user(&claims)?;
    let task = IssueId::from_uuid(id);
    Ok(no_store(
        ctx.services.task_lifecycle.read(task, actor).await?,
    ))
}
#[utoipa::path(post,path="/api/v1/issues/{id}/lifecycle/transitions",params(("id"=uuid::Uuid,Path)),request_body=TransitionCommand,
    responses((status=200,body=Readback),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn transition_task_lifecycle(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(id): Path<uuid::Uuid>,
    Json(command): Json<TransitionCommand>,
) -> Result<Response, AppError> {
    let actor = user(&claims)?;
    let task = IssueId::from_uuid(id);
    Ok(no_store(
        ctx.services
            .task_lifecycle
            .change(task, actor, &command)
            .await?,
    ))
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct TaskBoardColumn {
    pub wip_limit: Option<i64>,
    pub stage: domain::sdlc_lifecycle::LifecycleStage,
    pub issue_ids: Vec<String>,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct TaskBoardCard {
    pub issue: crate::dto::IssueResponse,
    pub lifecycle: Readback,
}
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct TaskBoard {
    pub project_id: String,
    pub project_key: String,
    pub columns: Vec<TaskBoardColumn>,
    pub issues: Vec<TaskBoardCard>,
    pub sprint: crate::dto::SprintResponse,
    pub backlog_total: usize,
}
#[utoipa::path(get,path="/api/v2/projects/{project_key}/board",params(("project_key"=String,Path)),
    responses((status=200,body=TaskBoard),(status=403),(status=404),(status=409),(status=503)),security(("bearer"=[])))]
pub async fn task_stage_board(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<UserClaims>,
    Path(key): Path<String>,
) -> Result<Response, AppError> {
    let actor = user(&claims)?;
    let key = key
        .parse()
        .map_err(|_| AppError::invalid_input("project_key"))?;
    let project = ctx.repos.projects.get_by_key(&key).await?;
    let snapshot = ctx
        .services
        .task_lifecycle
        .board(project.id, project.name.as_ref(), actor)
        .await?;
    let columns = snapshot
        .columns
        .into_iter()
        .map(|column| TaskBoardColumn {
            stage: column.stage,
            wip_limit: column.wip_limit,
            issue_ids: snapshot
                .cards
                .iter()
                .filter(|(_, row)| row.state.stage == column.stage)
                .map(|(_, row)| row.task_id.to_string())
                .collect(),
        })
        .collect();
    let issues = snapshot
        .cards
        .into_iter()
        .map(|(issue, lifecycle)| TaskBoardCard {
            issue: super::board::map_issue(issue),
            lifecycle,
        })
        .collect();
    let sprint = snapshot.sprint;
    Ok(no_store(TaskBoard {
        project_id: project.id.to_string(),
        project_key: project.key.to_string(),
        columns,
        issues,
        sprint: crate::dto::SprintResponse {
            id: sprint.id,
            name: sprint.name,
            goal: sprint.goal,
            state: sprint.state,
            velocity: sprint.velocity,
            remaining_days: sprint.remaining_days,
            issue_ids: sprint.issue_ids,
            start_date: sprint.start_date,
            end_date: sprint.end_date,
        },
        backlog_total: snapshot.backlog_total,
    }))
}
