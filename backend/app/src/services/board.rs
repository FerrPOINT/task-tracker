use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use crate::dto::{BacklogDto, BoardColumnDto, BoardDto, SprintDto};
use domain::{
    IssueQuery, IssueRepository, ProjectRepository, SprintRepository, StatusCategory,
    StatusRepository, WorkflowTransitionRepository,
};
use shared::{AppError, IssueId, ProjectKey, StatusId, UserId};

/// Upper bound on issues returned in the single-page backlog payload.
mod backlog {
    pub const BACKLOG_PAGE_LIMIT: usize = 100;
}

pub struct BoardServiceImpl {
    boards: Arc<dyn domain::BoardRepository>,
    issues: Arc<dyn IssueRepository>,
    sprints: Arc<dyn SprintRepository>,
    users: Arc<dyn domain::UserRepository>,
    statuses: Arc<dyn StatusRepository>,
    transitions: Arc<dyn WorkflowTransitionRepository>,
    projects: Arc<dyn ProjectRepository>,
    status_history: Arc<dyn domain::IssueStatusHistoryRepository>,
    authz: Authz,
}

impl BoardServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        boards: Arc<dyn domain::BoardRepository>,
        issues: Arc<dyn IssueRepository>,
        sprints: Arc<dyn SprintRepository>,
        users: Arc<dyn domain::UserRepository>,
        statuses: Arc<dyn StatusRepository>,
        transitions: Arc<dyn WorkflowTransitionRepository>,
        projects: Arc<dyn ProjectRepository>,
        status_history: Arc<dyn domain::IssueStatusHistoryRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            boards,
            issues,
            sprints,
            users,
            statuses,
            transitions,
            projects,
            status_history,
            authz,
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
    async fn get_board(
        &self,
        project_key: &ProjectKey,
        requester: UserId,
    ) -> Result<BoardDto, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        self.authz
            .require_project_access(project.id, requester)
            .await?;
        self.build_board_dto(project_key).await
    }

    async fn get_backlog(
        &self,
        project_key: &ProjectKey,
        requester: UserId,
    ) -> Result<BacklogDto, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        self.authz
            .require_project_access(project.id, requester)
            .await?;
        let board = self.boards.get_default_by_project_key(project_key).await?;
        let sprint = self.sprints.get_active_by_project(board.project_id).await?;
        let all_issues = self
            .issues
            .list(IssueQuery {
                project_id: Some(board.project_id),
                sort_by: Some("created".to_string()),
                sort_order: Some("desc".to_string()),
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
        // The backlog view renders a single list; shipping every historic
        // issue produced multi-megabyte responses and >32k-pixel pages on
        // long-lived projects. Keep the response bounded and report the
        // total so clients can paginate via search.
        let backlog_total = backlog_issues_raw.len();
        let backlog_issues_raw = backlog_issues_raw
            .into_iter()
            .take(backlog::BACKLOG_PAGE_LIMIT)
            .collect::<Vec<_>>();
        let backlog_issues = super::helpers::build_issue_dtos(
            Arc::clone(&self.users),
            backlog_issues_raw,
            project_label.as_str(),
        )
        .await?;

        Ok(BacklogDto {
            project_id: board.project_id.to_string(),
            project_key: project_key.to_string(),
            backlog_total,
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
        requester: UserId,
    ) -> Result<BoardDto, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        self.authz
            .require_project_edit(project.id, requester)
            .await?;
        let board = self.boards.get_default_by_project_key(project_key).await?;
        let issue = self.issues.get_by_id(issue_id).await?;
        if issue.project_id != board.project_id {
            return Err(AppError::invalid_input(
                "issue does not belong to this project",
            ));
        }
        let allowed = self
            .transitions
            .is_allowed(issue.status_id, status_id)
            .await?;
        if !allowed {
            return Err(AppError::invalid_input("workflow transition not allowed"));
        }
        if let Some(column) = board.columns.iter().find(|c| c.id == status_id) {
            if let Some(limit) = column.wip_limit {
                let target_count = self
                    .issues
                    .list(IssueQuery {
                        project_id: Some(board.project_id),
                        status_id: Some(status_id),
                        ..Default::default()
                    })
                    .await?
                    .len();
                if target_count >= limit as usize {
                    return Err(AppError::conflict(format!(
                        "WIP limit ({limit}) reached for {}",
                        column.name
                    )));
                }
            }
        }
        let mut updated = issue.clone();
        let from_status = updated.status_id;
        updated.change_status(status_id);
        self.issues.save(&updated).await?;
        // Reports (control chart / cumulative flow) are derived from status
        // history; persisting it here is what makes them truthful.
        self.status_history
            .save_for_project(
                &domain::IssueStatusHistory {
                    id: shared::IssueStatusHistoryId::new(),
                    issue_id: updated.id,
                    from_status_id: Some(from_status),
                    to_status_id: status_id,
                    changed_by_id: requester,
                    changed_at: shared::now(),
                },
                project.id,
            )
            .await?;
        self.build_board_dto(project_key).await
    }
}
