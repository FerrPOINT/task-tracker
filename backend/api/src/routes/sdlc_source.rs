use super::sdlc::service;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    routing::get,
};
use domain::{sdlc::Principal, sdlc_source::*};
use shared::AppError;
use std::sync::Arc;
use uuid::Uuid;

pub(super) fn router() -> Router<Arc<app::AppContext>> {
    Router::new()
        .route(
            "/projects/{project_id}/sdlc/source-policy",
            get(current).post(set),
        )
        .route(
            "/projects/{project_id}/sdlc/source-policy/versions/{revision}",
            get(version),
        )
        .route(
            "/projects/{project_id}/sdlc/source-policy/operations/{idempotency_key}",
            get(operation),
        )
        .route("/issues/{id}/sdlc/source-binding", get(binding))
        .route(
            "/issues/{id}/sdlc/source-enrollment",
            get(enrollment_preflight).post(enroll_draft),
        )
}

fn no_store() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers
}

fn matches_replay(policy: &ProjectSourcePolicy, input: &SetSourcePolicy) -> bool {
    policy.source == input.source
        && input.expected_source_revision.unwrap_or(0).checked_add(1)
            == Some(policy.source_revision)
}

#[utoipa::path(post,path="/api/v1/projects/{project_id}/sdlc/source-policy",tag="sdlc",
    params(("project_id"=Uuid,Path)),request_body=SetSourcePolicy,
    responses((status=201,body=ProjectSourcePolicy),(status=200,body=ProjectSourcePolicy),
        (status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn set(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(project): Path<Uuid>,
    Json(input): Json<SetSourcePolicy>,
) -> Result<(StatusCode, HeaderMap, Json<ProjectSourcePolicy>), AppError> {
    app::sdlc_source::human_owner(&actor)?;
    app::sdlc::validate_key(&input.idempotency_key)?;
    app::sdlc_source::validate_spec(&input.source)?;
    let repository = service(&ctx)?.source_repository()?;
    // Exact original operation works even if Forge is now unavailable; no new observation.
    match repository
        .source_policy_operation(project, &actor, &input.idempotency_key)
        .await
    {
        Ok(policy) if matches_replay(&policy, &input) => {
            return Ok((StatusCode::OK, no_store(), Json(policy)));
        }
        Ok(_) => return Err(AppError::conflict("source operation payload changed")),
        Err(AppError::NotFound(_)) => (),
        Err(error) => return Err(error),
    }
    let namespace = repository.source_policy_namespace(project, &actor).await?;
    let verified = ctx
        .task_repositories
        .verify_source(&namespace, &input.source)
        .await?;
    // No DB transaction spans HTTP. Owner, namespace and policy CAS are checked again.
    let (policy, replay) = repository
        .set_source_policy(project, &actor, input, verified)
        .await?;
    Ok((
        if replay {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        no_store(),
        Json(policy),
    ))
}

#[utoipa::path(get,path="/api/v1/projects/{project_id}/sdlc/source-policy",tag="sdlc",
    params(("project_id"=Uuid,Path)),responses((status=200,body=ProjectSourcePolicy),(status=401),(status=403),(status=404),(status=503)),security(("bearer"=[])))]
pub async fn current(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(project): Path<Uuid>,
) -> Result<(HeaderMap, Json<ProjectSourcePolicy>), AppError> {
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .source_repository()?
                .source_policy(project, &actor, None)
                .await?,
        ),
    ))
}

#[utoipa::path(get,path="/api/v1/projects/{project_id}/sdlc/source-policy/versions/{revision}",tag="sdlc",
    params(("project_id"=Uuid,Path),("revision"=i64,Path)),responses((status=200,body=ProjectSourcePolicy),(status=401),(status=403),(status=404),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn version(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((project, revision)): Path<(Uuid, i64)>,
) -> Result<(HeaderMap, Json<ProjectSourcePolicy>), AppError> {
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .source_repository()?
                .source_policy(project, &actor, Some(revision))
                .await?,
        ),
    ))
}

#[utoipa::path(get,path="/api/v1/projects/{project_id}/sdlc/source-policy/operations/{idempotency_key}",tag="sdlc",
    params(("project_id"=Uuid,Path),("idempotency_key"=String,Path)),responses((status=200,body=ProjectSourcePolicy),(status=401),(status=403),(status=404),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn operation(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((project, key)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Json<ProjectSourcePolicy>), AppError> {
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .source_repository()?
                .source_policy_operation(project, &actor, &key)
                .await?,
        ),
    ))
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/source-binding",tag="sdlc",
    params(("id"=Uuid,Path)),responses((status=200,body=TaskSourceReadback,description="Immutable binding and current context, or typed blocked state; never a Git write permit"),
        (status=401),(status=403),(status=404),(status=409),(status=503)),security(("bearer"=[])))]
pub async fn binding(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
) -> Result<(HeaderMap, Json<TaskSourceReadback>), AppError> {
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .source_repository()?
                .task_source(task, &actor)
                .await?,
        ),
    ))
}

#[utoipa::path(get,path="/api/v1/issues/{id}/sdlc/source-enrollment",tag="sdlc",
    params(("id"=Uuid,Path)),responses((status=200,body=DraftSourceEnrollmentPreflight),
        (status=401),(status=403),(status=404),(status=409),(status=503)),security(("bearer"=[])))]
pub async fn enrollment_preflight(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
) -> Result<(HeaderMap, Json<DraftSourceEnrollmentPreflight>), AppError> {
    app::sdlc_source::human_owner(&actor)?;
    Ok((
        no_store(),
        Json(
            service(&ctx)?
                .source_repository()?
                .draft_source_preflight(task, &actor)
                .await?,
        ),
    ))
}

#[utoipa::path(post,path="/api/v1/issues/{id}/sdlc/source-enrollment",tag="sdlc",
    params(("id"=Uuid,Path)),request_body=EnrollDraftSource,
    responses((status=201,body=DraftSourceEnrollmentReceipt),(status=200,body=DraftSourceEnrollmentReceipt),
        (status=401),(status=403),(status=404),(status=409),(status=422),(status=503)),security(("bearer"=[])))]
pub async fn enroll_draft(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(task): Path<Uuid>,
    Json(input): Json<EnrollDraftSource>,
) -> Result<(StatusCode, HeaderMap, Json<DraftSourceEnrollmentReceipt>), AppError> {
    app::sdlc_source::human_owner(&actor)?;
    app::sdlc_draft_source_enrollment::validate_command(&input)?;
    // One Tracker transaction serializes enrollment with existing confirmation and technical admission.
    let (receipt, replay) = service(&ctx)?
        .source_repository()?
        .enroll_draft_source(task, &actor, input)
        .await?;
    Ok((
        if replay {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        no_store(),
        Json(receipt),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn source_post_is_closed_owner_input_not_a_caller_verification_receipt() {
        let input = json!({"expected_source_revision":null,"source":{
            "forge_instance_id":Uuid::new_v4(),"repository_id":Uuid::new_v4(),
            "base_ref":"refs/heads/main","base_commit":"a".repeat(40)},"idempotency_key":"source:1"});
        assert!(serde_json::from_value::<SetSourcePolicy>(input.clone()).is_ok());
        for field in [
            "verification",
            "candidate_ref",
            "machine_subject",
            "effective_admin",
        ] {
            let mut changed = input.clone();
            changed[field] = json!(true);
            assert!(serde_json::from_value::<SetSourcePolicy>(changed).is_err());
        }
        let mut missing = input;
        missing
            .as_object_mut()
            .unwrap()
            .remove("expected_source_revision");
        assert!(serde_json::from_value::<SetSourcePolicy>(missing).is_err());
    }

    #[test]
    fn enrollment_body_cannot_supply_runtime_or_branch_authority() {
        let input = json!({"expected_preflight_sha256":"a".repeat(64),"source_revision":1,
            "policy_hash":"b".repeat(64),"idempotency_key":"enroll:1"});
        assert!(serde_json::from_value::<EnrollDraftSource>(input.clone()).is_ok());
        for field in [
            "runtime_observation",
            "candidate_ref",
            "owner_subject",
            "inventory_complete",
            "safe_checkpoint",
            "session_id",
        ] {
            let mut changed = input.clone();
            changed[field] = json!(true);
            assert!(serde_json::from_value::<EnrollDraftSource>(changed).is_err());
        }
    }
}
