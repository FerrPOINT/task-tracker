use async_trait::async_trait;
use std::sync::Arc;

use crate::auth::{JwtAuthService, UserClaims};
use crate::commands::{
    CreateCommentCommand, CreateIssueCommand, CreateProjectCommand, CreateWorklogCommand,
    LoginCommand, ProjectQueryDto, RegisterCommand, TransitionIssueCommand, UpdateCommentCommand,
    UpdateIssueCommand, UpdateProjectCommand, UpdateWorklogCommand,
};
use crate::dto::{
    AuthDto, BacklogDto, BoardDto, CommentDto, DashboardDto, IssueDto, ProjectDto, WorklogDto,
};
use crate::services::{
    BoardServiceImpl, CommentServiceImpl, DashboardServiceImpl, IssueServiceImpl,
    ProjectMemberService, ProjectMemberServiceImpl, ProjectServiceImpl, SearchServiceImpl,
    SprintService, SprintServiceImpl, WorklogServiceImpl,
};
use shared::{
    AppConfig, AppError, AttachmentId, CommentId, IssueId, IssueLinkId, LabelId, ProjectKey,
    StatusId, UserId, WorklogId,
};

#[derive(Clone)]
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub services: Services,
    pub repos: Arc<domain::Repositories>,
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
}

impl AppContext {
    pub fn new(
        config: Arc<AppConfig>,
        repos: Arc<domain::Repositories>,
        storage: Arc<dyn domain::FileStorage>,
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
                sprint,
            },
            repos,
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
    async fn delete(&self, id: IssueId) -> Result<(), AppError>;
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct IssueLinkDto {
    pub id: String,
    pub source_id: String,
    pub source_key: String,
    pub target_id: String,
    pub target_key: String,
    pub link_type: String,
}
