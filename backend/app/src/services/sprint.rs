use async_trait::async_trait;
use std::sync::Arc;

use crate::dto::{IssueDto, SprintDto};
use domain::{IssueQuery, IssueRepository, ProjectRepository};
use shared::{AppError, ProjectId, SprintId};

#[async_trait]
pub trait SprintService: Send + Sync {
    async fn create(
        &self,
        cmd: crate::commands::CreateSprintCommand,
    ) -> Result<SprintDto, AppError>;
    async fn list(&self, project_id: ProjectId) -> Result<Vec<SprintDto>, AppError>;
    async fn get_by_id(&self, id: SprintId) -> Result<SprintDto, AppError>;
    async fn update(
        &self,
        id: SprintId,
        cmd: crate::commands::UpdateSprintCommand,
    ) -> Result<SprintDto, AppError>;
    async fn start(&self, id: SprintId) -> Result<SprintDto, AppError>;
    async fn close(&self, id: SprintId) -> Result<SprintDto, AppError>;
    async fn move_issue(
        &self,
        cmd: crate::commands::MoveIssueToSprintCommand,
    ) -> Result<IssueDto, AppError>;
}

pub struct SprintServiceImpl {
    sprints: Arc<dyn domain::SprintRepository>,
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
}

impl SprintServiceImpl {
    pub fn new(
        sprints: Arc<dyn domain::SprintRepository>,
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        users: Arc<dyn domain::UserRepository>,
    ) -> Self {
        Self {
            sprints,
            issues,
            projects,
            users,
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
    ) -> Result<SprintDto, AppError> {
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

    async fn list(&self, project_id: ProjectId) -> Result<Vec<SprintDto>, AppError> {
        let sprints = self.sprints.list_by_project(project_id).await?;
        let mut result = Vec::with_capacity(sprints.len());
        for s in sprints {
            result.push(self.sprint_dto(s).await?);
        }
        Ok(result)
    }

    async fn get_by_id(&self, id: SprintId) -> Result<SprintDto, AppError> {
        let sprint = self.sprints.get_by_id(id).await?;
        self.sprint_dto(sprint).await
    }

    async fn update(
        &self,
        id: SprintId,
        cmd: crate::commands::UpdateSprintCommand,
    ) -> Result<SprintDto, AppError> {
        let mut sprint = self.sprints.get_by_id(id).await?;
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
        self.sprints.save(&sprint).await?;
        self.sprint_dto(sprint).await
    }

    async fn start(&self, id: SprintId) -> Result<SprintDto, AppError> {
        let mut sprint = self.sprints.get_by_id(id).await?;
        if sprint.state != domain::SprintState::Future {
            return Err(AppError::invalid_input("sprint is not in future state"));
        }
        sprint.state = domain::SprintState::Active;
        sprint.start_date = Some(sprint.start_date.unwrap_or_else(shared::now));
        self.sprints.save(&sprint).await?;
        self.sprint_dto(sprint).await
    }

    async fn close(&self, id: SprintId) -> Result<SprintDto, AppError> {
        let mut sprint = self.sprints.get_by_id(id).await?;
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
    ) -> Result<IssueDto, AppError> {
        let mut issue = self.issues.get_by_id(cmd.issue_id).await?;
        if let Some(sprint_id) = cmd.sprint_id {
            let _ = self.sprints.get_by_id(sprint_id).await?;
            issue.sprint_id = Some(sprint_id);
        } else {
            issue.sprint_id = None;
        }
        self.issues.save(&issue).await?;
        let name = super::helpers::project_name(self.projects.clone(), issue.project_id).await?;
        Ok(super::helpers::build_issue_dto(self.users.clone(), issue, name.as_str()).await)
    }
}
