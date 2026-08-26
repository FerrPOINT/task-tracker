use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use crate::dto::ProjectDto;
use domain::{Board, IssueQuery, IssueRepository, ProjectRepository};
use shared::{AppError, BoardId, ProjectId, ProjectKey, UserId};

pub struct ProjectServiceImpl {
    projects: Arc<dyn ProjectRepository>,
    issues: Arc<dyn IssueRepository>,
    users: Arc<dyn domain::UserRepository>,
    authz: Authz,
}

impl ProjectServiceImpl {
    pub fn new(
        projects: Arc<dyn ProjectRepository>,
        issues: Arc<dyn IssueRepository>,
        users: Arc<dyn domain::UserRepository>,
        boards: Arc<dyn domain::BoardRepository>,
        authz: Authz,
    ) -> Self {
        // `boards` is no longer stored: project creation persists the default
        // board atomically inside `ProjectRepository::save_with_board`.
        let _ = boards;
        Self {
            projects,
            issues,
            users,
            authz,
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
            columns: super::helpers::default_board_columns(),
        };
        // Project + default board must land atomically: a project whose
        // default_board_id points at a nonexistent board breaks board reads
        // and issue creation.
        self.projects.save_with_board(&project, &board).await?;
        Ok(ProjectDto::from_project(project, 0, 0, 0))
    }

    async fn list(
        &self,
        _query: crate::commands::ProjectQueryDto,
        requester: UserId,
    ) -> Result<Vec<ProjectDto>, AppError> {
        // Projects visible to a user: owned by them or with a membership row.
        // A global list leaks other people's projects into selectors (and then
        // fails with a bare 403 on first use).
        let accessible = self.authz.accessible_project_ids(requester).await?;
        let mut projects = Vec::with_capacity(accessible.len());
        for pid in accessible {
            if let Ok(p) = self.projects.get_by_id(pid).await {
                projects.push(p);
            }
        }
        let mut dtos = Vec::new();
        for project in projects {
            let counts = self.issues.list(IssueQuery::project(project.id)).await?;
            let (todo, in_progress, done) = super::helpers::count_by_status(&counts);
            dtos.push(ProjectDto::from_project(project, todo, in_progress, done));
        }
        Ok(dtos)
    }

    async fn get_by_key(&self, key: &ProjectKey) -> Result<ProjectDto, AppError> {
        let project = self.projects.get_by_key(key).await?;
        let counts = self.issues.list(IssueQuery::project(project.id)).await?;
        let (todo, in_progress, done) = super::helpers::count_by_status(&counts);
        Ok(ProjectDto::from_project(project, todo, in_progress, done))
    }

    async fn update(
        &self,
        key: &ProjectKey,
        cmd: crate::commands::UpdateProjectCommand,
        requester_id: UserId,
    ) -> Result<ProjectDto, AppError> {
        let project = self.projects.get_by_key(key).await?;
        self.authz.require_owner(project.id, requester_id).await?;
        let mut project = project;
        if let Some(name) = cmd.name {
            project.name = name.into();
            project.updated_at = shared::now();
        }
        if let Some(description) = cmd.description {
            project.description = description.map(Into::into);
            project.updated_at = shared::now();
        }
        self.projects.save(&project).await?;
        let counts = self.issues.list(IssueQuery::project(project.id)).await?;
        let (todo, in_progress, done) = super::helpers::count_by_status(&counts);
        Ok(ProjectDto::from_project(project, todo, in_progress, done))
    }

    async fn delete(&self, key: &ProjectKey, requester_id: UserId) -> Result<(), AppError> {
        let project = self.projects.get_by_key(key).await?;
        self.authz.require_owner(project.id, requester_id).await?;
        self.projects.delete(project.id).await
    }
}
