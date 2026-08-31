use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use crate::dto::DashboardDto;
use domain::{IssueQuery, IssueRepository, LabelRepository, ProjectRepository};
use shared::{AppError, UserId};

pub struct DashboardServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
    labels: Arc<dyn LabelRepository>,
    authz: Authz,
}

impl DashboardServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        users: Arc<dyn domain::UserRepository>,
        labels: Arc<dyn LabelRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            issues,
            projects,
            users,
            labels,
            authz,
        }
    }
}

#[async_trait]
impl crate::context::DashboardService for DashboardServiceImpl {
    async fn get_dashboard(&self, user_id: UserId) -> Result<DashboardDto, AppError> {
        let accessible_project_ids = self.authz.accessible_project_ids(user_id).await?;
        let issues = self
            .issues
            .list(IssueQuery {
                accessible_project_ids: Some(accessible_project_ids),
                ..IssueQuery::assignee(user_id)
            })
            .await?;
        let dtos = super::helpers::build_issue_dtos_for_dashboard(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            Arc::clone(&self.labels),
            issues,
        )
        .await?;
        Ok(DashboardDto {
            assigned_issues: dtos,
        })
    }
}
