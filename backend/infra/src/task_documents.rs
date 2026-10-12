//! Bounded server reader; caller PATs are never forwarded to Wiki.
use domain::task_documents::TaskDocumentsReader;
use shared::{AppError, resource_context::NamespaceRef};
use uuid::Uuid;
pub struct WikiReader {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    token: String,
}
impl WikiReader {
    pub fn from_deployment() -> Result<Option<Self>, AppError> {
        let Some(raw) = std::env::var("TT_NAMESPACE__WIKI_URL").ok() else {
            return Ok(None);
        };
        let endpoint = reqwest::Url::parse(&raw)
            .map_err(|_| AppError::invalid_input("invalid_wiki_reader_endpoint"))?;
        if !matches!(endpoint.scheme(), "http" | "https")
            || endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.path() != "/"
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
        {
            return Err(AppError::invalid_input("invalid_wiki_reader_endpoint"));
        }
        let file = std::env::var("TT_NAMESPACE__WIKI_TOKEN_FILE")
            .map_err(|_| AppError::invalid_input("wiki_reader_credential_required"))?;
        let token = std::fs::read_to_string(file)
            .map_err(|_| AppError::Unavailable("wiki_reader_credential_unavailable".into()))?
            .trim()
            .to_string();
        if token.is_empty() || token.contains(['\r', '\n']) {
            return Err(AppError::invalid_input("invalid_wiki_reader_credential"));
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| AppError::Internal("wiki_reader_client_failed".into()))?;
        Ok(Some(Self {
            client,
            endpoint,
            token,
        }))
    }
}
#[async_trait::async_trait]
impl TaskDocumentsReader for WikiReader {
    async fn read(
        &self,
        namespace: &NamespaceRef,
        instance: Uuid,
        task: Uuid,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        let endpoint = self
            .endpoint
            .join(&format!("api/v1/namespace-task-links/{instance}/{task}"))
            .map_err(|_| AppError::invalid_input("invalid_task_ref"))?;
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
            .map_err(|_| AppError::Unavailable("wiki_reader_unavailable".into()))?;
        if !response.status().is_success() {
            return Err(AppError::Unavailable("wiki_reader_rejected".into()));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| AppError::Unavailable("wiki_reader_unavailable".into()))?
        {
            if bytes.len() + chunk.len() > 65_536 {
                return Err(AppError::Unavailable("wiki_reader_invalid_response".into()));
            }
            bytes.extend_from_slice(&chunk);
        }
        let links: Vec<serde_json::Value> = serde_json::from_slice(&bytes)
            .map_err(|_| AppError::Unavailable("wiki_reader_invalid_response".into()))?;
        let expected = serde_json::to_value(namespace)
            .map_err(|_| AppError::Internal("invalid_namespace_ref".into()))?;
        if links.len() > 100
            || links.iter().any(|link| {
                link["namespace"] != expected
                    || link["document_id"]
                        .as_str()
                        .and_then(|s| Uuid::parse_str(s).ok())
                        .is_none()
                    || link["revision_id"]
                        .as_str()
                        .and_then(|s| Uuid::parse_str(s).ok())
                        .is_none()
            })
        {
            return Err(AppError::Unavailable("wiki_reader_invalid_response".into()));
        }
        Ok(links)
    }
}
