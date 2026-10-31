//! Explicit human request; receipt acceptance never claims that a process started.
use crate::middleware::auth::UserClaims;
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use domain::{
    sdlc::Principal,
    task_handoff::{
        CancelTaskHandoffCommand, CancelTaskHandoffReceipt, TaskExecutionBlocker as Blocker,
        TaskHandoffCommand, TaskHandoffOperation, TaskHandoffPreview,
    },
};
use pdlc_auth_core::service_bridge::VerifiedCentralToken;
use shared::{AppError, IssueId};
use std::sync::Arc;
use uuid::Uuid;

const APPLICATIONS: [&str; 4] = ["task-tracker", "fleet-control", "project-workflow", "ci-cd"];
fn human(actor: &Principal) -> Result<(), AppError> {
    if !actor.trusted_human
        || app::sdlc::has_pm_grant(&actor.scopes)
        || app::sdlc_lifecycle::has_delivery_grant(&actor.scopes)
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}
fn dispatch_access(proof: &VerifiedCentralToken) -> Result<(), AppError> {
    // Current applications and original PAT scopes intersect. A queue credential
    // must not turn a read-only initiating token into a write-capable executor.
    for method in ["GET", "POST"] {
        proof
            .require_applications(&APPLICATIONS, method)
            .map_err(|_| AppError::Forbidden)?;
    }
    Ok(())
}
fn response(status: StatusCode, value: impl serde::Serialize) -> Response {
    (status, [(header::CACHE_CONTROL, "no-store")], Json(value)).into_response()
}
fn repository(
    ctx: &app::AppContext,
) -> Result<&dyn app::task_handoff::CurrentStageHandoffRepository, AppError> {
    ctx.current_stage_handoff
        .as_deref()
        .ok_or_else(|| AppError::Unavailable("current_stage_handoff_not_configured".into()))
}
pub fn router() -> Router<Arc<app::AppContext>> {
    Router::new()
        .route("/sdlc/current-stage-candidates", get(requested))
        .route("/issues/{id}/handoff/execution", get(execution))
        .route("/issues/{id}/handoff", get(preview).post(request))
        .route("/issues/{id}/handoff/operations/{key}", get(operation))
        .route("/issues/{id}/handoff/{handoff_id}/cancel", post(cancel))
}
#[utoipa::path(get,operation_id="preview_task_handoff",path="/api/v1/issues/{id}/handoff",params(("id"=Uuid,Path)),responses((status=200,body=TaskHandoffPreview),(status=401),(status=403),(status=404),(status=503)),security(("bearer"=[])))]
pub async fn preview(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Extension(claims): Extension<UserClaims>,
    Extension(proof): Extension<VerifiedCentralToken>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    human(&actor)?;
    let user = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let state = ctx
        .services
        .task_lifecycle
        .read(IssueId::from_uuid(id), user)
        .await?
        .state;
    let mut view = TaskHandoffPreview {
        state,
        issue_sha256: None,
        routing_policy_version: None,
        current_request: None,
        required_applications: APPLICATIONS.iter().map(|s| (*s).to_owned()).collect(),
        blockers: vec![],
    };
    let Some(service) = ctx
        .sdlc
        .as_ref()
        .filter(|_| ctx.current_stage_handoff.is_some())
    else {
        view.blockers.push(Blocker::ExecutionNotConfigured);
        return Ok(response(StatusCode::OK, view));
    };
    let binding = service.repository.read(id, &actor).await?;
    if binding.owner_subject != actor.subject {
        view.blockers.push(Blocker::OwnerConsentRequired);
        return Ok(response(StatusCode::OK, view));
    }
    view.current_request = repository(&ctx)?.current_owner_handoff(id, &actor).await?;
    if let Some(current) = view.current_request.as_mut() {
        current.can_cancel &= proof.context.allows_service("task-tracker", "POST");
    }
    if dispatch_access(&proof).is_err() {
        view.blockers.push(Blocker::ApplicationAccessDenied);
        return Ok(response(StatusCode::OK, view));
    }
    if view.current_request.is_some() {
        view.blockers.push(Blocker::ExecutionBusy);
        return Ok(response(StatusCode::OK, view));
    }
    let prepared = service
        .repository
        .prepare_current_stage_handoff(id, &actor)
        .await?;
    view.state = prepared.state;
    view.issue_sha256 = Some(prepared.issue_sha256.clone());
    view.routing_policy_version = prepared.routing.as_ref().map(|p| p.version);
    view.blockers = prepared.blockers.clone();
    if view.blockers.is_empty() {
        if let Some(fleet) = service.fleet_configuration.as_ref() {
            // Fresh configuration/source facts only. Native admission occurs later.
            if let Err(error) = app::task_handoff_observations::observe_detailed(
                &prepared,
                fleet,
                ctx.task_repositories.as_ref(),
            )
            .await
            {
                view.blockers.push(error.blocker()?);
            }
        } else {
            view.blockers.push(Blocker::FleetUnavailable)
        }
    }
    Ok(response(StatusCode::OK, view))
}
#[utoipa::path(post,operation_id="request_task_handoff",path="/api/v1/issues/{id}/handoff",params(("id"=Uuid,Path)),request_body=TaskHandoffCommand,responses((status=202,body=TaskHandoffOperation),(status=200,body=TaskHandoffOperation),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn request(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Extension(proof): Extension<VerifiedCentralToken>,
    Path(id): Path<Uuid>,
    Json(command): Json<TaskHandoffCommand>,
) -> Result<Response, AppError> {
    human(&actor)?;
    command.validate()?;
    dispatch_access(&proof)?;
    let repo = repository(&ctx)?;
    if let Some(saved) = repo
        .handoff_operation(id, &actor, &command.idempotency_key)
        .await?
    {
        if saved.command_hash != app::sdlc::canonical_hash(&command)?
            || !saved.operation.execution_requested
        {
            return Err(AppError::conflict("task_handoff_idempotency_conflict"));
        }
        return Ok(response(StatusCode::OK, saved.operation));
    }
    let service = super::sdlc::service(&ctx)?;
    let prepared = service
        .repository
        .prepare_current_stage_handoff(id, &actor)
        .await?;
    app::task_handoff::validate_prepared_command(&prepared, &command)?;
    let fleet = service
        .fleet_configuration
        .as_ref()
        .ok_or_else(|| AppError::Unavailable("Fleet configuration reader not configured".into()))?;
    let observed =
        app::task_handoff_observations::observe(&prepared, fleet, ctx.task_repositories.as_ref())
            .await?;
    // Recheck original (not renewed) application/token proof after peer HTTP.
    dispatch_access(&proof)?;
    let (_, replayed) = repo
        .request_current_stage_handoff(id, &actor, &command, &observed)
        .await?;
    let saved = repo
        .handoff_operation(id, &actor, &command.idempotency_key)
        .await?
        .ok_or_else(|| AppError::internal("committed handoff operation missing"))?;
    Ok(response(
        if replayed {
            StatusCode::OK
        } else {
            StatusCode::ACCEPTED
        },
        saved.operation,
    ))
}
#[utoipa::path(get,operation_id="get_task_handoff_operation",path="/api/v1/issues/{id}/handoff/operations/{key}",params(("id"=Uuid,Path),("key"=String,Path)),responses((status=200,body=TaskHandoffOperation),(status=401),(status=403),(status=404),(status=503)),security(("bearer"=[])))]
pub async fn operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((id, key)): Path<(Uuid, String)>,
) -> Result<Response, AppError> {
    human(&actor)?;
    let saved = repository(&ctx)?
        .handoff_operation(id, &actor, &key)
        .await?
        .ok_or_else(|| AppError::not_found("handoff operation", key))?;
    Ok(response(StatusCode::OK, saved.operation))
}
#[utoipa::path(post,operation_id="cancel_task_handoff",path="/api/v1/issues/{id}/handoff/{handoff_id}/cancel",params(("id"=Uuid,Path),("handoff_id"=Uuid,Path)),request_body=CancelTaskHandoffCommand,responses((status=200,body=CancelTaskHandoffReceipt),(status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn cancel(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((id, handoff)): Path<(Uuid, Uuid)>,
    Json(command): Json<CancelTaskHandoffCommand>,
) -> Result<Response, AppError> {
    human(&actor)?;
    let (receipt, _) = repository(&ctx)?
        .cancel_current_stage_handoff(id, handoff, &actor, &command)
        .await?;
    Ok(response(StatusCode::OK, receipt))
}

#[utoipa::path(get,operation_id="read_requested_current_stage_handoffs",path="/api/v1/sdlc/current-stage-candidates",params(domain::sdlc_candidates::DeliveryCandidatesQuery),responses((status=200,body=domain::task_handoff::RequestedHandoffs),(status=400),(status=401),(status=403),(status=404),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn requested(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Query(query): Query<domain::sdlc_candidates::DeliveryCandidatesQuery>,
) -> Result<Response, AppError> {
    Ok(response(
        StatusCode::OK,
        repository(&ctx)?.requested_handoffs(&actor, query).await?,
    ))
}
#[utoipa::path(get,operation_id="read_requested_current_stage_execution",path="/api/v1/issues/{id}/handoff/execution",params(("id"=Uuid,Path)),responses((status=200,body=domain::current_stage_execution::CurrentStageExecution),(status=401),(status=403),(status=404),(status=409),(status=503)),security(("bearer"=[])))]
pub async fn execution(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    Ok(response(
        StatusCode::OK,
        repository(&ctx)?
            .requested_handoff_execution(id, &actor)
            .await?,
    ))
}
