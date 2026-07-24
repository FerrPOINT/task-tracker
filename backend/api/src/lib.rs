use axum::{
    Router,
    http::Method,
    middleware::from_fn_with_state,
    routing::{get, post},
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub mod dto;
pub mod middleware;
pub mod routes;

pub use dto::*;
pub use routes::*;

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::health::health,
        routes::auth::register,
        routes::auth::login,
        routes::projects::list_projects,
        routes::projects::create_project,
        routes::projects::get_project,
        routes::board::get_board,
        routes::board::get_backlog,
        routes::board::move_issue,
        routes::issues::create_issue,
        routes::issues::search_issues,
        routes::issues::get_issue,
        routes::issues::update_issue,
        routes::search::search_global,
        routes::dashboard::get_dashboard,
    ),
    components(schemas(
        dto::RegisterRequest,
        dto::LoginRequest,
        dto::AuthResponse,
        dto::ProjectResponse,
        dto::ProjectListResponse,
        dto::CreateProjectRequest,
        dto::IssueResponse,
        dto::IssueListResponse,
        dto::CreateIssueRequest,
        dto::UpdateIssueRequest,
        dto::MoveIssueRequest,
        dto::BoardColumnResponse,
        dto::SprintResponse,
        dto::BoardResponse,
        dto::BacklogResponse,
        dto::DashboardResponse,
    ))
)]
pub struct ApiDoc;

pub fn router(ctx: Arc<app::AppContext>) -> Router<Arc<app::AppContext>> {
    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_origin(Any)
        .allow_headers(Any);

    let public = Router::new()
        .route("/health", get(routes::health::health))
        .route("/auth/register", post(routes::auth::register))
        .route("/auth/login", post(routes::auth::login));

    let auth = from_fn_with_state(ctx.clone(), middleware::auth::bearer_auth);

    let protected = Router::new()
        .route(
            "/projects",
            get(routes::projects::list_projects).post(routes::projects::create_project),
        )
        .route(
            "/projects/{project_key}",
            get(routes::projects::get_project),
        )
        .route(
            "/projects/{project_key}/board",
            get(routes::board::get_board),
        )
        .route(
            "/projects/{project_key}/backlog",
            get(routes::board::get_backlog),
        )
        .route(
            "/projects/{project_key}/board/move",
            post(routes::board::move_issue),
        )
        .route(
            "/issues",
            post(routes::issues::create_issue).get(routes::issues::search_issues),
        )
        .route(
            "/issues/{id}",
            get(routes::issues::get_issue).patch(routes::issues::update_issue),
        )
        .route("/search", get(routes::search::search_global))
        .route("/dashboard", get(routes::dashboard::get_dashboard))
        .route_layer(auth);

    let api = public.merge(protected);

    Router::new()
        .nest("/api/v1", api)
        .merge(SwaggerUi::new("/swagger-ui").url("/api/v1/openapi.json", ApiDoc::openapi()))
        .layer(cors)
}

pub async fn serve(ctx: Arc<app::AppContext>) {
    let listener = tokio::net::TcpListener::bind(&ctx.config.server_addr())
        .await
        .expect("failed to bind");
    axum::serve(listener, router(ctx.clone()).with_state(ctx))
        .await
        .expect("server failed");
}
