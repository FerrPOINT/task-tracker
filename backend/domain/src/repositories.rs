use async_trait::async_trait;
use std::sync::Arc;

#[cfg(test)]
#[path = "repositories/tests.rs"]
mod tests;

use crate::{Board, Comment, Issue, IssueQuery, Project, ProjectMember, Sprint, User, Worklog};
use shared::{
    AppError, BoardId, CommentId, IssueId, ProjectId, ProjectKey, SprintId, UserId, WorklogId,
};

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn get_by_id(&self, id: UserId) -> Result<User, AppError>;
    async fn get_by_email(&self, email: &str) -> Result<User, AppError>;
    async fn get_by_refresh_token(&self, token_hash: &str) -> Result<User, AppError>;
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
pub trait CommentRepository: Send + Sync {
    async fn get_by_id(&self, id: CommentId) -> Result<Comment, AppError>;
    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<Comment>, AppError>;
    async fn save(&self, comment: &Comment) -> Result<CommentId, AppError>;
    async fn delete(&self, id: CommentId) -> Result<(), AppError>;
}

#[async_trait]
pub trait WorklogRepository: Send + Sync {
    async fn get_by_id(&self, id: WorklogId) -> Result<Worklog, AppError>;
    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<Worklog>, AppError>;
    async fn save(&self, worklog: &Worklog) -> Result<WorklogId, AppError>;
    async fn delete(&self, id: WorklogId) -> Result<(), AppError>;
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
    pub comments: Arc<dyn CommentRepository>,
    pub worklogs: Arc<dyn WorklogRepository>,
    pub members: Arc<dyn ProjectMemberRepository>,
}

impl Default for Repositories {
    fn default() -> Self {
        Self {
            users: Arc::new(StubUserRepository),
            projects: Arc::new(StubProjectRepository),
            issues: Arc::new(StubIssueRepository),
            boards: Arc::new(StubBoardRepository),
            sprints: Arc::new(StubSprintRepository),
            comments: Arc::new(StubCommentRepository),
            worklogs: Arc::new(StubWorklogRepository),
            members: Arc::new(StubProjectMemberRepository),
        }
    }
}

pub struct StubProjectMemberRepository;
#[async_trait]
impl ProjectMemberRepository for StubProjectMemberRepository {
    async fn list_by_project(
        &self,
        _project_id: ProjectId,
    ) -> Result<Vec<ProjectMember>, AppError> {
        Ok(vec![])
    }
    async fn get(
        &self,
        _project_id: ProjectId,
        _user_id: UserId,
    ) -> Result<ProjectMember, AppError> {
        Err(AppError::not_found("project member", _project_id))
    }
    async fn save(&self, _member: &ProjectMember) -> Result<(), AppError> {
        Ok(())
    }
    async fn delete(&self, _project_id: ProjectId, _user_id: UserId) -> Result<(), AppError> {
        Ok(())
    }
}

pub struct StubCommentRepository;
#[async_trait]
impl CommentRepository for StubCommentRepository {
    async fn get_by_id(&self, _id: CommentId) -> Result<Comment, AppError> {
        Err(AppError::not_found("comment", "stub"))
    }
    async fn list_by_issue(&self, _issue_id: IssueId) -> Result<Vec<Comment>, AppError> {
        Ok(vec![])
    }
    async fn save(&self, _comment: &Comment) -> Result<CommentId, AppError> {
        Ok(CommentId::new())
    }
    async fn delete(&self, _id: CommentId) -> Result<(), AppError> {
        Ok(())
    }
}

pub struct StubWorklogRepository;
#[async_trait]
impl WorklogRepository for StubWorklogRepository {
    async fn get_by_id(&self, _id: WorklogId) -> Result<Worklog, AppError> {
        Err(AppError::not_found("worklog", "stub"))
    }
    async fn list_by_issue(&self, _issue_id: IssueId) -> Result<Vec<Worklog>, AppError> {
        Ok(vec![])
    }
    async fn save(&self, _worklog: &Worklog) -> Result<WorklogId, AppError> {
        Ok(WorklogId::new())
    }
    async fn delete(&self, _id: WorklogId) -> Result<(), AppError> {
        Ok(())
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
    async fn get_by_refresh_token(&self, _token_hash: &str) -> Result<User, AppError> {
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

#[async_trait]
pub trait ProjectMemberRepository: Send + Sync {
    async fn list_by_project(&self, project_id: ProjectId) -> Result<Vec<ProjectMember>, AppError>;
    async fn get(&self, project_id: ProjectId, user_id: UserId) -> Result<ProjectMember, AppError>;
    async fn save(&self, member: &ProjectMember) -> Result<(), AppError>;
    async fn delete(&self, project_id: ProjectId, user_id: UserId) -> Result<(), AppError>;
}
