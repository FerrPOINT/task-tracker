//! Fleet owns runtime/configuration observation; Tracker owns command admission and history.
use crate::sdlc_pm_draft::PmDraftReservation;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de};
use shared::AppError;
use uuid::Uuid;

const NATIVE_TOOLS_V1: [&str; 6] = [
    "fleet_pm_checkpoint",
    "fleet_pm_context",
    "fleet_pm_question",
    "fleet_pm_requirements",
    "fleet_pm_skills",
    "fleet_pm_workflow",
];

const FLEET_MCP_ADVERTISED_TOOLS: [&str; 6] = [
    "tracker_clarifications",
    "tracker_context",
    "tracker_publish_question",
    "tracker_publish_revision",
    "tracker_requirements",
    "workflow_step",
];

const FLEET_MCP_WIRE_TOOLS: [&str; 6] = [
    "mcp__fleet_pm__tracker_clarifications",
    "mcp__fleet_pm__tracker_context",
    "mcp__fleet_pm__tracker_publish_question",
    "mcp__fleet_pm__tracker_publish_revision",
    "mcp__fleet_pm__tracker_requirements",
    "mcp__fleet_pm__workflow_step",
];

const FLEET_MCP_RUNTIME_SOURCE_REVISIONS: [&str; 1] = ["bbaf7af5c83546d19f8060f4097d3bb25cd1a3c3"];

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub task: String,
    pub execution_ref: String,
    pub tracker_instance_ref: String,
    pub tracker_project_ref: String,
    pub task_ref: String,
    pub root_ref: String,
    pub agent_ref: String,
    pub assignment_operation_key: String,
    pub assignment_ref: String,
    pub assignment_revision: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeConfiguration {
    pub model: String,
    pub provider: String,
    pub api_mode: String,
    pub route_sha256: String,
    pub tools: Vec<String>,
    pub output_limit: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeAdmission {
    pub contract_version: u8,
    pub observation_ref: Uuid,
    pub observed_at: DateTime<Utc>,
    pub identity: Identity,
    pub session_run_id: Uuid,
    pub native_run_ref: String,
    pub native_session_ref: String,
    pub binding_ref: String,
    pub fence: i64,
    pub effective_config_revision: i64,
    pub configuration_sha256: String,
    pub native_configuration: NativeConfiguration,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FleetMcpProfileConfiguration {
    pub evidence_basis: String,
    pub runtime_source_revision: String,
    pub mcp_server: String,
    pub platform_toolsets: Vec<String>,
    pub advertised_server_tools: Vec<String>,
    pub configured_wire_tools: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FleetMcpAdmission {
    pub contract_version: u8,
    pub profile: String,
    pub observation_ref: Uuid,
    pub observed_at: DateTime<Utc>,
    pub identity: Identity,
    pub session_run_id: Uuid,
    pub hermes_run_ref: String,
    pub hermes_session_ref: String,
    pub binding_ref: String,
    pub fence: i64,
    pub dispatch_operation_key: String,
    pub effective_config_revision: i64,
    pub configuration_sha256: String,
    pub dispatch_request_sha256: String,
    pub profile_configuration: FleetMcpProfileConfiguration,
}

pub fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}

fn sha1_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}

fn bounded_label(value: &str) -> bool {
    !value.is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn exact_strings(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected.iter())
            .all(|(actual, expected)| actual.as_str() == *expected)
}

fn validate_identity(identity: &Identity, reservation: &PmDraftReservation) -> bool {
    let assignment = &reservation.assignment;
    let binding = &reservation.binding;
    identity.task == reservation.execution.key
        && identity.execution_ref == assignment.execution_id.to_string()
        && identity.assignment_ref == assignment.assignment_id.to_string()
        && identity.assignment_revision == assignment.version
        && identity.agent_ref == assignment.agent_id.to_string()
        && identity.task_ref == binding.task_id.to_string()
        && identity.root_ref == binding.root_task_id.to_string()
        && identity.tracker_project_ref == binding.project_id.to_string()
        && identity.tracker_instance_ref == binding.tracker_instance_id
        && identity.assignment_operation_key == reservation.assignment_operation_key
}

fn fresh(observed_at: DateTime<Utc>) -> bool {
    let age = Utc::now().signed_duration_since(observed_at);
    age <= chrono::Duration::seconds(5) && age >= chrono::Duration::seconds(-2)
}

impl NativeAdmission {
    pub fn validate(&self, reservation: &PmDraftReservation) -> Result<(), AppError> {
        let configuration = &self.native_configuration;
        if self.contract_version != 1
            || self.observation_ref.is_nil()
            || self.session_run_id.is_nil()
            || self.fence < 1
            || self.effective_config_revision < 1
            || !hash(&self.configuration_sha256)
            || !fresh(self.observed_at)
            || !validate_identity(&self.identity, reservation)
            || configuration.api_mode != "chat_completions"
            || !exact_strings(&configuration.tools, &NATIVE_TOOLS_V1)
            || !(1..=1_000_000).contains(&configuration.output_limit)
            || !hash(&configuration.route_sha256)
            || [
                &self.native_run_ref,
                &self.native_session_ref,
                &self.binding_ref,
                &configuration.model,
                &configuration.provider,
            ]
            .iter()
            .any(|v| v.is_empty() || v.len() > 512 || v.chars().any(char::is_control))
        {
            return Err(AppError::conflict(
                "Fleet native admission differs from the current PM reservation",
            ));
        }
        Ok(())
    }
}

impl FleetMcpAdmission {
    pub fn validate(&self, reservation: &PmDraftReservation) -> Result<(), AppError> {
        let configuration = &self.profile_configuration;
        if self.contract_version != 2
            || self.profile != "fleet_mcp_custody_v1"
            || self.observation_ref.is_nil()
            || self.session_run_id.is_nil()
            || self.fence < 1
            || self.effective_config_revision < 1
            || !hash(&self.configuration_sha256)
            || !hash(&self.dispatch_request_sha256)
            || !fresh(self.observed_at)
            || !validate_identity(&self.identity, reservation)
            || !bounded_label(&self.hermes_run_ref)
            || !bounded_label(&self.hermes_session_ref)
            || !bounded_label(&self.binding_ref)
            || !bounded_label(&self.dispatch_operation_key)
            || configuration.evidence_basis != "fleet_effective_config_and_accepted_run"
            || configuration.mcp_server != "fleet_pm"
            || !sha1_revision(&configuration.runtime_source_revision)
            || !FLEET_MCP_RUNTIME_SOURCE_REVISIONS
                .contains(&configuration.runtime_source_revision.as_str())
            || !exact_strings(&configuration.platform_toolsets, &["fleet_pm"])
            || !exact_strings(
                &configuration.advertised_server_tools,
                &FLEET_MCP_ADVERTISED_TOOLS,
            )
            || !exact_strings(&configuration.configured_wire_tools, &FLEET_MCP_WIRE_TOOLS)
        {
            return Err(AppError::conflict(
                "Fleet MCP admission differs from the current PM reservation",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum AdmissionObservation {
    NativeV1(NativeAdmission),
    FleetMcpCustodyV1(FleetMcpAdmission),
}

impl<'de> Deserialize<'de> for AdmissionObservation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let version = value
            .get("contract_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| de::Error::custom("admission contract_version is required"))?;
        match version {
            1 => {
                if value.get("profile").is_some() {
                    return Err(de::Error::custom(
                        "native admission v1 must not include profile",
                    ));
                }
                NativeAdmission::deserialize(value)
                    .map(AdmissionObservation::NativeV1)
                    .map_err(de::Error::custom)
            }
            2 => {
                let profile = value
                    .get("profile")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| de::Error::custom("admission profile is required"))?;
                if profile != "fleet_mcp_custody_v1" {
                    return Err(de::Error::custom("unsupported admission profile"));
                }
                FleetMcpAdmission::deserialize(value)
                    .map(AdmissionObservation::FleetMcpCustodyV1)
                    .map_err(de::Error::custom)
            }
            _ => Err(de::Error::custom("unsupported admission contract_version")),
        }
    }
}

impl AdmissionObservation {
    pub fn validate(&self, reservation: &PmDraftReservation) -> Result<(), AppError> {
        match self {
            Self::NativeV1(observation) => observation.validate(reservation),
            Self::FleetMcpCustodyV1(observation) => observation.validate(reservation),
        }
    }

    pub fn session_run_id(&self) -> Uuid {
        match self {
            Self::NativeV1(observation) => observation.session_run_id,
            Self::FleetMcpCustodyV1(observation) => observation.session_run_id,
        }
    }

    pub fn observation_ref(&self) -> Uuid {
        match self {
            Self::NativeV1(observation) => observation.observation_ref,
            Self::FleetMcpCustodyV1(observation) => observation.observation_ref,
        }
    }

    pub fn fence(&self) -> i64 {
        match self {
            Self::NativeV1(observation) => observation.fence,
            Self::FleetMcpCustodyV1(observation) => observation.fence,
        }
    }

    pub fn same_immutable_run(&self, original: &Self) -> bool {
        match (self, original) {
            (Self::NativeV1(current), Self::NativeV1(original)) => {
                current.contract_version == original.contract_version
                    && current.session_run_id == original.session_run_id
                    && current.identity == original.identity
                    && current.fence == original.fence
                    && current.native_run_ref == original.native_run_ref
                    && current.native_session_ref == original.native_session_ref
                    && current.binding_ref == original.binding_ref
                    && current.effective_config_revision == original.effective_config_revision
                    && current.configuration_sha256 == original.configuration_sha256
                    && current.native_configuration == original.native_configuration
            }
            (Self::FleetMcpCustodyV1(current), Self::FleetMcpCustodyV1(original)) => {
                current.contract_version == original.contract_version
                    && current.profile == original.profile
                    && current.session_run_id == original.session_run_id
                    && current.identity == original.identity
                    && current.fence == original.fence
                    && current.hermes_run_ref == original.hermes_run_ref
                    && current.hermes_session_ref == original.hermes_session_ref
                    && current.binding_ref == original.binding_ref
                    && current.dispatch_operation_key == original.dispatch_operation_key
                    && current.effective_config_revision == original.effective_config_revision
                    && current.configuration_sha256 == original.configuration_sha256
                    && current.dispatch_request_sha256 == original.dispatch_request_sha256
                    && current.profile_configuration == original.profile_configuration
            }
            _ => false,
        }
    }

    pub fn same_profile(&self, original: &Self) -> bool {
        matches!(
            (self, original),
            (Self::NativeV1(_), Self::NativeV1(_))
                | (Self::FleetMcpCustodyV1(_), Self::FleetMcpCustodyV1(_))
        )
    }
}

pub enum CommandAdmission {
    Legacy,
    Replay(serde_json::Value),
    MachineLease(PmDraftReservation),
    History(PmDraftReservation),
}

#[cfg(test)]
#[path = "tests/sdlc_native_admission.rs"]
mod tests;
