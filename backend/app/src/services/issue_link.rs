use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use crate::services::helpers;
use domain::IssueRepository;
use shared::{AppError, IssueId, IssueKey, UserId};

pub struct IssueLinkServiceImpl {
    links: Arc<dyn domain::IssueLinkRepository>,
    issues: Arc<dyn IssueRepository>,
    watchers: Arc<dyn domain::WatcherRepository>,
    events: crate::context::EventBus,
    notifications: Arc<dyn domain::NotificationRepository>,
    notification_settings: Arc<dyn domain::UserNotificationSettingsRepository>,
    authz: Authz,
}

impl IssueLinkServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        links: Arc<dyn domain::IssueLinkRepository>,
        issues: Arc<dyn IssueRepository>,
        watchers: Arc<dyn domain::WatcherRepository>,
        events: crate::context::EventBus,
        notifications: Arc<dyn domain::NotificationRepository>,
        notification_settings: Arc<dyn domain::UserNotificationSettingsRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            links,
            issues,
            watchers,
            events,
            notifications,
            notification_settings,
            authz,
        }
    }

    fn publish_issue_updated(&self, issue: &domain::Issue) {
        self.events.publish(shared::TrackerEvent::IssueUpdated {
            issue_id: issue.id.to_string(),
            project_key: issue.key.project_key.to_string(),
        });
    }

    async fn link_notification_recipients(
        &self,
        issue: &domain::Issue,
        linked_issue: &domain::Issue,
    ) -> Vec<UserId> {
        let recipients = helpers::issue_notification_recipients(&self.watchers, issue).await;
        let mut allowed = Vec::with_capacity(recipients.len());
        for recipient_id in recipients {
            match self
                .can_read_link_endpoint(issue, linked_issue, recipient_id)
                .await
            {
                Ok(true) => allowed.push(recipient_id),
                Ok(false) => {}
                Err(err) => {
                    tracing::warn!(
                        recipient_id = %recipient_id,
                        issue_id = %issue.id,
                        linked_issue_id = %linked_issue.id,
                        error = %err,
                        "failed to check issue link notification visibility"
                    );
                }
            }
        }
        allowed
    }

    async fn notify_link_changed(
        &self,
        issue: &domain::Issue,
        linked_issue: &domain::Issue,
        link: &domain::IssueLink,
        requester: UserId,
        event_type: &str,
        title: String,
    ) {
        let recipients = self.link_notification_recipients(issue, linked_issue).await;
        helpers::notify_recipients(
            &self.notifications,
            &self.notification_settings,
            &self.events,
            recipients,
            issue,
            requester,
            event_type,
            title,
            Some(linked_issue.summary.as_ref().to_string()),
            serde_json::json!({
                "issue_key": issue.key.to_string(),
                "linked_issue_key": linked_issue.key.to_string(),
                "link_id": link.id.to_string(),
                "link_type": link.link_type.as_str(),
            }),
        )
        .await;
    }
}

#[async_trait]
impl crate::context::IssueLinkService for IssueLinkServiceImpl {
    async fn create(
        &self,
        source_id: IssueId,
        target_key: &str,
        link_type: &str,
        requester: UserId,
    ) -> Result<crate::context::IssueLinkDto, AppError> {
        let source = self.issues.get_by_id(source_id).await?;
        self.authz
            .require_project_edit(source.project_id, requester)
            .await?;
        // Validate the link type before resolving the target so bad input is 400, not 404.
        let lt: domain::LinkType = link_type.parse().map_err(AppError::invalid_input)?;
        let target_key_vo = IssueKey::parse(target_key)
            .map_err(|_| AppError::invalid_input("invalid target issue key"))?;
        let target = self.issues.get_by_key(&target_key_vo).await?;
        self.authz
            .require_project_access(target.project_id, requester)
            .await?;
        if source.id == target.id {
            return Err(AppError::invalid_input("cannot link an issue to itself"));
        }
        let existing_links = self.links.list_by_issue(source.id).await?;
        if existing_links.iter().any(|existing| {
            existing.link_type == lt
                && ((existing.source_id == source.id && existing.target_id == target.id)
                    || (lt == domain::LinkType::Relates
                        && existing.source_id == target.id
                        && existing.target_id == source.id))
        }) {
            return Err(AppError::conflict("issue link already exists"));
        }
        let link = domain::IssueLink {
            id: shared::IssueLinkId::new(),
            source_id: source.id,
            target_id: target.id,
            link_type: lt,
        };
        self.links.save(&link).await?;
        self.publish_issue_updated(&source);
        self.publish_issue_updated(&target);
        self.notify_link_changed(
            &source,
            &target,
            &link,
            requester,
            "issue_link_created",
            format!("{} linked to {}", source.key, target.key),
        )
        .await;
        self.notify_link_changed(
            &target,
            &source,
            &link,
            requester,
            "issue_link_created",
            format!("{} linked to {}", target.key, source.key),
        )
        .await;
        Ok(crate::context::IssueLinkDto {
            id: link.id.to_string(),
            source_id: source.id.to_string(),
            source_key: source.key.to_string(),
            target_id: target.id.to_string(),
            target_key: target.key.to_string(),
            link_type: link.link_type.as_str().to_string(),
        })
    }

    async fn list_by_issue(
        &self,
        issue_id: IssueId,
        requester: UserId,
    ) -> Result<Vec<crate::context::IssueLinkDto>, AppError> {
        let issue = self.issues.get_by_id(issue_id).await?;
        self.authz
            .require_project_access(issue.project_id, requester)
            .await?;
        let links = self.links.list_by_issue(issue_id).await?;
        let mut out = Vec::with_capacity(links.len());
        for link in links {
            let source = match self.issues.get_by_id_include_deleted(link.source_id).await {
                Ok(issue) => issue,
                Err(AppError::NotFound(_)) => continue,
                Err(err) => return Err(err),
            };
            let target = match self.issues.get_by_id_include_deleted(link.target_id).await {
                Ok(issue) => issue,
                Err(AppError::NotFound(_)) => continue,
                Err(err) => return Err(err),
            };
            if source.deleted_at.is_some() || target.deleted_at.is_some() {
                continue;
            }
            if !self
                .can_read_link_endpoint(&source, &target, requester)
                .await?
            {
                continue;
            }
            out.push(crate::context::IssueLinkDto {
                id: link.id.to_string(),
                source_id: link.source_id.to_string(),
                source_key: source.key.to_string(),
                target_id: link.target_id.to_string(),
                target_key: target.key.to_string(),
                link_type: link.link_type.as_str().to_string(),
            });
        }
        Ok(out)
    }

    async fn delete(
        &self,
        link_id: shared::IssueLinkId,
        requester: UserId,
    ) -> Result<(), AppError> {
        // Load the link first, then require edit access to the linked issue's
        // project. Without this any authenticated user could delete any link.
        let link = self.links.get_by_id(link_id).await?;
        let source = self.issues.get_by_id(link.source_id).await?;
        let target = match self.issues.get_by_id(link.target_id).await {
            Ok(issue) => Some(issue),
            Err(AppError::NotFound(_)) => None,
            Err(err) => return Err(err),
        };
        self.authz
            .require_project_edit(source.project_id, requester)
            .await?;
        self.links.delete(link_id).await?;
        self.publish_issue_updated(&source);
        if let Some(target) = target {
            self.publish_issue_updated(&target);
            self.notify_link_changed(
                &source,
                &target,
                &link,
                requester,
                "issue_link_deleted",
                format!("{} unlinked from {}", source.key, target.key),
            )
            .await;
            self.notify_link_changed(
                &target,
                &source,
                &link,
                requester,
                "issue_link_deleted",
                format!("{} unlinked from {}", target.key, source.key),
            )
            .await;
        }
        Ok(())
    }
}

impl IssueLinkServiceImpl {
    async fn can_read_link_endpoint(
        &self,
        source: &domain::Issue,
        target: &domain::Issue,
        requester: UserId,
    ) -> Result<bool, AppError> {
        for project_id in [source.project_id, target.project_id] {
            match self
                .authz
                .require_project_access(project_id, requester)
                .await
            {
                Ok(()) => {}
                Err(AppError::Forbidden) => return Ok(false),
                Err(error) => return Err(error),
            }
        }
        Ok(true)
    }
}
