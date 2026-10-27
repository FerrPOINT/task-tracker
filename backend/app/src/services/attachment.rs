use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use crate::services::helpers;
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
    watchers: Arc<dyn domain::WatcherRepository>,
    events: crate::context::EventBus,
    notifications: Arc<dyn domain::NotificationRepository>,
    notification_settings: Arc<dyn domain::UserNotificationSettingsRepository>,
    authz: Authz,
    artifact_config: Option<domain::sdlc::SdlcConfig>,
}

impl AttachmentServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        attachments: Arc<dyn domain::AttachmentRepository>,
        issues: Arc<dyn IssueRepository>,
        storage: Arc<dyn domain::FileStorage>,
        watchers: Arc<dyn domain::WatcherRepository>,
        events: crate::context::EventBus,
        notifications: Arc<dyn domain::NotificationRepository>,
        notification_settings: Arc<dyn domain::UserNotificationSettingsRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            attachments,
            issues,
            storage,
            watchers,
            events,
            notifications,
            notification_settings,
            authz,
            artifact_config: None,
        }
    }

    /// Main wires the same validated deployment config used by the existing SDLC repository.
    pub fn with_artifact_config(mut self, config: domain::sdlc::SdlcConfig) -> Self {
        self.artifact_config = Some(config);
        self
    }

    fn artifact_config(&self) -> Result<&domain::sdlc::SdlcConfig, AppError> {
        self.artifact_config
            .as_ref()
            .ok_or_else(|| AppError::Unavailable("role artifact policy not configured".into()))
    }

    fn to_dto(a: &domain::Attachment) -> Result<crate::context::AttachmentDto, AppError> {
        let seal = domain::attachment_artifact::Seal::parse(a.storage_key.as_ref())?;
        if let Some(seal) = &seal {
            seal.validate_attachment(a)?;
        }
        Ok(crate::context::AttachmentDto {
            id: a.id.to_string(),
            issue_id: a.issue_id.to_string(),
            author_id: a.author_id.to_string(),
            file_name: a.file_name.as_ref().to_string(),
            content_type: a.content_type.as_ref().to_string(),
            size_bytes: a.size_bytes,
            created_at: a.created_at.to_rfc3339(),
            artifact: seal.map(|seal| seal.artifact()),
        })
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
    fn authorize_artifact_route(
        &self,
        actor: &domain::sdlc::Principal,
        method: &str,
        path: &str,
    ) -> Result<(), AppError> {
        let config = self.artifact_config()?;
        if crate::attachment_pm_review::route(actor, method, path)? {
            return Ok(());
        }
        if crate::attachment_artifacts::delivery_attachment_route(actor, method, path)? {
            return Ok(());
        }
        crate::sdlc_lifecycle::scheduler_read(actor, config)?;
        let task_list = path
            .strip_prefix("/api/v1/issues/")
            .and_then(|part| part.strip_suffix("/attachments"))
            .is_some_and(|id| crate::sdlc::canonical_pm_uuid(id).is_some());
        if method == "GET" && (crate::attachment_artifacts::is_download_path(path) || task_list) {
            return Ok(());
        }
        Err(AppError::Forbidden)
    }
    async fn upload_artifact(
        &self,
        issue: IssueId,
        actor: &domain::sdlc::Principal,
        file_name: &str,
        content_type: &str,
        bytes: Vec<u8>,
    ) -> Result<crate::context::AttachmentDto, AppError> {
        crate::sdlc_lifecycle::producer(actor, true)?;
        let upload = domain::attachment_artifact::ArtifactUpload {
            file_name: sanitize_file_name(file_name),
            content_type: validate_content_type(content_type)?,
            bytes,
        };
        let attachment = self
            .attachments
            .publish_artifact(
                self.artifact_config()?,
                actor,
                issue,
                upload,
                self.storage.as_ref(),
            )
            .await?;
        Self::to_dto(&attachment)
    }

    async fn list_artifacts(
        &self,
        issue: IssueId,
        actor: &domain::sdlc::Principal,
    ) -> Result<Vec<crate::context::AttachmentDto>, AppError> {
        let items = self
            .attachments
            .list_artifacts(self.artifact_config()?, actor, issue)
            .await?;
        items.iter().map(Self::to_dto).collect()
    }

    async fn download_artifact(
        &self,
        id: shared::AttachmentId,
        actor: &domain::sdlc::Principal,
    ) -> Result<(crate::context::AttachmentDto, Vec<u8>), AppError> {
        let (attachment, bytes) = self
            .attachments
            .download_artifact(self.artifact_config()?, actor, id, self.storage.as_ref())
            .await?;
        crate::attachment_artifacts::verify_bytes(&attachment, &bytes)?;
        Ok((Self::to_dto(&attachment)?, bytes))
    }

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
        let issue_key = issue.key.to_string();
        self.events.publish(shared::TrackerEvent::IssueUpdated {
            issue_id: issue.id.to_string(),
            project_key: issue.key.project_key.to_string(),
        });
        helpers::notify_issue_recipients(
            &self.watchers,
            &self.notifications,
            &self.notification_settings,
            &self.events,
            &issue,
            author_id,
            "issue_attachment_added",
            format!("Attachment added to {issue_key}"),
            Some(file_name.clone()),
            serde_json::json!({
                "issue_key": issue_key,
                "attachment_id": attachment.id.to_string(),
                "file_name": file_name,
            }),
        )
        .await;
        Self::to_dto(&attachment)
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
        if items
            .iter()
            .any(|item| domain::attachment_artifact::is_sealed(item.storage_key.as_ref()))
        {
            self.attachments
                .authorize_sealed_human_read(issue_id, requester)
                .await?;
        }
        items.iter().map(Self::to_dto).collect()
    }

    async fn download(
        &self,
        attachment_id: shared::AttachmentId,
        requester: UserId,
    ) -> Result<(crate::context::AttachmentDto, Vec<u8>), AppError> {
        let a = self.attachments.get_by_id(attachment_id).await?;
        if domain::attachment_artifact::is_sealed(a.storage_key.as_ref()) {
            self.attachments
                .authorize_sealed_human_read(a.issue_id, requester)
                .await?;
        }
        let issue = self.issues.get_by_id(a.issue_id).await?;
        self.authz
            .require_project_access(issue.project_id, requester)
            .await?;
        let bytes = self
            .storage
            .get(&a.issue_id.to_string(), a.storage_key.as_ref())
            .await?;
        if domain::attachment_artifact::is_sealed(a.storage_key.as_ref()) {
            crate::attachment_artifacts::verify_bytes(&a, &bytes)?;
        }
        Ok((Self::to_dto(&a)?, bytes))
    }

    async fn delete(
        &self,
        attachment_id: shared::AttachmentId,
        requester: UserId,
    ) -> Result<(), AppError> {
        let a = self.attachments.get_by_id(attachment_id).await?;
        crate::attachment_artifacts::deny_sealed_delete(a.storage_key.as_ref())?;
        let issue = self.issues.get_by_id(a.issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, requester)
            .await?;
        self.attachments.delete(attachment_id).await?;
        if let Err(err) = self
            .storage
            .delete(&a.issue_id.to_string(), a.storage_key.as_ref())
            .await
        {
            tracing::warn!(
                attachment_id = %attachment_id,
                issue_id = %a.issue_id,
                storage_key = %a.storage_key.as_ref(),
                error = %err,
                "failed to delete attachment blob after metadata deletion"
            );
        }
        self.events.publish(shared::TrackerEvent::IssueUpdated {
            issue_id: issue.id.to_string(),
            project_key: issue.key.project_key.to_string(),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::AttachmentService;

    #[test]
    fn artifact_policy_is_opt_in_and_scheduler_is_read_download_only() {
        let repos = domain::Repositories::default();
        let service = AttachmentServiceImpl::new(
            repos.attachments,
            repos.issues,
            Arc::new(domain::InMemoryStorage::default()),
            repos.watchers,
            crate::context::EventBus::default(),
            repos.notifications,
            repos.notification_settings,
            Authz::new(repos.members, repos.projects),
        );
        let mut scheduler = domain::sdlc::Principal {
            subject: "scheduler".into(),
            human_session: false,
            trusted_human: false,
            scopes: ["task-tracker:read".into()].into(),
        };
        let download = format!("/api/v1/attachments/{}/download", uuid::Uuid::new_v4());
        assert!(matches!(
            service.authorize_artifact_route(&scheduler, "GET", &download),
            Err(AppError::Unavailable(_))
        ));
        let service = service.with_artifact_config(domain::sdlc::SdlcConfig {
            instance_id: "test".into(),
            orchestrator_subject: "pm".into(),
            verifier_subject: "verifier".into(),
            reservation_scheduler_subject: "scheduler".into(),
            single_task_project_ids: Default::default(),
        });
        service
            .authorize_artifact_route(&scheduler, "GET", &download)
            .unwrap();
        for method in ["POST", "HEAD", "PUT", "PATCH", "DELETE"] {
            assert!(
                service
                    .authorize_artifact_route(&scheduler, method, &download)
                    .is_err()
            );
        }
        assert!(
            service
                .authorize_artifact_route(
                    &scheduler,
                    "GET",
                    &format!("/api/v1/issues/{}/attachments", uuid::Uuid::new_v4())
                )
                .is_err()
        );
        scheduler.subject = "foreign-machine".into();
        assert!(
            service
                .authorize_artifact_route(&scheduler, "GET", &download)
                .is_err()
        );
        scheduler.subject = "scheduler".into();
        scheduler.human_session = true;
        assert!(
            service
                .authorize_artifact_route(&scheduler, "GET", &download)
                .is_err()
        );
    }

    #[test]
    fn legacy_attachment_dto_omits_artifact_and_never_fabricates_receipt() {
        let item = domain::Attachment {
            id: shared::AttachmentId::new(),
            issue_id: IssueId::new(),
            author_id: UserId::new(),
            file_name: "legacy.json".into(),
            content_type: "application/json".into(),
            size_bytes: 2,
            storage_key: "old-legacy.json".into(),
            created_at: shared::now(),
        };
        let dto = AttachmentServiceImpl::to_dto(&item).unwrap();
        let value = serde_json::to_value(dto).unwrap();
        assert!(value.get("artifact").is_none());
    }

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
