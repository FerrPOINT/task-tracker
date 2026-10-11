//! Fleet owns native/configuration observation; Tracker owns command admission and history.
use crate::sdlc_pm_draft::PmDraftReservation;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::AppError;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
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

pub fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}

impl NativeAdmission {
    pub fn validate(&self, reservation: &PmDraftReservation) -> Result<(), AppError> {
        let identity = &self.identity;
        let assignment = &reservation.assignment;
        let binding = &reservation.binding;
        let configuration = &self.native_configuration;
        let age = Utc::now().signed_duration_since(self.observed_at);
        let expected = [
            "fleet_pm_checkpoint",
            "fleet_pm_context",
            "fleet_pm_question",
            "fleet_pm_requirements",
            "fleet_pm_skills",
            "fleet_pm_workflow",
        ];
        if self.contract_version != 1
            || self.observation_ref.is_nil()
            || self.session_run_id.is_nil()
            || self.fence < 1
            || self.effective_config_revision < 1
            || !hash(&self.configuration_sha256)
            || age > chrono::Duration::seconds(5)
            || age < chrono::Duration::seconds(-2)
            || identity.task != reservation.execution.key
            || identity.execution_ref != assignment.execution_id.to_string()
            || identity.assignment_ref != assignment.assignment_id.to_string()
            || identity.assignment_revision != assignment.version
            || identity.agent_ref != assignment.agent_id.to_string()
            || identity.task_ref != binding.task_id.to_string()
            || identity.root_ref != binding.root_task_id.to_string()
            || identity.tracker_project_ref != binding.project_id.to_string()
            || identity.tracker_instance_ref != binding.tracker_instance_id
            || identity.assignment_operation_key != reservation.assignment_operation_key
            || configuration.api_mode != "chat_completions"
            || configuration.tools != expected
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

pub enum CommandAdmission {
    Legacy,
    Replay(serde_json::Value),
    Native(PmDraftReservation),
    History(PmDraftReservation),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdlc::PmAssignment;
    use crate::sdlc_metadata::MetadataInputRef;
    use crate::sdlc_pm_draft::*;

    fn fixture() -> (PmDraftReservation, NativeAdmission) {
        let execution = Uuid::new_v4();
        let agent = Uuid::new_v4();
        let task = Uuid::new_v4();
        let assignment = Uuid::new_v4();
        let project = Uuid::new_v4();
        let reservation = PmDraftReservation {
            contract_version: 1,
            variant: PmDraftVariant::Reserved,
            binding: PmDraftBinding {
                tracker_instance_id: "tracker-owner".into(),
                project_id: project,
                task_id: task,
                root_task_id: task,
                owner_subject: Uuid::new_v4().to_string(),
            },
            owner_cas: OwnerCas {
                expected_version: 0,
                version: 1,
            },
            assignment: PmAssignment {
                assignment_id: assignment,
                execution_id: execution,
                agent_id: agent,
                version: 1,
                machine_subject: Uuid::new_v4().to_string(),
            },
            execution: PmDraftExecution {
                ordinal: "1".into(),
                key: "SDLC-1".into(),
            },
            input: MetadataInputRef {
                snapshot_ref: Uuid::new_v4(),
                sha256: "a".repeat(64),
            },
            assignment_operation_key: "owner-assignment".into(),
            admission_state: PmDraftAdmissionState::Reserved,
            dispatch_allowed: false,
        };
        let observed = NativeAdmission {
            contract_version: 1,
            observation_ref: Uuid::new_v4(),
            observed_at: Utc::now(),
            identity: Identity {
                task: "SDLC-1".into(),
                execution_ref: execution.to_string(),
                tracker_instance_ref: "tracker-owner".into(),
                tracker_project_ref: project.to_string(),
                task_ref: task.to_string(),
                root_ref: task.to_string(),
                agent_ref: agent.to_string(),
                assignment_operation_key: "owner-assignment".into(),
                assignment_ref: assignment.to_string(),
                assignment_revision: 1,
            },
            session_run_id: Uuid::new_v4(),
            native_run_ref: "run_native".into(),
            native_session_ref: "native-session".into(),
            binding_ref: "native-bind".into(),
            fence: 1,
            effective_config_revision: 1,
            configuration_sha256: "a".repeat(64),
            native_configuration: NativeConfiguration {
                model: "model".into(),
                provider: "custom".into(),
                api_mode: "chat_completions".into(),
                route_sha256: "a".repeat(64),
                tools: [
                    "fleet_pm_checkpoint",
                    "fleet_pm_context",
                    "fleet_pm_question",
                    "fleet_pm_requirements",
                    "fleet_pm_skills",
                    "fleet_pm_workflow",
                ]
                .map(str::to_owned)
                .to_vec(),
                output_limit: 512,
            },
        };
        (reservation, observed)
    }
    #[test]
    fn native_owner_observation_is_exact_fresh_and_closed() {
        let (reservation, observed) = fixture();
        observed.validate(&reservation).unwrap();
        let mut variants = vec![];
        let mut wrong = observed.clone();
        wrong.identity.agent_ref = Uuid::new_v4().to_string();
        variants.push(wrong);
        let mut wrong = observed.clone();
        wrong.identity.assignment_revision += 1;
        variants.push(wrong);
        let mut wrong = observed.clone();
        wrong.observed_at -= chrono::Duration::seconds(6);
        variants.push(wrong);
        let mut wrong = observed.clone();
        wrong.native_configuration.tools.push("terminal".into());
        variants.push(wrong);
        let mut wrong = observed.clone();
        wrong.configuration_sha256 = "bad".into();
        variants.push(wrong);
        for wrong in variants {
            assert!(wrong.validate(&reservation).is_err());
        }
        let mut wire = serde_json::to_value(observed).unwrap();
        wire["allowed"] = serde_json::json!(true);
        assert!(serde_json::from_value::<NativeAdmission>(wire).is_err());
    }
}
