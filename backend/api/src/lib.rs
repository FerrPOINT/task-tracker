use axum::{Router, http::Method, routing::{get, post}};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub mod dto;
pub mod routes;

pub use dto::*;
pub use routes::*;

#[derive(OpenApi)]
#[openapi(
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
        dto::BoardColumnResponse,
        dto::SprintResponse,
        dto::BoardResponse,
        dto::BacklogResponse,
        dto::DashboardResponse,
    ))
)]
pub struct ApiDoc;

pub fn router() -> Router<Arc<app::AppContext>> {
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
        .allow_origin(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/v1/health", get(routes::health::health))
        .route("/api/v1/auth/register", post(routes::auth::register))
        .route("/api/v1/auth/login", post(routes::auth::login))
        .route("/api/v1/projects", get(routes::projects::list_projects).post(routes::projects::create_project))
        .route("/api/v1/projects/{project_key}/board", get(routes::board::get_board))
        .route("/api/v1/projects/{project_key}/backlog", get(routes::board::get_backlog))
        .route("/api/v1/issues", post(routes::issues::create_issue).get(routes::issues::search))
        .route("/api/v1/issues/{id}", get(routes::issues::get_issue))
        .route("/api/v1/search", get(routes::search::search))
        .route("/api/v1/dashboard", get(routes::dashboard::get_dashboard))
        .merge(SwaggerUi::new("/swagger-ui").url("/api/v1/openapi.json", ApiDoc::openapi()))
        .layer(cors)
}

pub async fn serve(ctx: Arc<app::AppContext>) {
    let listener = tokio::net::TcpListener::bind(&ctx.config.server_addr())
        .await
        .expect("failed to bind");
    axum::serve(listener, router().with_state(ctx))
        .await
        .expect("server failed");
}
