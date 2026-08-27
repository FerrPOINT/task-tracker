use async_trait::async_trait;
use std::sync::Arc;

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
    events: crate::context::EventBus,
    notifications: Arc<dyn domain::NotificationRepository>,
    authz: Authz,
}

impl CommentServiceImpl {
    pub fn new(
        comments: Arc<dyn domain::CommentRepository>,
        users: Arc<dyn domain::UserRepository>,
        issues: Arc<dyn domain::IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        events: crate::context::EventBus,
        notifications: Arc<dyn domain::NotificationRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            comments,
            users,
            issues,
            projects,
            events,
            notifications,
            authz,
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
        let comments = self.comments.list_by_issue(issue_id).await?;
        let page: Vec<_> = comments
            .into_iter()
            .skip(offset as usize)
            .take(effective_limit)
            .collect();
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
        if cmd.body.trim().is_empty() || cmd.body.chars().count() > 100_000 {
            return Err(AppError::invalid_input(
                "comment body must be between 1 and 100000 characters",
            ));
        }
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
        self.comments.delete(id).await
    }
}
