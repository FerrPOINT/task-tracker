use app::AppContext;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use shared::{
    AuthResponse, BacklogResponse, BoardColumnResponse, BoardResponse, CreateIssueRequest,
    CreateIssueResponse, ErrorResponse, Health, IssueResponse, LoginRequest, ProjectListResponse,
    ProjectResponse, RegisterRequest, SearchResultResponse, SprintResponse, UserResponse,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    info(title = "Task Tracker API", version = env!("CARGO_PKG_VERSION")),
    paths(
        health,
        list_projects,
        get_board,
        get_backlog,
        search_issues,
        create_issue,
        get_issue,
        login,
        register,
    ),
    components(schemas(
        Health,
        ErrorResponse,
        ProjectResponse,
        ProjectListResponse,
        IssueResponse,
        BoardColumnResponse,
        SprintResponse,
        BoardResponse,
        BacklogResponse,
        SearchResultResponse,
        CreateIssueRequest,
        CreateIssueResponse,
        LoginRequest,
        RegisterRequest,
        AuthResponse,
        UserResponse,
    ))
)]
struct ApiDoc;

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Health check", body = Health)
    )
)]
async fn health(State(_ctx): State<Arc<AppContext>>) -> Json<Health> {
    Json(Health {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

fn mock_user(id: &str, name: &str, email: &str) -> UserResponse {
    UserResponse {
        id: id.to_string(),
        name: name.to_string(),
        email: email.to_string(),
    }
}

fn mock_issue(id: &str, key: &str, summary: &str, status: &str, priority: &str) -> IssueResponse {
    IssueResponse {
        id: id.to_string(),
        key: key.to_string(),
        summary: summary.to_string(),
        description: "".to_string(),
        project_key: "TT".to_string(),
        project_name: "Task Tracker".to_string(),
        status: status.to_string(),
        assignee_id: Some("user-2".to_string()),
        assignee_name: Some("Ivan".to_string()),
        reporter_id: "user-1".to_string(),
        reporter_name: "Anna".to_string(),
        priority: priority.to_string(),
        labels: vec![],
        due_date: None,
        original_estimate_seconds: None,
        remaining_estimate_seconds: None,
        time_spent_seconds: 0,
    }
}

#[utoipa::path(
    get,
    path = "/projects",
    responses(
        (status = 200, description = "List of projects", body = ProjectListResponse)
    )
)]
async fn list_projects(State(_ctx): State<Arc<AppContext>>) -> Json<ProjectListResponse> {
    Json(ProjectListResponse {
        items: vec![ProjectResponse {
            id: "proj-1".to_string(),
            key: "TT".to_string(),
            name: "Task Tracker".to_string(),
            description: "Self-hosted task tracker".to_string(),
            owner_id: "user-1".to_string(),
            created_at: "2026-07-01T00:00:00Z".to_string(),
            todo_count: 4,
            in_progress_count: 1,
            done_count: 4,
        }],
    })
}

#[utoipa::path(
    get,
    path = "/projects/{project_key}/board",
    params(("project_key" = String, Path, description = "Project key")),
    responses(
        (status = 200, description = "Board for project", body = BoardResponse)
    )
)]
async fn get_board(
    State(_ctx): State<Arc<AppContext>>,
    Path(project_key): Path<String>,
) -> Json<BoardResponse> {
    let _ = project_key;
    let sprint = SprintResponse {
        id: "sprint-1".to_string(),
        name: "Sprint 1".to_string(),
        goal: "Ship MVP".to_string(),
        state: "active".to_string(),
        velocity: 24,
        remaining_days: Some(5),
        issue_ids: vec!["issue-10".to_string(), "issue-11".to_string()],
    };
    let issues = vec![
        mock_issue("issue-10", "TT-10", "Auth API", "To Do", "High"),
        mock_issue("issue-11", "TT-11", "Login page", "To Do", "Medium"),
        mock_issue("issue-12", "TT-12", "Password reset flow", "To Do", "High"),
        mock_issue("issue-13", "TT-13", "Invite users by email", "To Do", "Medium"),
        mock_issue("issue-7", "TT-7", "OAuth provider integration", "In Progress", "High"),
        mock_issue("issue-15", "TT-15", "Write E2E tests", "Done", "Medium"),
        mock_issue("issue-16", "TT-16", "Update API docs", "Done", "Low"),
        mock_issue("issue-17", "TT-17", "Fix navigation active state", "Done", "Low"),
        mock_issue("issue-18", "TT-18", "Optimize bundle size", "Done", "Medium"),
    ];
    let columns = vec![
        BoardColumnResponse {
            id: "col-todo".to_string(),
            name: "To Do".to_string(),
            wip_limit: Some(15),
            issue_ids: vec![
                "issue-10".to_string(),
                "issue-11".to_string(),
                "issue-12".to_string(),
                "issue-13".to_string(),
            ],
        },
        BoardColumnResponse {
            id: "col-inprogress".to_string(),
            name: "In Progress".to_string(),
            wip_limit: Some(5),
            issue_ids: vec!["issue-7".to_string()],
        },
        BoardColumnResponse {
            id: "col-done".to_string(),
            name: "Done".to_string(),
            wip_limit: None,
            issue_ids: vec![
                "issue-15".to_string(),
                "issue-16".to_string(),
                "issue-17".to_string(),
                "issue-18".to_string(),
            ],
        },
    ];
    Json(BoardResponse {
        columns,
        issues,
        sprint,
    })
}

#[utoipa::path(
    get,
    path = "/projects/{project_key}/backlog",
    params(("project_key" = String, Path, description = "Project key")),
    responses(
        (status = 200, description = "Backlog for project", body = BacklogResponse)
    )
)]
async fn get_backlog(
    State(_ctx): State<Arc<AppContext>>,
    Path(project_key): Path<String>,
) -> Json<BacklogResponse> {
    let _ = project_key;
    let sprint = SprintResponse {
        id: "sprint-1".to_string(),
        name: "Sprint 1".to_string(),
        goal: "Ship MVP".to_string(),
        state: "active".to_string(),
        velocity: 24,
        remaining_days: Some(5),
        issue_ids: vec!["issue-10".to_string(), "issue-11".to_string()],
    };
    let sprint_issues = vec![
        mock_issue("issue-10", "TT-10", "Auth API", "To Do", "High"),
        mock_issue("issue-11", "TT-11", "Login page", "To Do", "Medium"),
    ];
    let backlog_issues = vec![
        mock_issue("issue-1", "TT-1", "Implement auth", "To Do", "High"),
        mock_issue("issue-2", "TT-2", "Setup CI/CD", "To Do", "Medium"),
        mock_issue("issue-3", "TT-3", "Design system tokens", "To Do", "Low"),
        mock_issue("issue-4", "TT-4", "User profile page", "To Do", "Medium"),
        mock_issue("issue-5", "TT-5", "Email notifications", "To Do", "Low"),
        mock_issue("issue-6", "TT-6", "Upgrade dependencies", "To Do", "Low"),
    ];
    Json(BacklogResponse {
        sprint,
        sprint_issues,
        backlog_issues,
    })
}

#[utoipa::path(
    get,
    path = "/search",
    params(("q" = String, Query, description = "Search query")),
    responses(
        (status = 200, description = "Search results", body = SearchResultResponse)
    )
)]
async fn search_issues(State(_ctx): State<Arc<AppContext>>) -> Json<SearchResultResponse> {
    Json(SearchResultResponse {
        issues: vec![
            mock_issue("issue-7", "TT-7", "OAuth provider integration", "In Progress", "High"),
            mock_issue("issue-10", "TT-10", "Auth API", "To Do", "High"),
        ],
    })
}

#[utoipa::path(
    post,
    path = "/issues",
    request_body = CreateIssueRequest,
    responses(
        (status = 201, description = "Issue created", body = CreateIssueResponse)
    )
)]
async fn create_issue(
    State(_ctx): State<Arc<AppContext>>,
    Json(req): Json<CreateIssueRequest>,
) -> impl IntoResponse {
    let assignee_id = req.assignee_id.clone();
    let assignee_name = assignee_id.as_ref().map(|_| "Ivan".to_string());
    let issue = IssueResponse {
        id: "issue-new".to_string(),
        key: "TT-42".to_string(),
        summary: req.summary,
        description: req.description,
        project_key: req.project_key,
        project_name: "Task Tracker".to_string(),
        status: "To Do".to_string(),
        assignee_id,
        assignee_name,
        reporter_id: "me".to_string(),
        reporter_name: "Me".to_string(),
        priority: req.priority,
        labels: req.labels,
        due_date: req.due_date,
        original_estimate_seconds: req.original_estimate_seconds,
        remaining_estimate_seconds: req.original_estimate_seconds,
        time_spent_seconds: 0,
    };
    Json(CreateIssueResponse { issue })
}

#[utoipa::path(
    get,
    path = "/issues/{id}",
    params(("id" = String, Path, description = "Issue id")),
    responses(
        (status = 200, description = "Issue details", body = IssueResponse),
        (status = 404, description = "Issue not found", body = ErrorResponse)
    )
)]
async fn get_issue(
    State(_ctx): State<Arc<AppContext>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if id == "issue-detail" || id.starts_with("issue-") {
        return Json(mock_issue(
            &id,
            "TT-7",
            "OAuth provider integration",
            "In Progress",
            "High",
        ))
        .into_response();
    }
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "not_found".to_string(),
            message: "Issue not found".to_string(),
        }),
    )
        .into_response()
}

#[utoipa::path(
    post,
    path = "/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse)
    )
)]
async fn login(State(_ctx): State<Arc<AppContext>>, Json(req): Json<LoginRequest>) -> Json<AuthResponse> {
    Json(AuthResponse {
        token: "mock-token".to_string(),
        user: mock_user("user-1", req.email.as_str(), "Anna"),
    })
}

#[utoipa::path(
    post,
    path = "/auth/register",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "Registration successful", body = AuthResponse)
    )
)]
async fn register(
    State(_ctx): State<Arc<AppContext>>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    (
        StatusCode::CREATED,
        Json(AuthResponse {
            token: "mock-token".to_string(),
            user: mock_user("user-1", req.email.as_str(), &req.name),
        }),
    )
}

pub fn openapi_json() -> String {
    serde_json::to_string_pretty(&ApiDoc::openapi()).expect("serialize openapi")
}

fn api_routes(ctx: Arc<AppContext>) -> Router {
    Router::new()
        .route("/health", axum::routing::get(health))
        .route("/projects", axum::routing::get(list_projects))
        .route("/projects/{project_key}/board", axum::routing::get(get_board))
        .route("/projects/{project_key}/backlog", axum::routing::get(get_backlog))
        .route("/search", axum::routing::get(search_issues))
        .route("/issues", axum::routing::post(create_issue))
        .route("/issues/{id}", axum::routing::get(get_issue))
        .route("/auth/login", axum::routing::post(login))
        .route("/auth/register", axum::routing::post(register))
        .with_state(ctx)
}

pub fn router(ctx: Arc<AppContext>) -> Router {
    let swagger = SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi());

    Router::new()
        .merge(swagger)
        .nest("/api/v1", api_routes(ctx))
        .layer(CorsLayer::permissive())
}

pub async fn serve(ctx: Arc<AppContext>) -> anyhow::Result<()> {
    let addr = format!("{}:{}", ctx.config.server_address, ctx.config.server_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Server listening on http://{}", addr);
    axum::serve(listener, router(ctx)).await?;
    Ok(())
}
