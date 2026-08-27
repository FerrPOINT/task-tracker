use axum::{
    Router,
    http::HeaderName,
    http::HeaderValue,
    http::Method,
    middleware::from_fn_with_state,
    routing::{delete, get, patch, post, put},
};
use axum_prometheus::{GenericMetricLayer, PrometheusMetricLayer};
use metrics_exporter_prometheus::PrometheusHandle;
use std::sync::{Arc, OnceLock};
use tower::ServiceBuilder;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

/// Global Prometheus metrics handle — initialized once, reused across router builds.
static METRIC_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Return the global Prometheus handle, initializing the recorder on first call.
fn metric_handle() -> PrometheusHandle {
    METRIC_HANDLE
        .get_or_init(|| {
            let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
            let handle = recorder.handle();
            metrics::set_global_recorder(Box::new(recorder))
                .expect("failed to set global metrics recorder");
            handle
        })
        .clone()
}

/// A key extractor for tower-governor that tries to get the client IP from
/// `X-Forwarded-For`, `X-Real-Ip`, `Forwarded` headers, then `ConnectInfo`,
/// and finally falls back to `0.0.0.0` instead of returning an error.
///
/// This is more lenient than `PeerIpKeyExtractor` / `SmartIpKeyExtractor`
/// and ensures the rate limiter never produces a 500 when IP extraction
/// fails (e.g. in test environments without real connections).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FallbackIpKeyExtractor;

impl tower_governor::key_extractor::KeyExtractor for FallbackIpKeyExtractor {
    type Key = std::net::IpAddr;

    fn extract<T>(
        &self,
        req: &axum::http::Request<T>,
    ) -> Result<Self::Key, tower_governor::GovernorError> {
        // Try SmartIpKeyExtractor logic first, fall back to 0.0.0.0.
        Ok(tower_governor::key_extractor::SmartIpKeyExtractor
            .extract(req)
            .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)))
    }
}

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
        routes::issues::restore_issue,
        routes::issues::purge_issue,
        routes::issues::list_trash,
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
        routes::users::get_users_me,
        routes::users::list_users,
        routes::sprints::list_sprints,
        routes::sprints::create_sprint,
        routes::sprints::get_sprint,
        routes::sprints::update_sprint,
        routes::sprints::start_sprint,
        routes::sprints::close_sprint,
        routes::sprints::move_issue_to_sprint,
        routes::sprints::remove_issue_from_sprint,
        routes::notifications::list_notifications,
        routes::notifications::mark_notification_read,
        routes::notifications::mark_all_notifications_read,
        routes::notifications::get_notification_settings,
        routes::notifications::update_notification_settings,
        routes::reports::get_velocity_report,
        routes::reports::get_burndown_report,
        routes::reports::get_cumulative_flow_report,
        routes::reports::get_control_chart_report,
        routes::admin::list_users,
        routes::admin::create_user,
        routes::admin::update_user_status,
        routes::admin::list_audit_logs,
        routes::admin::list_system_settings,
        routes::admin::update_system_setting,
        routes::watchers_votes::watch_issue,
        routes::watchers_votes::unwatch_issue,
        routes::watchers_votes::list_watchers,
        routes::custom_fields::list_custom_fields,
        routes::custom_fields::create_custom_field,
        routes::custom_fields::update_custom_field,
        routes::custom_fields::delete_custom_field,
        routes::custom_fields::list_issue_custom_field_values,
        routes::custom_fields::set_custom_field_value,
        routes::watchers_votes::vote_issue,
        routes::watchers_votes::unvote_issue,
        routes::watchers_votes::list_votes,
        routes::components_versions::list_components,
        routes::components_versions::create_component,
        routes::components_versions::update_component,
        routes::components_versions::delete_component,
        routes::components_versions::list_versions,
        routes::components_versions::create_version,
        routes::components_versions::update_version,
        routes::components_versions::delete_version,
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
        routes::reports::VelocityResponse,
        routes::reports::VelocitySprintResponse,
        routes::reports::BurndownResponse,
        routes::reports::BurndownPointResponse,
        routes::reports::CumulativeFlowResponse,
        routes::reports::CumulativeFlowPointResponse,
        routes::reports::ControlChartResponse,
        routes::reports::ControlChartPointResponse,
        routes::admin::AdminUserResponse,
        routes::admin::AdminUserListResponse,
        routes::admin::AdminCreateUserRequest,
        routes::admin::UpdateUserStatusRequest,
        routes::admin::AuditLogResponse,
        routes::admin::AuditLogListResponse,
        routes::admin::SystemSettingResponse,
        routes::admin::SystemSettingListResponse,
        routes::admin::UpdateSystemSettingRequest,
        routes::watchers_votes::WatchRequest,
        routes::watchers_votes::WatcherResponse,
        routes::watchers_votes::WatcherListResponse,
        routes::custom_fields::CreateCustomFieldRequest,
        routes::custom_fields::UpdateCustomFieldRequest,
        routes::custom_fields::SetCustomFieldValueRequest,
        routes::custom_fields::CustomFieldResponse,
        routes::custom_fields::CustomFieldListResponse,
        routes::custom_fields::CustomFieldValueResponse,
        routes::custom_fields::CustomFieldValueListResponse,
        routes::watchers_votes::VoteResponse,
        routes::watchers_votes::VoteListResponse,
        routes::watchers_votes::VoteCountResponse,
        routes::watchers_votes::WatchStatusResponse,
        routes::watchers_votes::VoteStatusResponse,
        routes::components_versions::ComponentRequest,
        routes::components_versions::ComponentResponse,
        routes::components_versions::ComponentListResponse,
        routes::components_versions::VersionRequest,
        routes::components_versions::VersionResponse,
        routes::components_versions::VersionListResponse,
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

    // Rate limiter for auth endpoints (configurable, default 5 requests per 15 seconds per IP).
    let auth_limiter = GovernorConfigBuilder::default()
        .key_extractor(FallbackIpKeyExtractor)
        .period(std::time::Duration::from_secs(
            ctx.config.server.auth_rate_period_secs,
        ))
        .burst_size(ctx.config.server.auth_rate_burst)
        .finish()
        .expect("valid auth rate limit config");

    // General rate limiter for all API endpoints (default 60 requests per 60 seconds per IP).
    let general_limiter = GovernorConfigBuilder::default()
        .key_extractor(FallbackIpKeyExtractor)
        .period(std::time::Duration::from_secs(
            ctx.config.server.general_rate_period_secs,
        ))
        .burst_size(ctx.config.server.general_rate_burst)
        .finish()
        .expect("valid general rate limit config");

    let public = Router::new().route("/health", get(routes::health::health));

    // Auth endpoints get stricter rate limiting: 5 requests per 15 seconds per IP.
    let auth_routes = Router::new()
        .route("/auth/register", post(routes::auth::register))
        .route("/auth/login", post(routes::auth::login))
        // Refresh must stay public: it exists precisely for the moment the
        // access token has expired, so it cannot require a valid bearer.
        .route("/auth/refresh", post(routes::auth::refresh))
        .layer(GovernorLayer::new(auth_limiter));

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
            "/projects/{project_key}/members",
            get(routes::members::list_members).post(routes::members::add_member),
        )
        .route(
            "/projects/{project_key}/members/{user_id}",
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
            "/issues/{issue_id}/watch",
            post(routes::watchers_votes::watch_issue).delete(routes::watchers_votes::unwatch_issue),
        )
        .route(
            "/issues/{issue_id}/watchers",
            get(routes::watchers_votes::list_watchers),
        )
        .route(
            "/issues/{issue_id}/vote",
            post(routes::watchers_votes::vote_issue).delete(routes::watchers_votes::unvote_issue),
        )
        .route(
            "/issues/{issue_id}/votes",
            get(routes::watchers_votes::list_votes),
        )
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
        .route("/projects/{key}/trash", get(routes::issues::list_trash))
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
        .route("/issues/{id}/restore", post(routes::issues::restore_issue))
        .route("/issues/{id}/trash", delete(routes::issues::purge_issue))
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
        .route("/auth/logout", post(routes::auth::logout))
        .route("/auth/me", get(routes::users::get_me))
        .route("/users/me", get(routes::users::get_users_me))
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
        .route(
            "/reports/velocity",
            get(routes::reports::get_velocity_report),
        )
        .route(
            "/reports/burndown",
            get(routes::reports::get_burndown_report),
        )
        .route(
            "/reports/cumulative-flow",
            get(routes::reports::get_cumulative_flow_report),
        )
        .route(
            "/reports/control-chart",
            get(routes::reports::get_control_chart_report),
        )
        .route(
            "/admin/users",
            get(routes::admin::list_users).post(routes::admin::create_user),
        )
        .route(
            "/admin/users/{id}/status",
            put(routes::admin::update_user_status),
        )
        .route("/admin/audit-log", get(routes::admin::list_audit_logs))
        .route(
            "/admin/system-settings",
            get(routes::admin::list_system_settings).put(routes::admin::update_system_setting),
        )
        .route(
            "/projects/{project_key}/components",
            get(routes::components_versions::list_components)
                .post(routes::components_versions::create_component),
        )
        .route(
            "/projects/{project_key}/components/{component_id}",
            put(routes::components_versions::update_component)
                .delete(routes::components_versions::delete_component),
        )
        .route(
            "/projects/{project_key}/versions",
            get(routes::components_versions::list_versions)
                .post(routes::components_versions::create_version),
        )
        .route(
            "/projects/{project_key}/versions/{version_id}",
            put(routes::components_versions::update_version)
                .delete(routes::components_versions::delete_version),
        )
        .route(
            "/projects/{project_key}/custom-fields",
            get(routes::custom_fields::list_custom_fields)
                .post(routes::custom_fields::create_custom_field),
        )
        .route(
            "/custom-fields/{id}",
            put(routes::custom_fields::update_custom_field)
                .delete(routes::custom_fields::delete_custom_field),
        )
        .route(
            "/issues/{issue_id}/custom-fields",
            get(routes::custom_fields::list_issue_custom_field_values),
        )
        .route(
            "/issues/{issue_id}/custom-fields/{field_id}/value",
            put(routes::custom_fields::set_custom_field_value),
        )
        .route_layer(auth);

    let api = public.merge(auth_routes).merge(protected);

    // Prometheus metrics layer + handle for the /metrics endpoint.
    let handle = metric_handle();
    let prometheus_layer: PrometheusMetricLayer = GenericMetricLayer::new();

    Router::new()
        .route("/metrics", get(move || std::future::ready(handle.render())))
        .nest(
            "/api/v1",
            api.layer(GovernorLayer::new(general_limiter)),
        )
        .merge(SwaggerUi::new("/swagger-ui").url("/api/v1/openapi.json", ApiDoc::openapi()))
        .layer(
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::overriding(
                    axum::http::header::X_CONTENT_TYPE_OPTIONS,
                    HeaderValue::from_static("nosniff"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("x-frame-options"),
                    HeaderValue::from_static("DENY"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("x-xss-protection"),
                    HeaderValue::from_static("1; mode=block"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("referrer-policy"),
                    HeaderValue::from_static("no-referrer"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("content-security-policy"),
                    HeaderValue::from_static(
                        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; font-src 'self'",
                    ),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    HeaderName::from_static("strict-transport-security"),
                    HeaderValue::from_static("max-age=31536000; includeSubDomains"),
                )),
        )
        .layer(prometheus_layer)
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
