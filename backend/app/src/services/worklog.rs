use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use crate::commands::{CreateWorklogCommand, UpdateWorklogCommand};
use crate::dto::WorklogDto;
use domain::{IssueRepository, ProjectRepository};
use shared::{AppError, IssueId, UserId};

pub struct WorklogServiceImpl {
    worklogs: Arc<dyn domain::WorklogRepository>,
    users: Arc<dyn domain::UserRepository>,
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    events: crate::context::EventBus,
    authz: Authz,
}

impl WorklogServiceImpl {
    pub fn new(
        worklogs: Arc<dyn domain::WorklogRepository>,
        users: Arc<dyn domain::UserRepository>,
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        events: crate::context::EventBus,
        authz: Authz,
    ) -> Self {
        Self {
            worklogs,
            users,
            issues,
            projects,
            events,
            authz,
        }
    }

    fn publish_worklog_event(&self, issue: &domain::Issue, project_key: String) {
        self.events.publish(shared::TrackerEvent::WorklogLogged {
            issue_id: issue.id.to_string(),
            project_key,
        });
    }

    async fn publish_for_issue(&self, issue: &domain::Issue) {
        if let Ok(project) = self.projects.get_by_id(issue.project_id).await {
            self.publish_worklog_event(issue, project.key.to_string());
        }
    }
}

#[async_trait]
impl crate::context::WorklogService for WorklogServiceImpl {
    async fn list(
        &self,
        issue_id: IssueId,
        requester: UserId,
        limit: Option<u64>,
        offset: u64,
    ) -> Result<Vec<WorklogDto>, AppError> {
        let issue = self.issues.get_by_id(issue_id).await?;
        self.authz
            .require_project_access(issue.project_id, requester)
            .await?;
        let effective_limit = match limit {
            Some(l) if (1..=500).contains(&l) => l as usize,
            Some(_) => return Err(AppError::invalid_input("limit must be between 1 and 500")),
            None => 100,
        };
        let worklogs = self.worklogs.list_by_issue(issue_id).await?;
        let worklogs: Vec<_> = worklogs
            .into_iter()
            .skip(offset as usize)
            .take(effective_limit)
            .collect();
        let mut names: std::collections::HashMap<UserId, String> = std::collections::HashMap::new();
        for u in self.users.list().await.unwrap_or_default() {
            names.insert(u.id, u.display_name.as_ref().to_string());
        }
        let result = worklogs
            .into_iter()
            .map(|w| {
                let author = names.get(&w.author_id).cloned();
                WorklogDto::from_worklog(w, author)
            })
            .collect();
        Ok(result)
    }

    async fn create(
        &self,
        cmd: CreateWorklogCommand,
        requester: UserId,
    ) -> Result<WorklogDto, AppError> {
        let issue = self.issues.get_by_id(cmd.issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, requester)
            .await?;
        // Negative or absurd durations corrupt spent-time aggregation.
        if cmd.duration_seconds <= 0 || cmd.duration_seconds > 86_400 {
            return Err(AppError::invalid_input(
                "duration_seconds must be between 1 and 86400",
            ));
        }
        let worklog = domain::Worklog {
            id: shared::WorklogId::new(),
            issue_id: cmd.issue_id,
            author_id: cmd.author_id,
            started_at: cmd.started_at,
            duration_seconds: cmd.duration_seconds,
            description: cmd.description.map(|d| d.into()),
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        self.worklogs.save(&worklog).await?;
        self.publish_for_issue(&issue).await;
        let user = self.users.get_by_id(cmd.author_id).await.ok();
        Ok(WorklogDto::from_worklog(
            worklog,
            user.map(|u| u.display_name.as_ref().to_string()),
        ))
    }

    async fn update(
        &self,
        id: shared::WorklogId,
        cmd: UpdateWorklogCommand,
        requester: UserId,
    ) -> Result<WorklogDto, AppError> {
        let mut worklog = self.worklogs.get_by_id(id).await?;
        let issue = self.issues.get_by_id(worklog.issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, requester)
            .await?;
        if worklog.author_id != requester {
            return Err(AppError::Forbidden);
        }
        if let Some(started_at) = cmd.started_at {
            worklog.started_at = started_at;
        }
        if let Some(duration) = cmd.duration_seconds {
            if duration <= 0 || duration > 86_400 {
                return Err(AppError::invalid_input(
                    "duration_seconds must be between 1 and 86400",
                ));
            }
            worklog.duration_seconds = duration;
        }
        if let Some(description) = cmd.description {
            worklog.description = description.map(|d| d.into());
        }
        worklog.updated_at = shared::now();
        self.worklogs.save(&worklog).await?;
        self.publish_for_issue(&issue).await;
        let user = self.users.get_by_id(worklog.author_id).await.ok();
        Ok(WorklogDto::from_worklog(
            worklog,
            user.map(|u| u.display_name.as_ref().to_string()),
        ))
    }

    async fn delete(&self, id: shared::WorklogId, requester: UserId) -> Result<(), AppError> {
        let worklog = self.worklogs.get_by_id(id).await?;
        let issue = self.issues.get_by_id(worklog.issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, requester)
            .await?;
        if worklog.author_id != requester {
            return Err(AppError::Forbidden);
        }
        self.worklogs.delete(id).await?;
        self.publish_for_issue(&issue).await;
        Ok(())
    }
}
