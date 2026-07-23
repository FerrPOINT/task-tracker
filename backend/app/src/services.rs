use async_trait::async_trait;
use std::sync::Arc;

use crate::commands::{CreateIssueCommand, UpdateIssueCommand};
use crate::dto::{
    BacklogDto, BoardColumnDto, BoardDto, DashboardDto, IssueDto, ProjectDto, SprintDto,
};
use domain::{
    BoardColumn, ColumnCategory, IssueQuery, IssueRepository, ProjectRepository, SprintRepository,
};

use shared::{AppError, IssueId, ProjectKey, StatusId, UserId};

#[cfg(test)]
mod tests;

pub struct ProjectServiceImpl {
    projects: Arc<dyn ProjectRepository>,
    issues: Arc<dyn IssueRepository>,
}

impl ProjectServiceImpl {
    pub fn new(projects: Arc<dyn ProjectRepository>, issues: Arc<dyn IssueRepository>) -> Self {
        Self { projects, issues }
    }
}

#[async_trait]
impl crate::context::ProjectService for ProjectServiceImpl {
    async fn list(
        &self,
        _query: crate::commands::ProjectQueryDto,
    ) -> Result<Vec<ProjectDto>, AppError> {
        let projects = self
            .projects
            .list(domain::ProjectQuery {
                owner_id: None,
                limit: 100,
                offset: 0,
            })
            .await?;
        let mut dtos = Vec::new();
        for project in projects {
            let counts = self.issues.list(IssueQuery::project(project.id)).await?;
            let todo = counts
                .iter()
                .filter(|i| i.status_id == todo_status())
                .count() as i64;
            let in_progress = counts
                .iter()
                .filter(|i| i.status_id == in_progress_status() || i.status_id == review_status())
                .count() as i64;
            let done = counts
                .iter()
                .filter(|i| i.status_id == done_status())
                .count() as i64;
            dtos.push(ProjectDto::from_project(project, todo, in_progress, done));
        }
        Ok(dtos)
    }

    async fn get_by_key(&self, key: &ProjectKey) -> Result<ProjectDto, AppError> {
        let project = self.projects.get_by_key(key).await?;
        let counts = self.issues.list(IssueQuery::project(project.id)).await?;
        let todo = counts
            .iter()
            .filter(|i| i.status_id == todo_status())
            .count() as i64;
        let in_progress = counts
            .iter()
            .filter(|i| i.status_id == in_progress_status() || i.status_id == review_status())
            .count() as i64;
        let done = counts
            .iter()
            .filter(|i| i.status_id == done_status())
            .count() as i64;
        Ok(ProjectDto::from_project(project, todo, in_progress, done))
    }
}

pub struct IssueServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
}

impl IssueServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        users: Arc<dyn domain::UserRepository>,
    ) -> Self {
        Self {
            issues,
            projects,
            users,
        }
    }

    async fn resolve_names(
        &self,
        issue: &domain::Issue,
    ) -> Result<(Option<String>, Option<String>), AppError> {
        let assignee_name = if let Some(id) = issue.assignee_id {
            self.users
                .get_by_id(id)
                .await
                .map(|u| u.display_name.as_ref().to_string())
                .ok()
        } else {
            None
        };
        let reporter_name = self
            .users
            .get_by_id(issue.reporter_id)
            .await
            .map(|u| u.display_name.as_ref().to_string())
            .ok();
        Ok((assignee_name, reporter_name))
    }
}

#[async_trait]
impl crate::context::IssueService for IssueServiceImpl {
    async fn create(&self, cmd: CreateIssueCommand) -> Result<IssueDto, AppError> {
        let project = self.projects.get_by_key(&cmd.project_key).await?;
        let number = self.projects.next_issue_number(project.id).await?;
        let status_id = StatusId::from_uuid(
            cmd.status_id
                .parse()
                .map_err(|_| AppError::invalid_input("status_id"))?,
        );
        let mut issue = domain::Issue::create(
            &project,
            number,
            cmd.issue_type,
            status_id,
            cmd.summary,
            cmd.description.map(domain::RichText::from),
            cmd.reporter_id,
            cmd.priority,
        );
        if let Some(assignee_id) = cmd.assignee_id {
            issue.assign(Some(assignee_id));
        }
        self.issues.save(&issue).await?;
        let column = default_board_columns()
            .into_iter()
            .find(|c| c.id == issue.status_id)
            .map(|c| c.name.as_ref().to_string())
            .unwrap_or_else(|| "Todo".to_string());
        let (assignee_name, reporter_name) = self.resolve_names(&issue).await?;
        Ok(IssueDto::from_issue(
            issue,
            project.name.as_ref().to_string(),
            column,
            assignee_name,
            reporter_name,
        ))
    }

    async fn get_by_id(&self, id: IssueId) -> Result<IssueDto, AppError> {
        let issue = self.issues.get_by_id(id).await?;
        let project = self.projects.get_by_id(issue.project_id).await?;
        let column = default_board_columns()
            .into_iter()
            .find(|c| c.id == issue.status_id)
            .map(|c| c.name.as_ref().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let (assignee_name, reporter_name) = self.resolve_names(&issue).await?;
        Ok(IssueDto::from_issue(
            issue,
            project.name.as_ref().to_string(),
            column,
            assignee_name,
            reporter_name,
        ))
    }

    async fn update(&self, id: IssueId, cmd: UpdateIssueCommand) -> Result<IssueDto, AppError> {
        let mut issue = self.issues.get_by_id(id).await?;
        let project = self.projects.get_by_id(issue.project_id).await?;

        if let Some(summary) = cmd.summary {
            issue.summary = summary.into();
            issue.updated_at = shared::now();
        }
        if let Some(description) = cmd.description {
            issue.description = description.map(domain::RichText::from);
            issue.updated_at = shared::now();
        }
        if let Some(priority) = cmd.priority {
            issue.priority = priority;
            issue.updated_at = shared::now();
        }
        if let Some(status_id) = cmd.status_id {
            let sid = status_id
                .parse()
                .map_err(|_| AppError::invalid_input("status_id"))?;
            issue.change_status(StatusId::from_uuid(sid));
        }
        if let Some(assignee_id) = cmd.assignee_id {
            issue.assign(assignee_id);
        }

        self.issues.save(&issue).await?;
        let column = default_board_columns()
            .into_iter()
            .find(|c| c.id == issue.status_id)
            .map(|c| c.name.as_ref().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let (assignee_name, reporter_name) = self.resolve_names(&issue).await?;
        Ok(IssueDto::from_issue(
            issue,
            project.name.as_ref().to_string(),
            column,
            assignee_name,
            reporter_name,
        ))
    }

    async fn search(&self, q: &str) -> Result<Vec<IssueDto>, AppError> {
        let issues = self
            .issues
            .list(IssueQuery {
                search_text: Some(q.to_string()),
                ..Default::default()
            })
            .await?;
        let mut dtos = Vec::new();
        for issue in issues {
            let project = self.projects.get_by_id(issue.project_id).await?;
            let column = default_board_columns()
                .into_iter()
                .find(|c| c.id == issue.status_id)
                .map(|c| c.name.as_ref().to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            let (assignee_name, reporter_name) = self.resolve_names(&issue).await?;
            dtos.push(IssueDto::from_issue(
                issue,
                project.name.as_ref().to_string(),
                column,
                assignee_name,
                reporter_name,
            ));
        }
        Ok(dtos)
    }
}

pub struct BoardServiceImpl {
    boards: Arc<dyn domain::BoardRepository>,
    issues: Arc<dyn IssueRepository>,
    sprints: Arc<dyn SprintRepository>,
    users: Arc<dyn domain::UserRepository>,
}

impl BoardServiceImpl {
    pub fn new(
        boards: Arc<dyn domain::BoardRepository>,
        issues: Arc<dyn IssueRepository>,
        sprints: Arc<dyn SprintRepository>,
        users: Arc<dyn domain::UserRepository>,
    ) -> Self {
        Self {
            boards,
            issues,
            sprints,
            users,
        }
    }

    async fn resolve_names(&self, issue: &domain::Issue) -> (Option<String>, Option<String>) {
        let assignee_name = if let Some(id) = issue.assignee_id {
            self.users
                .get_by_id(id)
                .await
                .map(|u| u.display_name.as_ref().to_string())
                .ok()
        } else {
            None
        };
        let reporter_name = self
            .users
            .get_by_id(issue.reporter_id)
            .await
            .map(|u| u.display_name.as_ref().to_string())
            .ok();
        (assignee_name, reporter_name)
    }

    async fn build_board_dto(&self, project_key: &ProjectKey) -> Result<BoardDto, AppError> {
        let board = self.boards.get_default_by_project_key(project_key).await?;
        let sprint = self.sprints.get_active_by_project(board.project_id).await?;
        let issues = self
            .issues
            .list(IssueQuery {
                project_id: Some(board.project_id),
                ..Default::default()
            })
            .await?;

        let columns: Vec<BoardColumnDto> = board
            .columns
            .iter()
            .map(|c| BoardColumnDto {
                id: c.id.to_string(),
                name: c.name.as_ref().to_string(),
                wip_limit: c.wip_limit,
                issue_ids: issues
                    .iter()
                    .filter(|i| i.status_id == c.id)
                    .map(|i| i.id.to_string())
                    .collect(),
            })
            .collect();

        let mut dtos = Vec::new();
        for issue in issues {
            let column = board
                .columns
                .iter()
                .find(|c| c.id == issue.status_id)
                .map(|c| c.name.as_ref().to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            let (a, r) = self.resolve_names(&issue).await;
            dtos.push(IssueDto::from_issue(
                issue,
                project_key.to_string(),
                column,
                a,
                r,
            ));
        }

        let sprint_dto = sprint
            .map(|s| SprintDto::from_sprint(s, dtos.iter().map(|i| i.id.clone()).collect()))
            .unwrap_or_else(|| SprintDto {
                id: "none".to_string(),
                name: "Backlog".to_string(),
                goal: String::new(),
                state: "future".to_string(),
                velocity: 0,
                remaining_days: None,
                issue_ids: vec![],
            });

        Ok(BoardDto {
            columns,
            issues: dtos,
            sprint: sprint_dto,
        })
    }
}

#[async_trait]
impl crate::context::BoardService for BoardServiceImpl {
    async fn get_board(&self, project_key: &ProjectKey) -> Result<BoardDto, AppError> {
        self.build_board_dto(project_key).await
    }

    async fn get_backlog(&self, project_key: &ProjectKey) -> Result<BacklogDto, AppError> {
        let board = self.boards.get_default_by_project_key(project_key).await?;
        let sprint = self.sprints.get_active_by_project(board.project_id).await?;
        let all_issues = self
            .issues
            .list(IssueQuery {
                project_id: Some(board.project_id),
                ..Default::default()
            })
            .await?;

        let todo_status = board
            .columns
            .iter()
            .find(|c| c.category == ColumnCategory::Todo)
            .map(|c| c.id)
            .unwrap_or(StatusId::from_uuid(uuid::Uuid::nil()));

        let sprint_issues_raw: Vec<_> = all_issues
            .clone()
            .into_iter()
            .filter(|i| i.sprint_id.is_some() || i.status_id != todo_status)
            .collect();
        let backlog_issues_raw: Vec<_> = all_issues
            .into_iter()
            .filter(|i| i.sprint_id.is_none() && i.status_id == todo_status)
            .collect();

        let sprint_dto = sprint
            .map(|s| {
                SprintDto::from_sprint(
                    s,
                    sprint_issues_raw.iter().map(|i| i.id.to_string()).collect(),
                )
            })
            .unwrap_or_else(|| SprintDto {
                id: "none".to_string(),
                name: "Backlog".to_string(),
                goal: String::new(),
                state: "future".to_string(),
                velocity: 0,
                remaining_days: None,
                issue_ids: vec![],
            });

        let mut sprint_issues = Vec::new();
        for i in sprint_issues_raw {
            let column = board
                .columns
                .iter()
                .find(|c| c.id == i.status_id)
                .map(|c| c.name.as_ref().to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            let (a, r) = self.resolve_names(&i).await;
            sprint_issues.push(IssueDto::from_issue(
                i,
                project_key.to_string(),
                column,
                a,
                r,
            ));
        }

        let mut backlog_issues = Vec::new();
        for i in backlog_issues_raw {
            let column = board
                .columns
                .iter()
                .find(|c| c.id == i.status_id)
                .map(|c| c.name.as_ref().to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            let (a, r) = self.resolve_names(&i).await;
            backlog_issues.push(IssueDto::from_issue(
                i,
                project_key.to_string(),
                column,
                a,
                r,
            ));
        }

        Ok(BacklogDto {
            sprint: sprint_dto,
            sprint_issues,
            backlog_issues,
        })
    }

    async fn move_issue(
        &self,
        project_key: &ProjectKey,
        issue_id: IssueId,
        status_id: StatusId,
    ) -> Result<BoardDto, AppError> {
        let mut issue = self.issues.get_by_id(issue_id).await?;
        issue.change_status(status_id);
        self.issues.save(&issue).await?;
        self.build_board_dto(project_key).await
    }
}

pub struct DashboardServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
}

impl DashboardServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        users: Arc<dyn domain::UserRepository>,
    ) -> Self {
        Self {
            issues,
            projects,
            users,
        }
    }

    async fn resolve_names(&self, issue: &domain::Issue) -> (Option<String>, Option<String>) {
        let assignee_name = if let Some(id) = issue.assignee_id {
            self.users
                .get_by_id(id)
                .await
                .map(|u| u.display_name.as_ref().to_string())
                .ok()
        } else {
            None
        };
        let reporter_name = self
            .users
            .get_by_id(issue.reporter_id)
            .await
            .map(|u| u.display_name.as_ref().to_string())
            .ok();
        (assignee_name, reporter_name)
    }
}

#[async_trait]
impl crate::context::DashboardService for DashboardServiceImpl {
    async fn get_dashboard(&self, user_id: UserId) -> Result<DashboardDto, AppError> {
        let issues = self.issues.list(IssueQuery::assignee(user_id)).await?;
        let mut dtos = Vec::new();
        for issue in issues {
            let project = self.projects.get_by_id(issue.project_id).await?;
            let column = default_board_columns()
                .into_iter()
                .find(|c| c.id == issue.status_id)
                .map(|c| c.name.as_ref().to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            let (a, r) = self.resolve_names(&issue).await;
            dtos.push(IssueDto::from_issue(
                issue,
                project.name.as_ref().to_string(),
                column,
                a,
                r,
            ));
        }
        Ok(DashboardDto {
            assigned_issues: dtos,
        })
    }
}

pub struct SearchServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
}

impl SearchServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        users: Arc<dyn domain::UserRepository>,
    ) -> Self {
        Self {
            issues,
            projects,
            users,
        }
    }

    async fn resolve_names(&self, issue: &domain::Issue) -> (Option<String>, Option<String>) {
        let assignee_name = if let Some(id) = issue.assignee_id {
            self.users
                .get_by_id(id)
                .await
                .map(|u| u.display_name.as_ref().to_string())
                .ok()
        } else {
            None
        };
        let reporter_name = self
            .users
            .get_by_id(issue.reporter_id)
            .await
            .map(|u| u.display_name.as_ref().to_string())
            .ok();
        (assignee_name, reporter_name)
    }
}

#[async_trait]
impl crate::context::SearchService for SearchServiceImpl {
    async fn search(&self, q: &str) -> Result<Vec<IssueDto>, AppError> {
        let issues = self
            .issues
            .list(IssueQuery {
                search_text: Some(q.to_string()),
                ..Default::default()
            })
            .await?;
        let mut dtos = Vec::new();
        for issue in issues {
            let project = self.projects.get_by_id(issue.project_id).await?;
            let column = default_board_columns()
                .into_iter()
                .find(|c| c.id == issue.status_id)
                .map(|c| c.name.as_ref().to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            let (a, r) = self.resolve_names(&issue).await;
            dtos.push(IssueDto::from_issue(
                issue,
                project.name.as_ref().to_string(),
                column,
                a,
                r,
            ));
        }
        Ok(dtos)
    }
}

fn todo_status() -> StatusId {
    StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap())
}
fn in_progress_status() -> StatusId {
    StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap())
}
fn review_status() -> StatusId {
    StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000004").unwrap())
}
fn done_status() -> StatusId {
    StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap())
}

fn default_board_columns() -> Vec<BoardColumn> {
    vec![
        BoardColumn {
            id: todo_status(),
            name: "Todo".into(),
            category: ColumnCategory::Todo,
            wip_limit: None,
            position: 0,
        },
        BoardColumn {
            id: in_progress_status(),
            name: "In Progress".into(),
            category: ColumnCategory::InProgress,
            wip_limit: Some(5),
            position: 1,
        },
        BoardColumn {
            id: review_status(),
            name: "Review".into(),
            category: ColumnCategory::InProgress,
            wip_limit: None,
            position: 3,
        },
        BoardColumn {
            id: done_status(),
            name: "Done".into(),
            category: ColumnCategory::Done,
            wip_limit: None,
            position: 4,
        },
    ]
}
