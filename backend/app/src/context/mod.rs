mod dtos;
mod filters;
mod traits;

pub use dtos::*;
pub use filters::*;
pub use traits::*;

use std::sync::Arc;

use crate::auth::JwtAuthService;
use crate::authz::Authz;
use crate::services::{
    AdminServiceImpl, BoardServiceImpl, CommentServiceImpl, DashboardServiceImpl, IssueServiceImpl,
    ProjectMemberService, ProjectMemberServiceImpl, ProjectServiceImpl, SearchServiceImpl,
    SprintService, SprintServiceImpl, WorklogServiceImpl,
};
use shared::AppConfig;

/// Broadcast hub for real-time invalidation events (SSE).
/// Capacity is bounded; a lagging subscriber misses events and simply refetches.
#[derive(Clone)]
pub struct EventBus {
    tx: tokio::sync::broadcast::Sender<shared::TrackerEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(256);
        Self { tx }
    }
}

impl EventBus {
    pub fn publish(&self, event: shared::TrackerEvent) {
        // Ignore send errors: no subscribers is a normal state.
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<shared::TrackerEvent> {
        self.tx.subscribe()
    }
}

#[derive(Clone)]
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub services: Services,
    pub repos: Arc<domain::Repositories>,
    pub events: EventBus,
    pub email: Arc<dyn domain::EmailPort>,
    pub authz: Authz,
}

#[derive(Clone)]
pub struct Services {
    pub auth: Arc<dyn AuthService>,
    pub project: Arc<dyn ProjectService>,
    pub issue: Arc<dyn IssueService>,
    pub board: Arc<dyn BoardService>,
    pub search: Arc<dyn SearchService>,
    pub dashboard: Arc<dyn DashboardService>,
    pub comment: Arc<dyn CommentService>,
    pub worklog: Arc<dyn WorklogService>,
    pub member: Arc<dyn ProjectMemberService>,
    pub sprint: Arc<dyn SprintService>,
    pub status: Arc<dyn StatusService>,
    pub workflow: Arc<dyn WorkflowService>,
    pub issue_type: Arc<dyn IssueTypeService>,
    pub attachment: Arc<dyn AttachmentService>,
    pub label: Arc<dyn LabelService>,
    pub issue_link: Arc<dyn IssueLinkService>,
    pub notification: Arc<dyn NotificationService>,
    pub report: Arc<dyn ReportService>,
    pub admin: Arc<dyn AdminService>,
    pub watcher: Arc<dyn WatcherService>,
    pub vote: Arc<dyn VoteService>,
    pub component: Arc<dyn ComponentService>,
    pub version: Arc<dyn VersionService>,
    pub custom_field: Arc<dyn CustomFieldService>,
}

impl AppContext {
    pub fn new(
        config: Arc<AppConfig>,
        repos: Arc<domain::Repositories>,
        storage: Arc<dyn domain::FileStorage>,
    ) -> Self {
        Self::with_events(
            config,
            repos,
            storage,
            EventBus::default(),
            Arc::new(domain::StubEmailPort),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_events(
        config: Arc<AppConfig>,
        repos: Arc<domain::Repositories>,
        storage: Arc<dyn domain::FileStorage>,
        events: EventBus,
        email: Arc<dyn domain::EmailPort>,
    ) -> Self {
        let authz = Authz::new(repos.members.clone(), repos.projects.clone());
        let auth: Arc<dyn AuthService> = Arc::new(JwtAuthService::new(
            config.auth.clone(),
            repos.users.clone(),
        ));
        let project: Arc<dyn ProjectService> = Arc::new(ProjectServiceImpl::new(
            repos.projects.clone(),
            repos.issues.clone(),
            repos.users.clone(),
            repos.boards.clone(),
            authz.clone(),
        ));
        let issue: Arc<dyn IssueService> = Arc::new(IssueServiceImpl::new(
            repos.issues.clone(),
            repos.projects.clone(),
            repos.boards.clone(),
            repos.users.clone(),
            repos.statuses.clone(),
            repos.transitions.clone(),
            repos.issue_status_history.clone(),
            repos.sprints.clone(),
            repos.components.clone(),
            repos.versions.clone(),
            events.clone(),
            repos.notifications.clone(),
            authz.clone(),
        ));
        let board: Arc<dyn BoardService> = Arc::new(BoardServiceImpl::new(
            repos.boards.clone(),
            repos.issues.clone(),
            repos.sprints.clone(),
            repos.users.clone(),
            repos.statuses.clone(),
            repos.transitions.clone(),
            repos.projects.clone(),
            repos.issue_status_history.clone(),
            authz.clone(),
        ));
        let search: Arc<dyn SearchService> = Arc::new(SearchServiceImpl::new(
            repos.issues.clone(),
            repos.projects.clone(),
            repos.users.clone(),
            repos.statuses.clone(),
            authz.clone(),
        ));
        let dashboard: Arc<dyn DashboardService> = Arc::new(DashboardServiceImpl::new(
            repos.issues.clone(),
            repos.projects.clone(),
            repos.users.clone(),
        ));
        let sprint: Arc<dyn SprintService> = Arc::new(SprintServiceImpl::new(
            repos.sprints.clone(),
            repos.issues.clone(),
            repos.projects.clone(),
            repos.users.clone(),
            authz.clone(),
        ));
        Self {
            config,
            events: events.clone(),
            authz: authz.clone(),
            services: Services {
                auth,
                project,
                issue,
                board,
                search,
                dashboard,
                comment: Arc::new(CommentServiceImpl::new(
                    repos.comments.clone(),
                    repos.users.clone(),
                    repos.issues.clone(),
                    repos.projects.clone(),
                    events.clone(),
                    repos.notifications.clone(),
                    authz.clone(),
                )),
                worklog: Arc::new(WorklogServiceImpl::new(
                    repos.worklogs.clone(),
                    repos.users.clone(),
                    repos.issues.clone(),
                    repos.projects.clone(),
                    events.clone(),
                    authz.clone(),
                )),
                member: Arc::new(ProjectMemberServiceImpl::new(
                    repos.members.clone(),
                    repos.users.clone(),
                    authz.clone(),
                )),
                status: Arc::new(crate::services::StatusServiceImpl::new(
                    repos.statuses.clone(),
                )),
                workflow: Arc::new(crate::services::WorkflowServiceImpl::new(
                    repos.transitions.clone(),
                )),
                issue_type: Arc::new(crate::services::IssueTypeServiceImpl::new(
                    repos.issue_types.clone(),
                )),
                attachment: Arc::new(crate::services::AttachmentServiceImpl::new(
                    repos.attachments.clone(),
                    repos.issues.clone(),
                    storage,
                    authz.clone(),
                )),
                label: Arc::new(crate::services::LabelServiceImpl::new(
                    repos.labels.clone(),
                    repos.projects.clone(),
                    repos.issues.clone(),
                    authz.clone(),
                )),
                issue_link: Arc::new(crate::services::IssueLinkServiceImpl::new(
                    repos.issue_links.clone(),
                    repos.issues.clone(),
                    authz.clone(),
                )),
                notification: Arc::new(crate::services::NotificationServiceImpl::new(
                    repos.notifications.clone(),
                    repos.notification_settings.clone(),
                )),
                report: Arc::new(crate::services::ReportServiceImpl::new(
                    repos.issues.clone(),
                    repos.sprints.clone(),
                    repos.statuses.clone(),
                    repos.issue_status_history.clone(),
                    authz.clone(),
                )),
                admin: Arc::new(AdminServiceImpl::new(
                    repos.users.clone(),
                    repos.audit_logs.clone(),
                    repos.system_settings.clone(),
                )),
                watcher: Arc::new(crate::services::WatcherServiceImpl::new(
                    repos.watchers.clone(),
                    repos.issues.clone(),
                    repos.users.clone(),
                    repos.projects.clone(),
                    events.clone(),
                    authz.clone(),
                )),
                vote: Arc::new(crate::services::VoteServiceImpl::new(
                    repos.votes.clone(),
                    repos.issues.clone(),
                    repos.users.clone(),
                    authz.clone(),
                )),
                component: Arc::new(crate::services::ComponentServiceImpl::new(
                    repos.components.clone(),
                    repos.projects.clone(),
                    authz.clone(),
                )),
                version: Arc::new(crate::services::VersionServiceImpl::new(
                    repos.versions.clone(),
                    repos.projects.clone(),
                    authz.clone(),
                )),
                custom_field: Arc::new(crate::services::CustomFieldServiceImpl::new(
                    repos.custom_fields.clone(),
                    repos.projects.clone(),
                    repos.issues.clone(),
                    authz.clone(),
                )),
                sprint,
            },
            repos,
            email,
        }
    }
}
