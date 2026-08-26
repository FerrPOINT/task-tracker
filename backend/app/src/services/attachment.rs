use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use domain::IssueRepository;
use shared::{AppError, IssueId, UserId};

pub struct AttachmentServiceImpl {
    attachments: Arc<dyn domain::AttachmentRepository>,
    issues: Arc<dyn IssueRepository>,
    storage: Arc<dyn domain::FileStorage>,
    authz: Authz,
}

impl AttachmentServiceImpl {
    pub fn new(
        attachments: Arc<dyn domain::AttachmentRepository>,
        issues: Arc<dyn IssueRepository>,
        storage: Arc<dyn domain::FileStorage>,
        authz: Authz,
    ) -> Self {
        Self {
            attachments,
            issues,
            storage,
            authz,
        }
    }

    fn to_dto(a: &domain::Attachment) -> crate::context::AttachmentDto {
        crate::context::AttachmentDto {
            id: a.id.to_string(),
            issue_id: a.issue_id.to_string(),
            author_id: a.author_id.to_string(),
            file_name: a.file_name.as_ref().to_string(),
            content_type: a.content_type.as_ref().to_string(),
            size_bytes: a.size_bytes,
            created_at: a.created_at.to_rfc3339(),
        }
    }
}

#[async_trait]
impl crate::context::AttachmentService for AttachmentServiceImpl {
    async fn upload(
        &self,
        issue_id: IssueId,
        author_id: UserId,
        file_name: &str,
        content_type: &str,
        bytes: Vec<u8>,
    ) -> Result<crate::context::AttachmentDto, AppError> {
        let issue = self.issues.get_by_id(issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, author_id)
            .await?;
        let key = format!("{}-{}", uuid::Uuid::new_v4(), file_name);
        self.storage.put(&issue.id.to_string(), &key, bytes).await?;
        let attachment = domain::Attachment {
            id: shared::AttachmentId::new(),
            issue_id: issue.id,
            author_id,
            file_name: file_name.into(),
            content_type: content_type.into(),
            size_bytes: 0, // corrected below from stored file
            storage_key: key.as_str().into(),
            created_at: shared::now(),
        };
        // size from the uploaded bytes (validated in storage)
        let mut a = attachment;
        a.size_bytes = self.storage.get(&a.issue_id.to_string(), &key).await?.len() as i64;
        self.attachments.save(&a).await?;
        Ok(Self::to_dto(&a))
    }

    async fn list_by_issue(
        &self,
        issue_id: IssueId,
        requester: UserId,
    ) -> Result<Vec<crate::context::AttachmentDto>, AppError> {
        let issue = self.issues.get_by_id(issue_id).await?;
        self.authz
            .require_project_access(issue.project_id, requester)
            .await?;
        let items = self.attachments.list_by_issue(issue_id).await?;
        Ok(items.iter().map(Self::to_dto).collect())
    }

    async fn download(
        &self,
        attachment_id: shared::AttachmentId,
        requester: UserId,
    ) -> Result<(crate::context::AttachmentDto, Vec<u8>), AppError> {
        let a = self.attachments.get_by_id(attachment_id).await?;
        let issue = self.issues.get_by_id(a.issue_id).await?;
        self.authz
            .require_project_access(issue.project_id, requester)
            .await?;
        let bytes = self
            .storage
            .get(&a.issue_id.to_string(), a.storage_key.as_ref())
            .await?;
        Ok((Self::to_dto(&a), bytes))
    }

    async fn delete(
        &self,
        attachment_id: shared::AttachmentId,
        requester: UserId,
    ) -> Result<(), AppError> {
        let a = self.attachments.get_by_id(attachment_id).await?;
        let issue = self.issues.get_by_id(a.issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, requester)
            .await?;
        if a.author_id != requester {
            return Err(AppError::Forbidden);
        }
        self.storage
            .delete(&a.issue_id.to_string(), a.storage_key.as_ref())
            .await?;
        self.attachments.delete(attachment_id).await?;
        Ok(())
    }
}
