use async_trait::async_trait;
use shared::{AppError, resource_context::NamespaceRef};
use uuid::Uuid;
#[derive(serde::Serialize, serde::Deserialize, Clone, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RepositoryRef {
    pub forge_instance_id: Uuid,
    pub repository_id: Uuid,
}
#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TaskRepositoryLink {
    pub namespace: NamespaceRef,
    pub forge_instance_id: Uuid,
    pub repository_id: Uuid,
    pub public_name: String,
    pub kind: String,
}
#[async_trait]
pub trait TaskRepositoriesReader: Send + Sync {
    async fn evidence(
        &self,
        _namespace: &NamespaceRef,
        _tracker: Uuid,
        _task: Uuid,
        _offset: u32,
    ) -> Result<Vec<shared::resource_context::TaskPullEvidence>, AppError> {
        Err(AppError::Unavailable("forge_reader_not_configured".into()))
    }
    async fn verify(
        &self,
        namespace: &NamespaceRef,
        repository: &RepositoryRef,
    ) -> Result<serde_json::Value, AppError>;
    async fn list(
        &self,
        _namespace: &NamespaceRef,
        _offset: u32,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        Err(AppError::Unavailable("forge_reader_not_configured".into()))
    }
}
pub struct UnavailableTaskRepositories;
#[async_trait]
impl TaskRepositoriesReader for UnavailableTaskRepositories {
    async fn verify(
        &self,
        _namespace: &NamespaceRef,
        _repository: &RepositoryRef,
    ) -> Result<serde_json::Value, AppError> {
        Err(AppError::Unavailable("forge_reader_not_configured".into()))
    }
}
