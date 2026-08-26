use async_trait::async_trait;
use std::sync::Arc;

use crate::commands::{CreateWorklogCommand, UpdateWorklogCommand};
use crate::context::{WorklogService};
use crate::dto::{WorklogDto};
use domain::{IssueRepository, UserRepository, WorklogRepository};
use shared::{AppError, IssueId, UserId};

pub struct WorklogServiceImpl {
    worklogs: Arc<dyn domain::WorklogRepository>,
    users: Arc<dyn domain::UserRepository>,
    issues: Arc<dyn domain::IssueRepository>,
}

impl WorklogServiceImpl {
    pub fn new(
        worklogs: Arc<dyn domain::WorklogRepository>,
        users: Arc<dyn domain::UserRepository>,
        issues: Arc<dyn domain::IssueRepository>,
    ) -> Self {
        Self {
            worklogs,
            users,
            issues,
        }
    }
}

#[async_trait]
impl crate::context::WorklogService for WorklogServiceImpl {
    async fn list(
        &self,
        issue_id: IssueId,
        _requester: UserId,
    ) -> Result<Vec<WorklogDto>, AppError> {
        self.issues.get_by_id(issue_id).await?;
        let worklogs = self.worklogs.list_by_issue(issue_id).await?;
        let mut result = Vec::with_capacity(worklogs.len());
        for w in worklogs {
            let user = self.users.get_by_id(w.author_id).await.ok();
            result.push(WorklogDto::from_worklog(
                w,
                user.map(|u| u.display_name.as_ref().to_string()),
            ));
        }
        Ok(result)
    }

    async fn create(&self, cmd: CreateWorklogCommand) -> Result<WorklogDto, AppError> {
        self.issues.get_by_id(cmd.issue_id).await?;
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
        if worklog.author_id != requester {
            return Err(AppError::Unauthorized);
        }
        if let Some(started_at) = cmd.started_at {
            worklog.started_at = started_at;
        }
        if let Some(duration) = cmd.duration_seconds {
            worklog.duration_seconds = duration;
        }
        if let Some(description) = cmd.description {
            worklog.description = description.map(|d| d.into());
        }
        worklog.updated_at = shared::now();
        self.worklogs.save(&worklog).await?;
        let user = self.users.get_by_id(worklog.author_id).await.ok();
        Ok(WorklogDto::from_worklog(
            worklog,
            user.map(|u| u.display_name.as_ref().to_string()),
        ))
    }

    async fn delete(&self, id: shared::WorklogId, requester: UserId) -> Result<(), AppError> {
        let worklog = self.worklogs.get_by_id(id).await?;
        if worklog.author_id != requester {
            return Err(AppError::Unauthorized);
        }
        self.worklogs.delete(id).await
    }
}