use async_trait::async_trait;
use std::sync::{Arc, Mutex};

#[cfg(test)]
#[path = "memory/tests.rs"]
mod tests;

use crate::{
    Board, BoardRepository, Comment, CommentRepository, EventBus, Issue, IssueQuery,
    IssueRepository, Notification, NotificationRepository, NotificationUserSettings, Project,
    ProjectMember, ProjectMemberRepository, ProjectQuery, ProjectRepository, SavedFilter,
    SavedFilterRepository, Sprint, SprintRepository, UnitOfWork, User,
    UserNotificationSettingsRepository, UserRepository, Worklog, WorklogRepository,
};
use shared::{
    AppError, BoardId, CommentId, IssueId, NotificationId, ProjectId, ProjectKey, SavedFilterId,
    SprintId, UserId, WorklogId,
};

#[derive(Default)]
pub struct MemoryUserRepository {
    users: Arc<Mutex<Vec<User>>>,
}

#[async_trait]
impl UserRepository for MemoryUserRepository {
    async fn get_by_id(&self, id: UserId) -> Result<User, AppError> {
        let users = self.users.lock().unwrap();
        users
            .iter()
            .find(|u| u.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("user", id))
    }

    async fn get_by_email(&self, email: &str) -> Result<User, AppError> {
        let users = self.users.lock().unwrap();
        users
            .iter()
            .find(|u| u.email.as_ref() == email)
            .cloned()
            .ok_or_else(|| AppError::not_found("user", email))
    }

    async fn get_by_refresh_token(&self, token_hash: &str) -> Result<User, AppError> {
        let users = self.users.lock().unwrap();
        users
            .iter()
            .find(|u| {
                u.refresh_token_hash
                    .as_ref()
                    .is_some_and(|h| h.as_ref() == token_hash)
            })
            .cloned()
            .ok_or_else(|| AppError::not_found("user", "refresh"))
    }

    async fn save(&self, user: &User) -> Result<UserId, AppError> {
        let mut users = self.users.lock().unwrap();
        if let Some(idx) = users.iter().position(|u| u.id == user.id) {
            users[idx] = user.clone();
        } else {
            users.push(user.clone());
        }
        Ok(user.id)
    }

    async fn list(&self) -> Result<Vec<User>, AppError> {
        let users = self.users.lock().unwrap();
        Ok(users.clone())
    }
}

#[derive(Default)]
pub struct MemoryProjectRepository {
    projects: Arc<Mutex<Vec<Project>>>,
    issue_counters: Arc<Mutex<std::collections::HashMap<ProjectId, u32>>>,
}

#[async_trait]
impl ProjectRepository for MemoryProjectRepository {
    async fn get_by_id(&self, id: ProjectId) -> Result<Project, AppError> {
        let projects = self.projects.lock().unwrap();
        projects
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("project", id))
    }

    async fn get_by_key(&self, key: &ProjectKey) -> Result<Project, AppError> {
        let projects = self.projects.lock().unwrap();
        projects
            .iter()
            .find(|p| &p.key == key)
            .cloned()
            .ok_or_else(|| AppError::not_found("project", key))
    }

    async fn list(&self, _query: ProjectQuery) -> Result<Vec<Project>, AppError> {
        let projects = self.projects.lock().unwrap();
        Ok(projects.clone())
    }

    async fn save(&self, project: &Project) -> Result<ProjectId, AppError> {
        let mut projects = self.projects.lock().unwrap();
        if let Some(idx) = projects.iter().position(|p| p.id == project.id) {
            projects[idx] = project.clone();
        } else {
            if projects.iter().any(|p| p.key == project.key) {
                return Err(AppError::conflict(format!(
                    "project key {} already exists",
                    project.key
                )));
            }
            projects.push(project.clone());
        }
        Ok(project.id)
    }

    async fn next_issue_number(&self, project_id: ProjectId) -> Result<u32, AppError> {
        // Monotonic per-project counter so sequential and concurrent creation
        // never yields duplicate keys (mirrors the SQL repo MAX(number)+1 retry).
        let mut counters = self.issue_counters.lock().unwrap();
        let next = counters.entry(project_id).or_insert(0);
        *next += 1;
        Ok(*next)
    }

    async fn delete(&self, id: ProjectId) -> Result<(), AppError> {
        let mut projects = self.projects.lock().unwrap();
        if let Some(idx) = projects.iter().position(|p| p.id == id) {
            projects.remove(idx);
            Ok(())
        } else {
            Err(AppError::not_found("project", id))
        }
    }
}

#[derive(Default)]
pub struct MemoryIssueRepository {
    issues: Arc<Mutex<Vec<Issue>>>,
}

#[async_trait]
impl IssueRepository for MemoryIssueRepository {
    async fn get_by_id(&self, id: IssueId) -> Result<Issue, AppError> {
        let issues = self.issues.lock().unwrap();
        issues
            .iter()
            .find(|i| i.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("issue", id))
    }

    async fn get_by_key(&self, key: &shared::IssueKey) -> Result<Issue, AppError> {
        let issues = self.issues.lock().unwrap();
        issues
            .iter()
            .find(|i| &i.key == key)
            .cloned()
            .ok_or_else(|| AppError::not_found("issue", key))
    }

    async fn list(&self, query: IssueQuery) -> Result<Vec<Issue>, AppError> {
        let issues = self.issues.lock().unwrap();
        let mut result: Vec<Issue> = issues
            .iter()
            .filter(|i| query.project_id.is_none_or(|pid| i.project_id == pid))
            .filter(|i| query.status_id.is_none_or(|sid| i.status_id == sid))
            .filter(|i| {
                query
                    .assignee_id
                    .is_none_or(|aid| i.assignee_id == Some(aid))
            })
            .filter(|i| query.sprint_id.is_none_or(|spid| i.sprint_id == Some(spid)))
            .filter(|i| {
                query.search_text.as_ref().is_none_or(|q| {
                    i.summary
                        .as_ref()
                        .to_ascii_lowercase()
                        .contains(&q.to_ascii_lowercase())
                        || i.key
                            .to_string()
                            .to_ascii_lowercase()
                            .contains(&q.to_ascii_lowercase())
                })
            })
            .cloned()
            .collect();
        result.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
        let offset = query.offset as usize;
        let limit = query.limit as usize;
        Ok(result.into_iter().skip(offset).take(limit).collect())
    }

    async fn save(&self, issue: &Issue) -> Result<IssueId, AppError> {
        let mut issues = self.issues.lock().unwrap();
        if let Some(idx) = issues.iter().position(|i| i.id == issue.id) {
            issues[idx] = issue.clone();
        } else {
            issues.push(issue.clone());
        }
        Ok(issue.id)
    }

    async fn delete(&self, id: IssueId) -> Result<(), AppError> {
        let mut issues = self.issues.lock().unwrap();
        if let Some(idx) = issues.iter().position(|i| i.id == id) {
            issues.remove(idx);
            Ok(())
        } else {
            Err(AppError::not_found("issue", id))
        }
    }
}

#[derive(Default)]
pub struct MemoryBoardRepository {
    boards: Arc<Mutex<Vec<Board>>>,
}

#[async_trait]
impl BoardRepository for MemoryBoardRepository {
    async fn get_by_id(&self, id: BoardId) -> Result<Board, AppError> {
        let boards = self.boards.lock().unwrap();
        boards
            .iter()
            .find(|b| b.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("board", id))
    }

    async fn get_default_by_project(&self, project_id: ProjectId) -> Result<Board, AppError> {
        let boards = self.boards.lock().unwrap();
        boards
            .iter()
            .find(|b| b.project_id == project_id)
            .cloned()
            .ok_or_else(|| AppError::not_found("board", project_id))
    }

    async fn get_default_by_project_key(&self, key: &ProjectKey) -> Result<Board, AppError> {
        let boards = self.boards.lock().unwrap();
        boards
            .iter()
            .find(|b| {
                // best-effort project key lookup by scanning projects not available here
                b.columns.iter().any(|_| true) // placeholder; key check omitted
            })
            .cloned()
            .ok_or_else(|| AppError::not_found("board", key))
    }

    async fn save(&self, board: &Board) -> Result<(), AppError> {
        let mut boards = self.boards.lock().unwrap();
        if let Some(idx) = boards.iter().position(|b| b.id == board.id) {
            boards[idx] = board.clone();
        } else {
            boards.push(board.clone());
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct MemorySprintRepository {
    sprints: Arc<Mutex<Vec<Sprint>>>,
}

#[async_trait]
impl SprintRepository for MemorySprintRepository {
    async fn get_active_by_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Option<Sprint>, AppError> {
        let sprints = self.sprints.lock().unwrap();
        Ok(sprints
            .iter()
            .find(|s| s.project_id == project_id && matches!(s.state, crate::SprintState::Active))
            .cloned())
    }

    async fn get_by_id(&self, id: SprintId) -> Result<Sprint, AppError> {
        let sprints = self.sprints.lock().unwrap();
        sprints
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("sprint", id))
    }

    async fn list_by_project(&self, project_id: ProjectId) -> Result<Vec<Sprint>, AppError> {
        let sprints = self.sprints.lock().unwrap();
        Ok(sprints
            .iter()
            .filter(|s| s.project_id == project_id)
            .cloned()
            .collect())
    }

    async fn save(&self, sprint: &Sprint) -> Result<SprintId, AppError> {
        let mut sprints = self.sprints.lock().unwrap();
        if let Some(idx) = sprints.iter().position(|s| s.id == sprint.id) {
            sprints[idx] = sprint.clone();
        } else {
            sprints.push(sprint.clone());
        }
        Ok(sprint.id)
    }
}

pub struct MemoryUnitOfWork {
    repos: crate::Repositories,
}

impl MemoryUnitOfWork {
    pub fn new(repos: crate::Repositories) -> Self {
        Self { repos }
    }
}

#[async_trait]
impl UnitOfWork for MemoryUnitOfWork {
    async fn with_transaction<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a> FnOnce(
                &'a crate::Repositories,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<T, AppError>> + Send + 'a>,
            > + Send
            + 'static,
        T: Send + 'static,
    {
        f(&self.repos).await
    }
}

#[derive(Default)]
pub struct MemoryEventBus {
    events: Arc<Mutex<Vec<crate::ProjectEvent>>>,
}

#[async_trait]
impl EventBus for MemoryEventBus {
    async fn publish(&self, event: crate::ProjectEvent) -> Result<(), AppError> {
        let mut events = self.events.lock().unwrap();
        events.push(event);
        Ok(())
    }
}

impl MemoryEventBus {
    pub fn drained(&self) -> Vec<crate::ProjectEvent> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}

pub struct MemoryCommentRepository {
    comments: Arc<Mutex<Vec<Comment>>>,
}

impl MemoryCommentRepository {
    pub fn new() -> Self {
        Self {
            comments: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for MemoryCommentRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CommentRepository for MemoryCommentRepository {
    async fn get_by_id(&self, id: CommentId) -> Result<Comment, AppError> {
        let comments = self.comments.lock().unwrap();
        comments
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("comment", id))
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<Comment>, AppError> {
        let comments = self.comments.lock().unwrap();
        Ok(comments
            .iter()
            .filter(|c| c.issue_id == issue_id)
            .cloned()
            .collect())
    }

    async fn save(&self, comment: &Comment) -> Result<CommentId, AppError> {
        let mut comments = self.comments.lock().unwrap();
        if let Some(idx) = comments.iter().position(|c| c.id == comment.id) {
            comments[idx] = comment.clone();
        } else {
            comments.push(comment.clone());
        }
        Ok(comment.id)
    }

    async fn delete(&self, id: CommentId) -> Result<(), AppError> {
        let mut comments = self.comments.lock().unwrap();
        comments.retain(|c| c.id != id);
        Ok(())
    }
}

pub struct MemoryWorklogRepository {
    worklogs: Arc<Mutex<Vec<Worklog>>>,
}

impl MemoryWorklogRepository {
    pub fn new() -> Self {
        Self {
            worklogs: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for MemoryWorklogRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WorklogRepository for MemoryWorklogRepository {
    async fn get_by_id(&self, id: WorklogId) -> Result<Worklog, AppError> {
        let worklogs = self.worklogs.lock().unwrap();
        worklogs
            .iter()
            .find(|w| w.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("worklog", id))
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<Worklog>, AppError> {
        let worklogs = self.worklogs.lock().unwrap();
        Ok(worklogs
            .iter()
            .filter(|w| w.issue_id == issue_id)
            .cloned()
            .collect())
    }

    async fn save(&self, worklog: &Worklog) -> Result<WorklogId, AppError> {
        let mut worklogs = self.worklogs.lock().unwrap();
        if let Some(idx) = worklogs.iter().position(|w| w.id == worklog.id) {
            worklogs[idx] = worklog.clone();
        } else {
            worklogs.push(worklog.clone());
        }
        Ok(worklog.id)
    }

    async fn delete(&self, id: WorklogId) -> Result<(), AppError> {
        let mut worklogs = self.worklogs.lock().unwrap();
        worklogs.retain(|w| w.id != id);
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryProjectMemberRepository {
    members: Arc<Mutex<Vec<ProjectMember>>>,
}

#[async_trait]
impl ProjectMemberRepository for MemoryProjectMemberRepository {
    async fn list_by_project(&self, project_id: ProjectId) -> Result<Vec<ProjectMember>, AppError> {
        Ok(self
            .members
            .lock()
            .unwrap()
            .iter()
            .filter(|m| m.project_id == project_id)
            .cloned()
            .collect())
    }

    async fn get(&self, project_id: ProjectId, user_id: UserId) -> Result<ProjectMember, AppError> {
        self.members
            .lock()
            .unwrap()
            .iter()
            .find(|m| m.project_id == project_id && m.user_id == user_id)
            .cloned()
            .ok_or_else(|| AppError::not_found("project member", project_id))
    }

    async fn save(&self, member: &ProjectMember) -> Result<(), AppError> {
        let mut members = self.members.lock().unwrap();
        let idx = members
            .iter()
            .position(|m| m.project_id == member.project_id && m.user_id == member.user_id);
        if let Some(i) = idx {
            members[i] = member.clone();
        } else {
            members.push(member.clone());
        }
        Ok(())
    }

    async fn delete(&self, project_id: ProjectId, user_id: UserId) -> Result<(), AppError> {
        let mut members = self.members.lock().unwrap();
        let idx = members
            .iter()
            .position(|m| m.project_id == project_id && m.user_id == user_id);
        if let Some(i) = idx {
            members.remove(i);
        }
        Ok(())
    }
}

pub struct MemoryAttachmentRepository {
    attachments: Arc<Mutex<Vec<crate::Attachment>>>,
}

impl MemoryAttachmentRepository {
    pub fn new() -> Self {
        Self {
            attachments: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for MemoryAttachmentRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::AttachmentRepository for MemoryAttachmentRepository {
    async fn get_by_id(&self, id: shared::AttachmentId) -> Result<crate::Attachment, AppError> {
        let items = self.attachments.lock().unwrap();
        items
            .iter()
            .find(|a| a.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("attachment", id))
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<crate::Attachment>, AppError> {
        let items = self.attachments.lock().unwrap();
        Ok(items
            .iter()
            .filter(|a| a.issue_id == issue_id)
            .cloned()
            .collect())
    }

    async fn save(&self, attachment: &crate::Attachment) -> Result<shared::AttachmentId, AppError> {
        let mut items = self.attachments.lock().unwrap();
        if let Some(idx) = items.iter().position(|a| a.id == attachment.id) {
            items[idx] = attachment.clone();
        } else {
            items.push(attachment.clone());
        }
        Ok(attachment.id)
    }

    async fn delete(&self, id: shared::AttachmentId) -> Result<(), AppError> {
        let mut items = self.attachments.lock().unwrap();
        items.retain(|a| a.id != id);
        Ok(())
    }
}

pub struct MemoryLabelRepository {
    labels: Arc<Mutex<Vec<crate::Label>>>,
    issue_labels: Arc<Mutex<Vec<(IssueId, shared::LabelId)>>>,
}

impl MemoryLabelRepository {
    pub fn new() -> Self {
        Self {
            labels: Arc::new(Mutex::new(Vec::new())),
            issue_labels: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for MemoryLabelRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::LabelRepository for MemoryLabelRepository {
    async fn get_by_id(&self, id: shared::LabelId) -> Result<crate::Label, AppError> {
        self.labels
            .lock()
            .unwrap()
            .iter()
            .find(|l| l.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("label", id))
    }

    async fn list_by_project(&self, project_id: ProjectId) -> Result<Vec<crate::Label>, AppError> {
        Ok(self
            .labels
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.project_id == project_id)
            .cloned()
            .collect())
    }

    async fn save(&self, label: &crate::Label) -> Result<shared::LabelId, AppError> {
        let mut labels = self.labels.lock().unwrap();
        if let Some(idx) = labels.iter().position(|l| l.id == label.id) {
            labels[idx] = label.clone();
        } else {
            labels.push(label.clone());
        }
        Ok(label.id)
    }

    async fn delete(&self, id: shared::LabelId) -> Result<(), AppError> {
        self.labels.lock().unwrap().retain(|l| l.id != id);
        self.issue_labels
            .lock()
            .unwrap()
            .retain(|(_, lid)| *lid != id);
        Ok(())
    }

    async fn list_ids_by_issue(&self, issue_id: IssueId) -> Result<Vec<shared::LabelId>, AppError> {
        Ok(self
            .issue_labels
            .lock()
            .unwrap()
            .iter()
            .filter(|(iid, _)| *iid == issue_id)
            .map(|(_, lid)| *lid)
            .collect())
    }

    async fn attach(&self, issue_id: IssueId, label_id: shared::LabelId) -> Result<(), AppError> {
        let mut il = self.issue_labels.lock().unwrap();
        if !il.contains(&(issue_id, label_id)) {
            il.push((issue_id, label_id));
        }
        Ok(())
    }

    async fn detach(&self, issue_id: IssueId, label_id: shared::LabelId) -> Result<(), AppError> {
        self.issue_labels
            .lock()
            .unwrap()
            .retain(|(iid, lid)| !(*iid == issue_id && *lid == label_id));
        Ok(())
    }
}

pub struct MemoryIssueLinkRepository {
    links: Arc<Mutex<Vec<crate::IssueLink>>>,
}

impl MemoryIssueLinkRepository {
    pub fn new() -> Self {
        Self {
            links: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for MemoryIssueLinkRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::IssueLinkRepository for MemoryIssueLinkRepository {
    async fn save(&self, link: &crate::IssueLink) -> Result<shared::IssueLinkId, AppError> {
        let mut links = self.links.lock().unwrap();
        if let Some(idx) = links.iter().position(|l| l.id == link.id) {
            links[idx] = link.clone();
        } else {
            links.push(link.clone());
        }
        Ok(link.id)
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<crate::IssueLink>, AppError> {
        Ok(self
            .links
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.source_id == issue_id || l.target_id == issue_id)
            .cloned()
            .collect())
    }

    async fn delete(&self, id: shared::IssueLinkId) -> Result<(), AppError> {
        self.links.lock().unwrap().retain(|l| l.id != id);
        Ok(())
    }
}

#[derive(Default)]
pub struct MemorySavedFilterRepository {
    filters: Arc<Mutex<Vec<SavedFilter>>>,
}

#[async_trait]
impl SavedFilterRepository for MemorySavedFilterRepository {
    async fn get_by_id(&self, id: SavedFilterId) -> Result<SavedFilter, AppError> {
        self.filters
            .lock()
            .unwrap()
            .iter()
            .find(|filter| filter.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("saved_filter", id))
    }

    async fn list_by_owner(&self, owner_id: UserId) -> Result<Vec<SavedFilter>, AppError> {
        Ok(self
            .filters
            .lock()
            .unwrap()
            .iter()
            .filter(|filter| filter.owner_id == owner_id)
            .cloned()
            .collect())
    }

    async fn list_public(&self) -> Result<Vec<SavedFilter>, AppError> {
        Ok(self
            .filters
            .lock()
            .unwrap()
            .iter()
            .filter(|filter| filter.is_public)
            .cloned()
            .collect())
    }

    async fn save(&self, filter: &SavedFilter) -> Result<SavedFilterId, AppError> {
        let mut filters = self.filters.lock().unwrap();
        if let Some(index) = filters.iter().position(|existing| existing.id == filter.id) {
            filters[index] = filter.clone();
        } else {
            filters.push(filter.clone());
        }
        Ok(filter.id)
    }

    async fn delete(&self, id: SavedFilterId) -> Result<(), AppError> {
        let mut filters = self.filters.lock().unwrap();
        let Some(index) = filters.iter().position(|filter| filter.id == id) else {
            return Err(AppError::not_found("saved_filter", id));
        };
        filters.remove(index);
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryNotificationRepository {
    notifications: Arc<Mutex<Vec<Notification>>>,
    settings: Arc<Mutex<Vec<NotificationUserSettings>>>,
}

#[async_trait]
impl NotificationRepository for MemoryNotificationRepository {
    async fn save(&self, notification: &Notification) -> Result<NotificationId, AppError> {
        let mut notifications = self.notifications.lock().unwrap();
        if let Some(index) = notifications
            .iter()
            .position(|existing| existing.id == notification.id)
        {
            notifications[index] = notification.clone();
        } else {
            notifications.push(notification.clone());
        }
        Ok(notification.id)
    }

    async fn list_unread(&self, recipient_id: UserId) -> Result<Vec<Notification>, AppError> {
        let mut notifications: Vec<_> = self
            .notifications
            .lock()
            .unwrap()
            .iter()
            .filter(|notification| {
                notification.recipient_id == recipient_id && !notification.is_read
            })
            .cloned()
            .collect();
        notifications.sort_by_key(|notification| notification.created_at);
        Ok(notifications)
    }

    async fn mark_read(&self, id: NotificationId, recipient_id: UserId) -> Result<(), AppError> {
        let mut notifications = self.notifications.lock().unwrap();
        let notification = notifications
            .iter_mut()
            .find(|notification| notification.id == id && notification.recipient_id == recipient_id)
            .ok_or_else(|| AppError::not_found("notification", id))?;
        if !notification.is_read {
            notification.is_read = true;
            notification.read_at = Some(shared::now());
        }
        Ok(())
    }

    async fn mark_all_read(&self, recipient_id: UserId) -> Result<(), AppError> {
        let now = shared::now();
        for notification in self.notifications.lock().unwrap().iter_mut() {
            if notification.recipient_id == recipient_id && !notification.is_read {
                notification.is_read = true;
                notification.read_at = Some(now);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl UserNotificationSettingsRepository for MemoryNotificationRepository {
    async fn get_settings(&self, user_id: UserId) -> Result<NotificationUserSettings, AppError> {
        self.settings
            .lock()
            .unwrap()
            .iter()
            .find(|settings| settings.user_id == user_id)
            .cloned()
            .ok_or_else(|| AppError::not_found("notification settings", user_id))
    }

    async fn save_settings(&self, settings: &NotificationUserSettings) -> Result<(), AppError> {
        let mut all_settings = self.settings.lock().unwrap();
        if let Some(index) = all_settings
            .iter()
            .position(|existing| existing.user_id == settings.user_id)
        {
            all_settings[index] = settings.clone();
        } else {
            all_settings.push(settings.clone());
        }
        Ok(())
    }
}
