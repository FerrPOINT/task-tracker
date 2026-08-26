use async_trait::async_trait;
use std::sync::Arc;

use crate::commands::{CreateCommentCommand, UpdateCommentCommand};
use crate::dto::CommentDto;
use domain::ProjectRepository;
use shared::{AppError, IssueId, UserId};

pub struct CommentServiceImpl {
    comments: Arc<dyn domain::CommentRepository>,
    users: Arc<dyn domain::UserRepository>,
    issues: Arc<dyn domain::IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    events: crate::context::EventBus,
    notifications: Arc<dyn domain::NotificationRepository>,
}

impl CommentServiceImpl {
    pub fn new(
        comments: Arc<dyn domain::CommentRepository>,
        users: Arc<dyn domain::UserRepository>,
        issues: Arc<dyn domain::IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        events: crate::context::EventBus,
        notifications: Arc<dyn domain::NotificationRepository>,
    ) -> Self {
        Self {
            comments,
            users,
            issues,
            projects,
            events,
            notifications,
        }
    }

    /// Create a notification and publish a real-time SSE event.
    async fn create_notification(&self, notification: domain::Notification) {
        let recipient_id = notification.recipient_id;
        if let Ok(_id) = self.notifications.save(&notification).await {
            self.events
                .publish(shared::TrackerEvent::NotificationCreated {
                    recipient_id: recipient_id.to_string(),
                });
        }
    }
}

#[async_trait]
impl crate::context::CommentService for CommentServiceImpl {
    async fn list(
        &self,
        issue_id: IssueId,
        _requester: UserId,
    ) -> Result<Vec<CommentDto>, AppError> {
        self.issues.get_by_id(issue_id).await?;
        let comments = self.comments.list_by_issue(issue_id).await?;
        let mut result = Vec::with_capacity(comments.len());
        for c in comments {
            let user = self.users.get_by_id(c.author_id).await.ok();
            result.push(CommentDto::from_comment(
                c,
                user.map(|u| u.display_name.as_ref().to_string()),
            ));
        }
        Ok(result)
    }

    async fn create(&self, cmd: CreateCommentCommand) -> Result<CommentDto, AppError> {
        self.issues.get_by_id(cmd.issue_id).await?;
        let comment = domain::Comment {
            id: shared::CommentId::new(),
            issue_id: cmd.issue_id,
            author_id: cmd.author_id,
            body: domain::value_objects::RichText::new(cmd.body),
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        self.comments.save(&comment).await?;
        let user = self.users.get_by_id(cmd.author_id).await.ok();
        if let Ok(issue) = self.issues.get_by_id(cmd.issue_id).await {
            if let Ok(project) = self.projects.get_by_id(issue.project_id).await {
                self.events.publish(shared::TrackerEvent::IssueCommented {
                    issue_id: cmd.issue_id.to_string(),
                    project_key: project.key.to_string(),
                });
                // Notify reporter and assignee about new comment (if different from author)
                let key = issue.key.to_string();
                let action_url = format!("/projects/{}/issues/{}", project.key, issue.id);
                for recipient in [
                    issue.reporter_id,
                    issue.assignee_id.unwrap_or(issue.reporter_id),
                ] {
                    if recipient != cmd.author_id {
                        self.create_notification(domain::Notification {
                            id: shared::NotificationId::new(),
                            recipient_id: recipient,
                            event_type: "issue_commented".into(),
                            entity_type: "issue".into(),
                            entity_id: Some(issue.id.as_uuid()),
                            actor_id: Some(cmd.author_id),
                            title: format!("New comment on {}", key).into(),
                            body: None,
                            is_read: false,
                            read_at: None,
                            action_url: Some(action_url.clone().into()),
                            metadata: serde_json::json!({"issue_key": key}),
                            created_at: shared::now(),
                        })
                        .await;
                    }
                }
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
        if comment.author_id != requester {
            return Err(AppError::Unauthorized);
        }
        if let Some(body) = cmd.body {
            comment.body = domain::value_objects::RichText::new(body);
            comment.updated_at = shared::now();
        }
        self.comments.save(&comment).await?;
        let user = self.users.get_by_id(comment.author_id).await.ok();
        Ok(CommentDto::from_comment(
            comment,
            user.map(|u| u.display_name.as_ref().to_string()),
        ))
    }

    async fn delete(&self, id: shared::CommentId, requester: UserId) -> Result<(), AppError> {
        let comment = self.comments.get_by_id(id).await?;
        if comment.author_id != requester {
            return Err(AppError::Unauthorized);
        }
        self.comments.delete(id).await
    }
}
