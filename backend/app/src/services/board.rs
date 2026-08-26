use async_trait::async_trait;
use std::sync::Arc;

use crate::dto::{BacklogDto, BoardColumnDto, BoardDto, SprintDto};
use domain::{
    IssueQuery, IssueRepository, SprintRepository, StatusCategory, StatusRepository,
    WorkflowTransitionRepository,
};
use shared::{AppError, IssueId, ProjectKey, StatusId};

pub struct BoardServiceImpl {
    boards: Arc<dyn domain::BoardRepository>,
    issues: Arc<dyn IssueRepository>,
    sprints: Arc<dyn SprintRepository>,
    users: Arc<dyn domain::UserRepository>,
    statuses: Arc<dyn StatusRepository>,
    transitions: Arc<dyn WorkflowTransitionRepository>,
}

impl BoardServiceImpl {
    pub fn new(
        boards: Arc<dyn domain::BoardRepository>,
        issues: Arc<dyn IssueRepository>,
        sprints: Arc<dyn SprintRepository>,
        users: Arc<dyn domain::UserRepository>,
        statuses: Arc<dyn StatusRepository>,
        transitions: Arc<dyn WorkflowTransitionRepository>,
    ) -> Self {
        Self {
            boards,
            issues,
            sprints,
            users,
            statuses,
            transitions,
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

        let db_statuses = self.statuses.list_all().await.unwrap_or_default();
        let columns: Vec<BoardColumnDto> = if board.columns.iter().all(|c| c.id.as_uuid().is_nil())
        {
            db_statuses
                .iter()
                .map(|s| BoardColumnDto {
                    id: s.id.to_string(),
                    name: s.name.as_ref().to_string(),
                    wip_limit: None,
                    issue_ids: issues
                        .iter()
                        .filter(|i| i.status_id == s.id)
                        .map(|i| i.id.to_string())
                        .collect(),
                })
                .collect()
        } else {
            board
                .columns
                .iter()
                .map(|c| {
                    // Statuses are the single source of truth for names.
                    let name = db_statuses
                        .iter()
                        .find(|s| s.id == c.id)
                        .map(|s| s.name.as_ref().to_string())
                        .unwrap_or_else(|| c.name.as_ref().to_string());
                    BoardColumnDto {
                        id: c.id.to_string(),
                        name,
                        wip_limit: c.wip_limit,
                        issue_ids: issues
                            .iter()
                            .filter(|i| i.status_id == c.id)
                            .map(|i| i.id.to_string())
                            .collect(),
                    }
                })
                .collect()
        };

        let issue_dtos = super::helpers::build_issue_dtos(
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
                start_date: None,
                end_date: None,
            });

        Ok(BoardDto {
            project_id: board.project_id.to_string(),
            project_key: project_key.to_string(),
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

        let db_statuses = self.statuses.list_all().await.unwrap_or_default();
        let todo_status = db_statuses
            .iter()
            .find(|s| s.category == StatusCategory::Todo)
            .map(|s| s.id)
            .unwrap_or_else(|| {
                board
                    .columns
                    .iter()
                    .find(|c| c.category == StatusCategory::Todo)
                    .map(|c| c.id)
                    .unwrap_or(StatusId::from_uuid(uuid::Uuid::nil()))
            });

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
                start_date: None,
                end_date: None,
            });

        let project_label = project_key.to_string();
        let sprint_issues = super::helpers::build_issue_dtos(
            Arc::clone(&self.users),
            sprint_issues_raw,
            project_label.as_str(),
        )
        .await?;
        let backlog_issues = super::helpers::build_issue_dtos(
            Arc::clone(&self.users),
            backlog_issues_raw,
            project_label.as_str(),
        )
        .await?;

        Ok(BacklogDto {
            project_id: board.project_id.to_string(),
            project_key: project_key.to_string(),
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
        let issue = self.issues.get_by_id(issue_id).await?;
        let allowed = self
            .transitions
            .is_allowed(issue.status_id, status_id)
            .await?;
        if !allowed {
            return Err(AppError::invalid_input("workflow transition not allowed"));
        }
        let mut updated = issue.clone();
        updated.change_status(status_id);
        self.issues.save(&updated).await?;
        self.build_board_dto(project_key).await
    }
}
