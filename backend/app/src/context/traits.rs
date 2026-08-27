use async_trait::async_trait;

use crate::auth::UserClaims;
use crate::commands::{
    CreateCommentCommand, CreateIssueCommand, CreateProjectCommand, CreateWorklogCommand,
    LoginCommand, ProjectQueryDto, RegisterCommand, TransitionIssueCommand, UpdateCommentCommand,
    UpdateIssueCommand, UpdateNotificationSettingsCommand, UpdateProjectCommand,
    UpdateWorklogCommand,
};
use crate::dto::{
    AuthDto, BacklogDto, BoardDto, CommentDto, DashboardDto, IssueDto, ProjectDto, WorklogDto,
};
use shared::{
    AppError, AttachmentId, CommentId, IssueId, IssueLinkId, LabelId, ProjectKey, StatusId, UserId,
    WorklogId,
};

use crate::context::filters::SearchFilters;
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
    async fn list(
        &self,
        issue_id: IssueId,
        requester: UserId,
        limit: Option<u64>,
        offset: u64,
    ) -> Result<Vec<CommentDto>, AppError>;
    async fn create(
        &self,
        cmd: CreateCommentCommand,
        requester: UserId,
    ) -> Result<CommentDto, AppError>;
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
    async fn list(
        &self,
        issue_id: IssueId,
        requester: UserId,
        limit: Option<u64>,
        offset: u64,
    ) -> Result<Vec<WorklogDto>, AppError>;
    async fn create(
        &self,
        cmd: CreateWorklogCommand,
        requester: UserId,
    ) -> Result<WorklogDto, AppError>;
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
    async fn list(
        &self,
        query: ProjectQueryDto,
        requester: UserId,
    ) -> Result<Vec<ProjectDto>, AppError>;
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
    async fn create(
        &self,
        cmd: CreateIssueCommand,
        requester: UserId,
    ) -> Result<IssueDto, AppError>;
    async fn get_by_id(&self, id: IssueId, requester: UserId) -> Result<IssueDto, AppError>;
    async fn update(
        &self,
        id: IssueId,
        cmd: UpdateIssueCommand,
        requester: UserId,
    ) -> Result<IssueDto, AppError>;
    async fn transition(&self, cmd: TransitionIssueCommand) -> Result<IssueDto, AppError>;
    async fn search(
        &self,
        filters: crate::context::SearchFilters,
        requester: UserId,
    ) -> Result<Vec<IssueDto>, AppError>;
    /// Soft-delete an issue (move to trash).
    async fn delete(&self, id: IssueId, actor_id: UserId) -> Result<(), AppError>;
    /// Restore a soft-deleted issue from trash.
    async fn restore(&self, id: IssueId, actor_id: UserId) -> Result<IssueDto, AppError>;
    /// Permanently delete a trashed issue.
    async fn purge(&self, id: IssueId, actor_id: UserId) -> Result<(), AppError>;
    /// List soft-deleted (trashed) issues for a project.
    async fn list_trash(
        &self,
        project_key: &ProjectKey,
        requester: UserId,
    ) -> Result<Vec<IssueDto>, AppError>;
}

#[async_trait]
pub trait BoardService: Send + Sync {
    async fn get_board(
        &self,
        project_key: &ProjectKey,
        requester: UserId,
    ) -> Result<BoardDto, AppError>;
    async fn get_backlog(
        &self,
        project_key: &ProjectKey,
        requester: UserId,
    ) -> Result<BacklogDto, AppError>;
    async fn move_issue(
        &self,
        project_key: &ProjectKey,
        issue_id: IssueId,
        status_id: StatusId,
        requester: UserId,
    ) -> Result<BoardDto, AppError>;
}

#[async_trait]
pub trait SearchService: Send + Sync {
    async fn search(
        &self,
        filters: SearchFilters,
        requester: UserId,
    ) -> Result<Vec<IssueDto>, AppError>;
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
    ) -> Result<crate::context::AttachmentDto, AppError>;
    async fn list_by_issue(
        &self,
        issue_id: IssueId,
        requester: UserId,
    ) -> Result<Vec<crate::context::AttachmentDto>, AppError>;
    async fn download(
        &self,
        attachment_id: AttachmentId,
        requester: UserId,
    ) -> Result<(crate::context::AttachmentDto, Vec<u8>), AppError>;
    async fn delete(&self, attachment_id: AttachmentId, requester: UserId) -> Result<(), AppError>;
}

#[async_trait]
pub trait LabelService: Send + Sync {
    async fn create(
        &self,
        project_key: &ProjectKey,
        name: &str,
        color: &str,
        requester: UserId,
    ) -> Result<crate::context::LabelDto, AppError>;
    async fn list_by_project(
        &self,
        project_key: &ProjectKey,
        requester: UserId,
    ) -> Result<Vec<crate::context::LabelDto>, AppError>;
    async fn update(
        &self,
        label_id: LabelId,
        name: &str,
        color: &str,
        requester: UserId,
    ) -> Result<crate::context::LabelDto, AppError>;
    async fn delete(&self, label_id: LabelId, requester: UserId) -> Result<(), AppError>;
    async fn list_for_issue(
        &self,
        issue_id: IssueId,
        requester: UserId,
    ) -> Result<Vec<crate::context::LabelDto>, AppError>;
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

#[async_trait]
pub trait IssueLinkService: Send + Sync {
    async fn create(
        &self,
        source_id: IssueId,
        target_key: &str,
        link_type: &str,
        requester: UserId,
    ) -> Result<crate::context::IssueLinkDto, AppError>;
    async fn list_by_issue(
        &self,
        issue_id: IssueId,
        requester: UserId,
    ) -> Result<Vec<crate::context::IssueLinkDto>, AppError>;
    async fn delete(&self, link_id: IssueLinkId, requester: UserId) -> Result<(), AppError>;
}

#[async_trait]
pub trait NotificationService: Send + Sync {
    async fn list_unread(
        &self,
        user_id: UserId,
    ) -> Result<crate::context::NotificationListDto, AppError>;
    async fn mark_read(&self, id: String, user_id: UserId) -> Result<(), AppError>;
    async fn mark_all_read(&self, user_id: UserId) -> Result<(), AppError>;
    async fn get_settings(
        &self,
        user_id: UserId,
    ) -> Result<crate::context::NotificationSettingsDto, AppError>;
    async fn update_settings(
        &self,
        user_id: UserId,
        cmd: UpdateNotificationSettingsCommand,
    ) -> Result<crate::context::NotificationSettingsDto, AppError>;
}

#[async_trait]
pub trait ReportService: Send + Sync {
    async fn get_velocity(
        &self,
        project_id: shared::ProjectId,
        count: u32,
        requester: UserId,
    ) -> Result<Vec<crate::context::VelocitySprintDto>, AppError>;
    async fn get_burndown(
        &self,
        sprint_id: shared::SprintId,
        requester: UserId,
    ) -> Result<crate::context::BurndownDto, AppError>;
    async fn get_cumulative_flow(
        &self,
        project_id: shared::ProjectId,
        requester: UserId,
    ) -> Result<Vec<crate::context::CumulativeFlowPointDto>, AppError>;
    async fn get_control_chart(
        &self,
        project_id: shared::ProjectId,
        requester: UserId,
    ) -> Result<Vec<crate::context::ControlChartPointDto>, AppError>;
}

#[async_trait]
pub trait WatcherService: Send + Sync {
    async fn watch(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError>;
    async fn unwatch(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError>;
    async fn list_watchers(
        &self,
        issue_id: IssueId,
        requester: UserId,
    ) -> Result<Vec<crate::context::WatcherDto>, AppError>;
    async fn is_watching(&self, issue_id: IssueId, user_id: UserId) -> Result<bool, AppError>;
}

#[async_trait]
pub trait VoteService: Send + Sync {
    async fn vote(
        &self,
        issue_id: IssueId,
        user_id: UserId,
    ) -> Result<crate::context::VoteDto, AppError>;
    async fn unvote(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError>;
    async fn list_votes(
        &self,
        issue_id: IssueId,
        requester: UserId,
    ) -> Result<Vec<crate::context::VoteDto>, AppError>;
    async fn count_votes(&self, issue_id: IssueId) -> Result<u64, AppError>;
    async fn has_voted(&self, issue_id: IssueId, user_id: UserId) -> Result<bool, AppError>;
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
    ) -> Result<crate::context::CustomFieldDto, AppError>;
    async fn list_fields(
        &self,
        project_key: &ProjectKey,
        requester: UserId,
    ) -> Result<Vec<crate::context::CustomFieldDto>, AppError>;
    async fn update_field(
        &self,
        field_id: shared::CustomFieldId,
        name: &str,
        field_type: &str,
        options: &[String],
        is_required: bool,
        requester: UserId,
    ) -> Result<crate::context::CustomFieldDto, AppError>;
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
        requester: UserId,
    ) -> Result<Vec<crate::context::CustomFieldValueDto>, AppError>;
}

#[async_trait]
pub trait ComponentService: Send + Sync {
    async fn create(
        &self,
        project_key: &ProjectKey,
        name: &str,
        description: Option<&str>,
        requester: UserId,
    ) -> Result<crate::context::ComponentDto, AppError>;
    async fn list_by_project(
        &self,
        project_key: &ProjectKey,
        requester: UserId,
    ) -> Result<Vec<crate::context::ComponentDto>, AppError>;
    async fn update(
        &self,
        id: shared::ProjectComponentId,
        name: &str,
        description: Option<&str>,
        requester: UserId,
    ) -> Result<crate::context::ComponentDto, AppError>;
    async fn delete(
        &self,
        id: shared::ProjectComponentId,
        requester: UserId,
    ) -> Result<(), AppError>;
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
        requester: UserId,
    ) -> Result<crate::context::VersionDto, AppError>;
    async fn list_by_project(
        &self,
        project_key: &ProjectKey,
        requester: UserId,
    ) -> Result<Vec<crate::context::VersionDto>, AppError>;
    async fn update(
        &self,
        id: shared::ProjectVersionId,
        name: &str,
        description: Option<&str>,
        released: bool,
        release_date: Option<Option<chrono::DateTime<chrono::FixedOffset>>>,
        requester: UserId,
    ) -> Result<crate::context::VersionDto, AppError>;
    async fn delete(&self, id: shared::ProjectVersionId, requester: UserId)
    -> Result<(), AppError>;
}

// ---------------------------------------------------------------------------
// Phase 8: Admin service
// ---------------------------------------------------------------------------

#[async_trait]
pub trait AdminService: Send + Sync {
    /// List all users. `requester_id` must be a system admin.
    async fn list_users(
        &self,
        requester_id: UserId,
    ) -> Result<Vec<crate::context::AdminUserDto>, AppError>;

    /// Create a new user. `requester_id` must be a system admin. The password
    /// is hashed via argon2; the plaintext is never logged or persisted.
    async fn create_user(
        &self,
        requester_id: UserId,
        cmd: crate::context::AdminCreateUserCommand,
    ) -> Result<crate::context::AdminUserDto, AppError>;

    /// Update a user's active status. `requester_id` must be a system admin.
    /// Prevents deactivating the last active system admin.
    async fn update_user_status(
        &self,
        requester_id: UserId,
        user_id: UserId,
        is_active: bool,
    ) -> Result<crate::context::AdminUserDto, AppError>;

    /// List audit log entries (most recent first). `requester_id` must be a
    /// system admin.
    async fn list_audit_logs(
        &self,
        requester_id: UserId,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<crate::context::AuditLogDto>, AppError>;

    /// List all system settings. `requester_id` must be a system admin. Only
    /// safe keys are returned.
    async fn list_system_settings(
        &self,
        requester_id: UserId,
    ) -> Result<Vec<crate::context::SystemSettingDto>, AppError>;

    /// Update a system setting. `requester_id` must be a system admin. The key
    /// must be on the safe allowlist and the JSON value must be within the size
    /// limit.
    async fn update_system_setting(
        &self,
        requester_id: UserId,
        key: String,
        value: serde_json::Value,
    ) -> Result<crate::context::SystemSettingDto, AppError>;
}
