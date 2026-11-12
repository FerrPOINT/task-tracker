use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use crate::dto::{BoardDto, SdlcBoard, SprintDto};
use domain::{
    BoardRepository, IssueQuery, IssueRepository, LabelRepository, ProjectRepository,
    SprintRepository,
};
use shared::{AppError, ProjectKey, UserId};

/// Bounded offset pagination for a deterministic backlog order.
pub mod backlog {
    pub const BACKLOG_PAGE_LIMIT: usize = 100;
    pub const BACKLOG_MAX_PAGE_SIZE: usize = 200;
}

pub struct BoardServiceImpl {
    boards: Arc<dyn BoardRepository>,
    issues: Arc<dyn IssueRepository>,
    sprints: Arc<dyn SprintRepository>,
    users: Arc<dyn domain::UserRepository>,
    labels: Arc<dyn LabelRepository>,
    projects: Arc<dyn ProjectRepository>,
    authz: Authz,
}

impl BoardServiceImpl {
    pub fn new(
        boards: Arc<dyn BoardRepository>,
        issues: Arc<dyn IssueRepository>,
        sprints: Arc<dyn SprintRepository>,
        users: Arc<dyn domain::UserRepository>,
        labels: Arc<dyn LabelRepository>,
        projects: Arc<dyn ProjectRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            boards,
            issues,
            sprints,
            users,
            labels,
            projects,
            authz,
        }
    }

    async fn build_board_dto(
        &self,
        project: domain::Project,
        project_key: &ProjectKey,
    ) -> Result<SdlcBoard, AppError> {
        let sprint = self.sprints.get_active_by_project(project.id).await?;
        let issues = self
            .issues
            .list_unbounded(IssueQuery {
                project_id: Some(project.id),
                ..Default::default()
            })
            .await?;

        let tasks = self
            .boards
            .sdlc_tasks(
                project.id,
                &issues.iter().map(|issue| issue.id).collect::<Vec<_>>(),
            )
            .await?;
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
                project_id: project.id.to_string(),
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

        BoardDto {
            project_id: project.id.to_string(),
            project_key: project_key.to_string(),
            issues: issue_dtos,
            sprint: sprint_dto,
        }
        .with_sdlc_tasks(tasks)
    }
}

#[async_trait]
impl crate::context::BoardService for BoardServiceImpl {
    async fn get_board(
        &self,
        project_key: &ProjectKey,
        requester: UserId,
    ) -> Result<SdlcBoard, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        self.authz
            .require_project_access(project.id, requester)
            .await?;
        self.build_board_dto(project, project_key).await
    }
}
