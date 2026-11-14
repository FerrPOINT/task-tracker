use domain::{sdlc::TaskState, sdlc_configured_cli::*};
use reqwest::{Client, Url, header};
use serde::Deserialize;
use serde_json::{Value, json};
use shared::AppError;
use std::{collections::BTreeMap, time::Duration};

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
    role_authorizations: BTreeMap<String, header::HeaderValue>,
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
            role_authorizations: BTreeMap::new(),
        })
    }

    pub fn from_deployment() -> Result<Option<Self>, AppError> {
        match (
            std::env::var("TASKTRACKER_SDLC__WORKFLOW_URL").ok(),
            std::env::var("TASKTRACKER_SDLC__WORKFLOW_READ_TOKEN").ok(),
        ) {
            (None, None) => Ok(None),
            (Some(url), Some(token)) => {
                let mut reader = Self::new(&url, &token)?;
                if let Ok(raw) = std::env::var("TASKTRACKER_SDLC__WORKFLOW_READ_TOKENS_JSON") {
                    let tokens: BTreeMap<String, String> =
                        serde_json::from_str(&raw).map_err(|_| unavailable())?;
                    reader.configure_role_tokens(tokens)?;
                }
                Ok(Some(reader))
            }
            _ => Err(unavailable()),
        }
    }

    fn configure_role_tokens(&mut self, tokens: BTreeMap<String, String>) -> Result<(), AppError> {
        for (role, token) in tokens {
            let mut bytes = role.bytes();
            if !(2..=32).contains(&role.len())
                || !bytes.next().is_some_and(|b| b.is_ascii_lowercase())
                || !bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                || token.is_empty()
                || token.len() > 4096
                || !token.bytes().all(|b| b.is_ascii_graphic())
            {
                return Err(unavailable());
            }
            let mut value = header::HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|_| unavailable())?;
            value.set_sensitive(true);
            self.role_authorizations.insert(role, value);
        }
        Ok(())
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
        let selector = json!({"interaction_protocol":"configured-cli-v1","task":key,"namespace_id":command.namespace_id,
            "execution_ref":command.workflow_execution_ref,"operation_key":command.workflow_operation_key,
            "concrete_agent_ref":command.fence.agent_id,"assignment_ref":command.fence.assignment_id,
            "requirements_revision":command.requirement_revision,"requirements_sha256":command.content_hash,
            "result_version":command.workflow_result_version});
        let bytes = self.request(&selector, &self.authorization).await?;
        decode(&bytes, state, command)
    }

    pub async fn read_delivery(
        &self,
        state: &TaskState,
        lifecycle: &domain::sdlc_lifecycle::LifecycleReceipt,
        artifact: &domain::sdlc_lifecycle::ArtifactReceipt,
    ) -> Result<ConfiguredWorkflowReceipt, AppError> {
        let assignment = lifecycle.assignment.as_ref().ok_or_else(mismatch)?;
        let binding = lifecycle.binding.as_ref().ok_or_else(mismatch)?;
        // The retained TaskState key belongs to PM history. Each technical
        // assignment owns its separately allocated Workflow execution key.
        let execution = assignment.execution.as_ref().ok_or_else(mismatch)?;
        execution.validate().map_err(|_| mismatch())?;
        let authorization = self
            .role_authorizations
            .get(assignment.role.key())
            .ok_or_else(unavailable)?;
        let bytes = self
            .request(
                &json!({"interaction_protocol":"configured-cli-v1", "workflow_receipt":artifact}),
                authorization,
            )
            .await?;
        let (receipt, observed) = decode_artifact(&bytes)?;
        if receipt.task != execution.key
            || receipt.tracker_task_id != state.task_id
            || receipt.owner_subject != state.owner_subject
            || receipt.assignment_ref != assignment.assignment_id
            || receipt.tracker_execution_id != assignment.execution_id
            || receipt.concrete_agent_ref != assignment.agent_id
            || receipt.assignment_version != assignment.fencing_token
            || receipt.machine_subject != binding.machine_subject
            || receipt.requirements_revision.is_some()
            || receipt.requirements_sha256.is_some()
            || receipt.confirmation_id.is_some()
            || observed.reference != artifact.reference
            || observed.sha256 != artifact.sha256
        {
            return Err(mismatch());
        }
        Ok(receipt)
    }

    async fn request(
        &self,
        selector: &Value,
        authorization: &header::HeaderValue,
    ) -> Result<Vec<u8>, AppError> {
        let mut url = self.origin.clone();
        url.set_path("/internal/runtime/terminal-receipt/readback");
        // This existing POST is read-only evidence lookup. There is no retry,
        // enrollment, report, dispatch or caller-selected URL in this reader.
        let mut response = self
            .client
            .post(url)
            .json(selector)
            .header(header::AUTHORIZATION, authorization.clone())
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
        Ok(bytes)
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
    let (receipt, _) = decode_artifact(bytes)?;
    crate::sdlc_configured_cli::validate_receipt(state, command, &receipt)?;
    Ok(receipt)
}

fn decode_artifact(bytes: &[u8]) -> Result<(ConfiguredWorkflowReceipt, Artifact), AppError> {
    let response: Envelope = serde_json::from_slice(bytes).map_err(|_| mismatch())?;
    if !response.ok || !response.complete {
        return Err(mismatch());
    }
    let receipt: ConfiguredWorkflowReceipt =
        serde_json::from_value(response.receipt.clone()).map_err(|_| mismatch())?;
    if serde_json::to_value(&receipt).map_err(AppError::internal)? != response.receipt {
        return Err(mismatch());
    }
    if receipt.interaction_protocol != domain::sdlc::InteractionProtocol::ConfiguredCliV1
        || receipt.status != "done"
        || receipt.history_id < 1
        || receipt.result_version < 2
        || receipt.request_sha256.len() != 64
        || !receipt
            .request_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(mismatch());
    }
    if response.workflow_receipt.reference
        != format!(
            "workflow-configured-cli:{}:{}",
            receipt.execution_ref, receipt.history_id
        )
        || response.workflow_receipt.sha256 != crate::sdlc::canonical_hash(&receipt)?
    {
        return Err(mismatch());
    }
    Ok((receipt, response.workflow_receipt))
}

#[cfg(test)]
mod credential_tests {
    use super::*;

    #[tokio::test]
    async fn delivery_readback_uses_original_technical_execution_not_retained_pm_key() {
        use axum::{Json, Router, extract::State, http::HeaderMap, routing::post};
        use domain::sdlc_lifecycle::LifecycleReceipt;
        use std::sync::{Arc, Mutex};
        use uuid::Uuid;

        let task = Uuid::new_v4();
        let agent = Uuid::new_v4();
        let subject = Uuid::new_v4().to_string();
        let state: TaskState = serde_json::from_value(json!({
            "tracker_instance_id":"test", "project_id":Uuid::new_v4(),
            "task_id":task, "root_task_id":task, "owner_subject":Uuid::new_v4(),
            "stage":"Analysis", "confirmation_revision":null, "assignment":null,
            "questions":[], "revisions":[], "confirmations":[], "evidence":[],
            "interaction_protocol":"configured-cli-v1", "workflow_task_key":"SDLC-11"
        })).unwrap();
        let route = json!({"agent_id":agent,"fleet_config_revision":1,"namespace_id":"9",
            "namespace_name":"Analyst","workflow_id":"9","workflow_key":"analyst",
            "profile":"single-task-v1","instruction_profile":"single-task-v1",
            "instruction_manifest_sha256":"a".repeat(64),
            "workflow_catalog_version":1,"workflow_catalog_sha256":"a".repeat(64)});
        let routes: serde_json::Map<String, Value> = [
            "project_manager", "analyst", "architect", "developer", "reviewer", "tester", "devops",
        ].into_iter().map(|role| (role.into(), route.clone())).collect();
        let routing = json!({"contract_version":2,"snapshot_id":Uuid::new_v4(),
            "tracker_instance_id":"test","project_id":state.project_id,"task_id":task,
            "root_task_id":task,"confirmation_id":Uuid::new_v4(),"requirement_revision":3,
            "content_hash":"b".repeat(64),"created_at":chrono::Utc::now(),
            "policy":{"contract_version":2,"tracker_instance_id":"test","project_id":state.project_id,
                "version":1,"routing_hash":"c".repeat(64),"routes":routes,"verification":"declared",
                "native_ready":true,"dispatch_allowed":true,"author_subject":state.owner_subject,
                "created_at":chrono::Utc::now()}});
        let assignment = json!({"assignment_id":Uuid::new_v4(),"execution_id":Uuid::new_v4(),
            "execution":{"ordinal":"12","key":"SDLC-12"},"task_id":task,
            "owner_subject":state.owner_subject,"role":"analyst","stage":"Analysis","cycle":0,
            "attempt":1,"agent_id":agent,"machine_subject":subject,"routing":routing,
            "previous_result_id":null,"fencing_token":8,"lease_id":Uuid::new_v4(),"assignment_hash":"d".repeat(64)});
        let mut lifecycle: LifecycleReceipt = serde_json::from_value(json!({
            "task":{"task_id":task,"owner_subject":state.owner_subject,"version":8,"stage":"Analysis",
                "status":"InProgress","cycle":0,"transient_retries":0,"waiting_reason":null,
                "runtime":"running","routing":routing,"previous_result_id":null},
            "assignment":assignment,"lease":null,"result":null,
            "binding":{"chat_id":Uuid::new_v4(),"run_id":Uuid::new_v4(),"machine_subject":subject,
                "dispatch_receipt":{"reference":"fixture/dispatch","sha256":"e".repeat(64)}}
        })).unwrap();
        let assignment = lifecycle.assignment.as_ref().unwrap();
        let receipt = json!({"interaction_protocol":"configured-cli-v1","execution_ref":Uuid::new_v4(),
            "task":"SDLC-12","tracker_task_id":task,"namespace_id":9,"concrete_agent_ref":agent,
            "assignment_ref":assignment.assignment_id,"tracker_execution_id":assignment.execution_id,
            "assignment_version":8,"machine_subject":subject,"operation_key":"final-analysis",
            "request_sha256":"f".repeat(64),"result_version":17,"requirements_revision":null,
            "requirements_sha256":null,"owner_subject":state.owner_subject,"confirmation_id":null,
            "history_id":45,"status":"done"});
        let body = Arc::new(Mutex::new(Value::Null));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let router = Router::new().route("/internal/runtime/terminal-receipt/readback", post(
            |State(body): State<Arc<Mutex<Value>>>, headers: HeaderMap, Json(selector): Json<Value>| async move {
                assert_eq!(headers["authorization"], "Bearer analyst-readback");
                assert_eq!(selector.as_object().unwrap().len(), 2);
                assert_eq!(selector["interaction_protocol"], "configured-cli-v1");
                let body = body.lock().unwrap().clone();
                assert_eq!(selector["workflow_receipt"], body["workflow_receipt"]);
                Json(body)
            }
        )).with_state(body.clone());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let mut reader = WorkflowReceiptReader::new(&origin, "pm-readback").unwrap();
        reader.configure_role_tokens(BTreeMap::from([("analyst".into(), "analyst-readback".into())])).unwrap();
        for (field, value, accepted) in [
            ("task", json!("SDLC-12"), true),
            ("task", json!("SDLC-11"), false),
            ("task", json!("SDLC-13"), false),
            ("assignment_ref", json!(Uuid::new_v4()), false),
            ("machine_subject", json!(Uuid::new_v4()), false),
        ] {
            let mut actual = receipt.clone();
            actual[field] = value;
            let artifact = json!({"reference":format!("workflow-configured-cli:{}:45", actual["execution_ref"].as_str().unwrap()),
                "sha256":crate::sdlc::canonical_hash(&actual).unwrap()});
            *body.lock().unwrap() = json!({"ok":true,"complete":true,"receipt":actual,"workflow_receipt":artifact});
            let artifact = serde_json::from_value(artifact).unwrap();
            let result = reader.read_delivery(&state, &lifecycle, &artifact).await;
            assert_eq!(result.is_ok(), accepted, "{field}");
            if accepted {
                assert_eq!(serde_json::to_value(result.unwrap()).unwrap(), receipt);
                lifecycle.assignment.as_mut().unwrap().execution = None;
                assert!(reader.read_delivery(&state, &lifecycle, &artifact).await.is_err());
                lifecycle.assignment.as_mut().unwrap().execution = Some(
                    domain::sdlc_lifecycle::DeliveryExecution::from_ordinal(12).unwrap());
            }
        }
        server.abort();
        assert_eq!(state.retained_fields["workflow_task_key"], "SDLC-11");
    }

    #[test]
    fn catalog_roles_receive_distinct_sensitive_readback_credentials() {
        let mut reader = WorkflowReceiptReader::new("http://workflow:8811", "pm-fixture").unwrap();
        reader
            .configure_role_tokens(BTreeMap::from([
                ("developer".into(), "developer-fixture".into()),
                ("research_assistant".into(), "research-fixture".into()),
            ]))
            .unwrap();
        assert_eq!(reader.authorization, "Bearer pm-fixture");
        assert_eq!(
            reader.role_authorizations["research_assistant"],
            "Bearer research-fixture"
        );
        assert_eq!(
            reader.role_authorizations["developer"],
            "Bearer developer-fixture"
        );
        assert!(
            reader
                .role_authorizations
                .values()
                .all(header::HeaderValue::is_sensitive)
        );
        assert!(!reader.role_authorizations.contains_key("unknown"));
    }

    #[test]
    fn invalid_role_or_header_is_rejected_before_readback() {
        for (role, token) in [
            ("Research", "fixture"),
            ("../other", "fixture"),
            ("r", "fixture"),
            ("research", ""),
            ("research", "bad\r\nheader"),
        ] {
            let mut reader =
                WorkflowReceiptReader::new("http://workflow:8811", "pm-fixture").unwrap();
            assert!(
                reader
                    .configure_role_tokens(BTreeMap::from([(role.into(), token.into())]))
                    .is_err()
            );
        }
    }
}
