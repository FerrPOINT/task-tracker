use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use domain::IssueRepository;
use shared::{AppError, IssueId, IssueKey, UserId};

pub struct IssueLinkServiceImpl {
    links: Arc<dyn domain::IssueLinkRepository>,
    issues: Arc<dyn IssueRepository>,
    authz: Authz,
}

impl IssueLinkServiceImpl {
    pub fn new(
        links: Arc<dyn domain::IssueLinkRepository>,
        issues: Arc<dyn IssueRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            links,
            issues,
            authz,
        }
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
        if source.id == target.id {
            return Err(AppError::invalid_input("cannot link an issue to itself"));
        }
        let link = domain::IssueLink {
            id: shared::IssueLinkId::new(),
            source_id: source.id,
            target_id: target.id,
            link_type: lt,
        };
        self.links.save(&link).await?;
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
            let source = self.issues.get_by_id(link.source_id).await?;
            let target = self.issues.get_by_id(link.target_id).await?;
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
        self.authz
            .require_project_edit(source.project_id, requester)
            .await?;
        self.links.delete(link_id).await?;
        Ok(())
    }
}
