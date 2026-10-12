use axum::{
    Router,
    extract::DefaultBodyLimit,
    handler::Handler,
    http::HeaderName,
    http::HeaderValue,
    middleware::from_fn_with_state,
    routing::{delete, get, patch, post, put},
};
use axum_prometheus::{GenericMetricLayer, PrometheusMetricLayer};
use metrics_exporter_prometheus::PrometheusHandle;
use std::sync::{Arc, OnceLock};
use tower::ServiceBuilder;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_http::set_header::SetResponseHeaderLayer;
use utoipa::openapi::{
    info::License,
    path::Operation,
    security::{HttpAuthScheme, HttpBuilder, SecurityRequirement, SecurityScheme},
};
use utoipa::{Modify, OpenApi};
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

fn rate_per_second_period(rate_per_second: u64) -> std::time::Duration {
    // tower-governor uses the configured duration as the time for ONE permit.
    // Nanosecond precision keeps configured rates accurate above 1,000 rps
    // and avoids inflating rates through millisecond rounding.
    std::time::Duration::from_nanos(1_000_000_000 / rate_per_second.max(1))
}

#[derive(OpenApi)]
#[openapi(
    modifiers(&SecurityAddon),
    paths(
        routes::sdlc_routing::set,
        routes::sdlc_routing::current,
        routes::sdlc_routing::version,
        routes::sdlc_routing::operation,
        routes::sdlc_routing::snapshot,
        routes::sdlc_reservation::reserve,
        routes::sdlc_reservation::heartbeat,
        routes::sdlc_reservation::current,
        routes::sdlc_reservation::configuration_preflight,
        routes::sdlc_reservation::operation,
        routes::sdlc::project_access,
        routes::sdlc::project_directory,
        routes::sdlc::create_draft,
        routes::sdlc::draft_creation_operation,
        routes::sdlc::context,
        routes::sdlc::analysis_intent,
        routes::sdlc::pm_draft_input,
        routes::sdlc::pm_draft_assignment,
        routes::sdlc::reserve_pm_draft,
        routes::sdlc::claim_execution_lease,
        routes::sdlc::heartbeat_execution_lease,
        routes::sdlc::execution_lease,
        routes::sdlc::bind,
        routes::sdlc::assign,
        routes::sdlc::questions,
        routes::sdlc::publish_question,
        routes::sdlc::answer,
        routes::sdlc::cancel,
        routes::sdlc::revisions,
        routes::sdlc::current_revision,
        routes::sdlc::publish_revision,
        routes::sdlc::revision,
        routes::sdlc::diff,
        routes::sdlc::confirm,
        routes::sdlc::evidence,
        routes::sdlc::events,
        routes::namespace::apply,
        routes::namespace::available_resources,
        routes::namespace::stats,
        routes::namespace::contexts,
        routes::namespace::project_catalog,
        routes::namespace::context,
        routes::namespace::documents,
        routes::namespace::repositories,
        routes::namespace::delivery_evidence,
        routes::namespace::link_repository,
        routes::namespace::available_repositories,
        routes::namespace::readback,
        routes::namespace::task_context,
        routes::namespace::task_catalog,
        routes::health::catalog_health,
        routes::health::health,
        routes::auth::register,
        routes::auth::login,
        routes::auth::refresh_openapi,
        routes::auth::totp_setup,
        routes::auth::totp_enable,
        routes::auth::totp_disable,
        routes::auth::password_reset_request,
        routes::auth::password_reset_confirm,
        routes::auth::oidc_begin,
        routes::auth::oidc_callback,
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
        routes::exports::export_csv,
        routes::exports::export_json,
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
        routes::admin::AuditLogResponse,
        routes::admin::AuditLogListResponse,
        routes::admin::SystemSettingResponse,
        routes::admin::SystemSettingListResponse,
        routes::admin::UpdateSystemSettingRequest,
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

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let mut license =
            License::new("FerrPOINT Proprietary Source-Available Evaluation License v1.0");
        license.url = Some("./LICENSE".to_string());
        openapi.info.license = Some(license);

        let components = openapi
            .components
            .get_or_insert_with(utoipa::openapi::Components::new);

        components.add_security_scheme(
            "bearer",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );

        for (path, item) in openapi.paths.paths.iter_mut() {
            let security = match path.as_str() {
                "/health"
                | "/api/v1/health"
                | "/api/v1/auth/login"
                | "/api/v1/auth/register"
                | "/api/v1/auth/refresh" => None,
                _ => Some(bearer_security()),
            };

            apply_security(item.get.as_mut(), security.clone());
            apply_security(item.put.as_mut(), security.clone());
            apply_security(item.post.as_mut(), security.clone());
            apply_security(item.delete.as_mut(), security.clone());
            apply_security(item.patch.as_mut(), security.clone());
        }
    }
}

fn bearer_security() -> Vec<SecurityRequirement> {
    vec![SecurityRequirement::new("bearer", Vec::<String>::new())]
}

fn apply_security(operation: Option<&mut Operation>, security: Option<Vec<SecurityRequirement>>) {
    if let (Some(operation), Some(security)) = (operation, security) {
        operation.security = Some(security);
    }
}

pub fn router(ctx: Arc<app::AppContext>) -> Router<Arc<app::AppContext>> {
    let cors = sdlc_shared::cors::cors_layer(&ctx.config.server.cors_allowed_origins);

    // Rate limiter for auth endpoints (configurable, default 5 requests per 15 seconds per IP).
    let auth_limiter = GovernorConfigBuilder::default()
        .key_extractor(FallbackIpKeyExtractor)
        .period(std::time::Duration::from_secs(
            ctx.config.server.auth_rate_period_secs,
        ))
        .burst_size(ctx.config.server.auth_rate_burst)
        .finish()
        .expect("valid auth rate limit config");

    // General API throughput: a GCRA token bucket. `general_rate_per_second`
    // is converted to the duration for ONE permit; `burst_size` is the bucket
    // capacity a single interactive page load (board = ~8 parallel queries +
    // SSE) can spend without waiting.
    let general_limiter = GovernorConfigBuilder::default()
        .key_extractor(FallbackIpKeyExtractor)
        .period(rate_per_second_period(
            ctx.config.server.general_rate_per_second,
        ))
        .burst_size(ctx.config.server.general_rate_burst)
        .finish()
        .expect("valid general rate limit config");

    // /health stays public AND unthrottled: container/orchestrator probes
    // share one IP key with real traffic and must not be rate-limited.
    let public = Router::new();

    // Auth endpoints get stricter rate limiting: 5 requests per 15 seconds per IP.
    let auth_routes = Router::new()
        .route("/auth/register", post(routes::auth::register))
        .route("/auth/login", post(routes::auth::login))
        .route(
            "/auth/password/request",
            post(routes::auth::password_reset_request),
        )
        .route(
            "/auth/password/reset",
            post(routes::auth::password_reset_confirm),
        )
        // Refresh must stay public: it exists precisely for the moment the
        // access token has expired, so it cannot require a valid bearer.
        .route("/auth/refresh", post(routes::auth::refresh))
        // OIDC SSO (SYSTEM_ADMIN 4.2): browser redirect flow, no bearer.
        .route("/auth/oidc/begin", get(routes::auth::oidc_begin))
        .route("/auth/oidc/callback", get(routes::auth::oidc_callback))
        .layer(GovernorLayer::new(auth_limiter));
    let auth_routes = if std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_some() {
        Router::new()
    } else {
        auth_routes
    };

    let auth = from_fn_with_state(ctx.clone(), middleware::auth::bearer_auth);

    let protected = Router::new()
        .route("/tasks/{id}/documents", get(routes::namespace::documents))
        .route(
            "/tasks/{id}/delivery-evidence",
            get(routes::namespace::delivery_evidence),
        )
        .route(
            "/tasks/{id}/repositories",
            get(routes::namespace::repositories).post(routes::namespace::link_repository),
        )
        .route(
            "/tasks/{id}/available-repositories",
            get(routes::namespace::available_repositories),
        )
        .route("/namespace-contexts", get(routes::namespace::contexts))
        .route(
            "/namespace-available-resources",
            get(routes::namespace::available_resources),
        )
        .route(
            "/namespace-stats/{registry}/{namespace}",
            get(routes::namespace::stats),
        )
        .route(
            "/namespace-contexts/{registry}/{namespace}",
            get(routes::namespace::context),
        )
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
            get(routes::attachments::list_attachments).post(
                routes::attachments::upload_attachment
                    .layer(DefaultBodyLimit::max(ctx.config.storage.max_upload_bytes)),
            ),
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
        // NOTE: /events is mounted on its own router above (outside the
        // general rate limiter) — long-lived SSE reconnects must not burn
        // the shared per-IP burst bucket.
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
        .route("/export/csv", post(routes::exports::export_csv))
        .route("/export/json", post(routes::exports::export_json))
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

    let owner_routes = Router::new()
        .route(
            "/namespace-resources/tracker_project/{id}",
            get(routes::namespace::readback).put(routes::namespace::apply),
        )
        .route_layer(axum::middleware::from_fn(routes::namespace::owner_auth))
        .merge(
            Router::new()
                .route(
                    "/namespace-tasks/{id}",
                    get(routes::namespace::task_context),
                )
                .route("/namespace-tasks", get(routes::namespace::task_catalog))
                .route(
                    "/namespace-projects",
                    get(routes::namespace::project_catalog),
                )
                .route_layer(axum::middleware::from_fn(routes::namespace::reader_auth)),
        );
    let api = public
        .merge(auth_routes)
        .merge(protected)
        .merge(owner_routes)
        .merge(routes::sdlc::router());

    // The SSE stream is a long-lived connection, not a request/response the
    // burst limiter was designed for: each reconnect burns a permit from the
    // shared per-IP bucket and one flaky network moment can starve every
    // regular API call from the same client. Mount it outside the general
    // limiter (auth itself still applies via the auth middleware).
    let events_router = Router::new()
        .route("/events", get(routes::events::events))
        .layer(from_fn_with_state(
            ctx.clone(),
            middleware::auth::bearer_auth,
        ));

    // Prometheus metrics layer; /metrics exposure is configurable so production
    // deployments can keep the route internal or disabled at the edge.
    let prometheus_layer: PrometheusMetricLayer = GenericMetricLayer::new();

    let mut root = Router::new()
        .route("/health", get(routes::health::catalog_health))
        .route("/api/v1/health", get(routes::health::health))
        .nest("/api/v1", api.layer(GovernorLayer::new(general_limiter)))
        .nest("/api/v1", events_router)
        .merge(SwaggerUi::new("/swagger-ui").url("/api/v1/openapi.json", ApiDoc::openapi()));

    if ctx.config.metrics.public {
        let handle = metric_handle();
        root = root.route("/metrics", get(move || std::future::ready(handle.render())));
    }

    root
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
        .layer(axum::middleware::from_fn(sdlc_telemetry::request_id_mw))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn project_access_schema_is_strict_and_read_authenticated() {
        let schema = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let route = &schema["paths"]["/api/v1/sdlc/project-access"]["get"];
        assert_eq!(route["security"], serde_json::json!([{ "bearer": [] }]));
        assert_eq!(
            route["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ProjectAccess"
        );
        let response = &schema["components"]["schemas"]["ProjectAccess"];
        assert_eq!(response["additionalProperties"], false);
        assert_eq!(response["properties"].as_object().unwrap().len(), 3);
        assert_eq!(response["required"].as_array().unwrap().len(), 3);
        assert_eq!(
            response["properties"]["project_ids"]["items"]["format"],
            "uuid"
        );
    }

    #[test]
    fn project_directory_schema_is_strict_required_nullable_and_read_authenticated() {
        let schema = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let route = &schema["paths"]["/api/v1/sdlc/project-directory"]["get"];
        assert_eq!(route["security"], serde_json::json!([{ "bearer": [] }]));
        assert_eq!(
            route["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ProjectDirectory"
        );
        for status in ["400", "401", "403", "409", "422", "503"] {
            assert!(route["responses"][status].is_object());
        }
        let params = route["parameters"].as_array().unwrap();
        assert_eq!(params.len(), 2);
        let after = params.iter().find(|p| p["name"] == "after").unwrap();
        assert_eq!(after["schema"]["format"], "uuid");
        assert_eq!(after["required"], false);
        let limit = params.iter().find(|p| p["name"] == "limit").unwrap();
        assert_eq!(limit["required"], false);
        assert_eq!(limit["schema"]["minimum"], 1);
        assert_eq!(limit["schema"]["maximum"], 100);
        assert_eq!(limit["schema"]["default"], 50);
        for (name, fields) in [
            (
                "ProjectDirectory",
                vec![
                    "contract_version",
                    "tracker_instance_id",
                    "projects",
                    "next_cursor",
                ],
            ),
            ("ProjectDirectoryEntry", vec!["id", "key", "name"]),
        ] {
            let response = &schema["components"]["schemas"][name];
            assert_eq!(response["additionalProperties"], false);
            assert_eq!(
                response["properties"].as_object().unwrap().len(),
                fields.len()
            );
            let required = response["required"].as_array().unwrap();
            assert_eq!(required.len(), fields.len());
            for field in fields {
                assert!(required.contains(&serde_json::json!(field)));
            }
        }
        let response = &schema["components"]["schemas"]["ProjectDirectory"];
        assert_eq!(response["properties"]["contract_version"]["minimum"], 1);
        assert_eq!(response["properties"]["contract_version"]["maximum"], 1);
        assert_eq!(response["properties"]["next_cursor"]["format"], "uuid");
        assert!(
            response["properties"]["next_cursor"]["type"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("null"))
        );
    }

    #[test]
    fn draft_creation_schema_matches_strict_typed_wire() {
        let schema = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let readback = &schema["paths"]["/api/v1/projects/{project_id}/sdlc/drafts/operations/{idempotency_key}"]
            ["get"];
        assert_eq!(readback["security"], serde_json::json!([{"bearer": []}]));
        assert_eq!(
            readback["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/CreatedDraft"
        );
        for status in ["401", "403", "404", "409", "422", "503"] {
            assert!(readback["responses"][status].is_object());
        }
        let operation = &schema["paths"]["/api/v1/projects/{project_id}/sdlc/drafts"]["post"];
        assert_eq!(operation["security"], serde_json::json!([{ "bearer": [] }]));
        for status in ["200", "201"] {
            assert_eq!(
                operation["responses"][status]["content"]["application/json"]["schema"]["$ref"],
                "#/components/schemas/CreatedDraft"
            );
        }
        for (name, fields) in [
            (
                "CreateDraftCommand",
                vec!["title", "description", "idempotency_key"],
            ),
            (
                "CreatedDraft",
                vec![
                    "tracker_instance_id",
                    "project_id",
                    "task_id",
                    "root_task_id",
                    "task_key",
                    "owner_subject",
                    "stage",
                ],
            ),
        ] {
            let object = &schema["components"]["schemas"][name];
            assert_eq!(object["additionalProperties"], false);
            assert_eq!(
                object["properties"].as_object().unwrap().len(),
                fields.len()
            );
            assert_eq!(object["required"].as_array().unwrap().len(), fields.len());
            for field in fields {
                assert!(object["properties"][field].is_object());
                assert!(
                    object["required"]
                        .as_array()
                        .unwrap()
                        .contains(&serde_json::json!(field))
                );
            }
        }
        assert_eq!(
            schema["components"]["schemas"]["DraftStage"]["enum"],
            serde_json::json!(["Draft"])
        );
    }

    #[test]
    fn pm_draft_input_schema_is_strict_authenticated_and_separate_from_created_draft() {
        let schema = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let route = &schema["paths"]["/api/v1/issues/{id}/sdlc/pm-draft-input"]["get"];
        assert_eq!(route["security"], serde_json::json!([{"bearer": []}]));
        assert_eq!(
            route["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/PmDraftInputResponse"
        );
        for (name, fields) in [
            ("PmDraftInputResponse", 7),
            ("PmDraftInput", 4),
            ("CreatedDraft", 7),
        ] {
            let object = &schema["components"]["schemas"][name];
            assert_eq!(object["additionalProperties"], false);
            assert_eq!(object["properties"].as_object().unwrap().len(), fields);
            assert_eq!(object["required"].as_array().unwrap().len(), fields);
        }
        let input = &schema["components"]["schemas"]["PmDraftInput"]["properties"];
        assert_eq!(input["snapshot_ref"]["format"], "uuid");
        assert_eq!(input["sha256"]["pattern"], "^[0-9a-f]{64}$");
        assert_eq!(
            schema["components"]["schemas"]["PmDraftInputResponse"]["properties"]["contract_version"]
                ["maximum"].as_f64(),
            Some(1.0)
        );
    }

    #[test]
    fn metadata_outbox_schema_pins_projection_and_strict_resource_variants() {
        let schema = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let schemas = &schema["components"]["schemas"];
        let route = &schema["paths"]["/api/v1/issues/{id}/sdlc/events"]["get"];
        assert_eq!(route["operationId"], "sdlc_events");
        for name in ["projection", "after", "limit", "max_bytes"] {
            let parameter = route["parameters"]
                .as_array()
                .unwrap()
                .iter()
                .find(|parameter| parameter["name"] == name)
                .unwrap();
            assert_eq!(parameter["in"], "query", "{name}");
            assert_ne!(parameter["required"], true, "{name}");
        }
        assert_eq!(route["security"], serde_json::json!([{"bearer": []}]));
        assert_eq!(
            schemas["MetadataProjection"]["enum"],
            serde_json::json!(["metadata_v1"])
        );
        assert_eq!(schemas["MetadataPage"]["additionalProperties"], false);
        assert_eq!(
            schemas["MetadataPage"]["required"]
                .as_array()
                .unwrap()
                .len(),
            6
        );
        assert_eq!(
            schemas["MetadataError"]["required"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
        assert_eq!(
            schemas["MetadataPage"]["properties"]["next_after"]["type"],
            "string"
        );
        assert_eq!(
            schemas["OutboxEvent"]["properties"]["sequence"]["type"],
            "integer"
        );
        assert_eq!(
            schemas["OutboxResponse"]["required"],
            serde_json::json!(["events"])
        );
        let variants = schemas["MetadataEvent"]["oneOf"].as_array().unwrap();
        assert_eq!(variants.len(), 11);
        for variant in variants {
            assert_eq!(variant["additionalProperties"], false);
            assert_eq!(variant["required"].as_array().unwrap().len(), 7);
            assert_eq!(variant["properties"]["sequence"]["type"], "string");
            assert_eq!(variant["properties"]["created_at"]["format"], "date-time");
            let payload = &variant["properties"]["payload"];
            assert_eq!(payload["additionalProperties"], false);
            assert_eq!(payload["required"].as_array().unwrap().len(), 7);
            let kind = variant["properties"]["event_type"]["enum"][0]
                .as_str()
                .unwrap();
            assert!(kind.contains('.'), "wire event name: {kind}");
            if kind == "clarification.published" || kind == "clarification.cancelled" {
                let state = &payload["properties"]["resource"]["properties"]["state"];
                let state = if let Some(reference) = state["$ref"].as_str() {
                    &schemas[reference.strip_prefix("#/components/schemas/").unwrap()]
                } else {
                    state
                };
                assert_eq!(
                    state["enum"],
                    serde_json::json!([if kind == "clarification.published" {
                        "open"
                    } else {
                        "cancelled"
                    }])
                );
            }
        }
        for (name, object) in schemas.as_object().unwrap() {
            if name.starts_with("MetadataPayload_") {
                assert_eq!(object["additionalProperties"], false, "{name}");
                assert_eq!(object["required"].as_array().unwrap().len(), 7, "{name}");
            }
        }
    }

    #[test]
    fn analysis_intent_readback_is_strict_and_not_a_dispatch_command() {
        let schema = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let path = &schema["paths"]["/api/v1/issues/{id}/sdlc/analysis-intent"];
        assert!(path["post"].is_null());
        assert_eq!(path["get"]["security"], serde_json::json!([{"bearer": []}]));
        let object = &schema["components"]["schemas"]["AnalysisIntent"];
        assert_eq!(object["additionalProperties"], false);
        assert_eq!(object["properties"].as_object().unwrap().len(), 19);
        assert_eq!(object["required"].as_array().unwrap().len(), 19);
        assert_eq!(
            schema["components"]["schemas"]["AnalysisStage"]["enum"],
            serde_json::json!(["Analysis"])
        );
        assert_eq!(
            schema["components"]["schemas"]["AnalysisStatus"]["enum"],
            serde_json::json!(["Ready"])
        );
    }

    #[test]
    fn root_binding_schema_does_not_offer_child_materialization_authority() {
        let schema = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let binding = &schema["paths"]["/api/v1/issues/{id}/sdlc/binding"]["post"];
        assert_eq!(binding["security"], serde_json::json!([{"bearer": []}]));
        assert!(
            binding["responses"]["422"]["description"]
                .as_str()
                .unwrap()
                .contains("accepted Architect decomposition")
        );
        let command = &schema["components"]["schemas"]["BindCommand"];
        assert_eq!(command["additionalProperties"], false);
        assert_eq!(command["properties"].as_object().unwrap().len(), 2);
        assert_eq!(command["required"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn pm_draft_reservation_is_strict_and_never_dispatch_capability() {
        let doc = serde_json::to_value(ApiDoc::openapi()).unwrap();
        for (name, count) in [
            ("ReservePmDraft", 4),
            ("PmDraftReservation", 10),
            ("PmDraftReadback", 5),
            ("PmDraftBinding", 5),
        ] {
            let schema = &doc["components"]["schemas"][name];
            assert_eq!(schema["additionalProperties"], false, "{name}");
            assert_eq!(
                schema["required"].as_array().unwrap().len(),
                count,
                "{name}"
            );
        }
        let reserved = &doc["components"]["schemas"]["PmDraftReservation"];
        assert_eq!(
            reserved["properties"]["dispatch_allowed"]["enum"],
            serde_json::json!([false])
        );
        assert_eq!(
            doc["components"]["schemas"]["PmAssignment"]["required"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        let path = &doc["paths"]["/api/v1/issues/{id}/sdlc/pm-draft-assignment"];
        assert!(path["get"]["responses"]["200"].is_object());
        for status in ["200", "201", "401", "403", "404", "409", "422", "503"] {
            assert!(path["post"]["responses"][status].is_object());
        }
    }

    #[test]
    fn sdlc_revision_schema_matches_flat_strict_wire_document() {
        let schema = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let revision = &schema["components"]["schemas"]["RequirementsRevision"];
        assert!(revision.get("allOf").is_none());
        assert!(revision["properties"]["goal"].is_object());
        assert!(revision["properties"]["checklist"].is_object());
        assert!(revision["properties"]["revision"].is_object());
        assert_eq!(revision["additionalProperties"], false);
        assert_eq!(
            revision["properties"]["revision"]["maximum"].as_f64(),
            Some(9007199254740991f64)
        );
    }

    #[test]
    fn ownership_lease_schema_is_strict_separate_and_never_dispatch_authority() {
        let doc = serde_json::to_value(ApiDoc::openapi()).unwrap();
        for (name, fields) in [
            ("ClaimExecutionLease", 3),
            ("HeartbeatExecutionLease", 5),
            ("ExecutionLeaseReceipt", 8),
            ("ExecutionLeaseReadback", 9),
            ("ExecutionLease", 6),
            ("ExecutionLeaseOperation", 3),
        ] {
            let schema = &doc["components"]["schemas"][name];
            assert_eq!(schema["additionalProperties"], false, "{name}");
            assert_eq!(
                schema["properties"].as_object().unwrap().len(),
                fields,
                "{name}"
            );
            assert_eq!(
                schema["required"].as_array().unwrap().len(),
                fields,
                "{name}"
            );
        }
        for name in ["ExecutionLeaseReceipt", "ExecutionLeaseReadback"] {
            assert_eq!(
                doc["components"]["schemas"][name]["properties"]["dispatch_allowed"]["enum"],
                serde_json::json!([false])
            );
        }
        let path = "/api/v1/issues/{id}/sdlc/pm-draft-execution-lease";
        for (route, method) in [
            (path.to_string(), "get"),
            (path.to_string(), "post"),
            (format!("{path}/heartbeat"), "post"),
        ] {
            let operation = &doc["paths"][route][method];
            assert!(!operation["security"].as_array().unwrap().is_empty());
            for status in ["200", "401", "403", "404", "409", "422", "503"] {
                assert!(operation["responses"][status].is_object());
            }
        }
    }

    #[test]
    fn openapi_security_matches_runtime_protection() {
        let document = ApiDoc::openapi();
        let mut operation_ids = std::collections::HashSet::new();
        for (path, item) in document.paths.paths.iter() {
            let is_public = matches!(
                path.as_str(),
                "/health"
                    | "/api/v1/health"
                    | "/api/v1/auth/login"
                    | "/api/v1/auth/register"
                    | "/api/v1/auth/refresh"
            );

            for operation in [
                item.get.as_ref(),
                item.put.as_ref(),
                item.post.as_ref(),
                item.delete.as_ref(),
                item.patch.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                if let Some(operation_id) = &operation.operation_id {
                    assert!(
                        operation_ids.insert(operation_id),
                        "duplicate OpenAPI operation ID: {operation_id}"
                    );
                }
                if is_public {
                    assert!(
                        operation.security.is_none(),
                        "{path} should stay public in OpenAPI"
                    );
                } else {
                    assert!(
                        operation.security.is_some(),
                        "{path} must declare OpenAPI security"
                    );
                }
            }
        }

        let events = document
            .paths
            .paths
            .get("/api/v1/events")
            .and_then(|item| item.get.as_ref())
            .and_then(|operation| operation.security.as_ref())
            .expect("events must be documented");
        let events = serde_json::to_value(events).expect("serialize events security");
        assert_eq!(events, serde_json::json!([{ "bearer": [] }]));
    }

    #[test]
    fn general_rate_preserves_sub_millisecond_intervals() {
        assert_eq!(rate_per_second_period(60), Duration::from_nanos(16_666_666));
        assert_eq!(rate_per_second_period(5_000), Duration::from_nanos(200_000));
    }
}
