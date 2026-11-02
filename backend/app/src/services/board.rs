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
    /// Backlog belongs to the board's first Todo column, not an unrelated
    /// workflow whose status happens to come first in the global catalogue.
    fn backlog_status(board: &Board, statuses: &[domain::Status]) -> StatusId {
        board
            .columns
            .iter()
            .find(|column| column.category == StatusCategory::Todo && !column.id.as_uuid().is_nil())
            .map(|column| column.id)
            .or_else(|| {
                statuses
                    .iter()
                    .filter(|status| status.category == StatusCategory::Todo)
                    .min_by_key(|status| (!status.is_default, status.position, status.id.as_uuid()))
                    .map(|status| status.id)
            })
            .unwrap_or(StatusId::from_uuid(uuid::Uuid::nil()))
    }

    fn uses_stage_columns(board: &Board) -> bool {
        board.columns.is_empty()
            || board
                .columns
                .iter()
                .all(|column| column.id.as_uuid().is_nil())
            || {
                let legacy = super::helpers::legacy_board_columns();
                board.columns.len() == legacy.len()
                    && board.columns.iter().zip(&legacy).all(|(a, b)| {
                        a.id == b.id
                            && a.name == b.name
                            && a.category == b.category
                            && a.wip_limit == b.wip_limit
                            && a.position == b.position
                    })
            }
    }

    fn stage_columns(
        issues: &[domain::Issue],
        states: &[domain::BoardTaskState],
        statuses: &[domain::Status],
    ) -> Vec<BoardColumnDto> {
        use domain::sdlc::Stage;
        let stages = [
            Stage::Draft,
            Stage::Backlog,
            Stage::Analysis,
            Stage::Architecture,
            Stage::Development,
            Stage::Review,
            Stage::Testing,
            Stage::Rework,
            Stage::Deployment,
        ];
        let states = states
            .iter()
            .map(|state| (state.task_id, state))
            .collect::<std::collections::HashMap<_, _>>();
        let mut columns = stages
            .into_iter()
            .map(|stage| BoardColumnDto {
                id: format!("stage:{stage:?}"),
                name: format!("{stage:?}"),
                stage: Some(stage),
                wip_limit: None,
                issue_ids: issues
                    .iter()
                    .filter(|issue| {
                        states.get(&issue.id.as_uuid()).is_some_and(|state| {
                            state.stage == stage
                                || (stage == Stage::Draft && state.stage == Stage::Clarification)
                        })
                    })
                    .map(|issue| issue.id.to_string())
                    .collect(),
            })
            .collect::<Vec<_>>();
        // Ordinary tasks retain their real status; never invent an SDLC stage.
        for status in statuses {
            let issue_ids = issues
                .iter()
                .filter(|issue| {
                    issue.status_id == status.id && !states.contains_key(&issue.id.as_uuid())
                })
                .map(|issue| issue.id.to_string())
                .collect::<Vec<_>>();
            if !issue_ids.is_empty() {
                columns.push(BoardColumnDto {
                    id: status.id.to_string(),
                    name: status.name.to_string(),
                    stage: None,
                    wip_limit: None,
                    issue_ids,
                });
            }
        }
        columns
    }

    fn resolve_status_names(issues: &mut [crate::dto::IssueDto], statuses: &[domain::Status]) {
        let names = statuses
            .iter()
            .map(|status| (status.id.to_string(), status.name.to_string()))
            .collect::<std::collections::HashMap<_, _>>();
        for issue in issues {
            if let Some(name) = names.get(&issue.status_id) {
                issue.status = name.clone();
            }
        }
    }

    /// Snapshot of the target column's WIP capacity for the atomic guard.
    /// Counting happens again inside the critical section; this snapshot
    /// only builds the guard (limits/names) that survives into it.
    async fn build_transition_guard(
        &self,
        board: &Board,
        status_id: StatusId,
    ) -> Result<TransitionGuard, AppError> {
        if Self::uses_stage_columns(board) {
            let status = self.statuses.get_by_id(status_id).await?;
            return Ok(TransitionGuard {
                wip_limit: board
                    .columns
                    .iter()
                    .find(|column| column.id == status_id)
                    .and_then(|column| column.wip_limit)
                    .map(|limit| limit as u32),
                target_count: self
                    .issues
                    .count_by_project_status(board.project_id, status_id)
                    .await?,
                column_name: status.name.to_string(),
            });
        }
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
        let task_states = self.issues.list_board_task_states(board.project_id).await?;
        let todo_status = Self::backlog_status(&board, &db_statuses);
        let backlog_total = self
            .issues
            .count_backlog(board.project_id, todo_status)
            .await? as usize;
        let columns: Vec<BoardColumnDto> = if Self::uses_stage_columns(&board) {
            Self::stage_columns(&issues, &task_states, &db_statuses)
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
                        stage: None,
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

        let mut issue_dtos = super::helpers::build_issue_dtos(
            Arc::clone(&self.users),
            Arc::clone(&self.labels),
            issues,
            project_key.to_string().as_str(),
        )
        .await?;

        Self::resolve_status_names(&mut issue_dtos, &db_statuses);
        Ok(BoardDto {
            task_states,
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
        let todo_status = Self::backlog_status(&board, &db_statuses);

        let sprint_issues_raw: Vec<_> = all_non_backlog_candidates
            .into_iter()
            .filter(|i| {
                sprint
                    .as_ref()
                    .is_some_and(|active| i.sprint_id == Some(active.id))
            })
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
        let mut sprint_issues = super::helpers::build_issue_dtos(
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
        let mut backlog_issues = super::helpers::build_issue_dtos(
            Arc::clone(&self.users),
            Arc::clone(&self.labels),
            backlog_issues_raw,
            project_label.as_str(),
        )
        .await?;

        Self::resolve_status_names(&mut sprint_issues, &db_statuses);
        Self::resolve_status_names(&mut backlog_issues, &db_statuses);
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

#[cfg(test)]
mod tests {
    use super::*;
    use domain::sdlc::Stage;
    use domain::sdlc_lifecycle::LifecycleStatus;

    fn issue() -> domain::Issue {
        let owner = shared::UserId::new();
        let project = domain::Project {
            id: shared::ProjectId::new(),
            key: ProjectKey::new("BOARD"),
            name: "Board".into(),
            description: None,
            owner_id: owner,
            default_board_id: shared::BoardId::new(),
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        domain::Issue::create(
            &project,
            1,
            shared::IssueType::Task,
            StatusId::new(),
            "Stage projection",
            None,
            owner,
            shared::Priority::Medium,
        )
    }

    #[test]
    fn documented_stages_are_default_even_on_an_empty_board() {
        let columns = BoardServiceImpl::stage_columns(&[], &[], &[]);
        assert_eq!(
            columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            [
                "Draft",
                "Backlog",
                "Analysis",
                "Architecture",
                "Development",
                "Review",
                "Testing",
                "Rework",
                "Deployment"
            ]
        );
        assert!(BoardServiceImpl::uses_stage_columns(&Board::default()));
        assert!(super::super::helpers::default_board_columns().is_empty());
    }

    #[test]
    fn same_execution_status_does_not_merge_different_stages() {
        let a = issue();
        let mut b = issue();
        b.status_id = a.status_id;
        let states = [
            domain::BoardTaskState {
                task_id: a.id.as_uuid(),
                stage: Stage::Backlog,
                status: Some(LifecycleStatus::Ready),
            },
            domain::BoardTaskState {
                task_id: b.id.as_uuid(),
                stage: Stage::Analysis,
                status: Some(LifecycleStatus::Ready),
            },
        ];
        let columns = BoardServiceImpl::stage_columns(&[a.clone(), b.clone()], &states, &[]);
        assert_eq!(columns[1].issue_ids, [a.id.to_string()]);
        assert_eq!(columns[2].issue_ids, [b.id.to_string()]);
        assert_eq!(
            columns
                .iter()
                .map(|column| column.issue_ids.len())
                .sum::<usize>(),
            2
        );
    }

    #[test]
    fn clarification_is_draft_and_ordinary_issues_keep_their_real_status() {
        let a = issue();
        let b = issue();
        let status = domain::Status {
            id: b.status_id,
            name: "Custom status".into(),
            category: StatusCategory::Todo,
            position: 0,
            is_default: false,
            is_closed: false,
        };
        let columns = BoardServiceImpl::stage_columns(
            &[a.clone(), b.clone()],
            &[domain::BoardTaskState {
                task_id: a.id.as_uuid(),
                stage: Stage::Clarification,
                status: None,
            }],
            &[status],
        );
        assert_eq!(columns[0].issue_ids, [a.id.to_string()]);
        assert_eq!(columns.len(), 10);
        assert_eq!(columns[9].issue_ids, [b.id.to_string()]);
        assert!(columns[9].stage.is_none());
    }

    #[test]
    fn only_unchanged_legacy_template_is_adapted() {
        let mut board = Board {
            columns: super::super::helpers::legacy_board_columns(),
            ..Board::default()
        };
        assert!(BoardServiceImpl::uses_stage_columns(&board));
        board.columns[0].name = "Custom".into();
        assert!(!BoardServiceImpl::uses_stage_columns(&board));
    }
}
