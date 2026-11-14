//! Single-task delivery policy. Historical PM/Analysis wire types stay unchanged.
use crate::sdlc::{MachineFence, Principal, Stage, safe_version};
use crate::sdlc_routing::TaskRoutingSnapshot;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::AppError;
use uuid::Uuid;

pub const POOL_LIMIT: u8 = 2;
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
    pub const BOARD_ORDER: [Self; 9] = [
        Self::Draft,
        Self::Backlog,
        Self::Analysis,
        Self::Architecture,
        Self::Development,
        Self::Review,
        Self::Testing,
        Self::Rework,
        Self::Deployment,
    ];

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

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SdlcBoardTask {
    pub issue_id: Uuid,
    /// A persisted process exists; this grants no access to private SDLC context.
    pub enrolled: bool,
    pub stage: LifecycleStage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<LifecycleStatus>,
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
    /// Retained automatic-retry metadata; current recovery never increments it.
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
    /// Historical expiry recovery; only original operation-key replay is supported.
    RetryUnsubmitted,
    /// Historical automatic recovery; only original operation-key replay is supported.
    RetryTransient {
        replay_safety_receipt: ArtifactReceipt,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NeverSubmittedObservation {
    pub assignment_hash: String,
    pub reservation_key: String,
    pub reservation_payload_sha256: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_subject: Option<String>,
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
    StartAnalysis {
        pm_completion_id: Uuid,
        idempotency_key: String,
    },
    Reconcile {
        fence: DeliveryFence,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        terminal: Option<RuntimeTerminalObservation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        never_submitted: Option<NeverSubmittedObservation>,
        recovery: RecoveryDisposition,
        idempotency_key: String,
    },
    EnsureQueued {
        idempotency_key: String,
    },
    ResumeWithCurrentRouting {
        #[serde(deserialize_with = "safe_version")]
        #[schema(minimum = 1, maximum = 9007199254740991i64)]
        expected_task_version: i64,
        #[serde(deserialize_with = "safe_version")]
        #[schema(minimum = 1, maximum = 9007199254740991i64)]
        expected_routing_policy_version: i64,
        idempotency_key: String,
    },
    Reserve {
        expected_task_version: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        machine_subject: Option<String>,
        idempotency_key: String,
    },
    /// Historical operation decoding/replay only; new renewals are rejected.
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
            Self::StartAnalysis {
                idempotency_key, ..
            } => idempotency_key,
            Self::Reconcile {
                idempotency_key, ..
            } => idempotency_key,
            Self::ResumeWithCurrentRouting { idempotency_key, .. }
            | Self::EnsureQueued { idempotency_key }
            | Self::Reserve {
                idempotency_key, ..
            }
            | Self::Heartbeat { idempotency_key, .. }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_result_id: Option<Uuid>,
    pub terminal: TerminalReceipt,
    pub output: DeliveryResult,
    pub previous_stage: LifecycleStage,
    pub next_stage: LifecycleStage,
    pub next_status: LifecycleStatus,
    pub cycle: u8,
    pub created_at: DateTime<Utc>,
}

/// Read projection of immutable accepted output and its original assignment input.
/// Mutation receipts keep the original stored AcceptedDeliveryResult shape.
#[derive(Clone, Debug, PartialEq, utoipa::ToSchema)]
pub struct AcceptedDeliveryInput {
    #[serde(flatten)]
    pub result: AcceptedDeliveryResult,
    #[schema(required = true, nullable = true)]
    pub previous_result_id: Option<Uuid>,
}

impl Serialize for AcceptedDeliveryInput {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut value = serde_json::to_value(&self.result).map_err(serde::ser::Error::custom)?;
        value["previous_result_id"] = serde_json::to_value(self.previous_result_id)
            .map_err(serde::ser::Error::custom)?;
        value.serialize(serializer)
    }
}

#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
#[serde(tag = "source", content = "receipt", rename_all = "snake_case")]
pub enum DeliveryInput {
    Delivery(AcceptedDeliveryInput),
    LegacyAnalysis(crate::sdlc_reservation::AnalysisResultReceipt),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LifecycleReceipt {
    pub task: LifecycleTask,
    pub assignment: Option<DeliveryAssignment>,
    pub lease: Option<DeliveryBinding>,
    pub binding: Option<RunBinding>,
    pub result: Option<AcceptedDeliveryResult>,
}

/// Current assignment fencing; retained timestamps are archival data, not authority.
#[derive(Clone, Debug, PartialEq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeliveryBinding {
    pub lease_id: Uuid,
    pub version: i64,
    pub holder_subject: String,
    pub claimed_at: DateTime<Utc>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub heartbeat_at: Option<DateTime<Utc>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<DateTime<Utc>>,
}

impl Serialize for DeliveryBinding {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let timestamp = |value: DateTime<Utc>| value.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        let mut value = serde_json::json!({"lease_id":self.lease_id,"version":self.version,
            "holder_subject":self.holder_subject,"claimed_at":timestamp(self.claimed_at)});
        if let Some(heartbeat) = self.heartbeat_at {
            value["heartbeat_at"] = serde_json::json!(timestamp(heartbeat));
            value["expires_at"] = serde_json::json!(self.expires_at.map(timestamp));
        }
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DeliveryBinding {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Retained {
            lease_id: Uuid,
            version: i64,
            holder_subject: String,
            claimed_at: DateTime<Utc>,
            #[serde(default)]
            heartbeat_at: Option<DateTime<Utc>>,
            #[serde(default)]
            expires_at: Option<DateTime<Utc>>,
        }
        let encoded = serde_json::Value::deserialize(deserializer)?;
        let explicit_expiration = encoded.get("expires_at").is_some();
        let value: Retained = serde_json::from_value(encoded).map_err(serde::de::Error::custom)?;
        if value.lease_id.is_nil()
            || !(1..=crate::sdlc::MAX_SAFE_VERSION).contains(&value.version)
            || value.holder_subject.trim().is_empty()
            || (value.heartbeat_at.is_some() && !explicit_expiration)
            || (value.heartbeat_at.is_none() && value.expires_at.is_some())
        {
            return Err(serde::de::Error::custom("invalid delivery binding"));
        }
        Ok(Self {
            lease_id: value.lease_id,
            version: value.version,
            holder_subject: value.holder_subject,
            claimed_at: value.claimed_at,
            heartbeat_at: value.heartbeat_at,
            expires_at: value.expires_at,
        })
    }
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
    async fn reserve_critical_pm_review(
        &self,
        _task: Uuid,
        _actor: &Principal,
        _command: crate::sdlc_critical_pm_reservation::ReserveCriticalPmReview,
    ) -> Result<
        (
            crate::sdlc_critical_pm_reservation::CriticalPmReservationResult,
            bool,
        ),
        AppError,
    > {
        Err(AppError::Unavailable(
            "critical PM review repository not configured".into(),
        ))
    }

    async fn critical_pm_review_operation(
        &self,
        _task: Uuid,
        _actor: &Principal,
        _key: &str,
    ) -> Result<crate::sdlc_critical_pm_reservation::CriticalPmReservationResult, AppError> {
        Err(AppError::Unavailable(
            "critical PM review repository not configured".into(),
        ))
    }

    async fn return_critical_to_pm(
        &self,
        _task: Uuid,
        _actor: &Principal,
        _command: crate::sdlc_critical_blockers::ReturnCriticalToPm,
    ) -> Result<(crate::sdlc_critical_blockers::CriticalPolicyChange, bool), AppError> {
        Err(AppError::Unavailable(
            "critical PM return repository not configured".into(),
        ))
    }

    async fn critical_pm_return_operation(
        &self,
        _task: Uuid,
        _actor: &Principal,
        _key: &str,
    ) -> Result<crate::sdlc_critical_blockers::CriticalPolicyChange, AppError> {
        Err(AppError::Unavailable(
            "critical PM return repository not configured".into(),
        ))
    }

    async fn submit_critical_blocker(
        &self,
        _task: Uuid,
        _actor: &Principal,
        _command: crate::sdlc_critical_blockers::SubmitCriticalBlocker,
    ) -> Result<(crate::sdlc_critical_blockers::CriticalBlockerReceipt, bool), AppError> {
        Err(AppError::Unavailable(
            "critical blocker repository not configured".into(),
        ))
    }

    async fn critical_blocker(
        &self,
        _task: Uuid,
        _actor: &Principal,
        _blocker_id: Uuid,
    ) -> Result<crate::sdlc_critical_blockers::CriticalBlockerReceipt, AppError> {
        Err(AppError::Unavailable(
            "critical blocker repository not configured".into(),
        ))
    }

    async fn critical_blocker_operation(
        &self,
        _task: Uuid,
        _actor: &Principal,
        _key: &str,
    ) -> Result<crate::sdlc_critical_blockers::CriticalBlockerReceipt, AppError> {
        Err(AppError::Unavailable(
            "critical blocker repository not configured".into(),
        ))
    }

    async fn delivery_candidates(
        &self,
        _actor: &Principal,
        _query: crate::sdlc_candidates::DeliveryCandidatesQuery,
    ) -> Result<crate::sdlc_candidates::DeliveryCandidates, AppError> {
        Err(AppError::Unavailable(
            "delivery candidate discovery not configured".into(),
        ))
    }

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

    async fn pm_feedback_message(
        &self,
        task: Uuid,
        actor: &Principal,
        message_id: Uuid,
    ) -> Result<PmFeedbackReceipt, AppError>;

    async fn submit_delivery_proposal(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SubmitDeliveryProposal,
    ) -> Result<(DeliveryProposalReceipt, bool), AppError>;

    /// Result-only custody lookup. It never restores an active assignment.
    async fn delivery_proposal_context(
        &self,
        task: Uuid,
        actor: &Principal,
        fence: &DeliveryFence,
    ) -> Result<LifecycleReceipt, AppError>;

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

#[cfg(test)]
mod binding_tests {
    use super::*;
    use serde_json::json;

    fn retained() -> serde_json::Value {
        json!({"lease_id":Uuid::from_u128(1),"version":357,"holder_subject":"scheduler",
            "claimed_at":"2026-11-09T17:00:00.000000000Z",
            "heartbeat_at":"2026-11-09T17:58:21.000000000Z",
            "expires_at":"2026-11-09T17:58:51.000000000Z"})
    }

    #[test]
    fn retained_binding_roundtrips_without_changing_dates_or_identity() {
        let saved = retained();
        let binding: DeliveryBinding = serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(serde_json::to_value(binding).unwrap(), saved);
        let pm: crate::sdlc_execution_lease::ExecutionLease =
            serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(serde_json::to_value(pm).unwrap(), saved);
    }

    #[test]
    fn untimed_binding_omits_archival_dates_and_preserves_the_current_fence() {
        let mut saved = retained();
        let fields = saved.as_object_mut().unwrap();
        fields.remove("heartbeat_at");
        fields.remove("expires_at");
        let binding: DeliveryBinding = serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(binding.version, 357);
        assert!(binding.heartbeat_at.is_none() && binding.expires_at.is_none());
        assert_eq!(serde_json::to_value(binding).unwrap(), saved);
        assert!(
            serde_json::from_value::<crate::sdlc_execution_lease::ExecutionLease>(saved).is_err()
        );
    }

    #[test]
    fn partial_archive_pairs_and_invalid_core_fields_are_rejected() {
        for field in ["heartbeat_at", "expires_at"] {
            let mut saved = retained();
            saved.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<DeliveryBinding>(saved).is_err());
        }
        for (field, value) in [
            ("lease_id", json!(Uuid::nil())),
            ("version", json!(0)),
            ("holder_subject", json!("")),
            ("unexpected", json!(true)),
        ] {
            let mut saved = retained();
            saved[field] = value;
            assert!(serde_json::from_value::<DeliveryBinding>(saved).is_err());
        }
    }

    #[test]
    fn historical_heartbeat_decodes_only_for_original_operation_readback() {
        let saved = json!({
            "operation":"heartbeat","idempotency_key":"retired",
            "fence":{"assignment_id":Uuid::from_u128(2),"execution_id":Uuid::from_u128(3),
                "fencing_token":1,"lease_id":Uuid::from_u128(1),"expected_lease_version":357}
        });
        let command = serde_json::from_value::<LifecycleCommand>(saved.clone()).unwrap();
        assert!(matches!(command, LifecycleCommand::Heartbeat { .. }));
        assert_eq!(command.key(), "retired");
        assert_eq!(serde_json::to_value(command).unwrap(), saved);
    }

    #[test]
    fn retained_permanent_ownership_keeps_its_explicit_null_expiration() {
        let mut saved = retained();
        saved["expires_at"] = serde_json::Value::Null;
        let binding: DeliveryBinding = serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(serde_json::to_value(binding).unwrap(), saved);
    }
}
