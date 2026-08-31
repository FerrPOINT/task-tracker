use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use domain::IssueRepository;
use shared::{AppError, IssueId, UserId};

const ALLOWED_ATTACHMENT_CONTENT_TYPES: &[&str] = &[
    "application/gzip",
    "application/json",
    "application/pdf",
    "application/vnd.ms-excel",
    "application/vnd.ms-powerpoint",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.oasis.opendocument.presentation",
    "application/vnd.oasis.opendocument.spreadsheet",
    "application/vnd.oasis.opendocument.text",
    "application/zip",
    "image/gif",
    "image/jpeg",
    "image/png",
    "image/webp",
    "text/csv",
    "text/markdown",
    "text/plain",
];

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

fn sanitize_file_name(file_name: &str) -> String {
    let sanitized: String = file_name
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | '\0' | '\r' | '\n' | '\t' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect();
    let sanitized = sanitized
        .trim()
        .trim_matches('.')
        .chars()
        .take(255)
        .collect::<String>();

    if sanitized.is_empty() || sanitized.chars().all(|ch| ch == '_') {
        "upload.bin".to_string()
    } else {
        sanitized
    }
}

fn validate_content_type(content_type: &str) -> Result<String, AppError> {
    let normalized = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    if ALLOWED_ATTACHMENT_CONTENT_TYPES.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(AppError::invalid_input(
            "unsupported attachment content type",
        ))
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
        let file_name = sanitize_file_name(file_name);
        let content_type = validate_content_type(content_type)?;
        let key = format!("{}-{}", uuid::Uuid::new_v4(), file_name);
        let size_bytes = bytes.len() as i64;
        self.storage.put(&issue.id.to_string(), &key, bytes).await?;
        let attachment = domain::Attachment {
            id: shared::AttachmentId::new(),
            issue_id: issue.id,
            author_id,
            file_name: file_name.as_str().into(),
            content_type: content_type.as_str().into(),
            size_bytes,
            storage_key: key.as_str().into(),
            created_at: shared::now(),
        };
        if let Err(err) = self.attachments.save(&attachment).await {
            let _ = self
                .storage
                .delete(&issue.id.to_string(), key.as_str())
                .await;
            return Err(err);
        }
        Ok(Self::to_dto(&attachment))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_file_name_replaces_path_and_control_chars() {
        assert_eq!(
            sanitize_file_name("../report\r\nfinal.txt"),
            "_report__final.txt"
        );
    }

    #[test]
    fn sanitize_file_name_falls_back_when_empty() {
        assert_eq!(sanitize_file_name("..\n\t"), "upload.bin");
    }

    #[test]
    fn validate_content_type_normalizes_parameters() {
        assert_eq!(
            validate_content_type("Text/Plain; charset=utf-8").unwrap(),
            "text/plain"
        );
    }

    #[test]
    fn validate_content_type_rejects_executables() {
        assert!(validate_content_type("application/x-msdownload").is_err());
    }
}
