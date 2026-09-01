use async_trait::async_trait;
use std::{collections::HashSet, sync::Arc};

use crate::authz::Authz;
use crate::commands::{CreateCommentCommand, UpdateCommentCommand};
use crate::dto::CommentDto;
use domain::ProjectRepository;
use shared::{AppError, IssueId, UserId};

pub struct CommentServiceImpl {
    comments: Arc<dyn domain::CommentRepository>,
    users: Arc<dyn domain::UserRepository>,
    issues: Arc<dyn domain::IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    watchers: Arc<dyn domain::WatcherRepository>,
    events: crate::context::EventBus,
    notifications: Arc<dyn domain::NotificationRepository>,
    notification_settings: Arc<dyn domain::UserNotificationSettingsRepository>,
    authz: Authz,
}

impl CommentServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        comments: Arc<dyn domain::CommentRepository>,
        users: Arc<dyn domain::UserRepository>,
        issues: Arc<dyn domain::IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        watchers: Arc<dyn domain::WatcherRepository>,
        events: crate::context::EventBus,
        notifications: Arc<dyn domain::NotificationRepository>,
        notification_settings: Arc<dyn domain::UserNotificationSettingsRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            comments,
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

    /// Create a notification and publish a real-time SSE event.
    async fn create_notification(&self, notification: domain::Notification) {
        let recipient_id = notification.recipient_id;
        let event_type = notification.event_type.as_ref();
        let actor_is_recipient = notification.actor_id == Some(recipient_id);
        let allowed = match self.notification_settings.get_settings(recipient_id).await {
            Ok(settings) => {
                (settings.notify_own_changes || !actor_is_recipient)
                    && !settings
                        .disabled_event_types
                        .iter()
                        .any(|value| value.as_ref() == event_type)
            }
            // Missing settings preserve the existing default delivery behavior.
            Err(shared::AppError::NotFound(_)) => !actor_is_recipient,
            Err(_) => return,
        };
        if allowed && self.notifications.save(&notification).await.is_ok() {
            self.events
                .publish(shared::TrackerEvent::NotificationCreated {
                    recipient_id: recipient_id.to_string(),
                });
        }
    }

    async fn issue_recipients(&self, issue: &domain::Issue) -> Vec<UserId> {
        let mut recipients = vec![issue.reporter_id];
        if let Some(assignee_id) = issue.assignee_id {
            recipients.push(assignee_id);
        }
        if let Ok(watchers) = self.watchers.list_by_issue(issue.id).await {
            recipients.extend(watchers.into_iter().map(|watcher| watcher.user_id));
        }
        recipients
    }

    async fn publish_comment_event(&self, issue: &domain::Issue) {
        if let Ok(project) = self.projects.get_by_id(issue.project_id).await {
            self.events.publish(shared::TrackerEvent::IssueCommented {
                issue_id: issue.id.to_string(),
                project_key: project.key.to_string(),
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn notify_issue_recipients(
        &self,
        recipients: Vec<UserId>,
        issue: &domain::Issue,
        _project: &domain::Project,
        actor_id: UserId,
        event_type: &str,
        title: String,
        body: Option<String>,
        metadata: serde_json::Value,
    ) {
        let mut seen = HashSet::new();
        let action_url = format!("/issues/{}", issue.id);
        for recipient_id in recipients {
            if !seen.insert(recipient_id) {
                continue;
            }
            self.create_notification(domain::Notification {
                id: shared::NotificationId::new(),
                recipient_id,
                event_type: event_type.into(),
                entity_type: "issue".into(),
                entity_id: Some(issue.id.as_uuid()),
                actor_id: Some(actor_id),
                title: title.clone().into(),
                body: body.clone().map(Into::into),
                is_read: false,
                read_at: None,
                action_url: Some(action_url.clone().into()),
                metadata: metadata.clone(),
                created_at: shared::now(),
            })
            .await;
        }
    }
}

#[async_trait]
impl crate::context::CommentService for CommentServiceImpl {
    async fn list(
        &self,
        issue_id: IssueId,
        requester: UserId,
        limit: Option<u64>,
        offset: u64,
    ) -> Result<Vec<CommentDto>, AppError> {
        let issue = self.issues.get_by_id(issue_id).await?;
        self.authz
            .require_project_access(issue.project_id, requester)
            .await?;
        let effective_limit = match limit {
            Some(l) if (1..=500).contains(&l) => l as usize,
            Some(_) => return Err(AppError::invalid_input("limit must be between 1 and 500")),
            None => 100,
        };
        // Bounded SQL page: never load the whole thread to slice it in memory.
        let page = self
            .comments
            .list_by_issue_page(issue_id, effective_limit as u64, offset)
            .await?;
        let mut names: std::collections::HashMap<UserId, String> = std::collections::HashMap::new();
        for u in self.users.list().await.unwrap_or_default() {
            names.insert(u.id, u.display_name.as_ref().to_string());
        }
        let result = page
            .into_iter()
            .map(|c| {
                let author = names.get(&c.author_id).cloned();
                CommentDto::from_comment(c, author)
            })
            .collect();
        Ok(result)
    }

    async fn create(
        &self,
        cmd: CreateCommentCommand,
        requester: UserId,
    ) -> Result<CommentDto, AppError> {
        let issue = self.issues.get_by_id(cmd.issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, requester)
            .await?;
        if cmd.author_id != requester || cmd.actor_id != requester {
            return Err(AppError::Forbidden);
        }
        if cmd.body.trim().is_empty() || cmd.body.chars().count() > 100_000 {
            return Err(AppError::invalid_input(
                "comment body must be between 1 and 100000 characters",
            ));
        }
        let comment = domain::Comment {
            id: shared::CommentId::new(),
            issue_id: cmd.issue_id,
            author_id: requester,
            body: domain::value_objects::RichText::new(cmd.body),
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        self.comments.save(&comment).await?;
        let user = self.users.get_by_id(requester).await.ok();
        if let Ok(issue) = self.issues.get_by_id(cmd.issue_id).await {
            if let Ok(project) = self.projects.get_by_id(issue.project_id).await {
                self.events.publish(shared::TrackerEvent::IssueCommented {
                    issue_id: cmd.issue_id.to_string(),
                    project_key: project.key.to_string(),
                });
                let key = issue.key.to_string();
                self.notify_issue_recipients(
                    self.issue_recipients(&issue).await,
                    &issue,
                    &project,
                    requester,
                    "issue_commented",
                    format!("New comment on {}", key),
                    None,
                    serde_json::json!({"issue_key": key}),
                )
                .await;
            }
        }
        Ok(CommentDto::from_comment(
            comment,
            user.map(|u| u.display_name.as_ref().to_string()),
        ))
    }

    async fn update(
        &self,
        id: shared::CommentId,
        cmd: UpdateCommentCommand,
        requester: UserId,
    ) -> Result<CommentDto, AppError> {
        let mut comment = self.comments.get_by_id(id).await?;
        let issue = self.issues.get_by_id(comment.issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, requester)
            .await?;
        if comment.author_id != requester {
            return Err(AppError::Forbidden);
        }
        if let Some(body) = cmd.body {
            if body.trim().is_empty() || body.chars().count() > 100_000 {
                return Err(AppError::invalid_input(
                    "comment body must be between 1 and 100000 characters",
                ));
            }
            comment.body = domain::value_objects::RichText::new(body);
            comment.updated_at = shared::now();
        }
        self.comments.save(&comment).await?;
        self.publish_comment_event(&issue).await;
        let user = self.users.get_by_id(comment.author_id).await.ok();
        Ok(CommentDto::from_comment(
            comment,
            user.map(|u| u.display_name.as_ref().to_string()),
        ))
    }

    async fn delete(&self, id: shared::CommentId, requester: UserId) -> Result<(), AppError> {
        let comment = self.comments.get_by_id(id).await?;
        let issue = self.issues.get_by_id(comment.issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, requester)
            .await?;
        if comment.author_id != requester {
            return Err(AppError::Forbidden);
        }
        self.comments.delete(id).await?;
        self.publish_comment_event(&issue).await;
        Ok(())
    }
}
