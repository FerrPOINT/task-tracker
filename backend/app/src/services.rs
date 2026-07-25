use async_trait::async_trait;
use std::sync::Arc;

use crate::commands::{CreateIssueCommand, UpdateIssueCommand};
use crate::dto::{
    BacklogDto, BoardColumnDto, BoardDto, DashboardDto, IssueDto, ProjectDto, SprintDto,
};
use domain::{
    Board, ColumnCategory, Issue, IssueQuery, IssueRepository, ProjectRepository, SprintRepository,
};
use shared::{AppError, BoardId, IssueId, ProjectId, ProjectKey, StatusId, UserId};

mod helpers;

#[cfg(test)]
mod tests;

pub struct ProjectServiceImpl {
    projects: Arc<dyn ProjectRepository>,
    issues: Arc<dyn IssueRepository>,
    users: Arc<dyn domain::UserRepository>,
    boards: Arc<dyn domain::BoardRepository>,
}

impl ProjectServiceImpl {
    pub fn new(
        projects: Arc<dyn ProjectRepository>,
        issues: Arc<dyn IssueRepository>,
        users: Arc<dyn domain::UserRepository>,
        boards: Arc<dyn domain::BoardRepository>,
    ) -> Self {
        Self {
            projects,
            issues,
            users,
            boards,
        }
    }
}

#[async_trait]
impl crate::context::ProjectService for ProjectServiceImpl {
    async fn create(
        &self,
        cmd: crate::commands::CreateProjectCommand,
    ) -> Result<ProjectDto, AppError> {
        let owner = self.users.get_by_id(cmd.owner_id).await?;
        let board_id = BoardId::new();
        let project = domain::Project {
            id: ProjectId::new(),
            key: cmd.key,
            name: cmd.name.into(),
            description: cmd.description.map(Into::into),
            owner_id: owner.id,
            default_board_id: board_id,
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        let board = Board {
            id: board_id,
            project_id: project.id,
            name: "Board".into(),
            columns: helpers::default_board_columns(),
        };
        self.projects.save(&project).await?;
        self.boards.save(&board).await?;
        Ok(ProjectDto::from_project(project, 0, 0, 0))
    }

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
            let (todo, in_progress, done) = helpers::count_by_status(&counts);
            dtos.push(ProjectDto::from_project(project, todo, in_progress, done));
        }
        Ok(dtos)
    }

    async fn get_by_key(&self, key: &ProjectKey) -> Result<ProjectDto, AppError> {
        let project = self.projects.get_by_key(key).await?;
        let counts = self.issues.list(IssueQuery::project(project.id)).await?;
        let (todo, in_progress, done) = helpers::count_by_status(&counts);
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
        let mut issue = Issue::create(
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
        let column = helpers::issue_status_column(issue.status_id);
        let (assignee_name, reporter_name) =
            helpers::resolve_names(self.users.clone(), &issue).await;
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
        let name = helpers::project_name(self.projects.clone(), issue.project_id).await?;
        Ok(helpers::build_issue_dto(self.users.clone(), issue, name.as_str()).await)
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
        let column = helpers::issue_status_column(issue.status_id);
        let (assignee_name, reporter_name) =
            helpers::resolve_names(self.users.clone(), &issue).await;
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
        helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            issues,
        )
        .await
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

        let issue_dtos = helpers::build_issue_dtos(
            Arc::clone(&self.users),
            issues,
            project_key.to_string().as_str(),
        )
        .await?;

        let sprint_dto = sprint
            .map(|s| SprintDto::from_sprint(s, issue_dtos.iter().map(|i| i.id.clone()).collect()))
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
            issues: issue_dtos,
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

        let project_label = project_key.to_string();
        let sprint_issues = helpers::build_issue_dtos(
            Arc::clone(&self.users),
            sprint_issues_raw,
            project_label.as_str(),
        )
        .await?;
        let backlog_issues = helpers::build_issue_dtos(
            Arc::clone(&self.users),
            backlog_issues_raw,
            project_label.as_str(),
        )
        .await?;

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
}

#[async_trait]
impl crate::context::DashboardService for DashboardServiceImpl {
    async fn get_dashboard(&self, user_id: UserId) -> Result<DashboardDto, AppError> {
        let issues = self.issues.list(IssueQuery::assignee(user_id)).await?;
        let dtos = helpers::build_issue_dtos_for_dashboard(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            issues,
        )
        .await?;
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
        helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            issues,
        )
        .await
    }
}
