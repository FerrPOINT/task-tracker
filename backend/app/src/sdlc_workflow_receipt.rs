use domain::{sdlc::TaskState, sdlc_configured_cli::*};
use reqwest::{Client, Url, header};
use serde::Deserialize;
use serde_json::{Value, json};
use shared::AppError;
use std::time::Duration;

fn unavailable() -> AppError {
    AppError::Unavailable("Workflow terminal readback unavailable".into())
}
fn mismatch() -> AppError {
    AppError::conflict("Workflow terminal readback differs from completion")
}

#[derive(Clone)]
pub struct WorkflowReceiptReader {
    client: Client,
    origin: Url,
    authorization: header::HeaderValue,
}

impl WorkflowReceiptReader {
    pub fn new(origin: &str, token: &str) -> Result<Self, AppError> {
        let url = Url::parse(origin).map_err(|_| unavailable())?;
        if origin.len() > 512
            || origin.contains('\\')
            || origin.chars().any(|c| c.is_whitespace() || c.is_control())
            || !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || url.as_str().trim_end_matches('/') != origin.trim_end_matches('/')
            || token.is_empty()
            || token.len() > 4096
            || !token.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(unavailable());
        }
        let mut authorization =
            header::HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| unavailable())?;
        authorization.set_sensitive(true);
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .build()
            .map_err(|_| unavailable())?;
        Ok(Self {
            client,
            origin: url,
            authorization,
        })
    }

    pub fn from_deployment() -> Result<Option<Self>, AppError> {
        match (
            std::env::var("TASKTRACKER_SDLC__WORKFLOW_URL").ok(),
            std::env::var("TASKTRACKER_SDLC__WORKFLOW_READ_TOKEN").ok(),
        ) {
            (None, None) => Ok(None),
            (Some(url), Some(token)) => Self::new(&url, &token).map(Some),
            _ => Err(unavailable()),
        }
    }

    pub async fn read(
        &self,
        state: &TaskState,
        command: &CompleteConfiguredPm,
    ) -> Result<ConfiguredWorkflowReceipt, AppError> {
        let key = state
            .retained_fields
            .get("workflow_task_key")
            .and_then(Value::as_str)
            .ok_or_else(mismatch)?;
        let mut url = self.origin.clone();
        url.set_path("/internal/runtime/terminal-receipt/readback");
        let selector = json!({"interaction_protocol":"configured-cli-v1","task":key,"namespace_id":command.namespace_id,
            "execution_ref":command.workflow_execution_ref,"operation_key":command.workflow_operation_key,
            "concrete_agent_ref":command.fence.agent_id,"assignment_ref":command.fence.assignment_id,
            "requirements_revision":command.requirement_revision,"requirements_sha256":command.content_hash,
            "result_version":command.workflow_result_version});
        // This existing POST is read-only evidence lookup. There is no retry,
        // enrollment, report, dispatch or caller-selected URL in this reader.
        let mut response = self
            .client
            .post(url)
            .json(&selector)
            .header(header::AUTHORIZATION, self.authorization.clone())
            .header(header::CACHE_CONTROL, "no-cache, no-store")
            .header(header::ACCEPT_ENCODING, "identity")
            .send()
            .await
            .map_err(|_| unavailable())?;
        if response.status() != reqwest::StatusCode::OK
            || response.content_length().is_some_and(|n| n > 262_144)
            || response
                .headers()
                .get(header::CONTENT_ENCODING)
                .is_some_and(|v| v != "identity")
        {
            return Err(unavailable());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
            if bytes.len().saturating_add(chunk.len()) > 262_144 {
                return Err(unavailable());
            }
            bytes.extend_from_slice(&chunk);
        }
        decode(&bytes, state, command)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    ok: bool,
    complete: bool,
    receipt: Value,
    workflow_receipt: Artifact,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    reference: String,
    sha256: String,
}

pub(super) fn decode(
    bytes: &[u8],
    state: &TaskState,
    command: &CompleteConfiguredPm,
) -> Result<ConfiguredWorkflowReceipt, AppError> {
    let response: Envelope = serde_json::from_slice(bytes).map_err(|_| mismatch())?;
    if !response.ok || !response.complete {
        return Err(mismatch());
    }
    let receipt: ConfiguredWorkflowReceipt =
        serde_json::from_value(response.receipt.clone()).map_err(|_| mismatch())?;
    if serde_json::to_value(&receipt).map_err(AppError::internal)? != response.receipt {
        return Err(mismatch());
    }
    crate::sdlc_configured_cli::validate_receipt(state, command, &receipt)?;
    if response.workflow_receipt.reference
        != format!(
            "workflow-configured-cli:{}:{}",
            receipt.execution_ref, receipt.history_id
        )
        || response.workflow_receipt.sha256 != crate::sdlc::canonical_hash(&receipt)?
    {
        return Err(mismatch());
    }
    Ok(receipt)
}
