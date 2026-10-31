//! Ordinary human task commands; execution has separate, explicit admission.
use crate::authz::Authz;
use domain::task_lifecycle::{Readback, StageProjection, TransitionCommand};
use shared::{AppError, IssueId, ProjectId, UserId};
use std::sync::Arc;

pub struct TaskLifecycleService {
    issues: Arc<dyn domain::IssueRepository>,
    authz: Authz,
}
impl TaskLifecycleService {
    pub fn new(issues: Arc<dyn domain::IssueRepository>, authz: Authz) -> Self {
        Self { issues, authz }
    }
    pub async fn active(&self) -> Result<bool, AppError> {
        self.issues.task_lifecycle_active().await
    }
    async fn may_edit(&self, project: ProjectId, actor: UserId) -> Result<bool, AppError> {
        match self.authz.require_project_edit(project, actor).await {
            Ok(()) => Ok(true),
            Err(AppError::Forbidden) => Ok(false),
            Err(error) => Err(error),
        }
    }
    pub async fn read(&self, task: IssueId, actor: UserId) -> Result<Readback, AppError> {
        let issue = self.issues.get_by_id(task).await?;
        self.authz
            .require_project_access(issue.project_id, actor)
            .await?;
        self.issues
            .task_lifecycle(task, self.may_edit(issue.project_id, actor).await?)
            .await
    }
    pub async fn change(
        &self,
        task: IssueId,
        actor: UserId,
        command: &TransitionCommand,
    ) -> Result<Readback, AppError> {
        if !self.active().await? {
            return Err(AppError::conflict("task_lifecycle_not_active"));
        }
        let issue = self.issues.get_by_id(task).await?;
        self.authz
            .require_project_edit(issue.project_id, actor)
            .await?;
        self.issues
            .transition_task_lifecycle(task, actor, command)
            .await
    }
    pub async fn project(
        &self,
        project: ProjectId,
        actor: UserId,
    ) -> Result<(Vec<Readback>, Vec<StageProjection>), AppError> {
        self.authz.require_project_access(project, actor).await?;
        let rows = self
            .issues
            .project_task_lifecycles(project, self.may_edit(project, actor).await?)
            .await?;
        let columns = self.issues.task_lifecycle_columns(project).await?;
        Ok((rows, columns))
    }
}

pub struct StageBoardDto {
    pub cards: Vec<(crate::dto::IssueDto, Readback)>,
    pub columns: Vec<StageProjection>,
    pub sprint: crate::dto::SprintDto,
    pub backlog_total: usize,
}
impl TaskLifecycleService {
    pub async fn board(
        &self,
        project: ProjectId,
        project_name: &str,
        actor: UserId,
    ) -> Result<StageBoardDto, AppError> {
        self.authz.require_project_access(project, actor).await?;
        let snapshot = self
            .issues
            .task_stage_board_snapshot(project, self.may_edit(project, actor).await?)
            .await?;
        let backlog_total = snapshot
            .cards
            .iter()
            .filter(|card| model_category(card.lifecycle.state) == domain::StatusCategory::Todo)
            .count();
        let sprint = snapshot
            .sprint
            .map(|s| {
                let ids = snapshot
                    .cards
                    .iter()
                    .filter(|card| card.issue.sprint_id == Some(s.id))
                    .map(|card| card.issue.id.to_string())
                    .collect();
                crate::dto::SprintDto::from_sprint(s, ids)
            })
            .unwrap_or_else(|| crate::dto::SprintDto {
                id: "none".into(),
                project_id: project.to_string(),
                name: "Backlog".into(),
                goal: String::new(),
                state: "future".into(),
                velocity: 0,
                remaining_days: None,
                issue_ids: Vec::new(),
                start_date: None,
                end_date: None,
            });
        let cards = snapshot
            .cards
            .into_iter()
            .map(|card| {
                let mut dto = crate::dto::IssueDto::from_issue(
                    card.issue,
                    project_name.to_owned(),
                    format!("{:?}", card.lifecycle.state.status),
                    card.assignee_name,
                    card.reporter_name,
                );
                dto.lifecycle = Some(card.lifecycle.state);
                dto.labels = card.labels;
                (dto, card.lifecycle)
            })
            .collect();
        Ok(StageBoardDto {
            cards,
            columns: snapshot.columns,
            sprint,
            backlog_total,
        })
    }
}
fn model_category(state: domain::task_lifecycle::State) -> domain::StatusCategory {
    domain::task_lifecycle::category(state.stage, state.status)
}
