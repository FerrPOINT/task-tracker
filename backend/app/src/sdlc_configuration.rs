//! Fresh Fleet owner observation. Matching configuration never grants execution.
use chrono::{DateTime, Utc};
use domain::sdlc_routing::RoleRoute;
use reqwest::{Client, Url, header};
use serde::Deserialize;
use shared::AppError;
use std::{collections::BTreeMap, time::Duration};
use uuid::Uuid;

fn unavailable() -> AppError {
    AppError::Unavailable("Fleet configuration observation unavailable".into())
}
fn mismatch() -> AppError {
    AppError::conflict("Fleet configuration differs from frozen routing assignment")
}

#[derive(Clone)]
pub struct FleetConfigurationReader {
    client: Client,
    origin: Url,
    authorization: header::HeaderValue,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Package {
    schema: String,
    repository: String,
    commit: String,
    #[serde(rename = "manifestSha256")]
    manifest_sha256: String,
    role: String,
    namespace: String,
    profile: String,
    modes: Vec<String>,
    #[serde(rename = "roleInstructionSha256")]
    role_instruction_sha256: String,
    #[serde(rename = "skillSha256")]
    skill_sha256: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowBinding {
    schema: String,
    namespace_id: String,
    namespace_name: String,
    workflow_id: String,
    workflow_key: String,
    role_key: String,
    profile: String,
    catalog_version: u32,
    catalog_sha256: String,
    skills_revision: String,
    runtime_ready: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    contract_version: u8,
    observation_ref: Uuid,
    agent_id: Uuid,
    sdlc_role: String,
    effective_revision: i64,
    package: Package,
    workflow_binding: WorkflowBinding,
    observed_at: DateTime<Utc>,
    managed_files_verified: bool,
    runtime_ready: bool,
    blockers: Vec<String>,
}

pub struct MatchedConfiguration {
    pub observation_ref: Uuid,
    pub observed_at: DateTime<Utc>,
}

impl MatchedConfiguration {
    pub fn ensure_fresh(&self) -> Result<(), AppError> {
        let age = Utc::now().signed_duration_since(self.observed_at);
        if age > chrono::Duration::seconds(5) || age < chrono::Duration::seconds(-2) {
            return Err(mismatch());
        }
        Ok(())
    }
}

fn sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn compare(
    route: &RoleRoute,
    role: &str,
    mode: &str,
    observation: Observation,
) -> Result<MatchedConfiguration, AppError> {
    let p = &observation.package;
    let w = &observation.workflow_binding;
    let observed_role = if observation.sdlc_role == "dev_ops" {
        "devops"
    } else {
        &observation.sdlc_role
    };
    if observation.contract_version != 1
        || observation.observation_ref.is_nil()
        || observation.agent_id != route.agent_id
        || observed_role != role
        || observation.effective_revision != route.fleet_config_revision
        || !observation.managed_files_verified
        || observation.runtime_ready
        || w.runtime_ready
        || observation.blockers.is_empty()
        || observation.blockers.len() > 32
        || observation.blockers.iter().any(|b| {
            b.is_empty()
                || b.len() > 128
                || !b.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        })
        || p.schema != "base-sdlc/package-proof/v1"
        || p.repository != "https://github.com/FerrPOINT/services-base.git"
        || p.commit != route.package_commit
        || p.manifest_sha256 != route.package_manifest_sha256
        || p.role != role
        || p.namespace != route.namespace_name
        || p.profile != route.profile
        || !p.modes.iter().any(|item| item == mode)
        || p.skill_sha256.is_empty()
        || !sha256(&p.role_instruction_sha256)
        || p.skill_sha256.values().any(|h| !sha256(h))
        || w.schema != "base-sdlc/workflow-binding/v1"
        || w.namespace_id != route.namespace_id
        || w.namespace_name != route.namespace_name
        || w.workflow_id != route.workflow_id
        || w.workflow_key != route.workflow_key
        || w.role_key != role
        || w.profile != route.profile
        || w.catalog_version != route.workflow_catalog_version
        || w.catalog_sha256 != route.workflow_catalog_sha256
        || w.skills_revision != route.package_commit
    {
        return Err(mismatch());
    }
    let matched = MatchedConfiguration {
        observation_ref: observation.observation_ref,
        observed_at: observation.observed_at,
    };
    matched.ensure_fresh()?;
    Ok(matched)
}

impl FleetConfigurationReader {
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
            || !token.starts_with("sdlc_pat_")
            || !(41..=136).contains(&token.len())
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

    pub async fn read(
        &self,
        route: &RoleRoute,
        role: &str,
        mode: &str,
    ) -> Result<MatchedConfiguration, AppError> {
        let mut url = self.origin.clone();
        url.set_path(&format!(
            "/internal/runtime/v1/agents/{}/configuration",
            route.agent_id
        ));
        let mut response = self
            .client
            .get(url)
            .header(header::AUTHORIZATION, self.authorization.clone())
            .header(header::CACHE_CONTROL, "no-cache, no-store")
            .header(header::ACCEPT_ENCODING, "identity")
            .send()
            .await
            .map_err(|_| unavailable())?;
        if response.status() != reqwest::StatusCode::OK
            || response
                .headers()
                .get(header::CONTENT_ENCODING)
                .is_some_and(|v| v != "identity")
            || response.content_length().is_some_and(|n| n > 65_536)
        {
            return Err(unavailable());
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
            if body.len().saturating_add(chunk.len()) > 65_536 {
                return Err(unavailable());
            }
            body.extend_from_slice(&chunk);
        }
        let value = serde_json::from_slice(&body).map_err(|_| unavailable())?;
        compare(route, role, mode, value)
    }
}

#[cfg(test)]
#[path = "sdlc_configuration_tests.rs"]
mod tests;
