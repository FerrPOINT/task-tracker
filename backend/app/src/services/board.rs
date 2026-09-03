use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use crate::dto::{BacklogDto, BoardColumnDto, BoardDto, SprintDto};
use domain::{
    Board, IssueQuery, IssueRepository, LabelRepository, ProjectRepository, SprintRepository,
    StatusCategory, StatusRepository, TransitionGuard, WorkflowTransitionRepository,
};
use shared::{AppError, IssueId, ProjectKey, StatusId, UserId};

/// Bounded offset pagination for a deterministic backlog order.
pub mod backlog {
    pub const BACKLOG_PAGE_LIMIT: usize = 100;
    pub const BACKLOG_MAX_PAGE_SIZE: usize = 200;
}

pub struct BoardServiceImpl {
    boards: Arc<dyn domain::BoardRepository>,
    issues: Arc<dyn IssueRepository>,
    sprints: Arc<dyn SprintRepository>,
    users: Arc<dyn domain::UserRepository>,
    labels: Arc<dyn LabelRepository>,
    statuses: Arc<dyn StatusRepository>,
    transitions: Arc<dyn WorkflowTransitionRepository>,
    projects: Arc<dyn ProjectRepository>,
    events: crate::context::EventBus,
    authz: Authz,
}

impl BoardServiceImpl {
    /// Snapshot of the target column's WIP capacity for the atomic guard.
    /// Counting happens again inside the critical section; this snapshot
    /// only builds the guard (limits/names) that survives into it.
    async fn build_transition_guard(
        &self,
        board: &Board,
        status_id: StatusId,
    ) -> Result<TransitionGuard, AppError> {
        let column = board
            .columns
            .iter()
            .find(|c| c.id == status_id)
            .ok_or_else(|| AppError::invalid_input("unknown target column"))?;
        let target_count = self
            .issues
            .count_by_project_status(board.project_id, status_id)
            .await?;
        Ok(TransitionGuard {
            wip_limit: column.wip_limit.map(|v| v as u32),
            target_count,
            column_name: column.name.as_ref().to_string(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        boards: Arc<dyn domain::BoardRepository>,
        issues: Arc<dyn IssueRepository>,
        sprints: Arc<dyn SprintRepository>,
        users: Arc<dyn domain::UserRepository>,
        labels: Arc<dyn LabelRepository>,
        statuses: Arc<dyn StatusRepository>,
        transitions: Arc<dyn WorkflowTransitionRepository>,
        projects: Arc<dyn ProjectRepository>,
        events: crate::context::EventBus,
        authz: Authz,
    ) -> Self {
        Self {
            boards,
            issues,
            sprints,
            users,
            labels,
            statuses,
            transitions,
            projects,
            events,
            authz,
        }
    }

    async fn build_board_dto(&self, project_key: &ProjectKey) -> Result<BoardDto, AppError> {
        let board = self.boards.get_default_by_project_key(project_key).await?;
        let sprint = self.sprints.get_active_by_project(board.project_id).await?;
        let issues = self
            .issues
            .list_unbounded(IssueQuery {
                project_id: Some(board.project_id),
                ..Default::default()
            })
            .await?;

        let db_statuses = self.statuses.list_all().await?;
        // Backlog = issues in the first Todo-category status that are not in a sprint.
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
        let backlog_total = self
            .issues
            .count_backlog(board.project_id, todo_status)
            .await? as usize;
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

        let sprint_dto = sprint
            .map(|s| {
                let issue_ids = issues
                    .iter()
                    .filter(|issue| issue.sprint_id == Some(s.id))
                    .map(|issue| issue.id.to_string())
                    .collect();
                SprintDto::from_sprint(s, issue_ids)
            })
            .unwrap_or_else(|| SprintDto {
                id: "none".to_string(),
                project_id: board.project_id.to_string(),
                name: "Backlog".to_string(),
                goal: String::new(),
                state: "future".to_string(),
                velocity: 0,
                remaining_days: None,
                issue_ids: vec![],
                start_date: None,
                end_date: None,
            });

        let issue_dtos = super::helpers::build_issue_dtos(
            Arc::clone(&self.users),
            Arc::clone(&self.labels),
            issues,
            project_key.to_string().as_str(),
        )
        .await?;

        Ok(BoardDto {
            project_id: board.project_id.to_string(),
            project_key: project_key.to_string(),
            columns,
            issues: issue_dtos,
            sprint: sprint_dto,
            backlog_total,
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
        offset: u32,
        limit: u32,
    ) -> Result<BacklogDto, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        self.authz
            .require_project_access(project.id, requester)
            .await?;
        let board = self.boards.get_default_by_project_key(project_key).await?;
        let sprint = self.sprints.get_active_by_project(board.project_id).await?;
        let all_non_backlog_candidates = self
            .issues
            .list_unbounded(IssueQuery {
                project_id: Some(board.project_id),
                sort_by: Some("created".to_string()),
                sort_order: Some("desc".to_string()),
                ..Default::default()
            })
            .await?;

        let db_statuses = self.statuses.list_all().await?;
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

        let sprint_issues_raw: Vec<_> = all_non_backlog_candidates
            .into_iter()
            .filter(|i| i.sprint_id.is_some() || i.status_id != todo_status)
            .collect();

        let sprint_dto = sprint
            .map(|s| {
                let issue_ids = sprint_issues_raw
                    .iter()
                    .filter(|issue| issue.sprint_id == Some(s.id))
                    .map(|issue| issue.id.to_string())
                    .collect();
                SprintDto::from_sprint(s, issue_ids)
            })
            .unwrap_or_else(|| SprintDto {
                id: "none".to_string(),
                project_id: board.project_id.to_string(),
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
            Arc::clone(&self.labels),
            sprint_issues_raw,
            project_label.as_str(),
        )
        .await?;
        let offset = offset as usize;
        let page_limit = (limit as usize).clamp(1, backlog::BACKLOG_MAX_PAGE_SIZE);
        let backlog_total = self
            .issues
            .count_backlog(board.project_id, todo_status)
            .await? as usize;
        let backlog_issues_raw = self
            .issues
            .list_backlog_page(
                board.project_id,
                todo_status,
                page_limit as u64,
                offset as u64,
            )
            .await?;
        let backlog_issues = super::helpers::build_issue_dtos(
            Arc::clone(&self.users),
            Arc::clone(&self.labels),
            backlog_issues_raw,
            project_label.as_str(),
        )
        .await?;

        Ok(BacklogDto {
            project_id: board.project_id.to_string(),
            project_key: project_key.to_string(),
            backlog_total,
            backlog_offset: offset,
            backlog_limit: page_limit,
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
        if status_id == issue.status_id {
            return self.build_board_dto(project_key).await;
        }
        let allowed = self
            .transitions
            .is_allowed(issue.status_id, status_id)
            .await?;
        if !allowed {
            return Err(AppError::invalid_input("workflow transition not allowed"));
        }
        let guard = self.build_transition_guard(&board, status_id).await?;
        // Atomic: WIP re-check, issue update and history insert happen in one
        // critical section so concurrent movers cannot both observe capacity.
        self.issues
            .change_status_atomic(
                issue.id,
                project.id,
                issue.status_id,
                status_id,
                requester,
                &guard,
            )
            .await?;
        self.events.publish(shared::TrackerEvent::IssueMoved {
            issue_id: issue.id.to_string(),
            project_key: project.key.to_string(),
        });
        self.build_board_dto(project_key).await
    }
}
