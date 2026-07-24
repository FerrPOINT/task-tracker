use async_trait::async_trait;
use std::sync::Arc;

use crate::auth::{JwtAuthService, UserClaims};
use crate::commands::{
    CreateIssueCommand, CreateProjectCommand, LoginCommand, ProjectQueryDto, RegisterCommand,
    UpdateIssueCommand,
};
use crate::dto::{AuthDto, BacklogDto, BoardDto, DashboardDto, IssueDto, ProjectDto};
use crate::services::{
    BoardServiceImpl, DashboardServiceImpl, IssueServiceImpl, ProjectServiceImpl, SearchServiceImpl,
};
use shared::{AppConfig, AppError, IssueId, ProjectKey, StatusId, UserId};

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
}

impl AppContext {
    pub fn new(config: Arc<AppConfig>, repos: Arc<domain::Repositories>) -> Self {
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
            repos.users.clone(),
        ));
        let board: Arc<dyn BoardService> = Arc::new(BoardServiceImpl::new(
            repos.boards.clone(),
            repos.issues.clone(),
            repos.sprints.clone(),
            repos.users.clone(),
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

        Self {
            config,
            services: Services {
                auth,
                project,
                issue,
                board,
                search,
                dashboard,
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
}

#[async_trait]
pub trait ProjectService: Send + Sync {
    async fn create(&self, cmd: CreateProjectCommand) -> Result<ProjectDto, AppError>;
    async fn list(&self, query: ProjectQueryDto) -> Result<Vec<ProjectDto>, AppError>;
    async fn get_by_key(&self, key: &ProjectKey) -> Result<ProjectDto, AppError>;
}

#[async_trait]
pub trait IssueService: Send + Sync {
    async fn create(&self, cmd: CreateIssueCommand) -> Result<IssueDto, AppError>;
    async fn get_by_id(&self, id: IssueId) -> Result<IssueDto, AppError>;
    async fn update(&self, id: IssueId, cmd: UpdateIssueCommand) -> Result<IssueDto, AppError>;
    async fn search(&self, q: &str) -> Result<Vec<IssueDto>, AppError>;
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

#[async_trait]
pub trait SearchService: Send + Sync {
    async fn search(&self, q: &str) -> Result<Vec<IssueDto>, AppError>;
}

#[async_trait]
pub trait DashboardService: Send + Sync {
    async fn get_dashboard(&self, user_id: UserId) -> Result<DashboardDto, AppError>;
}
