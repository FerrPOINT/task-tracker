use axum::{Extension, Json, extract::State, http::StatusCode};
use shared::{AppError, IssueId, ProjectId, ProjectKey, SprintId};
use std::sync::Arc;

use crate::dto::{
    CreateSprintRequest, MoveIssueToSprintRequest, SprintListResponse, SprintResponse,
    UpdateSprintRequest,
};

fn map_sprint_response(dto: app::dto::SprintDto) -> SprintResponse {
    SprintResponse {
        id: dto.id,
        name: dto.name,
        goal: dto.goal,
        state: dto.state,
        velocity: dto.velocity,
        remaining_days: dto.remaining_days,
        issue_ids: dto.issue_ids,
        start_date: dto.start_date,
        end_date: dto.end_date,
    }
}

fn map_issue(i: app::dto::IssueDto) -> crate::dto::IssueResponse {
    crate::dto::IssueResponse {
        id: i.id,
        key: i.key,
        summary: i.summary,
        description: i.description,
        issue_type: i.issue_type,
        project_key: i.project_key,
        status: i.status,
        status_id: i.status_id,
        priority: i.priority,
        labels: i.labels,
        assignee_id: i.assignee_id,
        assignee_name: i.assignee_name,
        reporter_id: i.reporter_id,
        reporter_name: i.reporter_name,
        project_name: i.project_name,
        sprint_id: i.sprint_id,
        original_estimate_seconds: i.original_estimate_seconds,
        remaining_estimate_seconds: i.remaining_estimate_seconds,
        time_spent_seconds: i.time_spent_seconds,
    }
}

fn parse_project_id(project: &app::dto::ProjectDto) -> Result<ProjectId, AppError> {
    project
        .id
        .parse::<uuid::Uuid>()
        .map(ProjectId::from_uuid)
        .map_err(|_| AppError::invalid_input("project_id"))
}

fn parse_sprint_id(sprint_id: &str) -> Result<SprintId, AppError> {
    sprint_id
        .parse::<uuid::Uuid>()
        .map(SprintId::from_uuid)
        .map_err(|_| AppError::invalid_input("sprint_id"))
}

fn parse_issue_id(issue_id: &str) -> Result<IssueId, AppError> {
    issue_id
        .parse::<uuid::Uuid>()
        .map(IssueId::from_uuid)
        .map_err(|_| AppError::invalid_input("issue_id"))
}

fn ensure_sprint_belongs_to_project(
    sprint: &app::dto::SprintDto,
    project_id: ProjectId,
    sprint_id: SprintId,
) -> Result<(), AppError> {
    let sprint_project_id = sprint
        .project_id
        .parse::<uuid::Uuid>()
        .map(ProjectId::from_uuid)
        .map_err(|_| AppError::internal("invalid sprint project id"))?;
    if sprint_project_id != project_id {
        return Err(AppError::not_found("sprint", sprint_id));
    }
    Ok(())
}

fn ensure_issue_belongs_to_project(
    issue: &app::dto::IssueDto,
    project_key: &ProjectKey,
) -> Result<(), AppError> {
    if issue.project_key != project_key.to_string() {
        return Err(AppError::invalid_input(
            "issue does not belong to this project",
        ));
    }
    Ok(())
}

fn ensure_issue_is_in_sprint(
    issue: &app::dto::IssueDto,
    sprint_id: SprintId,
) -> Result<(), AppError> {
    let expected_sprint_id = sprint_id.to_string();
    if issue.sprint_id.as_deref() != Some(expected_sprint_id.as_str()) {
        return Err(AppError::invalid_input("issue is not in this sprint"));
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_key}/sprints",
    params(("project_key" = String, Path, description = "Project key")),
    responses((status = 200, body = SprintListResponse))
)]
pub async fn list_sprints(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
    axum::extract::Path(key): axum::extract::Path<String>,
) -> Result<Json<SprintListResponse>, AppError> {
    let key = ProjectKey::new(key.as_str());
    if !key.is_valid() {
        return Err(AppError::invalid_input("project_key"));
    }
    let requester = claims
        .sub
        .parse::<shared::UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let project = ctx.services.project.get_by_key(&key, requester).await?;
    let project_id = parse_project_id(&project)?;
    let items = ctx.services.sprint.list(project_id, requester).await?;
    Ok(Json(SprintListResponse {
        sprints: items.into_iter().map(map_sprint_response).collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_key}/sprints",
    params(("project_key" = String, Path, description = "Project key")),
    request_body = CreateSprintRequest,
    responses((status = 201, body = SprintResponse))
)]
pub async fn create_sprint(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
    axum::extract::Path(key): axum::extract::Path<String>,
    Json(req): Json<CreateSprintRequest>,
) -> Result<(StatusCode, Json<SprintResponse>), AppError> {
    let key = ProjectKey::new(key.as_str());
    if !key.is_valid() {
        return Err(AppError::invalid_input("project_key"));
    }
    let requester = claims
        .sub
        .parse::<shared::UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let project = ctx.services.project.get_by_key(&key, requester).await?;
    let project_id = parse_project_id(&project)?;
    let cmd = app::commands::CreateSprintCommand {
        project_id,
        name: req.name,
        goal: req.goal,
        start_date: req.start_date,
        end_date: req.end_date,
    };
    let dto = ctx.services.sprint.create(cmd, requester).await?;
    Ok((StatusCode::CREATED, Json(map_sprint_response(dto))))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_key}/sprints/{sprint_id}",
    params(
        ("project_key" = String, Path, description = "Project key"),
        ("sprint_id" = String, Path, description = "Sprint id"),
    ),
    responses((status = 200, body = SprintResponse))
)]
pub async fn get_sprint(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
    axum::extract::Path((key, sprint_id)): axum::extract::Path<(String, String)>,
) -> Result<Json<SprintResponse>, AppError> {
    let key = ProjectKey::new(key.as_str());
    if !key.is_valid() {
        return Err(AppError::invalid_input("project_key"));
    }
    let requester = claims
        .sub
        .parse::<shared::UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let project = ctx.services.project.get_by_key(&key, requester).await?;
    let project_id = parse_project_id(&project)?;
    let id = parse_sprint_id(&sprint_id)?;
    let dto = ctx.services.sprint.get_by_id(id, requester).await?;
    ensure_sprint_belongs_to_project(&dto, project_id, id)?;
    Ok(Json(map_sprint_response(dto)))
}

#[utoipa::path(
    patch,
    path = "/api/v1/projects/{project_key}/sprints/{sprint_id}",
    params(
        ("project_key" = String, Path, description = "Project key"),
        ("sprint_id" = String, Path, description = "Sprint id"),
    ),
    request_body = UpdateSprintRequest,
    responses((status = 200, body = SprintResponse))
)]
pub async fn update_sprint(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
    axum::extract::Path((key, sprint_id)): axum::extract::Path<(String, String)>,
    Json(req): Json<UpdateSprintRequest>,
) -> Result<Json<SprintResponse>, AppError> {
    let key = ProjectKey::new(key.as_str());
    if !key.is_valid() {
        return Err(AppError::invalid_input("project_key"));
    }
    let requester = claims
        .sub
        .parse::<shared::UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let project = ctx.services.project.get_by_key(&key, requester).await?;
    let project_id = parse_project_id(&project)?;
    let id = parse_sprint_id(&sprint_id)?;
    let existing = ctx.services.sprint.get_by_id(id, requester).await?;
    ensure_sprint_belongs_to_project(&existing, project_id, id)?;
    let cmd = app::commands::UpdateSprintCommand {
        name: req.name,
        goal: req.goal,
        start_date: req.start_date,
        end_date: req.end_date,
    };
    let dto = ctx.services.sprint.update(id, cmd, requester).await?;
    Ok(Json(map_sprint_response(dto)))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_key}/sprints/{sprint_id}/start",
    params(
        ("project_key" = String, Path, description = "Project key"),
        ("sprint_id" = String, Path, description = "Sprint id"),
    ),
    responses((status = 200, body = SprintResponse))
)]
pub async fn start_sprint(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
    axum::extract::Path((key, sprint_id)): axum::extract::Path<(String, String)>,
) -> Result<Json<SprintResponse>, AppError> {
    let key = ProjectKey::new(key.as_str());
    if !key.is_valid() {
        return Err(AppError::invalid_input("project_key"));
    }
    let requester = claims
        .sub
        .parse::<shared::UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let project = ctx.services.project.get_by_key(&key, requester).await?;
    let project_id = parse_project_id(&project)?;
    let id = parse_sprint_id(&sprint_id)?;
    let existing = ctx.services.sprint.get_by_id(id, requester).await?;
    ensure_sprint_belongs_to_project(&existing, project_id, id)?;
    let dto = ctx.services.sprint.start(id, requester).await?;
    Ok(Json(map_sprint_response(dto)))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_key}/sprints/{sprint_id}/close",
    params(
        ("project_key" = String, Path, description = "Project key"),
        ("sprint_id" = String, Path, description = "Sprint id"),
    ),
    responses((status = 200, body = SprintResponse))
)]
pub async fn close_sprint(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
    axum::extract::Path((key, sprint_id)): axum::extract::Path<(String, String)>,
) -> Result<Json<SprintResponse>, AppError> {
    let key = ProjectKey::new(key.as_str());
    if !key.is_valid() {
        return Err(AppError::invalid_input("project_key"));
    }
    let requester = claims
        .sub
        .parse::<shared::UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let project = ctx.services.project.get_by_key(&key, requester).await?;
    let project_id = parse_project_id(&project)?;
    let id = parse_sprint_id(&sprint_id)?;
    let existing = ctx.services.sprint.get_by_id(id, requester).await?;
    ensure_sprint_belongs_to_project(&existing, project_id, id)?;
    let dto = ctx.services.sprint.close(id, requester).await?;
    Ok(Json(map_sprint_response(dto)))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_key}/sprints/{sprint_id}/issues",
    params(
        ("project_key" = String, Path, description = "Project key"),
        ("sprint_id" = String, Path, description = "Sprint id"),
    ),
    request_body = MoveIssueToSprintRequest,
    responses((status = 200, body = crate::dto::IssueResponse))
)]
pub async fn move_issue_to_sprint(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
    axum::extract::Path((key, sprint_id)): axum::extract::Path<(String, String)>,
    Json(req): Json<MoveIssueToSprintRequest>,
) -> Result<Json<crate::dto::IssueResponse>, AppError> {
    let key = ProjectKey::new(key.as_str());
    if !key.is_valid() {
        return Err(AppError::invalid_input("project_key"));
    }
    let requester = claims
        .sub
        .parse::<shared::UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let project = ctx.services.project.get_by_key(&key, requester).await?;
    let project_id = parse_project_id(&project)?;
    let sprint_id = parse_sprint_id(&sprint_id)?;
    let sprint = ctx.services.sprint.get_by_id(sprint_id, requester).await?;
    ensure_sprint_belongs_to_project(&sprint, project_id, sprint_id)?;
    let issue_id = parse_issue_id(&req.issue_id)?;
    let issue = ctx.services.issue.get_by_id(issue_id, requester).await?;
    ensure_issue_belongs_to_project(&issue, &key)?;
    let dto = ctx
        .services
        .sprint
        .move_issue(
            app::commands::MoveIssueToSprintCommand {
                issue_id,
                sprint_id: Some(sprint_id),
            },
            requester,
        )
        .await?;
    Ok(Json(map_issue(dto)))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_key}/sprints/{sprint_id}/remove-issue",
    params(
        ("project_key" = String, Path, description = "Project key"),
        ("sprint_id" = String, Path, description = "Sprint id"),
    ),
    request_body = MoveIssueToSprintRequest,
    responses((status = 200, body = crate::dto::IssueResponse))
)]
pub async fn remove_issue_from_sprint(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<crate::middleware::auth::UserClaims>,
    axum::extract::Path((key, sprint_id)): axum::extract::Path<(String, String)>,
    Json(req): Json<MoveIssueToSprintRequest>,
) -> Result<Json<crate::dto::IssueResponse>, AppError> {
    let key = ProjectKey::new(key.as_str());
    if !key.is_valid() {
        return Err(AppError::invalid_input("project_key"));
    }
    let requester = claims
        .sub
        .parse::<shared::UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let project = ctx.services.project.get_by_key(&key, requester).await?;
    let project_id = parse_project_id(&project)?;
    let sprint_id = parse_sprint_id(&sprint_id)?;
    let sprint = ctx.services.sprint.get_by_id(sprint_id, requester).await?;
    ensure_sprint_belongs_to_project(&sprint, project_id, sprint_id)?;
    let issue_id = parse_issue_id(&req.issue_id)?;
    let issue = ctx.services.issue.get_by_id(issue_id, requester).await?;
    ensure_issue_belongs_to_project(&issue, &key)?;
    ensure_issue_is_in_sprint(&issue, sprint_id)?;
    let dto = ctx
        .services
        .sprint
        .move_issue(
            app::commands::MoveIssueToSprintCommand {
                issue_id,
                sprint_id: None,
            },
            requester,
        )
        .await?;
    Ok(Json(map_issue(dto)))
}
