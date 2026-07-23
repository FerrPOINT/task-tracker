use async_trait::async_trait;
use std::sync::Arc;

#[cfg(test)]
#[path = "repositories/tests.rs"]
mod tests;

use crate::{Board, Issue, IssueQuery, Project, Sprint, User};
use shared::{AppError, BoardId, IssueId, ProjectId, ProjectKey, SprintId, UserId};

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn get_by_id(&self, id: UserId) -> Result<User, AppError>;
    async fn get_by_email(&self, email: &str) -> Result<User, AppError>;
    async fn save(&self, user: &User) -> Result<UserId, AppError>;
}

#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn get_by_id(&self, id: ProjectId) -> Result<Project, AppError>;
    async fn get_by_key(&self, key: &ProjectKey) -> Result<Project, AppError>;
    async fn list(&self, query: ProjectQuery) -> Result<Vec<Project>, AppError>;
    async fn save(&self, project: &Project) -> Result<ProjectId, AppError>;
    async fn next_issue_number(&self, project_id: ProjectId) -> Result<u32, AppError>;
}

#[derive(Debug, Clone, Default)]
pub struct ProjectQuery {
    pub owner_id: Option<UserId>,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
pub trait IssueRepository: Send + Sync {
    async fn get_by_id(&self, id: IssueId) -> Result<Issue, AppError>;
    async fn get_by_key(&self, key: &shared::IssueKey) -> Result<Issue, AppError>;
    async fn list(&self, query: IssueQuery) -> Result<Vec<Issue>, AppError>;
    async fn save(&self, issue: &Issue) -> Result<IssueId, AppError>;
    async fn delete(&self, id: IssueId) -> Result<(), AppError>;
}

#[async_trait]
pub trait BoardRepository: Send + Sync {
    async fn get_by_id(&self, id: BoardId) -> Result<Board, AppError>;
    async fn get_default_by_project(&self, project_id: ProjectId) -> Result<Board, AppError>;
    async fn get_default_by_project_key(&self, key: &ProjectKey) -> Result<Board, AppError>;
    async fn save(&self, board: &Board) -> Result<(), AppError>;
}

#[async_trait]
pub trait SprintRepository: Send + Sync {
    async fn get_active_by_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Option<Sprint>, AppError>;
    async fn get_by_id(&self, id: SprintId) -> Result<Sprint, AppError>;
    async fn save(&self, sprint: &Sprint) -> Result<SprintId, AppError>;
}

#[async_trait]
pub trait UnitOfWork: Send + Sync {
    async fn with_transaction<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a> FnOnce(
                &'a Repositories,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<T, AppError>> + Send + 'a>,
            > + Send
            + 'static,
        T: Send + 'static;
}

#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: crate::ProjectEvent) -> Result<(), AppError>;
}

#[derive(Clone)]
pub struct Repositories {
    pub users: Arc<dyn UserRepository>,
    pub projects: Arc<dyn ProjectRepository>,
    pub issues: Arc<dyn IssueRepository>,
    pub boards: Arc<dyn BoardRepository>,
    pub sprints: Arc<dyn SprintRepository>,
}

impl Default for Repositories {
    fn default() -> Self {
        Self {
            users: Arc::new(StubUserRepository),
            projects: Arc::new(StubProjectRepository),
            issues: Arc::new(StubIssueRepository),
            boards: Arc::new(StubBoardRepository),
            sprints: Arc::new(StubSprintRepository),
        }
    }
}

pub struct StubUserRepository;
#[async_trait]
impl UserRepository for StubUserRepository {
    async fn get_by_id(&self, _id: UserId) -> Result<User, AppError> {
        Err(AppError::not_found("user", "stub"))
    }
    async fn get_by_email(&self, _email: &str) -> Result<User, AppError> {
        Err(AppError::not_found("user", "stub"))
    }
    async fn save(&self, _user: &User) -> Result<UserId, AppError> {
        Ok(UserId::new())
    }
}

pub struct StubProjectRepository;
#[async_trait]
impl ProjectRepository for StubProjectRepository {
    async fn get_by_id(&self, _id: ProjectId) -> Result<Project, AppError> {
        Err(AppError::not_found("project", "stub"))
    }
    async fn get_by_key(&self, _key: &ProjectKey) -> Result<Project, AppError> {
        Err(AppError::not_found("project", "stub"))
    }
    async fn list(&self, _query: ProjectQuery) -> Result<Vec<Project>, AppError> {
        Ok(vec![])
    }
    async fn save(&self, _project: &Project) -> Result<ProjectId, AppError> {
        Ok(ProjectId::new())
    }
    async fn next_issue_number(&self, _project_id: ProjectId) -> Result<u32, AppError> {
        Ok(1)
    }
}

pub struct StubIssueRepository;
#[async_trait]
impl IssueRepository for StubIssueRepository {
    async fn get_by_id(&self, _id: IssueId) -> Result<Issue, AppError> {
        Err(AppError::not_found("issue", "stub"))
    }
    async fn get_by_key(&self, _key: &shared::IssueKey) -> Result<Issue, AppError> {
        Err(AppError::not_found("issue", "stub"))
    }
    async fn list(&self, _query: IssueQuery) -> Result<Vec<Issue>, AppError> {
        Ok(vec![])
    }
    async fn save(&self, _issue: &Issue) -> Result<IssueId, AppError> {
        Ok(IssueId::new())
    }
    async fn delete(&self, _id: IssueId) -> Result<(), AppError> {
        Ok(())
    }
}

pub struct StubBoardRepository;
#[async_trait]
impl BoardRepository for StubBoardRepository {
    async fn get_by_id(&self, _id: BoardId) -> Result<Board, AppError> {
        Err(AppError::not_found("board", "stub"))
    }
    async fn get_default_by_project(&self, _project_id: ProjectId) -> Result<Board, AppError> {
        Err(AppError::not_found("board", "stub"))
    }
    async fn get_default_by_project_key(&self, _key: &ProjectKey) -> Result<Board, AppError> {
        Err(AppError::not_found("board", "stub"))
    }
    async fn save(&self, _board: &Board) -> Result<(), AppError> {
        Ok(())
    }
}

pub struct StubSprintRepository;
#[async_trait]
impl SprintRepository for StubSprintRepository {
    async fn get_active_by_project(
        &self,
        _project_id: ProjectId,
    ) -> Result<Option<Sprint>, AppError> {
        Ok(None)
    }
    async fn get_by_id(&self, _id: SprintId) -> Result<Sprint, AppError> {
        Err(AppError::not_found("sprint", "stub"))
    }
    async fn save(&self, _sprint: &Sprint) -> Result<SprintId, AppError> {
        Ok(SprintId::new())
    }
}

pub struct StubUnitOfWork;
#[async_trait]
impl UnitOfWork for StubUnitOfWork {
    async fn with_transaction<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a> FnOnce(
                &'a Repositories,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<T, AppError>> + Send + 'a>,
            > + Send
            + 'static,
        T: Send + 'static,
    {
        f(&Repositories::default()).await
    }
}

pub struct StubEventBus;
#[async_trait]
impl EventBus for StubEventBus {
    async fn publish(&self, _event: crate::ProjectEvent) -> Result<(), AppError> {
        Ok(())
    }
}
