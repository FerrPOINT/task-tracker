use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use domain::{IssueRepository, ProjectRepository};
use shared::{AppError, IssueId, UserId};

pub struct WatcherServiceImpl {
    watchers: Arc<dyn domain::WatcherRepository>,
    issues: Arc<dyn IssueRepository>,
    users: Arc<dyn domain::UserRepository>,
    projects: Arc<dyn ProjectRepository>,
    events: crate::context::EventBus,
    authz: Authz,
}

impl WatcherServiceImpl {
    pub fn new(
        watchers: Arc<dyn domain::WatcherRepository>,
        issues: Arc<dyn IssueRepository>,
        users: Arc<dyn domain::UserRepository>,
        projects: Arc<dyn ProjectRepository>,
        events: crate::context::EventBus,
        authz: Authz,
    ) -> Self {
        Self {
            watchers,
            issues,
            users,
            projects,
            events,
            authz,
        }
    }
}

#[async_trait]
impl crate::context::WatcherService for WatcherServiceImpl {
    async fn watch(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError> {
        // Verify the issue exists
        let issue = self.issues.get_by_id(issue_id).await?;
        self.authz
            .require_project_access(issue.project_id, user_id)
            .await?;
        // Verify the user exists
        self.users.get_by_id(user_id).await?;
        self.watchers.add(issue_id, user_id).await?;
        let issue = self.issues.get_by_id(issue_id).await?;
        let project = self.projects.get_by_id(issue.project_id).await?;
        self.events.publish(shared::TrackerEvent::IssueUpdated {
            issue_id: issue_id.to_string(),
            project_key: project.key.to_string(),
        });
        Ok(())
    }

    async fn unwatch(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError> {
        self.watchers.remove(issue_id, user_id).await?;
        Ok(())
    }

    async fn list_watchers(
        &self,
        issue_id: IssueId,
        requester: UserId,
    ) -> Result<Vec<crate::context::WatcherDto>, AppError> {
        let issue = self.issues.get_by_id(issue_id).await?;
        self.authz
            .require_project_access(issue.project_id, requester)
            .await?;
        let watchers = self.watchers.list_by_issue(issue_id).await?;
        let mut dtos = Vec::with_capacity(watchers.len());
        for w in watchers {
            let user = self.users.get_by_id(w.user_id).await?;
            dtos.push(crate::context::WatcherDto {
                user_id: w.user_id.to_string(),
                username: user.username.as_ref().to_string(),
                display_name: user.display_name.as_ref().to_string(),
            });
        }
        Ok(dtos)
    }

    async fn is_watching(&self, issue_id: IssueId, user_id: UserId) -> Result<bool, AppError> {
        self.watchers.is_watching(issue_id, user_id).await
    }
}
