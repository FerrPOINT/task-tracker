use async_trait::async_trait;
use std::sync::Arc;

use crate::auth::{JwtAuthService, UserClaims};
use crate::commands::{
    CreateCommentCommand, CreateIssueCommand, CreateProjectCommand, CreateWorklogCommand,
    LoginCommand, ProjectQueryDto, RegisterCommand, TransitionIssueCommand, UpdateCommentCommand,
    UpdateIssueCommand, UpdateNotificationSettingsCommand, UpdateProjectCommand,
    UpdateWorklogCommand,
};
use crate::dto::{
    AuthDto, BacklogDto, BoardDto, CommentDto, DashboardDto, IssueDto, ProjectDto, WorklogDto,
};
use crate::services::{
    AdminServiceImpl, BoardServiceImpl, CommentServiceImpl, DashboardServiceImpl, IssueServiceImpl,
    ProjectMemberService, ProjectMemberServiceImpl, ProjectServiceImpl, SearchServiceImpl,
    SprintService, SprintServiceImpl, WorklogServiceImpl,
};
use shared::{
    AppConfig, AppError, AttachmentId, CommentId, IssueId, IssueLinkId, LabelId, ProjectKey,
    StatusId, UserId, WorklogId,
};

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
        let auth: Arc<dyn AuthService> = Arc::new(JwtAuthService::new(
            config.auth.clone(),
            repos.users.clone(),
        ));
        let project: Arc<dyn ProjectService> = Arc::new(ProjectServiceImpl::new(
            repos.projects.clone(),
            repos.issues.clone(),
            repos.users.clone(),
            repos.boards.clone(),
        ));
        let issue: Arc<dyn IssueService> = Arc::new(IssueServiceImpl::new(
            repos.issues.clone(),
            repos.projects.clone(),
            repos.boards.clone(),
            repos.users.clone(),
            repos.statuses.clone(),
            repos.transitions.clone(),
            events.clone(),
            repos.notifications.clone(),
        ));
        let board: Arc<dyn BoardService> = Arc::new(BoardServiceImpl::new(
            repos.boards.clone(),
            repos.issues.clone(),
            repos.sprints.clone(),
            repos.users.clone(),
            repos.statuses.clone(),
            repos.transitions.clone(),
        ));
        let search: Arc<dyn SearchService> = Arc::new(SearchServiceImpl::new(
            repos.issues.clone(),
            repos.projects.clone(),
            repos.users.clone(),
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
        ));
        Self {
            config,
            events: events.clone(),
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
                )),
                worklog: Arc::new(WorklogServiceImpl::new(
                    repos.worklogs.clone(),
                    repos.users.clone(),
                    repos.issues.clone(),
                )),
                member: Arc::new(ProjectMemberServiceImpl::new(
                    repos.members.clone(),
                    repos.users.clone(),
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
                )),
                label: Arc::new(crate::services::LabelServiceImpl::new(
                    repos.labels.clone(),
                    repos.projects.clone(),
                    repos.issues.clone(),
                )),
                issue_link: Arc::new(crate::services::IssueLinkServiceImpl::new(
                    repos.issue_links.clone(),
                    repos.issues.clone(),
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
                )),
                vote: Arc::new(crate::services::VoteServiceImpl::new(
                    repos.votes.clone(),
                    repos.issues.clone(),
                )),
                component: Arc::new(crate::services::ComponentServiceImpl::new(
                    repos.components.clone(),
                    repos.projects.clone(),
                )),
                version: Arc::new(crate::services::VersionServiceImpl::new(
                    repos.versions.clone(),
                    repos.projects.clone(),
                )),
                custom_field: Arc::new(crate::services::CustomFieldServiceImpl::new(
                    repos.custom_fields.clone(),
                    repos.projects.clone(),
                    repos.issues.clone(),
                )),
                sprint,
            },
            repos,
            email,
        }
    }
}

#[async_trait]
pub trait AuthService: Send + Sync {
    async fn register(&self, cmd: RegisterCommand) -> Result<AuthDto, AppError>;
    async fn login(&self, cmd: LoginCommand) -> Result<AuthDto, AppError>;
    fn verify_token(&self, token: &str) -> Result<UserClaims, AppError>;
    async fn refresh(&self, refresh_token: &str) -> Result<AuthDto, AppError>;
    async fn logout(&self, user_id: UserId) -> Result<(), AppError>;
    async fn me(&self, user_id: UserId) -> Result<crate::dto::UserDto, AppError>;
    async fn list_users(&self) -> Result<Vec<crate::dto::UserDto>, AppError>;
}

#[async_trait]
pub trait StatusService: Send + Sync {
    async fn list_statuses(&self) -> Result<Vec<domain::Status>, AppError>;
}

#[async_trait]
pub trait WorkflowService: Send + Sync {
    async fn list_transitions(&self) -> Result<Vec<domain::WorkflowTransition>, AppError>;
    async fn is_transition_allowed(
        &self,
        from_status_id: StatusId,
        to_status_id: StatusId,
    ) -> Result<bool, AppError>;
}

#[async_trait]
pub trait IssueTypeService: Send + Sync {
    async fn list_issue_types(&self) -> Result<Vec<domain::IssueTypeEntity>, AppError>;
}

#[async_trait]
pub trait CommentService: Send + Sync {
    async fn list(&self, issue_id: IssueId, requester: UserId)
    -> Result<Vec<CommentDto>, AppError>;
    async fn create(&self, cmd: CreateCommentCommand) -> Result<CommentDto, AppError>;
    async fn update(
        &self,
        id: CommentId,
        cmd: UpdateCommentCommand,
        requester: UserId,
    ) -> Result<CommentDto, AppError>;
    async fn delete(&self, id: CommentId, requester: UserId) -> Result<(), AppError>;
}

#[async_trait]
pub trait WorklogService: Send + Sync {
    async fn list(&self, issue_id: IssueId, requester: UserId)
    -> Result<Vec<WorklogDto>, AppError>;
    async fn create(&self, cmd: CreateWorklogCommand) -> Result<WorklogDto, AppError>;
    async fn update(
        &self,
        id: WorklogId,
        cmd: UpdateWorklogCommand,
        requester: UserId,
    ) -> Result<WorklogDto, AppError>;
    async fn delete(&self, id: WorklogId, requester: UserId) -> Result<(), AppError>;
}

#[async_trait]
pub trait ProjectService: Send + Sync {
    async fn create(&self, cmd: CreateProjectCommand) -> Result<ProjectDto, AppError>;
    async fn list(&self, query: ProjectQueryDto) -> Result<Vec<ProjectDto>, AppError>;
    async fn get_by_key(&self, key: &ProjectKey) -> Result<ProjectDto, AppError>;
    async fn update(
        &self,
        key: &ProjectKey,
        cmd: UpdateProjectCommand,
        requester_id: UserId,
    ) -> Result<ProjectDto, AppError>;
    async fn delete(&self, key: &ProjectKey, requester_id: UserId) -> Result<(), AppError>;
}

#[async_trait]
pub trait IssueService: Send + Sync {
    async fn create(&self, cmd: CreateIssueCommand) -> Result<IssueDto, AppError>;
    async fn get_by_id(&self, id: IssueId) -> Result<IssueDto, AppError>;
    async fn update(&self, id: IssueId, cmd: UpdateIssueCommand) -> Result<IssueDto, AppError>;
    async fn transition(&self, cmd: TransitionIssueCommand) -> Result<IssueDto, AppError>;
    async fn search(
        &self,
        filters: crate::context::SearchFilters,
    ) -> Result<Vec<IssueDto>, AppError>;
    /// Soft-delete an issue (move to trash).
    async fn delete(&self, id: IssueId) -> Result<(), AppError>;
    /// Restore a soft-deleted issue from trash.
    async fn restore(&self, id: IssueId) -> Result<IssueDto, AppError>;
    /// Permanently delete a trashed issue.
    async fn purge(&self, id: IssueId) -> Result<(), AppError>;
    /// List soft-deleted (trashed) issues for a project.
    async fn list_trash(&self, project_key: &ProjectKey) -> Result<Vec<IssueDto>, AppError>;
}

#[async_trait]
pub trait BoardService: Send + Sync {
    async fn get_board(&self, project_key: &ProjectKey) -> Result<BoardDto, AppError>;
    async fn get_backlog(&self, project_key: &ProjectKey) -> Result<BacklogDto, AppError>;
    async fn move_issue(
        &self,
        project_key: &ProjectKey,
        issue_id: IssueId,
        status_id: StatusId,
    ) -> Result<BoardDto, AppError>;
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub q: Option<String>,
    pub project_key: Option<String>,
    pub priority: Option<String>,
    pub assignee_id: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub jql: Option<String>,
    pub user_id: Option<String>,
}

#[async_trait]
pub trait SearchService: Send + Sync {
    async fn search(&self, filters: SearchFilters) -> Result<Vec<IssueDto>, AppError>;
}

#[async_trait]
pub trait DashboardService: Send + Sync {
    async fn get_dashboard(&self, user_id: UserId) -> Result<DashboardDto, AppError>;
}

#[async_trait]
pub trait AttachmentService: Send + Sync {
    async fn upload(
        &self,
        issue_id: IssueId,
        author_id: UserId,
        file_name: &str,
        content_type: &str,
        bytes: Vec<u8>,
    ) -> Result<AttachmentDto, AppError>;
    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<AttachmentDto>, AppError>;
    async fn download(
        &self,
        attachment_id: AttachmentId,
    ) -> Result<(AttachmentDto, Vec<u8>), AppError>;
    async fn delete(&self, attachment_id: AttachmentId, requester: UserId) -> Result<(), AppError>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AttachmentDto {
    pub id: String,
    pub issue_id: String,
    pub author_id: String,
    pub file_name: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub created_at: String,
}

#[async_trait]
pub trait LabelService: Send + Sync {
    async fn create(
        &self,
        project_key: &ProjectKey,
        name: &str,
        color: &str,
        requester: UserId,
    ) -> Result<LabelDto, AppError>;
    async fn list_by_project(&self, project_key: &ProjectKey) -> Result<Vec<LabelDto>, AppError>;
    async fn update(
        &self,
        label_id: LabelId,
        name: &str,
        color: &str,
        requester: UserId,
    ) -> Result<LabelDto, AppError>;
    async fn delete(&self, label_id: LabelId, requester: UserId) -> Result<(), AppError>;
    async fn list_for_issue(&self, issue_id: IssueId) -> Result<Vec<LabelDto>, AppError>;
    async fn attach(
        &self,
        issue_id: IssueId,
        label_id: LabelId,
        requester: UserId,
    ) -> Result<(), AppError>;
    async fn detach(
        &self,
        issue_id: IssueId,
        label_id: LabelId,
        requester: UserId,
    ) -> Result<(), AppError>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LabelDto {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub color: String,
}

#[async_trait]
pub trait IssueLinkService: Send + Sync {
    async fn create(
        &self,
        source_id: IssueId,
        target_key: &str,
        link_type: &str,
        requester: UserId,
    ) -> Result<IssueLinkDto, AppError>;
    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<IssueLinkDto>, AppError>;
    async fn delete(&self, link_id: IssueLinkId, requester: UserId) -> Result<(), AppError>;
}

#[async_trait]
pub trait NotificationService: Send + Sync {
    async fn list_unread(&self, user_id: UserId) -> Result<NotificationListDto, AppError>;
    async fn mark_read(&self, id: String, user_id: UserId) -> Result<(), AppError>;
    async fn mark_all_read(&self, user_id: UserId) -> Result<(), AppError>;
    async fn get_settings(&self, user_id: UserId) -> Result<NotificationSettingsDto, AppError>;
    async fn update_settings(
        &self,
        user_id: UserId,
        cmd: UpdateNotificationSettingsCommand,
    ) -> Result<NotificationSettingsDto, AppError>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NotificationDto {
    pub id: String,
    pub event_type: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub actor_id: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub is_read: bool,
    pub action_url: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NotificationListDto {
    pub notifications: Vec<NotificationDto>,
    pub unread_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NotificationSettingsDto {
    pub email_frequency: String,
    pub disabled_event_types: Vec<String>,
    pub notify_own_changes: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IssueLinkDto {
    pub id: String,
    pub source_id: String,
    pub source_key: String,
    pub target_id: String,
    pub target_key: String,
    pub link_type: String,
}
#[async_trait]
pub trait ReportService: Send + Sync {
    async fn get_velocity(
        &self,
        project_id: shared::ProjectId,
        count: u32,
    ) -> Result<Vec<VelocitySprintDto>, AppError>;
    async fn get_burndown(&self, sprint_id: shared::SprintId) -> Result<BurndownDto, AppError>;
    async fn get_cumulative_flow(
        &self,
        project_id: shared::ProjectId,
    ) -> Result<Vec<CumulativeFlowPointDto>, AppError>;
    async fn get_control_chart(
        &self,
        project_id: shared::ProjectId,
    ) -> Result<Vec<ControlChartPointDto>, AppError>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VelocitySprintDto {
    pub name: String,
    pub committed: usize,
    pub completed: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BurndownDto {
    pub sprint_name: String,
    pub points: Vec<BurndownPointDto>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BurndownPointDto {
    pub date: String,
    pub remaining: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CumulativeFlowPointDto {
    pub date: String,
    pub todo: usize,
    pub in_progress: usize,
    pub done: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ControlChartPointDto {
    pub issue_key: String,
    pub cycle_time_days: f64,
}

#[async_trait]
pub trait WatcherService: Send + Sync {
    async fn watch(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError>;
    async fn unwatch(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError>;
    async fn list_watchers(&self, issue_id: IssueId) -> Result<Vec<WatcherDto>, AppError>;
    async fn is_watching(&self, issue_id: IssueId, user_id: UserId) -> Result<bool, AppError>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WatcherDto {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
}

#[async_trait]
pub trait VoteService: Send + Sync {
    async fn vote(&self, issue_id: IssueId, user_id: UserId) -> Result<VoteDto, AppError>;
    async fn unvote(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError>;
    async fn list_votes(&self, issue_id: IssueId) -> Result<Vec<VoteDto>, AppError>;
    async fn count_votes(&self, issue_id: IssueId) -> Result<u64, AppError>;
    async fn has_voted(&self, issue_id: IssueId, user_id: UserId) -> Result<bool, AppError>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VoteDto {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub voted_at: String,
}

#[async_trait]
pub trait CustomFieldService: Send + Sync {
    async fn create_field(
        &self,
        project_key: &ProjectKey,
        name: &str,
        field_type: &str,
        options: &[String],
        is_required: bool,
        requester: UserId,
    ) -> Result<CustomFieldDto, AppError>;
    async fn list_fields(&self, project_key: &ProjectKey) -> Result<Vec<CustomFieldDto>, AppError>;
    async fn update_field(
        &self,
        field_id: shared::CustomFieldId,
        name: &str,
        field_type: &str,
        options: &[String],
        is_required: bool,
        requester: UserId,
    ) -> Result<CustomFieldDto, AppError>;
    async fn delete_field(
        &self,
        field_id: shared::CustomFieldId,
        requester: UserId,
    ) -> Result<(), AppError>;
    async fn set_value(
        &self,
        issue_id: IssueId,
        field_id: shared::CustomFieldId,
        value: serde_json::Value,
        requester: UserId,
    ) -> Result<(), AppError>;
    async fn get_values_for_issue(
        &self,
        issue_id: IssueId,
    ) -> Result<Vec<CustomFieldValueDto>, AppError>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CustomFieldDto {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub field_type: String,
    pub options: Vec<String>,
    pub is_required: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CustomFieldValueDto {
    pub field_id: String,
    pub value: serde_json::Value,
}

#[async_trait]
pub trait ComponentService: Send + Sync {
    async fn create(
        &self,
        project_key: &ProjectKey,
        name: &str,
        description: Option<&str>,
    ) -> Result<ComponentDto, AppError>;
    async fn list_by_project(
        &self,
        project_key: &ProjectKey,
    ) -> Result<Vec<ComponentDto>, AppError>;
    async fn update(
        &self,
        id: shared::ProjectComponentId,
        name: &str,
        description: Option<&str>,
    ) -> Result<ComponentDto, AppError>;
    async fn delete(&self, id: shared::ProjectComponentId) -> Result<(), AppError>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ComponentDto {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
}

#[async_trait]
pub trait VersionService: Send + Sync {
    async fn create(
        &self,
        project_key: &ProjectKey,
        name: &str,
        description: Option<&str>,
        released: bool,
        release_date: Option<chrono::DateTime<chrono::FixedOffset>>,
    ) -> Result<VersionDto, AppError>;
    async fn list_by_project(&self, project_key: &ProjectKey) -> Result<Vec<VersionDto>, AppError>;
    async fn update(
        &self,
        id: shared::ProjectVersionId,
        name: &str,
        description: Option<&str>,
        released: bool,
        release_date: Option<Option<chrono::DateTime<chrono::FixedOffset>>>,
    ) -> Result<VersionDto, AppError>;
    async fn delete(&self, id: shared::ProjectVersionId) -> Result<(), AppError>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VersionDto {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: Option<String>,
    pub released: bool,
    pub release_date: Option<String>,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Phase 8: Admin service
// ---------------------------------------------------------------------------

/// Admin user DTO — includes `is_system_admin` and `is_active` flags that the
/// regular [`crate::dto::UserDto`] intentionally omits.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdminUserDto {
    pub id: String,
    pub email: String,
    pub username: String,
    pub display_name: String,
    pub is_system_admin: bool,
    pub is_active: bool,
}

impl From<domain::User> for AdminUserDto {
    fn from(user: domain::User) -> Self {
        Self {
            id: user.id.to_string(),
            email: user.email.as_ref().to_string(),
            username: user.username.as_ref().to_string(),
            display_name: user.display_name.as_ref().to_string(),
            is_system_admin: user.is_system_admin,
            is_active: user.is_active,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AuditLogDto {
    pub id: String,
    pub actor_id: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

impl From<domain::AuditLog> for AuditLogDto {
    fn from(entry: domain::AuditLog) -> Self {
        Self {
            id: entry.id.to_string(),
            actor_id: entry.actor_id.to_string(),
            action: entry.action.as_ref().to_string(),
            entity_type: entry.entity_type.as_ref().to_string(),
            entity_id: entry.entity_id.map(|id| id.to_string()),
            metadata: entry.metadata,
            created_at: entry.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemSettingDto {
    pub key: String,
    pub value: serde_json::Value,
    pub updated_at: String,
}

impl From<domain::SystemSetting> for SystemSettingDto {
    fn from(setting: domain::SystemSetting) -> Self {
        Self {
            key: setting.key.as_ref().to_string(),
            value: setting.value,
            updated_at: setting.updated_at.to_rfc3339(),
        }
    }
}

/// Command for creating a new user from the admin panel.
/// The password is hashed before storage and never persisted in plaintext.
#[derive(Debug, Clone)]
pub struct AdminCreateUserCommand {
    pub email: String,
    pub username: String,
    pub display_name: String,
    pub password: String,
    pub is_system_admin: bool,
}

#[async_trait]
pub trait AdminService: Send + Sync {
    /// List all users. `requester_id` must be a system admin.
    async fn list_users(&self, requester_id: UserId) -> Result<Vec<AdminUserDto>, AppError>;

    /// Create a new user. `requester_id` must be a system admin. The password
    /// is hashed via argon2; the plaintext is never logged or persisted.
    async fn create_user(
        &self,
        requester_id: UserId,
        cmd: AdminCreateUserCommand,
    ) -> Result<AdminUserDto, AppError>;

    /// Update a user's active status. `requester_id` must be a system admin.
    /// Prevents deactivating the last active system admin.
    async fn update_user_status(
        &self,
        requester_id: UserId,
        user_id: UserId,
        is_active: bool,
    ) -> Result<AdminUserDto, AppError>;

    /// List audit log entries (most recent first). `requester_id` must be a
    /// system admin.
    async fn list_audit_logs(
        &self,
        requester_id: UserId,
        limit: u64,
    ) -> Result<Vec<AuditLogDto>, AppError>;

    /// List all system settings. `requester_id` must be a system admin. Only
    /// safe keys are returned.
    async fn list_system_settings(
        &self,
        requester_id: UserId,
    ) -> Result<Vec<SystemSettingDto>, AppError>;

    /// Update a system setting. `requester_id` must be a system admin. The key
    /// must be on the safe allowlist and the JSON value must be within the size
    /// limit.
    async fn update_system_setting(
        &self,
        requester_id: UserId,
        key: String,
        value: serde_json::Value,
    ) -> Result<SystemSettingDto, AppError>;
}
