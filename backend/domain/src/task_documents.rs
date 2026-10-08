use async_trait::async_trait;
use shared::{AppError, resource_context::NamespaceRef};
use uuid::Uuid;
#[async_trait]
pub trait TaskDocumentsReader: Send + Sync {
    async fn read(
        &self,
        namespace: &NamespaceRef,
        tracker_instance: Uuid,
        task: Uuid,
    ) -> Result<Vec<serde_json::Value>, AppError>;
}
pub struct UnavailableTaskDocuments;
#[async_trait]
impl TaskDocumentsReader for UnavailableTaskDocuments {
    async fn read(
        &self,
        _namespace: &NamespaceRef,
        _instance: Uuid,
        _task: Uuid,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        Err(AppError::Unavailable("wiki_reader_not_configured".into()))
    }
}
