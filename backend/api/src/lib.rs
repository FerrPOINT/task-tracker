use axum::{
    Router,
    http::HeaderValue,
    http::Method,
    middleware::from_fn_with_state,
    routing::{delete, get, patch, post, put},
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
        routes::auth::refresh_openapi,
        routes::auth::logout_openapi,
        routes::projects::list_projects,
        routes::projects::create_project,
        routes::projects::get_project,
        routes::projects::update_project,
        routes::projects::delete_project,
        routes::members::list_members,
        routes::members::add_member,
        routes::members::remove_member,
        routes::board::get_board,
        routes::board::get_backlog,
        routes::board::move_issue,
        routes::comments::list_comments,
        routes::comments::create_comment,
        routes::comments::update_comment,
        routes::comments::delete_comment,
        routes::issues::create_issue,
        routes::issues::search_issues,
        routes::issues::get_issue,
        routes::issues::update_issue,
        routes::issues::delete_issue,
        routes::transitions::transition_issue,
        routes::search::search_global,
        routes::attachments::list_attachments,
        routes::labels::list_labels,
        routes::labels::create_label,
        routes::labels::update_label,
        routes::labels::delete_label,
        routes::labels::list_issue_labels,
        routes::labels::attach_label,
        routes::labels::detach_label,
        routes::links::list_links,
        routes::links::create_link,
        routes::links::delete_link,
        routes::attachments::upload_attachment,
        routes::attachments::download_attachment,
        routes::attachments::delete_attachment,
        routes::events::events,
        routes::workflow::list_statuses,
        routes::workflow::list_transitions,
        routes::workflow::list_issue_types,
        routes::worklogs::list_worklogs,
        routes::worklogs::create_worklog,
        routes::worklogs::update_worklog,
        routes::worklogs::delete_worklog,
        routes::dashboard::get_dashboard,
        routes::users::get_me,
        routes::users::list_users,
        routes::sprints::list_sprints,
        routes::sprints::create_sprint,
        routes::sprints::get_sprint,
        routes::sprints::update_sprint,
        routes::sprints::start_sprint,
        routes::sprints::close_sprint,
        routes::sprints::move_issue_to_sprint,
        routes::sprints::remove_issue_from_sprint,
        routes::saved_filters::list_filters,
        routes::saved_filters::create_filter,
        routes::saved_filters::get_filter,
        routes::saved_filters::delete_filter,
        routes::saved_filters::execute_filter,
        routes::notifications::list_notifications,
        routes::notifications::mark_notification_read,
        routes::notifications::mark_all_notifications_read,
        routes::notifications::get_notification_settings,
        routes::notifications::update_notification_settings,
    ),
    components(schemas(
        dto::RegisterRequest,
        dto::LoginRequest,
        dto::AuthResponse,
        dto::UserResponse,
        dto::UserListResponse,
        dto::ProjectResponse,
        dto::ProjectListResponse,
        dto::CreateProjectRequest,
        dto::UpdateProjectRequest,
        dto::IssueResponse,
        dto::IssueListResponse,
        dto::CreateIssueRequest,
        dto::UpdateIssueRequest,
        dto::MoveIssueRequest,
        dto::BoardColumnResponse,
        dto::CommentResponse,
        dto::CommentListResponse,
        dto::CreateCommentRequest,
        dto::UpdateCommentRequest,
        dto::WorklogResponse,
        dto::WorklogListResponse,
        dto::CreateWorklogRequest,
        dto::UpdateWorklogRequest,
        dto::SprintResponse,
        dto::SprintListResponse,
        dto::CreateSprintRequest,
        dto::UpdateSprintRequest,
        dto::MoveIssueToSprintRequest,
        dto::BoardResponse,
        dto::BacklogResponse,
        dto::DashboardResponse,
        dto::StatusResponse,
        dto::TransitionResponse,
        dto::IssueTypeResponse,
        crate::dto::AttachmentResponse,
        crate::dto::AttachmentListResponse,
        routes::notifications::NotificationListResponse,
        routes::notifications::NotificationSettingsResponse,
        routes::notifications::UpdateNotificationSettingsRequest,
    ))
)]
pub struct ApiDoc;

pub fn router(ctx: Arc<app::AppContext>) -> Router<Arc<app::AppContext>> {
    let cors = if ctx.config.server.cors_allowed_origins.len() == 1
        && ctx.config.server.cors_allowed_origins[0] == "*"
    {
        CorsLayer::new()
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
            ])
            .allow_origin(Any)
            .allow_headers(Any)
    } else {
        let origins: Vec<HeaderValue> = ctx
            .config
            .server
            .cors_allowed_origins
            .iter()
            .filter(|o| !o.is_empty())
            .map(|o| {
                o.parse::<HeaderValue>()
                    .expect("invalid cors allowed origin")
            })
            .collect();
        let allowed = tower_http::cors::AllowOrigin::list(origins);
        CorsLayer::new()
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
            ])
            .allow_origin(allowed)
            .allow_headers(Any)
    };

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
            get(routes::projects::get_project)
                .patch(routes::projects::update_project)
                .delete(routes::projects::delete_project),
        )
        .route(
            "/projects/{project_id}/members",
            get(routes::members::list_members).post(routes::members::add_member),
        )
        .route(
            "/projects/{project_id}/members/{user_id}",
            delete(routes::members::remove_member),
        )
        .route(
            "/projects/{project_key}/board",
            get(routes::board::get_board),
        )
        .route(
            "/issues/{issue_id}/attachments",
            get(routes::attachments::list_attachments).post(routes::attachments::upload_attachment),
        )
        .route(
            "/projects/{project_key}/labels",
            get(routes::labels::list_labels).post(routes::labels::create_label),
        )
        .route(
            "/labels/{id}",
            put(routes::labels::update_label).delete(routes::labels::delete_label),
        )
        .route(
            "/issues/{issue_id}/labels",
            get(routes::labels::list_issue_labels).post(routes::labels::attach_label),
        )
        .route(
            "/issues/{issue_id}/labels/{label_id}",
            delete(routes::labels::detach_label),
        )
        .route(
            "/issues/{issue_id}/links",
            get(routes::links::list_links).post(routes::links::create_link),
        )
        .route("/issue-links/{id}", delete(routes::links::delete_link))
        .route(
            "/attachments/{id}/download",
            get(routes::attachments::download_attachment),
        )
        .route(
            "/attachments/{id}",
            delete(routes::attachments::delete_attachment),
        )
        .route("/events", get(routes::events::events))
        .route("/statuses", get(routes::workflow::list_statuses))
        .route("/transitions", get(routes::workflow::list_transitions))
        .route("/issue-types", get(routes::workflow::list_issue_types))
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
            get(routes::issues::get_issue)
                .patch(routes::issues::update_issue)
                .delete(routes::issues::delete_issue),
        )
        .route(
            "/issues/{id}/transition",
            post(routes::transitions::transition_issue),
        )
        .route(
            "/issues/{issue_id}/comments",
            get(routes::comments::list_comments).post(routes::comments::create_comment),
        )
        .route(
            "/comments/{id}",
            patch(routes::comments::update_comment).delete(routes::comments::delete_comment),
        )
        .route(
            "/issues/{issue_id}/worklogs",
            get(routes::worklogs::list_worklogs).post(routes::worklogs::create_worklog),
        )
        .route(
            "/worklogs/{id}",
            patch(routes::worklogs::update_worklog).delete(routes::worklogs::delete_worklog),
        )
        .route("/search", get(routes::search::search_global))
        .route(
            "/filters",
            get(routes::saved_filters::list_filters).post(routes::saved_filters::create_filter),
        )
        .route(
            "/filters/{id}",
            get(routes::saved_filters::get_filter).delete(routes::saved_filters::delete_filter),
        )
        .route(
            "/filters/{id}/execute",
            get(routes::saved_filters::execute_filter),
        )
        .route(
            "/notifications",
            get(routes::notifications::list_notifications),
        )
        .route(
            "/notifications/{id}/read",
            patch(routes::notifications::mark_notification_read),
        )
        .route(
            "/notifications/read-all",
            post(routes::notifications::mark_all_notifications_read),
        )
        .route(
            "/notification-settings",
            get(routes::notifications::get_notification_settings)
                .patch(routes::notifications::update_notification_settings),
        )
        .route("/dashboard", get(routes::dashboard::get_dashboard))
        .route("/auth/refresh", post(routes::auth::refresh))
        .route("/auth/logout", post(routes::auth::logout))
        .route("/users/me", get(routes::users::get_me))
        .route("/users", get(routes::users::list_users))
        .route(
            "/projects/{project_key}/sprints",
            get(routes::sprints::list_sprints).post(routes::sprints::create_sprint),
        )
        .route(
            "/projects/{project_key}/sprints/{sprint_id}",
            get(routes::sprints::get_sprint).patch(routes::sprints::update_sprint),
        )
        .route(
            "/projects/{project_key}/sprints/{sprint_id}/start",
            post(routes::sprints::start_sprint),
        )
        .route(
            "/projects/{project_key}/sprints/{sprint_id}/close",
            post(routes::sprints::close_sprint),
        )
        .route(
            "/projects/{project_key}/sprints/{sprint_id}/issues",
            post(routes::sprints::move_issue_to_sprint),
        )
        .route(
            "/projects/{project_key}/sprints/{sprint_id}/remove-issue",
            post(routes::sprints::remove_issue_from_sprint),
        )
        .route_layer(auth);

    let api = public.merge(protected);

    Router::new()
        .nest("/api/v1", api)
        .merge(SwaggerUi::new("/swagger-ui").url("/api/v1/openapi.json", ApiDoc::openapi()))
        .layer(cors)
}

pub async fn bind(ctx: Arc<app::AppContext>) -> Result<tokio::net::TcpListener, std::io::Error> {
    tokio::net::TcpListener::bind(&ctx.config.server_addr()).await
}

pub async fn serve_forever(
    listener: tokio::net::TcpListener,
    ctx: Arc<app::AppContext>,
) -> Result<(), std::io::Error> {
    axum::serve(listener, router(ctx.clone()).with_state(ctx)).await
}

pub async fn serve(ctx: Arc<app::AppContext>) {
    let listener = bind(ctx.clone()).await.expect("failed to bind");
    serve_forever(listener, ctx).await.expect("server failed");
}
