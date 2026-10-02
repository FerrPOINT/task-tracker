use axum::{
    Extension, Json, Router,
    extract::{OriginalUri, Path, Query, State},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use domain::sdlc::*;
use domain::sdlc_metadata::*;
use serde::{Deserialize, Serialize};
use shared::AppError;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Serialize, utoipa::ToSchema)]
pub struct QuestionsResponse {
    pub questions: Vec<Question>,
}
#[derive(Serialize, utoipa::ToSchema)]
pub struct RevisionsResponse {
    pub revisions: Vec<RequirementsRevision>,
}
#[derive(Serialize, utoipa::ToSchema)]
pub struct RevisionDiff {
    pub before: RequirementsRevision,
    pub after: RequirementsRevision,
}
#[derive(Deserialize, utoipa::IntoParams)]
pub struct DiffQuery {
    #[serde(deserialize_with = "domain::sdlc::safe_version")]
    pub against: i64,
}
#[derive(Deserialize, utoipa::IntoParams)]
pub struct EventsQuery {
    #[serde(default)]
    pub after: i64,
}
#[derive(Serialize, utoipa::ToSchema)]
pub struct OutboxResponse {
    pub events: Vec<OutboxEvent>,
}

#[derive(Deserialize)]
struct ProjectionQuery {
    projection: Option<String>,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct MetadataEventsQuery {
    #[param(value_type = Option<MetadataProjection>)]
    pub projection: Option<String>,
    /// Canonical nonnegative decimal i64 for metadata_v1; legacy i64 otherwise.
    pub after: Option<String>,
    #[param(value_type = Option<u16>, minimum = 1, maximum = 100)]
    pub limit: Option<String>,
    #[param(value_type = Option<u32>, minimum = 1024, maximum = 1048576)]
    pub max_bytes: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum EventsSuccess {
    Legacy(OutboxResponse),
    Metadata(MetadataPage),
}

fn decimal(value: &str) -> Result<i64, AppError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(AppError::validation("invalid metadata query"));
    }
    value
        .parse::<i64>()
        .map_err(|_| AppError::validation("invalid metadata query"))
}

impl MetadataEventsQuery {
    fn bounds(&self) -> Result<(i64, u16, usize), AppError> {
        let after = decimal(self.after.as_deref().unwrap_or("0"))?;
        let limit = decimal(self.limit.as_deref().unwrap_or("100"))?;
        let max_bytes = decimal(self.max_bytes.as_deref().unwrap_or("262144"))?;
        if !(1..=100).contains(&limit)
            || !(METADATA_MIN_BYTES as i64..=METADATA_MAX_BYTES as i64).contains(&max_bytes)
        {
            return Err(AppError::validation("invalid metadata query"));
        }
        Ok((after, limit as u16, max_bytes as usize))
    }
}

fn service(ctx: &app::AppContext) -> Result<&app::sdlc::SdlcService, AppError> {
    ctx.sdlc
        .as_ref()
        .ok_or_else(|| AppError::Unavailable("Tracker SDLC is not configured".into()))
}

pub fn router() -> Router<Arc<app::AppContext>> {
    Router::new()
        .route("/sdlc/project-access", get(project_access))
        .route("/projects/{project_id}/sdlc/drafts", post(create_draft))
        .route("/issues/{id}/sdlc/context", get(context))
        .route("/issues/{id}/sdlc/pm-draft-input", get(pm_draft_input))
        .route("/issues/{id}/sdlc/binding", post(bind))
        .route("/issues/{id}/sdlc/assignment", post(assign))
        .route(
            "/issues/{id}/sdlc/clarifications",
            get(questions).post(publish_question),
        )
        .route(
            "/issues/{id}/sdlc/clarifications/{question_id}/answers",
            post(answer),
        )
        .route(
            "/issues/{id}/sdlc/clarifications/{question_id}/cancel",
            post(cancel),
        )
        .route(
            "/issues/{id}/sdlc/requirements",
            get(current_revision).post(publish_revision),
        )
        .route("/issues/{id}/sdlc/requirements/revisions", get(revisions))
        .route("/issues/{id}/sdlc/requirements/{revision}", get(revision))
        .route("/issues/{id}/sdlc/requirements/{revision}/diff", get(diff))
        .route(
            "/issues/{id}/sdlc/requirements/{revision}/confirm",
            post(confirm),
        )
        .route("/issues/{id}/sdlc/evidence", post(evidence))
        .route("/issues/{id}/sdlc/events", get(events))
        .route_layer(middleware::from_fn(
            crate::middleware::sdlc_auth::strict_central_auth,
        ))
}

#[utoipa::path(
    get, path="/api/v1/sdlc/project-access", tag="sdlc",
    responses((status=200, body=ProjectAccess), (status=401, description="Verified Central Auth bearer required"),
        (status=403, description="Service read access and active central-subject identity required"),
        (status=503, description="SDLC or Central Auth unavailable")),
    security(("bearer"=[]))
)]
pub async fn project_access(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
) -> Result<Json<ProjectAccess>, AppError> {
    Ok(Json(service(&ctx)?.project_access(&actor).await?))
}

#[utoipa::path(
    post, path="/api/v1/projects/{project_id}/sdlc/drafts",
    params(("project_id"=Uuid, Path)), request_body=CreateDraftCommand,
    responses(
        (status=201, description="Bound Draft created", body=CreatedDraft),
        (status=200, description="Exact durable creation replay", body=CreatedDraft),
        (status=401, description="Valid Central Auth bearer required"),
        (status=403, description="Human session and explicit project write access required"),
        (status=404, description="Project or original task no longer exists"),
        (status=409, description="Idempotency payload or ownership binding conflict"),
        (status=422, description="Invalid or unknown command fields"),
        (status=503, description="SDLC or Central Auth unavailable")
    ), security(("bearer"=[]))
)]
pub async fn create_draft(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(project): Path<Uuid>,
    Json(command): Json<CreateDraftCommand>,
) -> Result<(axum::http::StatusCode, Json<CreatedDraft>), AppError> {
    let (draft, replayed) = service(&ctx)?
        .create_draft(project, &actor, command)
        .await?;
    Ok((
        if replayed {
            axum::http::StatusCode::OK
        } else {
            axum::http::StatusCode::CREATED
        },
        Json(draft),
    ))
}

#[utoipa::path(get, path="/api/v1/issues/{id}/sdlc/context", params(("id"=Uuid, Path)), responses((status=200,body=SdlcContext)), security(("bearer"=[])))]
pub async fn context(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Json<SdlcContext>, AppError> {
    Ok(Json(service(&ctx)?.context(id, &actor).await?))
}

#[utoipa::path(
    get, path="/api/v1/issues/{id}/sdlc/pm-draft-input", tag="sdlc",
    params(("id"=Uuid, Path)),
    responses(
        (status=200, body=PmDraftInputResponse),
        (status=401, description="Verified Central Auth bearer required"),
        (status=403, description="Service read access and active explicit project access required"),
        (status=404, description="Issue or SDLC binding not found"),
        (status=409, description="Original immutable creation input unavailable or inconsistent"),
        (status=503, description="SDLC or Central Auth unavailable")
    ), security(("bearer"=[]))
)]
pub async fn pm_draft_input(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Json<PmDraftInputResponse>, AppError> {
    Ok(Json(service(&ctx)?.pm_draft_input(id, &actor).await?))
}

#[utoipa::path(post, path="/api/v1/issues/{id}/sdlc/binding", params(("id"=Uuid, Path)), request_body=BindCommand, responses((status=200,body=SdlcContext)), security(("bearer"=[])))]
pub async fn bind(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(id): Path<Uuid>,
    Json(command): Json<BindCommand>,
) -> Result<Json<SdlcContext>, AppError> {
    Ok(Json(
        service(&ctx)?
            .repository
            .bind(id, &actor, command)
            .await?
            .context(&actor),
    ))
}

#[utoipa::path(post, path="/api/v1/issues/{id}/sdlc/assignment", params(("id"=Uuid, Path)), request_body=AssignCommand, responses((status=200,body=PmAssignment)), security(("bearer"=[])))]
pub async fn assign(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(id): Path<Uuid>,
    Json(command): Json<AssignCommand>,
) -> Result<Json<PmAssignment>, AppError> {
    Ok(Json(
        service(&ctx)?
            .execute(id, &actor, SdlcCommand::Assign(command))
            .await?,
    ))
}

#[utoipa::path(get, path="/api/v1/issues/{id}/sdlc/clarifications", params(("id"=Uuid, Path)), responses((status=200,body=QuestionsResponse)), security(("bearer"=[])))]
pub async fn questions(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Json<QuestionsResponse>, AppError> {
    Ok(Json(QuestionsResponse {
        questions: service(&ctx)?.repository.read(id, &actor).await?.questions,
    }))
}

#[utoipa::path(post, path="/api/v1/issues/{id}/sdlc/clarifications", params(("id"=Uuid, Path)), request_body=PublishQuestion, responses((status=200,body=Question)), security(("bearer"=[])))]
pub async fn publish_question(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(id): Path<Uuid>,
    Json(command): Json<PublishQuestion>,
) -> Result<Json<Question>, AppError> {
    Ok(Json(
        service(&ctx)?
            .execute(id, &actor, SdlcCommand::PublishQuestion(command))
            .await?,
    ))
}

#[utoipa::path(post, path="/api/v1/issues/{id}/sdlc/clarifications/{question_id}/answers", params(("id"=Uuid, Path),("question_id"=Uuid,Path)), request_body=AnswerCommand, responses((status=200,body=Answer)), security(("bearer"=[])))]
pub async fn answer(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((id, question_id)): Path<(Uuid, Uuid)>,
    Json(command): Json<AnswerCommand>,
) -> Result<Json<Answer>, AppError> {
    Ok(Json(
        service(&ctx)?
            .execute(
                id,
                &actor,
                SdlcCommand::Answer {
                    question_id,
                    command,
                },
            )
            .await?,
    ))
}

#[utoipa::path(post, path="/api/v1/issues/{id}/sdlc/clarifications/{question_id}/cancel", params(("id"=Uuid, Path),("question_id"=Uuid,Path)), request_body=CancelQuestion, responses((status=200,body=Question)), security(("bearer"=[])))]
pub async fn cancel(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((id, question_id)): Path<(Uuid, Uuid)>,
    Json(command): Json<CancelQuestion>,
) -> Result<Json<Question>, AppError> {
    Ok(Json(
        service(&ctx)?
            .execute(
                id,
                &actor,
                SdlcCommand::Cancel {
                    question_id,
                    command,
                },
            )
            .await?,
    ))
}

#[utoipa::path(get, path="/api/v1/issues/{id}/sdlc/requirements/revisions", params(("id"=Uuid, Path)), responses((status=200,body=RevisionsResponse)), security(("bearer"=[])))]
pub async fn revisions(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Json<RevisionsResponse>, AppError> {
    Ok(Json(RevisionsResponse {
        revisions: service(&ctx)?.repository.read(id, &actor).await?.revisions,
    }))
}

#[utoipa::path(get, path="/api/v1/issues/{id}/sdlc/requirements", params(("id"=Uuid, Path)), responses((status=200,body=RequirementsRevision)), security(("bearer"=[])))]
pub async fn current_revision(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(id): Path<Uuid>,
) -> Result<Json<RequirementsRevision>, AppError> {
    Ok(Json(
        service(&ctx)?
            .repository
            .read(id, &actor)
            .await?
            .revisions
            .last()
            .cloned()
            .ok_or_else(|| AppError::not_found("requirements", id))?,
    ))
}

#[utoipa::path(post, path="/api/v1/issues/{id}/sdlc/requirements", params(("id"=Uuid, Path)), request_body=PublishRevision, responses((status=200,body=RequirementsRevision)), security(("bearer"=[])))]
pub async fn publish_revision(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(id): Path<Uuid>,
    Json(command): Json<PublishRevision>,
) -> Result<Json<RequirementsRevision>, AppError> {
    Ok(Json(
        service(&ctx)?
            .execute(id, &actor, SdlcCommand::PublishRevision(command))
            .await?,
    ))
}

fn find_revision(state: &TaskState, number: i64) -> Result<RequirementsRevision, AppError> {
    if !(1..=MAX_SAFE_VERSION).contains(&number) {
        return Err(AppError::validation(
            "revision is outside JavaScript safe integer range",
        ));
    }
    state
        .revisions
        .iter()
        .find(|r| r.revision == number)
        .cloned()
        .ok_or_else(|| AppError::not_found("requirements revision", number))
}

#[utoipa::path(get, path="/api/v1/issues/{id}/sdlc/requirements/{revision}", params(("id"=Uuid, Path),("revision"=i64,Path)), responses((status=200,body=RequirementsRevision)), security(("bearer"=[])))]
pub async fn revision(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((id, number)): Path<(Uuid, i64)>,
) -> Result<Json<RequirementsRevision>, AppError> {
    Ok(Json(find_revision(
        &service(&ctx)?.repository.read(id, &actor).await?,
        number,
    )?))
}

#[utoipa::path(get, path="/api/v1/issues/{id}/sdlc/requirements/{revision}/diff", params(("id"=Uuid, Path),("revision"=i64,Path),DiffQuery), responses((status=200,body=RevisionDiff)), security(("bearer"=[])))]
pub async fn diff(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((id, number)): Path<(Uuid, i64)>,
    Query(query): Query<DiffQuery>,
) -> Result<Json<RevisionDiff>, AppError> {
    let state = service(&ctx)?.repository.read(id, &actor).await?;
    Ok(Json(RevisionDiff {
        before: find_revision(&state, query.against)?,
        after: find_revision(&state, number)?,
    }))
}

#[utoipa::path(post, path="/api/v1/issues/{id}/sdlc/requirements/{revision}/confirm", params(("id"=Uuid, Path),("revision"=i64,Path)), request_body=ConfirmCommand, responses((status=200,body=Confirmation)), security(("bearer"=[])))]
pub async fn confirm(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path((id, revision)): Path<(Uuid, i64)>,
    Json(command): Json<ConfirmCommand>,
) -> Result<Json<Confirmation>, AppError> {
    if !(1..=MAX_SAFE_VERSION).contains(&revision) {
        return Err(AppError::validation(
            "revision is outside JavaScript safe integer range",
        ));
    }
    Ok(Json(
        service(&ctx)?
            .execute(id, &actor, SdlcCommand::Confirm { revision, command })
            .await?,
    ))
}

#[utoipa::path(post, path="/api/v1/issues/{id}/sdlc/evidence", params(("id"=Uuid, Path)), request_body=EvidenceCommand, responses((status=200,body=Evidence)), security(("bearer"=[])))]
pub async fn evidence(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(id): Path<Uuid>,
    Json(command): Json<EvidenceCommand>,
) -> Result<Json<Evidence>, AppError> {
    Ok(Json(
        service(&ctx)?
            .execute(id, &actor, SdlcCommand::Evidence(command))
            .await?,
    ))
}

#[utoipa::path(get, path="/api/v1/issues/{id}/sdlc/events", operation_id="sdlc_events", params(("id"=Uuid, Path), MetadataEventsQuery), responses((status=200,body=EventsSuccess), (status=409,body=MetadataError), (status=422,body=MetadataError,description="Metadata budget too small; invalid query uses the standard validation envelope")), security(("bearer"=[])))]
pub async fn events(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(actor): Extension<Principal>,
    Path(id): Path<Uuid>,
    OriginalUri(uri): OriginalUri,
) -> Result<Response, AppError> {
    let projection = match Query::<ProjectionQuery>::try_from_uri(&uri) {
        Ok(Query(query)) => query.projection,
        Err(error) => return Ok(error.into_response()),
    };
    if let Some(projection) = projection {
        if projection != "metadata_v1" {
            return Err(AppError::validation("unsupported metadata projection"));
        }
        let Query(query) = Query::<MetadataEventsQuery>::try_from_uri(&uri)
            .map_err(|_| AppError::validation("invalid metadata query"))?;
        let (after, limit, max_bytes) = query.bounds()?;
        let wire = service(&ctx)?
            .metadata_outbox(id, &actor, after, limit, max_bytes)
            .await?;
        let status = axum::http::StatusCode::from_u16(wire.status).map_err(AppError::internal)?;
        return Ok((
            status,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            wire.bytes,
        )
            .into_response());
    }
    let Query(query) = match Query::<EventsQuery>::try_from_uri(&uri) {
        Ok(query) => query,
        Err(error) => return Ok(error.into_response()),
    };
    Ok(Json(OutboxResponse {
        events: service(&ctx)?
            .repository
            .outbox(id, &actor, query.after)
            .await?,
    })
    .into_response())
}

#[cfg(test)]
mod metadata_query_tests {
    use super::*;

    #[test]
    fn metadata_bounds_are_canonical_and_legacy_query_keeps_its_semantics() {
        for after in ["-1", "+1", "01", "", "9223372036854775808", "1.0", " 1"] {
            assert!(decimal(after).is_err());
        }
        assert_eq!(decimal("9223372036854775807").unwrap(), i64::MAX);
        for (name, value) in [
            ("limit", "0"),
            ("limit", "101"),
            ("max_bytes", "1023"),
            ("max_bytes", "1048577"),
            ("limit", "01"),
        ] {
            let uri = format!("/events?projection=metadata_v1&{name}={value}")
                .parse()
                .unwrap();
            assert!(
                Query::<MetadataEventsQuery>::try_from_uri(&uri)
                    .unwrap()
                    .0
                    .bounds()
                    .is_err()
            );
        }
        let uri = "/events?after=01&limit=bad&limit=also-bad&max_bytes=0"
            .parse()
            .unwrap();
        assert_eq!(Query::<EventsQuery>::try_from_uri(&uri).unwrap().0.after, 1);
        let uri = "/events?after=abc".parse().unwrap();
        assert!(Query::<EventsQuery>::try_from_uri(&uri).is_err());
    }
}
