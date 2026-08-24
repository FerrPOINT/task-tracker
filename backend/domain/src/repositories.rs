use async_trait::async_trait;
use std::sync::Arc;

#[cfg(test)]
#[path = "repositories/tests.rs"]
mod tests;

use crate::{
    Board, Comment, Issue, IssueLink, IssueQuery, IssueStatusHistory, IssueTypeEntity, Label,
    Project, ProjectMember, SavedFilter, Sprint, Status, User, WorkflowTransition, Worklog,
};
use shared::IssueTypeId;
use shared::{
    AppError, AttachmentId, BoardId, CommentId, IssueId, IssueKey, IssueLinkId, LabelId, ProjectId,
    ProjectKey, SavedFilterId, SprintId, StatusId, UserId, WorklogId,
};

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn get_by_id(&self, id: UserId) -> Result<User, AppError>;
    async fn get_by_email(&self, email: &str) -> Result<User, AppError>;
    async fn get_by_refresh_token(&self, token_hash: &str) -> Result<User, AppError>;
    async fn save(&self, user: &User) -> Result<UserId, AppError>;
    async fn list(&self) -> Result<Vec<User>, AppError>;
}

#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn get_by_id(&self, id: ProjectId) -> Result<Project, AppError>;
    async fn get_by_key(&self, key: &ProjectKey) -> Result<Project, AppError>;
    async fn list(&self, query: ProjectQuery) -> Result<Vec<Project>, AppError>;
    async fn save(&self, project: &Project) -> Result<ProjectId, AppError>;
    async fn delete(&self, id: ProjectId) -> Result<(), AppError>;
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
    async fn get_by_key(&self, key: &IssueKey) -> Result<Issue, AppError>;
    async fn list(&self, query: IssueQuery) -> Result<Vec<Issue>, AppError>;
    async fn save(&self, issue: &Issue) -> Result<IssueId, AppError>;
    async fn delete(&self, id: IssueId) -> Result<(), AppError>;
}

#[async_trait]
pub trait StatusRepository: Send + Sync {
    async fn get_by_id(&self, id: StatusId) -> Result<Status, AppError>;
    async fn list_all(&self) -> Result<Vec<Status>, AppError>;
    async fn get_default(&self) -> Result<Status, AppError>;
}

#[async_trait]
pub trait WorkflowTransitionRepository: Send + Sync {
    async fn list_all(&self) -> Result<Vec<WorkflowTransition>, AppError>;
    async fn is_allowed(
        &self,
        from_status_id: StatusId,
        to_status_id: StatusId,
    ) -> Result<bool, AppError>;
}

#[async_trait]
pub trait IssueTypeRepository: Send + Sync {
    async fn get_by_id(&self, id: IssueTypeId) -> Result<IssueTypeEntity, AppError>;
    async fn list_all(&self) -> Result<Vec<IssueTypeEntity>, AppError>;
}

#[async_trait]
pub trait IssueStatusHistoryRepository: Send + Sync {
    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<IssueStatusHistory>, AppError>;
    async fn save(&self, entry: &IssueStatusHistory) -> Result<(), AppError>;
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
    async fn list_by_project(&self, project_id: ProjectId) -> Result<Vec<Sprint>, AppError>;
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
pub trait AttachmentRepository: Send + Sync {
    async fn get_by_id(&self, id: AttachmentId) -> Result<crate::Attachment, AppError>;
    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<crate::Attachment>, AppError>;
    async fn save(&self, attachment: &crate::Attachment) -> Result<AttachmentId, AppError>;
    async fn delete(&self, id: AttachmentId) -> Result<(), AppError>;
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
    pub statuses: Arc<dyn StatusRepository>,
    pub transitions: Arc<dyn WorkflowTransitionRepository>,
    pub issue_types: Arc<dyn IssueTypeRepository>,
    pub attachments: Arc<dyn AttachmentRepository>,
    pub labels: Arc<dyn LabelRepository>,
    pub issue_links: Arc<dyn IssueLinkRepository>,
    pub saved_filters: Arc<dyn SavedFilterRepository>,
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
            statuses: Arc::new(StubStatusRepository),
            transitions: Arc::new(StubWorkflowTransitionRepository),
            issue_types: Arc::new(StubIssueTypeRepository),
            attachments: Arc::new(StubAttachmentRepository),
            labels: Arc::new(StubLabelRepository),
            issue_links: Arc::new(StubIssueLinkRepository),
            saved_filters: Arc::new(StubSavedFilterRepository),
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

pub struct StubAttachmentRepository;
#[async_trait]
impl AttachmentRepository for StubAttachmentRepository {
    async fn get_by_id(&self, _id: AttachmentId) -> Result<crate::Attachment, AppError> {
        Err(AppError::not_found("attachment", "stub"))
    }
    async fn list_by_issue(&self, _issue_id: IssueId) -> Result<Vec<crate::Attachment>, AppError> {
        Ok(vec![])
    }
    async fn save(&self, _attachment: &crate::Attachment) -> Result<AttachmentId, AppError> {
        Ok(AttachmentId::new())
    }
    async fn delete(&self, _id: AttachmentId) -> Result<(), AppError> {
        Ok(())
    }
}

pub struct StubLabelRepository;
#[async_trait]
impl LabelRepository for StubLabelRepository {
    async fn get_by_id(&self, _id: LabelId) -> Result<Label, AppError> {
        Err(AppError::not_found("label", "stub"))
    }
    async fn list_by_project(&self, _project_id: ProjectId) -> Result<Vec<Label>, AppError> {
        Ok(vec![])
    }
    async fn save(&self, _label: &Label) -> Result<LabelId, AppError> {
        Ok(LabelId::new())
    }
    async fn delete(&self, _id: LabelId) -> Result<(), AppError> {
        Ok(())
    }
    async fn list_ids_by_issue(&self, _issue_id: IssueId) -> Result<Vec<LabelId>, AppError> {
        Ok(vec![])
    }
    async fn attach(&self, _issue_id: IssueId, _label_id: LabelId) -> Result<(), AppError> {
        Ok(())
    }
    async fn detach(&self, _issue_id: IssueId, _label_id: LabelId) -> Result<(), AppError> {
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

    async fn list(&self) -> Result<Vec<User>, AppError> {
        Ok(vec![])
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

    async fn delete(&self, _id: ProjectId) -> Result<(), AppError> {
        Ok(())
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
        Err(AppError::not_found("issue", "stub"))
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
    async fn list_by_project(&self, _project_id: ProjectId) -> Result<Vec<Sprint>, AppError> {
        Ok(vec![])
    }
    async fn save(&self, _sprint: &Sprint) -> Result<SprintId, AppError> {
        Ok(SprintId::new())
    }
}

pub struct StubStatusRepository;
#[async_trait]
impl StatusRepository for StubStatusRepository {
    async fn get_by_id(&self, _id: StatusId) -> Result<Status, AppError> {
        Err(AppError::not_found("status", "stub"))
    }
    async fn list_all(&self) -> Result<Vec<Status>, AppError> {
        Ok(vec![])
    }
    async fn get_default(&self) -> Result<Status, AppError> {
        Err(AppError::not_found("status", "stub"))
    }
}

pub struct StubWorkflowTransitionRepository;
#[async_trait]
impl WorkflowTransitionRepository for StubWorkflowTransitionRepository {
    async fn list_all(&self) -> Result<Vec<WorkflowTransition>, AppError> {
        Ok(vec![])
    }
    async fn is_allowed(
        &self,
        _from_status_id: StatusId,
        _to_status_id: StatusId,
    ) -> Result<bool, AppError> {
        Ok(true)
    }
}

pub struct StubIssueTypeRepository;
#[async_trait]
impl IssueTypeRepository for StubIssueTypeRepository {
    async fn get_by_id(&self, _id: IssueTypeId) -> Result<IssueTypeEntity, AppError> {
        Err(AppError::not_found("issue type", "stub"))
    }
    async fn list_all(&self) -> Result<Vec<IssueTypeEntity>, AppError> {
        Ok(vec![])
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

#[async_trait]
pub trait FileStorage: Send + Sync {
    async fn put(&self, issue_id: &str, key: &str, bytes: Vec<u8>) -> Result<(), AppError>;
    async fn get(&self, issue_id: &str, key: &str) -> Result<Vec<u8>, AppError>;
    async fn delete(&self, issue_id: &str, key: &str) -> Result<(), AppError>;
}

/// In-memory FileStorage for tests.
#[derive(Default)]
pub struct InMemoryStorage {
    files: std::sync::Mutex<std::collections::HashMap<(String, String), Vec<u8>>>,
}

#[async_trait]
impl FileStorage for InMemoryStorage {
    async fn put(&self, issue_id: &str, key: &str, bytes: Vec<u8>) -> Result<(), AppError> {
        if bytes.is_empty() {
            return Err(AppError::invalid_input("file is empty"));
        }
        self.files
            .lock()
            .unwrap()
            .insert((issue_id.to_string(), key.to_string()), bytes);
        Ok(())
    }
    async fn get(&self, issue_id: &str, key: &str) -> Result<Vec<u8>, AppError> {
        self.files
            .lock()
            .unwrap()
            .get(&(issue_id.to_string(), key.to_string()))
            .cloned()
            .ok_or_else(|| AppError::not_found("attachment file", key))
    }
    async fn delete(&self, issue_id: &str, key: &str) -> Result<(), AppError> {
        self.files
            .lock()
            .unwrap()
            .remove(&(issue_id.to_string(), key.to_string()));
        Ok(())
    }
}

#[async_trait]
pub trait LabelRepository: Send + Sync {
    async fn get_by_id(&self, id: LabelId) -> Result<Label, AppError>;
    async fn list_by_project(&self, project_id: ProjectId) -> Result<Vec<Label>, AppError>;
    async fn save(&self, label: &Label) -> Result<LabelId, AppError>;
    async fn delete(&self, id: LabelId) -> Result<(), AppError>;
    async fn list_ids_by_issue(&self, issue_id: IssueId) -> Result<Vec<LabelId>, AppError>;
    async fn attach(&self, issue_id: IssueId, label_id: LabelId) -> Result<(), AppError>;
    async fn detach(&self, issue_id: IssueId, label_id: LabelId) -> Result<(), AppError>;
}

#[async_trait]
pub trait IssueLinkRepository: Send + Sync {
    async fn save(&self, link: &IssueLink) -> Result<IssueLinkId, AppError>;
    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<IssueLink>, AppError>;
    async fn delete(&self, id: IssueLinkId) -> Result<(), AppError>;
}

pub struct StubIssueLinkRepository;
#[async_trait]
impl IssueLinkRepository for StubIssueLinkRepository {
    async fn save(&self, _link: &IssueLink) -> Result<IssueLinkId, AppError> {
        Ok(IssueLinkId::new())
    }
    async fn list_by_issue(&self, _issue_id: IssueId) -> Result<Vec<IssueLink>, AppError> {
        Ok(vec![])
    }
    async fn delete(&self, _id: IssueLinkId) -> Result<(), AppError> {
        Ok(())
    }
}

#[async_trait]
pub trait SavedFilterRepository: Send + Sync {
    async fn get_by_id(&self, id: SavedFilterId) -> Result<SavedFilter, AppError>;
    async fn list_by_owner(&self, owner_id: UserId) -> Result<Vec<SavedFilter>, AppError>;
    async fn list_public(&self) -> Result<Vec<SavedFilter>, AppError>;
    async fn save(&self, filter: &SavedFilter) -> Result<SavedFilterId, AppError>;
    async fn delete(&self, id: SavedFilterId) -> Result<(), AppError>;
}

pub struct StubSavedFilterRepository;
#[async_trait]
impl SavedFilterRepository for StubSavedFilterRepository {
    async fn get_by_id(&self, _id: SavedFilterId) -> Result<SavedFilter, AppError> {
        Err(AppError::not_found("saved_filter", "stub"))
    }
    async fn list_by_owner(&self, _owner_id: UserId) -> Result<Vec<SavedFilter>, AppError> {
        Ok(vec![])
    }
    async fn list_public(&self) -> Result<Vec<SavedFilter>, AppError> {
        Ok(vec![])
    }
    async fn save(&self, _filter: &SavedFilter) -> Result<SavedFilterId, AppError> {
        Ok(SavedFilterId::new())
    }
    async fn delete(&self, _id: SavedFilterId) -> Result<(), AppError> {
        Ok(())
    }
}
