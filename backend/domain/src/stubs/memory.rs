use async_trait::async_trait;
use std::sync::{Arc, Mutex};

#[cfg(test)]
#[path = "memory/tests.rs"]
mod tests;

use crate::{
    Board, BoardRepository, EventBus, Issue, IssueQuery, IssueRepository, Project, ProjectQuery,
    ProjectRepository, Sprint, SprintRepository, UnitOfWork, User, UserRepository,
};
use shared::{AppError, BoardId, IssueId, ProjectId, ProjectKey, SprintId, UserId};

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

    async fn save(&self, user: &User) -> Result<UserId, AppError> {
        let mut users = self.users.lock().unwrap();
        if let Some(idx) = users.iter().position(|u| u.id == user.id) {
            users[idx] = user.clone();
        } else {
            users.push(user.clone());
        }
        Ok(user.id)
    }
}

#[derive(Default)]
pub struct MemoryProjectRepository {
    projects: Arc<Mutex<Vec<Project>>>,
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
            projects.push(project.clone());
        }
        Ok(project.id)
    }

    async fn next_issue_number(&self, project_id: ProjectId) -> Result<u32, AppError> {
        let projects = self.projects.lock().unwrap();
        let project = projects
            .iter()
            .find(|p| p.id == project_id)
            .ok_or_else(|| AppError::not_found("project", project_id))?;
        let count = projects.iter().filter(|p| p.id == project.id).count();
        Ok(count as u32 + 1)
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
            .filter(|i| query.project_id.map_or(true, |pid| i.project_id == pid))
            .filter(|i| query.status_id.map_or(true, |sid| i.status_id == sid))
            .filter(|i| {
                query
                    .assignee_id
                    .map_or(true, |aid| i.assignee_id == Some(aid))
            })
            .filter(|i| {
                query
                    .sprint_id
                    .map_or(true, |spid| i.sprint_id == Some(spid))
            })
            .filter(|i| {
                query.search_text.as_ref().map_or(true, |q| {
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
        issues.retain(|i| i.id != id);
        Ok(())
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
