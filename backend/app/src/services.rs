use async_trait::async_trait;
use std::sync::Arc;

use crate::dto::{BoardDto, BacklogDto, IssueDto, ProjectDto};
use domain::{
    Board, BoardColumn, ColumnCategory, Issue, IssueQuery, IssueRepository, ProjectRepository,
    SprintRepository, UserRepository,
};
use shared::{AppError, IssueId, ProjectKey, StatusId, UserId};
use crate::dto::{SprintDto, BoardColumnDto};
use crate::commands::CreateIssueCommand;
use crate::context::{
    ProjectService, IssueService, BoardService, SearchService,
};

pub struct ProjectServiceImpl {
    projects: Arc<dyn ProjectRepository>,
}

impl ProjectServiceImpl {
    pub fn new(projects: Arc<dyn ProjectRepository>) -> Self {
        Self { projects }
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
        Ok(projects
            .into_iter()
            .map(|p| ProjectDto::from_project(p, 0, 0, 0))
            .collect())
    }

    async fn get_by_key(
        &self,
        key: &ProjectKey,
    ) -> Result<ProjectDto, AppError> {
        let project = self.projects.get_by_key(key).await?;
        Ok(ProjectDto::from_project(project, 0, 0, 0))
    }
}

pub struct IssueServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn UserRepository>,
}

impl IssueServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        users: Arc<dyn UserRepository>,
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
    async fn create(
        &self,
        cmd: CreateIssueCommand,
    ) -> Result<IssueDto, AppError> {
        let project = self.projects.get_by_key(&cmd.project_key).await?;
        let number = self.projects.next_issue_number(project.id).await?;
        let status_id = StatusId::from_uuid(
            cmd.status_id.parse().map_err(|_| AppError::invalid_input("status_id"))?
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
        let column = default_board_columns()
            .into_iter()
            .find(|c| c.id == issue.status_id)
            .map(|c| c.name.as_ref().to_string())
            .unwrap_or_else(|| "Todo".to_string());
        Ok(IssueDto::from_issue(issue, project.name.as_ref().to_string(), column))
    }

    async fn get_by_id(
        &self,
        id: IssueId,
    ) -> Result<IssueDto, AppError> {
        let issue = self.issues.get_by_id(id).await?;
        let project = self.projects.get_by_id(issue.project_id).await?;
        let column = default_board_columns()
            .into_iter()
            .find(|c| c.id == issue.status_id)
            .map(|c| c.name.as_ref().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        Ok(IssueDto::from_issue(issue, project.name.as_ref().to_string(), column))
    }

    async fn search(
        &self,
        q: &str,
    ) -> Result<Vec<IssueDto>, AppError> {
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
            dtos.push(IssueDto::from_issue(issue, project.name.as_ref().to_string(), column));
        }
        Ok(dtos)
    }
}

pub struct BoardServiceImpl {
    boards: Arc<dyn domain::BoardRepository>,
    issues: Arc<dyn IssueRepository>,
    sprints: Arc<dyn SprintRepository>,
}

impl BoardServiceImpl {
    pub fn new(
        boards: Arc<dyn domain::BoardRepository>,
        issues: Arc<dyn IssueRepository>,
        sprints: Arc<dyn SprintRepository>,
    ) -> Self {
        Self {
            boards,
            issues,
            sprints,
        }
    }
}

#[async_trait]
impl crate::context::BoardService for BoardServiceImpl {
    async fn get_board(
        &self,
        project_key: &ProjectKey,
    ) -> Result<BoardDto, AppError> {
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
            dtos.push(IssueDto::from_issue(issue, project_key.to_string(), column));
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

    async fn get_backlog(
        &self,
        project_key: &ProjectKey,
    ) -> Result<BacklogDto, AppError> {
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

        let sprint_issues: Vec<_> = all_issues
            .clone()
            .into_iter()
            .filter(|i| i.sprint_id.is_some() || i.status_id != todo_status)
            .collect();
        let backlog_issues: Vec<_> = all_issues
            .into_iter()
            .filter(|i| i.sprint_id.is_none() && i.status_id == todo_status)
            .collect();

        let sprint_dto = sprint
            .map(|s| {
                SprintDto::from_sprint(
                    s,
                    sprint_issues.iter().map(|i| i.id.to_string()).collect(),
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

        let map_issues = |issues: Vec<Issue>| {
            issues
                .into_iter()
                .map(|i| {
                    let column = board
                        .columns
                        .iter()
                        .find(|c| c.id == i.status_id)
                        .map(|c| c.name.as_ref().to_string())
                        .unwrap_or_else(|| "Unknown".to_string());
                    IssueDto::from_issue(i, project_key.to_string(), column)
                })
                .collect()
        };

        Ok(BacklogDto {
            sprint: sprint_dto,
            sprint_issues: map_issues(sprint_issues),
            backlog_issues: map_issues(backlog_issues),
        })
    }
}

pub struct SearchServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
}

impl SearchServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
    ) -> Self {
        Self { issues, projects }
    }
}

#[async_trait]
impl crate::context::SearchService for SearchServiceImpl {
    async fn search(
        &self,
        q: &str,
    ) -> Result<Vec<IssueDto>, AppError> {
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
            dtos.push(IssueDto::from_issue(issue, project.name.as_ref().to_string(), column));
        }
        Ok(dtos)
    }
}

fn default_board_columns() -> Vec<BoardColumn> {
    vec![
        BoardColumn {
            id: StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
            name: "Todo".into(),
            category: ColumnCategory::Todo,
            wip_limit: None,
            position: 0,
        },
        BoardColumn {
            id: StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()),
            name: "In Progress".into(),
            category: ColumnCategory::InProgress,
            wip_limit: Some(5),
            position: 1,
        },
        BoardColumn {
            id: StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap()),
            name: "Done".into(),
            category: ColumnCategory::Done,
            wip_limit: None,
            position: 2,
        },
    ]
}
