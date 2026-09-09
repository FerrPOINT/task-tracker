use axum::{
    Extension, Json,
    body::Body,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::dto::IssueResponse;
use shared::{AppError, UserId};

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ExportIssuesRequest {
    pub project_key: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExportIssuesResponse {
    pub issues: Vec<IssueResponse>,
}

#[utoipa::path(
    post,
    path = "/api/v1/export/csv",
    request_body = ExportIssuesRequest,
    responses((status = 200, description = "CSV issue export", content_type = "text/csv")),
    security(("bearer" = []))
)]
pub async fn export_csv(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Json(request): Json<ExportIssuesRequest>,
) -> Result<Response, AppError> {
    let issues = export_issues(&ctx, &claims, request).await?;
    let mut csv = String::from(
        "key,summary,description,issue_type,project_key,status,priority,assignee_id,reporter_id,created_at,updated_at\n",
    );
    for issue in issues {
        let row = [
            issue.key,
            issue.summary,
            issue.description,
            issue.issue_type,
            issue.project_key,
            issue.status,
            issue.priority,
            issue.assignee_id.unwrap_or_default(),
            issue.reporter_id,
            issue.created_at.to_rfc3339(),
            issue.updated_at.to_rfc3339(),
        ];
        csv.push_str(&row.into_iter().map(csv_cell).collect::<Vec<_>>().join(","));
        csv.push('\n');
    }
    download_response("text/csv; charset=utf-8", "issues.csv", csv.into_bytes())
}

#[utoipa::path(
    post,
    path = "/api/v1/export/json",
    request_body = ExportIssuesRequest,
    responses((status = 200, description = "JSON issue export", body = ExportIssuesResponse)),
    security(("bearer" = []))
)]
pub async fn export_json(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
    Json(request): Json<ExportIssuesRequest>,
) -> Result<Response, AppError> {
    let issues = export_issues(&ctx, &claims, request).await?;
    let bytes = serde_json::to_vec(&ExportIssuesResponse { issues })
        .map_err(|_| AppError::internal("failed to serialize export"))?;
    download_response("application/json", "issues.json", bytes)
}

async fn export_issues(
    ctx: &Arc<app::AppContext>,
    claims: &app::auth::UserClaims,
    request: ExportIssuesRequest,
) -> Result<Vec<IssueResponse>, AppError> {
    let requester = claims
        .sub
        .parse::<UserId>()
        .map_err(|_| AppError::invalid_input("invalid user id in token"))?;
    let issues = ctx
        .services
        .issue
        .export_project(
            &request
                .project_key
                .parse()
                .map_err(AppError::invalid_input)?,
            requester,
        )
        .await?;
    Ok(issues.into_iter().map(map_issue).collect())
}

fn map_issue(issue: app::dto::IssueDto) -> IssueResponse {
    IssueResponse {
        id: issue.id,
        key: issue.key,
        summary: issue.summary,
        description: issue.description,
        issue_type: issue.issue_type,
        project_key: issue.project_key,
        status: issue.status,
        status_id: issue.status_id,
        priority: issue.priority,
        labels: issue.labels,
        assignee_id: issue.assignee_id,
        assignee_name: issue.assignee_name,
        reporter_id: issue.reporter_id,
        reporter_name: issue.reporter_name,
        project_name: issue.project_name,
        sprint_id: issue.sprint_id,
        original_estimate_seconds: issue.original_estimate_seconds,
        remaining_estimate_seconds: issue.remaining_estimate_seconds,
        time_spent_seconds: issue.time_spent_seconds,
        created_at: issue.created_at,
        updated_at: issue.updated_at,
    }
}

fn csv_cell(value: String) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn download_response(
    content_type: &'static str,
    file_name: &'static str,
    bytes: Vec<u8>,
) -> Result<Response, AppError> {
    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static(match file_name {
            "issues.csv" => "attachment; filename=\"issues.csv\"",
            "issues.json" => "attachment; filename=\"issues.json\"",
            _ => "attachment",
        }),
    );
    Ok(response)
}
