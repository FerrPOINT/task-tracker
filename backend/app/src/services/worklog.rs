use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use crate::commands::{CreateWorklogCommand, UpdateWorklogCommand};
use crate::dto::WorklogDto;
use crate::services::helpers;
use domain::{IssueRepository, ProjectRepository};
use shared::{AppError, IssueId, UserId};

pub struct WorklogServiceImpl {
    worklogs: Arc<dyn domain::WorklogRepository>,
    users: Arc<dyn domain::UserRepository>,
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    watchers: Arc<dyn domain::WatcherRepository>,
    events: crate::context::EventBus,
    notifications: Arc<dyn domain::NotificationRepository>,
    notification_settings: Arc<dyn domain::UserNotificationSettingsRepository>,
    authz: Authz,
}

impl WorklogServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        worklogs: Arc<dyn domain::WorklogRepository>,
        users: Arc<dyn domain::UserRepository>,
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        watchers: Arc<dyn domain::WatcherRepository>,
        events: crate::context::EventBus,
        notifications: Arc<dyn domain::NotificationRepository>,
        notification_settings: Arc<dyn domain::UserNotificationSettingsRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            worklogs,
            users,
            issues,
            projects,
            watchers,
            events,
            notifications,
            notification_settings,
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

    async fn notify_worklog_logged(
        &self,
        issue: &domain::Issue,
        actor_id: UserId,
        worklog: &domain::Worklog,
    ) {
        let issue_key = issue.key.to_string();
        helpers::notify_issue_recipients(
            &self.watchers,
            &self.notifications,
            &self.notification_settings,
            &self.events,
            issue,
            actor_id,
            "issue_worklog_logged",
            format!("Work logged on {}", issue_key),
            worklog
                .description
                .as_ref()
                .map(|description| description.as_ref().to_string()),
            serde_json::json!({
                "issue_key": issue_key,
                "worklog_id": worklog.id.to_string(),
                "duration_seconds": worklog.duration_seconds,
                "started_at": worklog.started_at.to_rfc3339(),
            }),
        )
        .await;
    }

    async fn sync_issue_time_tracking(&self, issue: &mut domain::Issue) -> Result<(), AppError> {
        let spent: i64 = self
            .worklogs
            .list_by_issue(issue.id)
            .await?
            .iter()
            .map(|worklog| worklog.duration_seconds)
            .sum();
        issue.time_spent_seconds = spent;
        if let Some(original) = issue.original_estimate_seconds {
            issue.remaining_estimate_seconds = Some((original - spent).max(0));
        }
        issue.updated_at = shared::now();
        self.issues.save(issue).await.map(|_| ())
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
        let worklogs = self
            .worklogs
            .list_by_issue_page(issue_id, effective_limit as u64, offset)
            .await?;
        let mut names: std::collections::HashMap<UserId, String> = std::collections::HashMap::new();
        for u in self.users.list().await? {
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
        let mut issue = self.issues.get_by_id(cmd.issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, requester)
            .await?;
        if cmd.author_id != requester {
            return Err(AppError::Forbidden);
        }
        // Negative or absurd durations corrupt spent-time aggregation.
        if cmd.duration_seconds <= 0 || cmd.duration_seconds > 86_400 {
            return Err(AppError::invalid_input(
                "duration_seconds must be between 1 and 86400",
            ));
        }
        let user = self.users.get_by_id(requester).await?;
        let worklog = domain::Worklog {
            id: shared::WorklogId::new(),
            issue_id: cmd.issue_id,
            author_id: requester,
            started_at: cmd.started_at,
            duration_seconds: cmd.duration_seconds,
            description: cmd.description.map(|d| d.into()),
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        self.worklogs.save(&worklog).await?;
        if let Err(err) = self.sync_issue_time_tracking(&mut issue).await {
            if let Err(rollback_err) = self.worklogs.delete(worklog.id).await {
                tracing::warn!(
                    error = %rollback_err,
                    worklog_id = %worklog.id,
                    "failed to rollback worklog create after issue time tracking sync failed"
                );
            }
            return Err(err);
        }
        self.publish_for_issue(&issue).await;
        self.notify_worklog_logged(&issue, requester, &worklog)
            .await;
        Ok(WorklogDto::from_worklog(
            worklog,
            Some(user.display_name.as_ref().to_string()),
        ))
    }

    async fn update(
        &self,
        id: shared::WorklogId,
        cmd: UpdateWorklogCommand,
        requester: UserId,
    ) -> Result<WorklogDto, AppError> {
        let mut worklog = self.worklogs.get_by_id(id).await?;
        let mut issue = self.issues.get_by_id(worklog.issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, requester)
            .await?;
        if worklog.author_id != requester {
            return Err(AppError::Forbidden);
        }
        let user = self.users.get_by_id(worklog.author_id).await?;
        let previous = worklog.clone();
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
        if let Err(err) = self.sync_issue_time_tracking(&mut issue).await {
            if let Err(rollback_err) = self.worklogs.save(&previous).await {
                tracing::warn!(
                    error = %rollback_err,
                    worklog_id = %worklog.id,
                    "failed to rollback worklog update after issue time tracking sync failed"
                );
            }
            return Err(err);
        }
        self.publish_for_issue(&issue).await;
        Ok(WorklogDto::from_worklog(
            worklog,
            Some(user.display_name.as_ref().to_string()),
        ))
    }

    async fn delete(&self, id: shared::WorklogId, requester: UserId) -> Result<(), AppError> {
        let worklog = self.worklogs.get_by_id(id).await?;
        let mut issue = self.issues.get_by_id(worklog.issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, requester)
            .await?;
        if worklog.author_id != requester {
            return Err(AppError::Forbidden);
        }
        self.worklogs.delete(id).await?;
        if let Err(err) = self.sync_issue_time_tracking(&mut issue).await {
            if let Err(rollback_err) = self.worklogs.save(&worklog).await {
                tracing::warn!(
                    error = %rollback_err,
                    worklog_id = %id,
                    "failed to rollback worklog delete after issue time tracking sync failed"
                );
            }
            return Err(err);
        }
        self.publish_for_issue(&issue).await;
        Ok(())
    }
}
