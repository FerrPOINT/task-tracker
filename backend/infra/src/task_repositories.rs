//! Verification uses a registered machine reader, never a caller PAT.
use domain::task_repositories::{RepositoryRef, TaskRepositoriesReader};
use shared::{AppError, resource_context::NamespaceRef};
use uuid::Uuid;
pub struct ForgeReader {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    token: String,
    instance: Uuid,
}
impl ForgeReader {
    pub fn from_deployment() -> Result<Option<Self>, AppError> {
        let Some(raw) = std::env::var("TT_NAMESPACE__FORGE_URL").ok() else {
            return Ok(None);
        };
        let endpoint = reqwest::Url::parse(&raw)
            .map_err(|_| AppError::invalid_input("invalid_forge_reader_endpoint"))?;
        if !matches!(endpoint.scheme(), "http" | "https")
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.path() != "/"
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(AppError::invalid_input("invalid_forge_reader_endpoint"));
        }
        let instance = std::env::var("TT_NAMESPACE__FORGE_INSTANCE_ID")
            .ok()
            .and_then(|value| value.parse().ok())
            .filter(|id: &Uuid| !id.is_nil())
            .ok_or_else(|| AppError::invalid_input("forge_reader_instance_required"))?;
        let file = std::env::var("TT_NAMESPACE__FORGE_TOKEN_FILE")
            .map_err(|_| AppError::invalid_input("forge_reader_credential_required"))?;
        let token = std::fs::read_to_string(file)
            .map_err(|_| AppError::Unavailable("forge_reader_credential_unavailable".into()))?
            .trim()
            .to_string();
        if token.is_empty() || token.contains(['\r', '\n']) {
            return Err(AppError::invalid_input("invalid_forge_reader_credential"));
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| AppError::Internal("forge_reader_client_failed".into()))?;
        Ok(Some(Self {
            client,
            endpoint,
            token,
            instance,
        }))
    }
    async fn get(
        &self,
        namespace: &NamespaceRef,
        path: &str,
        offset: u32,
    ) -> Result<serde_json::Value, AppError> {
        let endpoint = self
            .endpoint
            .join(path)
            .map_err(|_| AppError::invalid_input("invalid_repository_ref"))?;
        let mut response = self
            .client
            .get(endpoint)
            .query(&[
                (
                    "registry_instance_id",
                    namespace.registry_instance_id.to_string(),
                ),
                ("namespace_id", namespace.namespace_id.to_string()),
                ("offset", offset.to_string()),
            ])
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|_| AppError::Unavailable("forge_reader_unavailable".into()))?;
        if !response.status().is_success() {
            return Err(AppError::Unavailable("forge_reader_rejected".into()));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| AppError::Unavailable("forge_reader_unavailable".into()))?
        {
            if bytes.len() + chunk.len() > 65_536 {
                return Err(AppError::Unavailable(
                    "forge_reader_invalid_response".into(),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| AppError::Unavailable("forge_reader_invalid_response".into()))
    }
}
#[async_trait::async_trait]
impl TaskRepositoriesReader for ForgeReader {
    async fn evidence(
        &self,
        namespace: &NamespaceRef,
        tracker: Uuid,
        task: Uuid,
        offset: u32,
    ) -> Result<Vec<shared::resource_context::TaskPullEvidence>, AppError> {
        let value = self
            .get(
                namespace,
                &format!("api/v1/namespace-task-evidence/{tracker}/{task}"),
                offset,
            )
            .await?;
        let items: Vec<shared::resource_context::TaskPullEvidence> = serde_json::from_value(value)
            .map_err(|_| AppError::Unavailable("forge_reader_invalid_response".into()))?;
        if items.len() > 10
            || items.iter().any(|item| {
                item.namespace != *namespace
                    || item.repository.forge_instance_id != self.instance
                    || item.repository.repository_id.is_nil()
                    || item.pull_request_id.is_nil()
            })
        {
            return Err(AppError::Unavailable(
                "forge_reader_invalid_response".into(),
            ));
        }
        Ok(items)
    }
    async fn list(
        &self,
        namespace: &NamespaceRef,
        offset: u32,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let value = self
            .get(namespace, "api/v1/namespace-repositories", offset)
            .await?;
        let values: Vec<serde_json::Value> = serde_json::from_value(value)
            .map_err(|_| AppError::Unavailable("forge_reader_invalid_response".into()))?;
        let expected = serde_json::to_value(namespace).map_err(AppError::internal)?;
        if values.len() > 50
            || values.iter().any(|value| {
                value["namespace"] != expected
                    || value["forge_instance_id"] != self.instance.to_string()
                    || value["repository_id"]
                        .as_str()
                        .and_then(|id| id.parse::<Uuid>().ok())
                        .is_none()
            })
        {
            return Err(AppError::Unavailable(
                "forge_reader_invalid_response".into(),
            ));
        }
        Ok(values)
    }
    async fn verify(
        &self,
        namespace: &NamespaceRef,
        repository: &RepositoryRef,
    ) -> Result<serde_json::Value, AppError> {
        if repository.forge_instance_id != self.instance || repository.repository_id.is_nil() {
            return Err(AppError::invalid_input("unregistered_repository_ref"));
        }
        let endpoint = self
            .endpoint
            .join(&format!(
                "api/v1/namespace-repositories/{}",
                repository.repository_id
            ))
            .map_err(|_| AppError::invalid_input("invalid_repository_ref"))?;
        let mut response = self
            .client
            .get(endpoint)
            .query(&[
                (
                    "registry_instance_id",
                    namespace.registry_instance_id.to_string(),
                ),
                ("namespace_id", namespace.namespace_id.to_string()),
            ])
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|_| AppError::Unavailable("forge_reader_unavailable".into()))?;
        if !response.status().is_success() {
            return Err(AppError::invalid_input("repository_ref_not_verified"));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| AppError::Unavailable("forge_reader_unavailable".into()))?
        {
            if bytes.len() + chunk.len() > 65_536 {
                return Err(AppError::Unavailable(
                    "forge_reader_invalid_response".into(),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| AppError::Unavailable("forge_reader_invalid_response".into()))?;
        if value["namespace"] != serde_json::to_value(namespace).map_err(AppError::internal)?
            || value["forge_instance_id"] != repository.forge_instance_id.to_string()
            || value["repository_id"] != repository.repository_id.to_string()
            || value["public_name"].as_str().is_none()
        {
            return Err(AppError::Unavailable(
                "forge_reader_invalid_response".into(),
            ));
        }
        Ok(value)
    }
}
