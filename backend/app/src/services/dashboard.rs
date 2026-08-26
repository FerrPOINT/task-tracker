use async_trait::async_trait;
use std::sync::Arc;

use crate::dto::DashboardDto;
use domain::{IssueQuery, IssueRepository, ProjectRepository};
use shared::{AppError, UserId};

pub struct DashboardServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
}

impl DashboardServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        users: Arc<dyn domain::UserRepository>,
    ) -> Self {
        Self {
            issues,
            projects,
            users,
        }
    }
}

#[async_trait]
impl crate::context::DashboardService for DashboardServiceImpl {
    async fn get_dashboard(&self, user_id: UserId) -> Result<DashboardDto, AppError> {
        let issues = self.issues.list(IssueQuery::assignee(user_id)).await?;
        let dtos = super::helpers::build_issue_dtos_for_dashboard(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            issues,
        )
        .await?;
        Ok(DashboardDto {
            assigned_issues: dtos,
        })
    }
}
