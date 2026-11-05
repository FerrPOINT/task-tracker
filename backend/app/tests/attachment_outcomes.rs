//! Exercise the public service without compiling the unrelated lib-test suite.
use app::{AttachmentService, AttachmentServiceImpl, authz::Authz, context::EventBus};
use async_trait::async_trait;
use domain::{AttachmentRepository, FileStorage, IssueRepository, ProjectRepository};
use shared::{AppError, AttachmentId, IssueId, UserId};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Storage {
    inner: domain::InMemoryStorage,
    keys: Mutex<Vec<(String, String)>>,
}
#[async_trait]
impl FileStorage for Storage {
    async fn put(&self, issue: &str, key: &str, bytes: Vec<u8>) -> Result<(), AppError> {
        self.inner.put(issue, key, bytes).await?;
        self.keys.lock().unwrap().push((issue.into(), key.into()));
        Ok(())
    }
    async fn get(&self, issue: &str, key: &str) -> Result<Vec<u8>, AppError> {
        self.inner.get(issue, key).await
    }
    async fn delete(&self, issue: &str, key: &str) -> Result<(), AppError> {
        self.inner.delete(issue, key).await
    }
}
struct Metadata {
    outcome: &'static str,
    entered: tokio::sync::Notify,
    saved: Mutex<Option<domain::Attachment>>,
}
#[async_trait]
impl AttachmentRepository for Metadata {
    async fn get_by_id(&self, _: AttachmentId) -> Result<domain::Attachment, AppError> {
        self.saved
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| AppError::NotFound("fixture".into()))
    }
    async fn list_by_issue(&self, _: IssueId) -> Result<Vec<domain::Attachment>, AppError> {
        Ok(self.saved.lock().unwrap().clone().into_iter().collect())
    }
    async fn save(
        &self,
        value: &domain::Attachment,
    ) -> Result<AttachmentId, domain::AttachmentSaveError> {
        *self.saved.lock().unwrap() = Some(value.clone());
        self.entered.notify_one();
        match self.outcome {
            "rejected" => Err(domain::AttachmentSaveError::NotCommitted(
                AppError::Internal("rejected fixture".into()),
            )),
            "unknown" => Err(domain::AttachmentSaveError::Unknown(AppError::Database(
                "lost acknowledgement".into(),
            ))),
            "waiting" => std::future::pending().await,
            _ => Ok(value.id),
        }
    }
    async fn delete(&self, _: AttachmentId) -> Result<(), AppError> {
        Ok(())
    }
}
async fn fixture(
    outcome: &'static str,
) -> (
    Arc<AttachmentServiceImpl>,
    Arc<Metadata>,
    Arc<Storage>,
    IssueId,
    UserId,
) {
    let user = UserId::new();
    let project = domain::Project {
        id: shared::ProjectId::new(),
        key: shared::ProjectKey::new("FIXTURE"),
        name: "Owned fixture".into(),
        description: None,
        owner_id: user,
        default_board_id: shared::BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    let issue = domain::Issue::create(
        &project,
        1,
        shared::IssueType::Task,
        shared::StatusId::new(),
        "Attachment",
        None,
        user,
        shared::Priority::Medium,
    );
    let projects = Arc::new(domain::MemoryProjectRepository::default());
    projects.save(&project).await.unwrap();
    let issues = Arc::new(domain::MemoryIssueRepository::default());
    issues.save(&issue).await.unwrap();
    let metadata = Arc::new(Metadata {
        outcome,
        entered: tokio::sync::Notify::new(),
        saved: Mutex::new(None),
    });
    let storage = Arc::new(Storage::default());
    let service = Arc::new(AttachmentServiceImpl::new(
        metadata.clone(),
        issues,
        storage.clone(),
        Arc::new(domain::MemoryWatcherRepository::default()),
        EventBus::default(),
        Arc::new(domain::MemoryNotificationRepository::default()),
        Arc::new(domain::StubUserNotificationSettingsRepository),
        Authz::new(
            Arc::new(domain::MemoryProjectMemberRepository::default()),
            projects,
        ),
    ));
    (service, metadata, storage, issue.id, user)
}
#[tokio::test]
async fn only_confirmed_rejection_deletes_the_blob() {
    for outcome in ["rejected", "unknown", "committed"] {
        let (service, metadata, storage, issue, user) = fixture(outcome).await;
        let result = service
            .upload(issue, user, "А файл.txt", "text/plain", b"payload".to_vec())
            .await;
        match outcome {
            "rejected" => assert!(matches!(result, Err(AppError::Internal(_)))),
            "unknown" => assert!(matches!(result, Err(AppError::Unavailable(_)))),
            _ => assert!(result.is_ok()),
        }
        let attachment = metadata.saved.lock().unwrap().clone().unwrap();
        assert!(
            attachment
                .storage_key
                .as_ref()
                .starts_with(&attachment.id.to_string()),
            "blob and metadata must use the original ID"
        );
        let blob = storage
            .get(&issue.to_string(), attachment.storage_key.as_ref())
            .await;
        if outcome == "rejected" {
            assert!(blob.is_err());
        } else {
            assert_eq!(blob.unwrap(), b"payload");
        }
    }
}
#[tokio::test]
async fn canceled_upload_keeps_the_blob_while_metadata_outcome_is_unknown() {
    let (service, metadata, storage, issue, user) = fixture("waiting").await;
    let task = tokio::spawn(async move {
        service
            .upload(
                issue,
                user,
                "pending.txt",
                "text/plain",
                b"pending".to_vec(),
            )
            .await
    });
    metadata.entered.notified().await;
    task.abort();
    let _ = task.await;
    let attachment = metadata.saved.lock().unwrap().clone().unwrap();
    assert_eq!(
        storage
            .get(&issue.to_string(), attachment.storage_key.as_ref())
            .await
            .unwrap(),
        b"pending"
    );
}
