//! Explicit human entry into the current stage. No historical agent result is inferred.
use crate::{
    sdlc::RequirementsDocument,
    task_lifecycle::{Action, State, TransitionCommand},
};
use serde::{Deserialize, Serialize};
use shared::AppError;

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskHandoffCommand {
    pub expected_revision: i64,
    pub expected_routing_policy_version: i64,
    pub confirmed_issue_sha256: String,
    pub requirements: RequirementsDocument,
    pub idempotency_key: String,
}
impl TaskHandoffCommand {
    pub fn validate(&self) -> Result<(), AppError> {
        TransitionCommand {
            action: Action::Start,
            expected_revision: self.expected_revision,
            idempotency_key: self.idempotency_key.clone(),
        }
        .validate()?;
        if !(1..=9_007_199_254_740_991).contains(&self.expected_routing_policy_version)
            || self.confirmed_issue_sha256.len() != 64
            || !self
                .confirmed_issue_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.requirements.goal.trim().is_empty()
            || serde_json::to_vec(&self.requirements)
                .map_err(AppError::internal)?
                .len()
                > 262144
        {
            return Err(AppError::validation("invalid current-stage handoff input"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskExecutionBlocker {
    ModelNotActive,
    ExecutionNotConfigured,
    RoutingNotConfigured,
    RequiredInputMissing,
    ApplicationAccessDenied,
    ExecutionBusy,
    OwnerConsentRequired,
    ConfigurationChanged,
    SourceUnavailable,
    WorkflowUnavailable,
    FleetUnavailable,
    AlreadyCompleted,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskHandoffPreview {
    pub state: State,
    pub issue_sha256: String,
    pub routing_policy_version: Option<i64>,
    pub required_applications: Vec<String>,
    pub blockers: Vec<TaskExecutionBlocker>,
}

/// A genuine human consent; not a PM execution, deployment or terminal receipt.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskHandoffReceipt {
    pub contract_version: u8,
    pub handoff_id: uuid::Uuid,
    pub task_id: uuid::Uuid,
    pub project_id: uuid::Uuid,
    pub actor_subject: String,
    pub entry_state: State,
    pub resulting_revision: i64,
    pub requirement_revision: i64,
    pub requirements_sha256: String,
    pub confirmation_id: uuid::Uuid,
    pub routing_snapshot_id: uuid::Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn command() -> serde_json::Value {
        serde_json::json!({"expected_revision":7,"expected_routing_policy_version":3,
            "confirmed_issue_sha256":"a".repeat(64),"idempotency_key":"human-original",
            "requirements":{"goal":"Human-authored current inputs","scope":[],"exclusions":[],
                "scenarios":[],"acceptance_criteria":[],"constraints":[],"dependencies":[],
                "assumptions":[],"checklist":[],"prerequisites":[]}})
    }
    #[test]
    fn caller_cannot_inject_a_stage_actor_or_previous_agent_result() {
        for key in ["stage", "actor_subject", "previous_result_id"] {
            let mut value = command();
            value[key] = serde_json::json!("invented");
            assert!(serde_json::from_value::<TaskHandoffCommand>(value).is_err());
        }
    }
    #[test]
    fn input_requires_a_real_document_hash_revision_and_original_key() {
        let valid: TaskHandoffCommand = serde_json::from_value(command()).unwrap();
        valid.validate().unwrap();
        for key in ["confirmed_issue_sha256", "idempotency_key"] {
            let mut value = command();
            value[key] = serde_json::json!("");
            assert!(
                serde_json::from_value::<TaskHandoffCommand>(value)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        let mut value = command();
        value["requirements"]["goal"] = serde_json::json!("  ");
        assert!(
            serde_json::from_value::<TaskHandoffCommand>(value)
                .unwrap()
                .validate()
                .is_err()
        );
    }
}

/// Existing machine professions; these do not introduce human authorization roles.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskExecutionRole {
    ProjectManager,
    Analyst,
    Architect,
    Developer,
    Reviewer,
    Tester,
    #[serde(rename = "devops")]
    DevOps,
}
impl TaskExecutionRole {
    pub fn key(self) -> &'static str {
        match self {
            Self::ProjectManager => "project_manager",
            Self::Analyst => "analyst",
            Self::Architect => "architect",
            Self::Developer => "developer",
            Self::Reviewer => "reviewer",
            Self::Tester => "tester",
            Self::DevOps => "devops",
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskExecutionMode {
    Draft,
    Analysis,
    Decomposition,
    Initial,
    Rework,
    Delivery,
}
impl TaskExecutionMode {
    pub fn key(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Analysis => "analysis",
            Self::Decomposition => "decomposition",
            Self::Initial => "initial",
            Self::Rework => "rework",
            Self::Delivery => "delivery",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CurrentStageEntry {
    pub state: State,
    pub role: TaskExecutionRole,
    pub mode: TaskExecutionMode,
}
impl CurrentStageEntry {
    /// Derive only from the server's locked business state; never reset an in-progress stage.
    pub fn derive(state: State, execution_busy: bool) -> Result<Self, AppError> {
        use crate::sdlc_lifecycle::{LifecycleStage as Stage, LifecycleStatus as Status};
        use TaskExecutionMode as Mode;
        use TaskExecutionRole as Role;
        if execution_busy {
            return Err(AppError::conflict("task_lifecycle_execution_busy"));
        }
        if state.status == Status::Completed {
            return Err(AppError::conflict(
                "task_lifecycle_completed_stage_requires_transition",
            ));
        }
        let (role, mode) = match state.stage {
            Stage::Draft | Stage::Backlog => (Role::ProjectManager, Mode::Draft),
            Stage::Analysis => (Role::Analyst, Mode::Analysis),
            Stage::Architecture => (Role::Architect, Mode::Decomposition),
            Stage::Development => (Role::Developer, Mode::Initial),
            Stage::Rework => (Role::Developer, Mode::Rework),
            Stage::Review => (Role::Reviewer, Mode::Delivery),
            Stage::Testing => (Role::Tester, Mode::Delivery),
            Stage::Deployment => (Role::DevOps, Mode::Delivery),
        };
        Ok(Self { state, role, mode })
    }
    pub fn route<'a>(
        &self,
        policy: &'a crate::sdlc_routing::RoutingPolicy,
    ) -> &'a crate::sdlc_routing::RoleRoute {
        // Closed role enum and the owner routing schema contain exactly the same seven professions.
        match self.role {
            TaskExecutionRole::ProjectManager => &policy.routes.project_manager,
            TaskExecutionRole::Analyst => &policy.routes.analyst,
            TaskExecutionRole::Architect => &policy.routes.architect,
            TaskExecutionRole::Developer => &policy.routes.developer,
            TaskExecutionRole::Reviewer => &policy.routes.reviewer,
            TaskExecutionRole::Tester => &policy.routes.tester,
            TaskExecutionRole::DevOps => &policy.routes.devops,
        }
    }
}

#[cfg(test)]
mod current_stage_entry_tests {
    use super::*;
    use crate::{
        sdlc_lifecycle::{LifecycleStage as Stage, LifecycleStatus as Status},
        task_lifecycle::STAGES,
    };
    #[test]
    fn current_stage_selects_existing_profession_mode_without_resetting_manual_progress() {
        let expected = [
            ("project_manager", "draft"),
            ("project_manager", "draft"),
            ("analyst", "analysis"),
            ("architect", "decomposition"),
            ("developer", "initial"),
            ("reviewer", "delivery"),
            ("tester", "delivery"),
            ("developer", "rework"),
            ("devops", "delivery"),
        ];
        for (stage, (role, mode)) in STAGES.into_iter().zip(expected) {
            for status in [Status::Ready, Status::InProgress] {
                let state = State {
                    stage,
                    status,
                    revision: 17,
                };
                let entry = CurrentStageEntry::derive(state, false).unwrap();
                assert_eq!(entry.state, state);
                assert_eq!(entry.role.key(), role);
                assert_eq!(entry.mode.key(), mode);
                if let Some(delivery_role) = stage.role() {
                    assert_eq!(entry.role.key(), delivery_role.key());
                } else {
                    assert!(matches!(stage, Stage::Draft | Stage::Backlog));
                }
                assert!(CurrentStageEntry::derive(state, true).is_err());
            }
        }
    }
    #[test]
    fn no_completed_stage_is_silently_restarted_or_moved_by_handoff() {
        for stage in STAGES {
            assert!(
                CurrentStageEntry::derive(
                    State {
                        stage,
                        status: Status::Completed,
                        revision: 4
                    },
                    false
                )
                .is_err()
            );
        }
    }
}

/// Internal read plan, not an HTTP request or evidence of a runnable agent.
/// The application must obtain fresh owner observations and then revalidate at commit.
#[derive(Clone, Debug)]
pub struct PreparedCurrentStageHandoff {
    pub task_id: uuid::Uuid,
    pub project_id: uuid::Uuid,
    pub owner_subject: String,
    pub state: State,
    pub issue_sha256: String,
    pub entry: Option<CurrentStageEntry>,
    pub routing: Option<crate::sdlc_routing::RoutingPolicy>,
    pub source: Option<crate::sdlc_source::ProjectSourcePolicy>,
    pub blockers: Vec<TaskExecutionBlocker>,
}

#[cfg(test)]
mod manual_authority_tests {
    #[test]
    fn retained_assignment_is_history_not_authority_for_a_manual_task() {
        use crate::sdlc::{Principal, TaskState};
        let task = uuid::Uuid::new_v4();
        let state: TaskState = serde_json::from_value(serde_json::json!({
            "tracker_instance_id":"fixture", "project_id":uuid::Uuid::new_v4(),
            "task_id":task,"root_task_id":task,"owner_subject":"human-owner",
            "stage":"Backlog","status":"Ready","confirmation_revision":null,
            "interaction_protocol":"configured-cli-v1","workflow_task_key":"SDLC-7",
            "assignment":{"assignment_id":uuid::Uuid::new_v4(),"execution_id":uuid::Uuid::new_v4(),
                "agent_id":uuid::Uuid::new_v4(),"version":2,"machine_subject":"previous-producer"},
            "questions":[],"revisions":[],"confirmations":[],"evidence":[]
        }))
        .unwrap();
        let actor = Principal {
            subject: "human-owner".into(),
            human_session: true,
            trusted_human: true,
            scopes: Default::default(),
        };
        let historical = serde_json::to_value(&state).unwrap();
        let live = state.context(&actor);
        assert!(live.assignment.is_some());
        assert_eq!(live.expected_assignment_version, Some(2));
        let manual = state.manual_context(&actor);
        assert!(manual.assignment.is_none() && manual.expected_assignment_version.is_none());
        assert!(!manual.permissions.can_answer && !manual.permissions.can_confirm);
        assert_eq!(manual.waiting_reason.as_deref(), Some("manual_control"));
        assert_eq!(manual.stage, live.stage);
        assert_eq!(manual.status, live.status);
        assert_eq!(manual.task_id, live.task_id);
        assert_eq!(manual.workflow_task_key, live.workflow_task_key);
        assert_eq!(serde_json::to_value(state).unwrap(), historical);
    }
}
