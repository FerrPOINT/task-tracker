use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use crate::dto::{IssueDto, SprintDto};
use domain::{IssueQuery, IssueRepository, ProjectRepository, SprintRepository};
use shared::{AppError, ProjectId, SprintId, UserId};

#[async_trait]
pub trait SprintService: Send + Sync {
    async fn create(
        &self,
        cmd: crate::commands::CreateSprintCommand,
        requester: UserId,
    ) -> Result<SprintDto, AppError>;
    async fn list(
        &self,
        project_id: ProjectId,
        requester: UserId,
    ) -> Result<Vec<SprintDto>, AppError>;
    async fn get_by_id(&self, id: SprintId, requester: UserId) -> Result<SprintDto, AppError>;
    async fn update(
        &self,
        id: SprintId,
        cmd: crate::commands::UpdateSprintCommand,
        requester: UserId,
    ) -> Result<SprintDto, AppError>;
    async fn start(&self, id: SprintId, requester: UserId) -> Result<SprintDto, AppError>;
    async fn close(&self, id: SprintId, requester: UserId) -> Result<SprintDto, AppError>;
    async fn move_issue(
        &self,
        cmd: crate::commands::MoveIssueToSprintCommand,
        requester: UserId,
    ) -> Result<IssueDto, AppError>;
}

pub struct SprintServiceImpl {
    sprints: Arc<dyn SprintRepository>,
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
    authz: Authz,
}

impl SprintServiceImpl {
    pub fn new(
        sprints: Arc<dyn SprintRepository>,
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        users: Arc<dyn domain::UserRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            sprints,
            issues,
            projects,
            users,
            authz,
        }
    }

    async fn sprint_dto(&self, sprint: domain::Sprint) -> Result<SprintDto, AppError> {
        let issues = self
            .issues
            .list(IssueQuery {
                sprint_id: Some(sprint.id),
                ..Default::default()
            })
            .await?;
        Ok(SprintDto::from_sprint(
            sprint,
            issues.into_iter().map(|i| i.id.to_string()).collect(),
        ))
    }
}

#[async_trait]
impl SprintService for SprintServiceImpl {
    async fn create(
        &self,
        cmd: crate::commands::CreateSprintCommand,
        requester: UserId,
    ) -> Result<SprintDto, AppError> {
        self.authz
            .require_project_edit(cmd.project_id, requester)
            .await?;
        // A sprint ending before it starts is nonsense and breaks burndown
        // math (remaining_days goes negative/null).
        if let (Some(start), Some(end)) = (cmd.start_date, cmd.end_date) {
            if end < start {
                return Err(AppError::invalid_input(
                    "end_date must not be earlier than start_date",
                ));
            }
        }
        let sprint = domain::Sprint {
            id: SprintId::new(),
            project_id: cmd.project_id,
            name: cmd.name.into(),
            goal: cmd.goal.map(Into::into),
            state: domain::SprintState::Future,
            start_date: cmd.start_date,
            end_date: cmd.end_date,
            velocity: None,
        };
        self.sprints.save(&sprint).await?;
        self.sprint_dto(sprint).await
    }

    async fn list(
        &self,
        project_id: ProjectId,
        requester: UserId,
    ) -> Result<Vec<SprintDto>, AppError> {
        self.authz
            .require_project_access(project_id, requester)
            .await?;
        let sprints = self.sprints.list_by_project(project_id).await?;
        let mut result = Vec::with_capacity(sprints.len());
        for s in sprints {
            result.push(self.sprint_dto(s).await?);
        }
        Ok(result)
    }

    async fn get_by_id(&self, id: SprintId, requester: UserId) -> Result<SprintDto, AppError> {
        let sprint = self.sprints.get_by_id(id).await?;
        self.authz
            .require_project_access(sprint.project_id, requester)
            .await?;
        self.sprint_dto(sprint).await
    }

    async fn update(
        &self,
        id: SprintId,
        cmd: crate::commands::UpdateSprintCommand,
        requester: UserId,
    ) -> Result<SprintDto, AppError> {
        let sprint = self.sprints.get_by_id(id).await?;
        self.authz
            .require_project_edit(sprint.project_id, requester)
            .await?;
        let mut sprint = sprint;
        if let Some(name) = cmd.name {
            sprint.name = name.into();
        }
        if let Some(goal) = cmd.goal {
            sprint.goal = goal.map(Into::into);
        }
        if let Some(start_date) = cmd.start_date {
            sprint.start_date = start_date;
        }
        if let Some(end_date) = cmd.end_date {
            sprint.end_date = end_date;
        }
        // Same date sanity as create: reject inverted ranges after merging
        // partial updates.
        if let (Some(start), Some(end)) = (sprint.start_date, sprint.end_date) {
            if end < start {
                return Err(AppError::invalid_input(
                    "end_date must not be earlier than start_date",
                ));
            }
        }
        self.sprints.save(&sprint).await?;
        self.sprint_dto(sprint).await
    }

    async fn start(&self, id: SprintId, requester: UserId) -> Result<SprintDto, AppError> {
        let sprint = self.sprints.get_by_id(id).await?;
        self.authz
            .require_project_edit(sprint.project_id, requester)
            .await?;
        let mut sprint = sprint;
        if sprint.state != domain::SprintState::Future {
            return Err(AppError::invalid_input("sprint is not in future state"));
        }
        // One active sprint per project: board pickers and burndown assume a
        // unique active sprint (get_active_by_project would go ambiguous).
        if let Ok(Some(current)) = self.sprints.get_active_by_project(sprint.project_id).await {
            if current.id != sprint.id {
                return Err(AppError::conflict("project already has an active sprint"));
            }
        }
        sprint.state = domain::SprintState::Active;
        sprint.start_date = Some(sprint.start_date.unwrap_or_else(shared::now));
        self.sprints.save(&sprint).await?;
        self.sprint_dto(sprint).await
    }

    async fn close(&self, id: SprintId, requester: UserId) -> Result<SprintDto, AppError> {
        let sprint = self.sprints.get_by_id(id).await?;
        self.authz
            .require_project_edit(sprint.project_id, requester)
            .await?;
        let mut sprint = sprint;
        if sprint.state != domain::SprintState::Active {
            return Err(AppError::invalid_input("sprint is not active"));
        }
        sprint.state = domain::SprintState::Closed;
        sprint.end_date = Some(sprint.end_date.unwrap_or_else(shared::now));
        self.sprints.save(&sprint).await?;
        self.sprint_dto(sprint).await
    }

    async fn move_issue(
        &self,
        cmd: crate::commands::MoveIssueToSprintCommand,
        requester: UserId,
    ) -> Result<IssueDto, AppError> {
        let mut issue = self.issues.get_by_id(cmd.issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, requester)
            .await?;
        if let Some(sprint_id) = cmd.sprint_id {
            let sprint = self.sprints.get_by_id(sprint_id).await?;
            // A sprint from another project must never be assignable to this
            // issue; it would corrupt cross-project reporting.
            if sprint.project_id != issue.project_id {
                return Err(AppError::invalid_input(
                    "sprint belongs to a different project",
                ));
            }
            issue.sprint_id = Some(sprint_id);
        } else {
            issue.sprint_id = None;
        }
        self.issues.save(&issue).await?;
        let name = super::helpers::project_name(self.projects.clone(), issue.project_id).await?;
        Ok(super::helpers::build_issue_dto(self.users.clone(), issue, name.as_str()).await)
    }
}
