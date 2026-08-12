use async_trait::async_trait;
use domain::{
    Board, BoardRepository, Issue, IssueQuery, IssueRepository, Project, ProjectQuery,
    ProjectRepository, Sprint, SprintRepository, User, UserRepository,
};
use shared::{AppError, BoardId, IssueId, ProjectId, ProjectKey, SprintId, UserId};
use std::sync::Arc;

#[derive(Default)]
pub struct FailingUserRepository;

#[async_trait]
impl UserRepository for FailingUserRepository {
    async fn get_by_id(&self, _id: UserId) -> Result<User, AppError> {
        Err(AppError::Internal("failing user repo".into()))
    }

    async fn get_by_email(&self, _email: &str) -> Result<User, AppError> {
        Err(AppError::Internal("failing user repo".into()))
    }

    async fn get_by_refresh_token(&self, _token_hash: &str) -> Result<User, AppError> {
        Err(AppError::not_found("user", "stub"))
    }

    async fn save(&self, _user: &User) -> Result<UserId, AppError> {
        Err(AppError::Internal("failing user repo".into()))
    }

    async fn list(&self) -> Result<Vec<User>, AppError> {
        Err(AppError::Internal("failing user repo".into()))
    }
}

#[derive(Default)]
pub struct FailingProjectRepository;

#[async_trait]
impl ProjectRepository for FailingProjectRepository {
    async fn get_by_id(&self, _id: ProjectId) -> Result<Project, AppError> {
        Err(AppError::Internal("failing project repo".into()))
    }

    async fn get_by_key(&self, _key: &ProjectKey) -> Result<Project, AppError> {
        Err(AppError::Internal("failing project repo".into()))
    }

    async fn list(&self, _query: ProjectQuery) -> Result<Vec<Project>, AppError> {
        Err(AppError::Internal("failing project repo".into()))
    }

    async fn save(&self, _project: &Project) -> Result<ProjectId, AppError> {
        Err(AppError::Internal("failing project repo".into()))
    }

    async fn delete(&self, _id: ProjectId) -> Result<(), AppError> {
        Err(AppError::Internal("failing project repo".into()))
    }

    async fn next_issue_number(&self, _project_id: ProjectId) -> Result<u32, AppError> {
        Err(AppError::Internal("failing project repo".into()))
    }
}

#[derive(Default)]
pub struct FailingIssueRepository;

#[async_trait]
impl IssueRepository for FailingIssueRepository {
    async fn get_by_id(&self, _id: IssueId) -> Result<Issue, AppError> {
        Err(AppError::Internal("failing issue repo".into()))
    }

    async fn get_by_key(&self, _key: &shared::IssueKey) -> Result<Issue, AppError> {
        Err(AppError::Internal("failing issue repo".into()))
    }

    async fn list(&self, _query: IssueQuery) -> Result<Vec<Issue>, AppError> {
        Err(AppError::Internal("failing issue repo".into()))
    }

    async fn save(&self, _issue: &Issue) -> Result<IssueId, AppError> {
        Err(AppError::Internal("failing issue repo".into()))
    }

    async fn delete(&self, _id: IssueId) -> Result<(), AppError> {
        Err(AppError::Internal("failing issue repo".into()))
    }
}

#[derive(Default)]
pub struct FailingBoardRepository;

#[async_trait]
impl BoardRepository for FailingBoardRepository {
    async fn get_by_id(&self, _id: BoardId) -> Result<Board, AppError> {
        Err(AppError::Internal("failing board repo".into()))
    }

    async fn get_default_by_project(&self, _project_id: ProjectId) -> Result<Board, AppError> {
        Err(AppError::Internal("failing board repo".into()))
    }

    async fn get_default_by_project_key(
        &self,
        _project_key: &ProjectKey,
    ) -> Result<Board, AppError> {
        Err(AppError::Internal("failing board repo".into()))
    }

    async fn save(&self, _board: &Board) -> Result<(), AppError> {
        Err(AppError::Internal("failing board repo".into()))
    }
}

#[derive(Default)]
pub struct FailingSprintRepository;

#[async_trait]
impl SprintRepository for FailingSprintRepository {
    async fn get_active_by_project(
        &self,
        _project_id: ProjectId,
    ) -> Result<Option<Sprint>, AppError> {
        Err(AppError::Internal("failing sprint repo".into()))
    }

    async fn get_by_id(&self, _id: SprintId) -> Result<Sprint, AppError> {
        Err(AppError::Internal("failing sprint repo".into()))
    }

    async fn save(&self, _sprint: &Sprint) -> Result<SprintId, AppError> {
        Err(AppError::Internal("failing sprint repo".into()))
    }

    async fn list_by_project(&self, _project_id: ProjectId) -> Result<Vec<Sprint>, AppError> {
        Err(AppError::Internal("failing sprint repo".into()))
    }
}

pub fn failing_context_with_config(config: Arc<shared::AppConfig>) -> Arc<app::AppContext> {
    use domain::Repositories;

    let repos = Arc::new(Repositories {
        users: Arc::new(FailingUserRepository),
        projects: Arc::new(FailingProjectRepository),
        issues: Arc::new(FailingIssueRepository),
        boards: Arc::new(FailingBoardRepository),
        sprints: Arc::new(FailingSprintRepository),
        comments: Arc::new(domain::StubCommentRepository),
        worklogs: Arc::new(domain::StubWorklogRepository),
        members: Arc::new(domain::StubProjectMemberRepository),
    });
    Arc::new(app::AppContext::new(config, repos))
}

pub fn failing_context() -> Arc<app::AppContext> {
    use domain::Repositories;

    let repos = Arc::new(Repositories {
        users: Arc::new(FailingUserRepository),
        projects: Arc::new(FailingProjectRepository),
        issues: Arc::new(FailingIssueRepository),
        boards: Arc::new(FailingBoardRepository),
        sprints: Arc::new(FailingSprintRepository),
        comments: Arc::new(domain::StubCommentRepository),
        worklogs: Arc::new(domain::StubWorklogRepository),
        members: Arc::new(domain::StubProjectMemberRepository),
    });
    Arc::new(app::AppContext::new(
        Arc::new(shared::AppConfig {
            database: shared::DatabaseConfig::default(),
            server: shared::ServerConfig::default(),
            auth: shared::AuthConfig {
                jwt_secret: "test-secret-32-chars-long!!!!!".to_string(),
                access_token_ttl_minutes: 15,
                refresh_token_ttl_days: 7,
                refresh_cookie_name: "refresh_token".to_string(),
                refresh_cookie_secure: true,
                refresh_cookie_same_site: "Lax".to_string(),
                refresh_cookie_domain: None,
                refresh_cookie_path: "/api/v1/auth".to_string(),
            },
        }),
        repos,
    ))
}
