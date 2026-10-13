//! Single-task delivery policy. Historical PM/Analysis wire types stay unchanged.
use crate::sdlc::{MachineFence, Principal, Stage, safe_version};
use crate::sdlc_execution_lease::ExecutionLease;
use crate::sdlc_routing::TaskRoutingSnapshot;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::AppError;
use uuid::Uuid;

pub const POOL_LIMIT: u8 = 2;
pub const LEASE_TTL_SECONDS: i64 = 30;
pub const HEARTBEAT_SECONDS: i64 = 10;
pub const MAX_TRANSIENT_RETRIES: u8 = 3;
pub const MAX_REWORK_CYCLES: u8 = 3;
pub const DELIVERY_GRANT_PREFIX: &str = "task-tracker:sdlc:delivery:";

#[derive(Clone, Debug, PartialEq, Eq, utoipa::ToSchema)]
pub struct DeliveryGrant {
    pub task_id: Uuid,
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
    pub agent_id: Uuid,
    pub fencing_token: i64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
pub enum LifecycleStage {
    Draft,
    Backlog,
    Analysis,
    Architecture,
    Development,
    Review,
    Testing,
    Rework,
    Deployment,
}

impl LifecycleStage {
    pub fn from_legacy(stage: Stage) -> Self {
        match stage {
            Stage::Draft | Stage::Clarification => Self::Draft,
            Stage::Backlog => Self::Backlog,
            Stage::Analysis => Self::Analysis,
            Stage::Architecture => Self::Architecture,
            Stage::Development => Self::Development,
            Stage::Review => Self::Review,
            Stage::Testing => Self::Testing,
            Stage::Rework => Self::Rework,
            Stage::Deployment => Self::Deployment,
        }
    }

    pub fn role(self) -> Option<DeliveryRole> {
        match self {
            Self::Draft | Self::Backlog => None,
            Self::Analysis => Some(DeliveryRole::Analyst),
            Self::Architecture => Some(DeliveryRole::Architect),
            Self::Development | Self::Rework => Some(DeliveryRole::Developer),
            Self::Review => Some(DeliveryRole::Reviewer),
            Self::Testing => Some(DeliveryRole::Tester),
            Self::Deployment => Some(DeliveryRole::DevOps),
        }
    }

    pub fn canonical(self) -> Stage {
        match self {
            Self::Draft => Stage::Draft,
            Self::Backlog => Stage::Backlog,
            Self::Analysis => Stage::Analysis,
            Self::Architecture => Stage::Architecture,
            Self::Development => Stage::Development,
            Self::Review => Stage::Review,
            Self::Testing => Stage::Testing,
            Self::Rework => Stage::Rework,
            Self::Deployment => Stage::Deployment,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq, Eq)]
pub enum LifecycleStatus {
    Ready,
    InProgress,
    Completed,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryRole {
    Analyst,
    Architect,
    Developer,
    Reviewer,
    Tester,
    #[serde(rename = "devops")]
    DevOps,
}

impl DeliveryRole {
    pub fn key(self) -> &'static str {
        match self {
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
pub enum RuntimeState {
    Queued,
    Reserved,
    Running,
    Terminal,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtifactReceipt {
    pub reference: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(tag = "role", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeliveryResult {
    Analyst {
        analysis: ArtifactReceipt,
    },
    Architect {
        design: ArtifactReceipt,
    },
    Developer {
        commit_sha: String,
        changes: ArtifactReceipt,
        checks: Vec<ArtifactReceipt>,
    },
    Reviewer {
        verdict: ReviewVerdict,
        report: ArtifactReceipt,
    },
    Tester {
        verdict: TestVerdict,
        report: ArtifactReceipt,
    },
    #[serde(rename = "devops")]
    DevOps {
        deployment: ArtifactReceipt,
        verification: ArtifactReceipt,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    Approved,
    Rework,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TestVerdict {
    Passed,
    Rework,
}

impl DeliveryResult {
    pub fn role(&self) -> DeliveryRole {
        match self {
            Self::Analyst { .. } => DeliveryRole::Analyst,
            Self::Architect { .. } => DeliveryRole::Architect,
            Self::Developer { .. } => DeliveryRole::Developer,
            Self::Reviewer { .. } => DeliveryRole::Reviewer,
            Self::Tester { .. } => DeliveryRole::Tester,
            Self::DevOps { .. } => DeliveryRole::DevOps,
        }
    }

    pub fn artifacts(&self) -> Vec<&ArtifactReceipt> {
        match self {
            Self::Analyst { analysis } => vec![analysis],
            Self::Architect { design } => vec![design],
            Self::Developer {
                changes, checks, ..
            } => std::iter::once(changes).chain(checks.iter()).collect(),
            Self::Reviewer { report, .. } | Self::Tester { report, .. } => vec![report],
            Self::DevOps {
                deployment,
                verification,
            } => vec![deployment, verification],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LifecycleTask {
    pub task_id: Uuid,
    pub owner_subject: String,
    #[serde(deserialize_with = "safe_version")]
    pub version: i64,
    pub stage: LifecycleStage,
    pub status: LifecycleStatus,
    pub cycle: u8,
    pub transient_retries: u8,
    pub waiting_reason: Option<String>,
    pub runtime: RuntimeState,
    pub routing: TaskRoutingSnapshot,
    pub previous_result_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_chat_id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTerminalState {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(tag = "disposition", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecoveryDisposition {
    Stop,
    RetryTransient {
        replay_safety_receipt: ArtifactReceipt,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeTerminalObservation {
    pub assignment_hash: String,
    pub chat_id: Uuid,
    pub run_id: Uuid,
    pub state: RuntimeTerminalState,
    pub receipt: ArtifactReceipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeliveryAssignment {
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<DeliveryExecution>,
    pub task_id: Uuid,
    pub owner_subject: String,
    pub role: DeliveryRole,
    pub stage: LifecycleStage,
    pub cycle: u8,
    pub attempt: u8,
    pub agent_id: Uuid,
    pub routing: TaskRoutingSnapshot,
    pub previous_result_id: Option<Uuid>,
    pub fencing_token: i64,
    pub lease_id: Uuid,
    pub assignment_hash: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeliveryExecution {
    #[schema(pattern = "^[1-9][0-9]{0,18}$")]
    pub ordinal: String,
    #[schema(pattern = "^SDLC-[1-9][0-9]{0,18}$")]
    pub key: String,
}

impl DeliveryExecution {
    pub fn from_ordinal(ordinal: i64) -> Result<Self, AppError> {
        let execution = Self {
            ordinal: ordinal.to_string(),
            key: format!("SDLC-{ordinal}"),
        };
        execution.validate()?;
        Ok(execution)
    }

    pub fn validate(&self) -> Result<(), AppError> {
        if !self
            .ordinal
            .parse::<i64>()
            .is_ok_and(|n| n > 0 && n.to_string() == self.ordinal)
            || self.key != format!("SDLC-{}", self.ordinal)
        {
            return Err(AppError::validation(
                "invalid Tracker delivery execution reference",
            ));
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for DeliveryExecution {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            ordinal: String,
            key: String,
        }
        let Wire { ordinal, key } = Wire::deserialize(d)?;
        let execution = Self { ordinal, key };
        execution
            .validate()
            .map_err(<D::Error as serde::de::Error>::custom)?;
        Ok(execution)
    }
}

impl DeliveryAssignment {
    pub fn scope(&self) -> String {
        format!(
            "{DELIVERY_GRANT_PREFIX}{}:{}:{}:{}:{}",
            self.task_id, self.assignment_id, self.execution_id, self.agent_id, self.fencing_token
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeliveryFence {
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
    pub fencing_token: i64,
    pub lease_id: Uuid,
    pub expected_lease_version: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RunBinding {
    pub chat_id: Uuid,
    pub run_id: Uuid,
    pub machine_subject: String,
    pub dispatch_receipt: ArtifactReceipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmitDeliveryProposal {
    pub fence: DeliveryFence,
    pub workflow_receipt: ArtifactReceipt,
    pub result: DeliveryResult,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeliveryProposalReceipt {
    pub proposal_id: Uuid,
    pub task_id: Uuid,
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
    pub agent_id: Uuid,
    pub producer_subject: String,
    pub workflow_receipt: ArtifactReceipt,
    pub result: DeliveryResult,
    pub output_sha256: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmitPmFeedback {
    pub message_id: Uuid,
    pub fence: MachineFence,
    pub text: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PmFeedbackReceipt {
    pub message_id: Uuid,
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub root_task_id: Uuid,
    pub owner_subject: String,
    pub fence: MachineFence,
    pub text: String,
    pub content_hash: String,
    pub requirement_revision: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// Supplied only by the existing Fleet consumer after its actual checkpoint/readback.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PmFeedbackCheckpoint {
    pub fence: MachineFence,
    pub checkpoint_id: Uuid,
    pub checkpoint_version: i64,
    pub previous_run_id: Uuid,
    pub previous_run_terminal: ArtifactReceipt,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PmFeedbackResumeContract {
    pub feedback_message_id: Uuid,
    pub fence: MachineFence,
    pub checkpoint_id: Uuid,
    pub checkpoint_version: i64,
    pub previous_run_id: Uuid,
    pub lease_id: Uuid,
    pub lease_version: i64,
    pub resume_operation_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TerminalReceipt {
    pub chat_id: Uuid,
    pub run_id: Uuid,
    pub fleet_receipt: ArtifactReceipt,
    pub workflow_receipt: ArtifactReceipt,
    pub output_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum LifecycleCommand {
    Reconcile {
        fence: DeliveryFence,
        terminal: RuntimeTerminalObservation,
        recovery: RecoveryDisposition,
        idempotency_key: String,
    },
    EnsureQueued {
        idempotency_key: String,
    },
    Reserve {
        expected_task_version: i64,
        idempotency_key: String,
    },
    Heartbeat {
        fence: DeliveryFence,
        idempotency_key: String,
    },
    BindRun {
        fence: DeliveryFence,
        binding: RunBinding,
        idempotency_key: String,
    },
    Complete {
        fence: DeliveryFence,
        terminal: TerminalReceipt,
        result: DeliveryResult,
        idempotency_key: String,
    },
}

impl LifecycleCommand {
    pub fn key(&self) -> &str {
        match self {
            Self::Reconcile {
                idempotency_key, ..
            } => idempotency_key,
            Self::EnsureQueued { idempotency_key }
            | Self::Reserve {
                idempotency_key, ..
            }
            | Self::Heartbeat {
                idempotency_key, ..
            }
            | Self::BindRun {
                idempotency_key, ..
            }
            | Self::Complete {
                idempotency_key, ..
            } => idempotency_key,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AcceptedDeliveryResult {
    pub result_id: Uuid,
    pub assignment_id: Uuid,
    pub task_id: Uuid,
    pub terminal: TerminalReceipt,
    pub output: DeliveryResult,
    pub previous_stage: LifecycleStage,
    pub next_stage: LifecycleStage,
    pub next_status: LifecycleStatus,
    pub cycle: u8,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "source", content = "receipt", rename_all = "snake_case")]
pub enum DeliveryInput {
    Delivery(AcceptedDeliveryResult),
    LegacyAnalysis(crate::sdlc_reservation::AnalysisResultReceipt),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LifecycleReceipt {
    pub task: LifecycleTask,
    pub assignment: Option<DeliveryAssignment>,
    pub lease: Option<ExecutionLease>,
    pub binding: Option<RunBinding>,
    pub result: Option<AcceptedDeliveryResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LifecycleReadback {
    #[serde(serialize_with = "crate::sdlc_execution_lease::utc_nanos")]
    pub observed_at: DateTime<Utc>,
    pub receipt: LifecycleReceipt,
    pub runtime_state: RuntimeState,
}

#[async_trait::async_trait]
pub trait LifecycleRepository: Send + Sync {
    async fn submit_pm_feedback(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SubmitPmFeedback,
    ) -> Result<(PmFeedbackReceipt, bool), AppError>;

    async fn pm_feedback_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<PmFeedbackReceipt, AppError>;

    async fn submit_delivery_proposal(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SubmitDeliveryProposal,
    ) -> Result<(DeliveryProposalReceipt, bool), AppError>;

    async fn delivery_proposal_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<DeliveryProposalReceipt, AppError>;

    async fn lifecycle_result(
        &self,
        task: Uuid,
        actor: &Principal,
        result_id: Uuid,
    ) -> Result<DeliveryInput, AppError>;

    async fn lifecycle_command(
        &self,
        task: Uuid,
        actor: &Principal,
        command: LifecycleCommand,
    ) -> Result<(LifecycleReceipt, bool), AppError>;

    async fn lifecycle_read(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<LifecycleReadback, AppError>;

    async fn lifecycle_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<LifecycleReceipt, AppError>;
}
