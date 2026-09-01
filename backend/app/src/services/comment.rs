use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use crate::commands::{CreateCommentCommand, UpdateCommentCommand};
use crate::dto::CommentDto;
use crate::services::helpers;
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

    async fn publish_comment_event(&self, issue: &domain::Issue) {
        if let Ok(project) = self.projects.get_by_id(issue.project_id).await {
            self.events.publish(shared::TrackerEvent::IssueCommented {
                issue_id: issue.id.to_string(),
                project_key: project.key.to_string(),
            });
        }
    }

    async fn notify_comment_event(
        &self,
        issue: &domain::Issue,
        actor_id: UserId,
        event_type: &str,
        title: String,
        comment_id: shared::CommentId,
    ) {
        helpers::notify_issue_recipients(
            &self.watchers,
            &self.notifications,
            &self.notification_settings,
            &self.events,
            issue,
            actor_id,
            event_type,
            title,
            None,
            serde_json::json!({
                "issue_key": issue.key.to_string(),
                "comment_id": comment_id.to_string(),
            }),
        )
        .await;
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
        for u in self.users.list().await? {
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
        let user = self.users.get_by_id(requester).await?;
        let comment = domain::Comment {
            id: shared::CommentId::new(),
            issue_id: cmd.issue_id,
            author_id: requester,
            body: domain::value_objects::RichText::new(cmd.body),
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        self.comments.save(&comment).await?;
        if let Ok(issue) = self.issues.get_by_id(cmd.issue_id).await {
            if let Ok(project) = self.projects.get_by_id(issue.project_id).await {
                self.events.publish(shared::TrackerEvent::IssueCommented {
                    issue_id: cmd.issue_id.to_string(),
                    project_key: project.key.to_string(),
                });
                let key = issue.key.to_string();
                helpers::notify_issue_recipients(
                    &self.watchers,
                    &self.notifications,
                    &self.notification_settings,
                    &self.events,
                    &issue,
                    requester,
                    "issue_commented",
                    format!("New comment on {}", key),
                    None,
                    serde_json::json!({
                        "issue_key": key,
                        "comment_id": comment.id.to_string(),
                    }),
                )
                .await;
            }
        }
        Ok(CommentDto::from_comment(
            comment,
            Some(user.display_name.as_ref().to_string()),
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
        let user = self.users.get_by_id(comment.author_id).await?;
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
        self.notify_comment_event(
            &issue,
            requester,
            "issue_comment_edited",
            format!("Comment edited on {}", issue.key),
            comment.id,
        )
        .await;
        Ok(CommentDto::from_comment(
            comment,
            Some(user.display_name.as_ref().to_string()),
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
        self.notify_comment_event(
            &issue,
            requester,
            "issue_comment_deleted",
            format!("Comment deleted on {}", issue.key),
            id,
        )
        .await;
        Ok(())
    }
}
