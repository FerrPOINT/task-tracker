//! Owner-policy source identity with a server-allocated ref, not a Git write permit.
use crate::sdlc::{Principal, safe_optional_version};
use crate::sdlc_lifecycle::LifecycleReadback;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::{AppError, resource_context::NamespaceRef};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SourcePolicySpec {
    pub forge_instance_id: Uuid,
    pub repository_id: Uuid,
    pub base_ref: String,
    pub base_commit: String,
}

/// Read only from the configured Forge namespace reader, never accepted as HTTP input.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VerifiedSource {
    pub namespace: NamespaceRef,
    pub forge_instance_id: Uuid,
    pub repository_id: Uuid,
    pub public_name: String,
    pub base_ref: String,
    pub base_commit: String,
    pub default_ref: String,
    pub protected_refs: Vec<String>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSourcePolicy {
    #[serde(deserialize_with = "safe_optional_version")]
    #[schema(
        required = true,
        nullable = true,
        minimum = 1,
        maximum = 9007199254740991i64
    )]
    pub expected_source_revision: Option<i64>,
    pub source: SourcePolicySpec,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectSourcePolicy {
    pub contract_version: u8,
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    pub source_revision: i64,
    pub source_hash: String,
    pub source: SourcePolicySpec,
    pub verification: VerifiedSource,
    pub author_subject: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskSourceBinding {
    pub contract_version: u8,
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub root_task_id: Uuid,
    pub confirmation_id: Uuid,
    pub requirement_revision: i64,
    pub content_hash: String,
    pub routing_snapshot_id: Uuid,
    pub namespace: NamespaceRef,
    pub forge_instance_id: Uuid,
    pub repository_id: Uuid,
    pub base_ref: String,
    pub base_commit: String,
    pub candidate_ref: String,
    pub source_revision: i64,
    pub policy_hash: String,
    pub source_hash: String,
    pub owner_subject: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceBlockReason {
    MissingBinding,
    MissingProjectSourcePolicy,
    MissingCurrentExecution,
    SourceContextChanged,
    ExecutionNotRunning,
    LeaseExpired,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum TaskSourceReadback {
    Blocked {
        reason: SourceBlockReason,
        binding: Option<TaskSourceBinding>,
        current: Option<LifecycleReadback>,
    },
    Bound {
        binding: TaskSourceBinding,
        current: LifecycleReadback,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EnrollDraftSource {
    pub expected_preflight_sha256: String,
    pub source_revision: i64,
    pub policy_hash: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftSourceEnrollmentPreflight {
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub state_sha256: String,
    pub selection_sha256: String,
    pub policy: ProjectSourcePolicy,
    pub preflight_sha256: String,
}

/// Metadata-only receipt. It is neither a published source binding nor a runtime permit.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftSourceEnrollmentReceipt {
    pub enrollment_id: Uuid,
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub owner_subject: String,
    pub source_revision: i64,
    pub policy_hash: String,
    pub request_sha256: String,
    pub source_hash: String,
    pub created_at: DateTime<Utc>,
}

#[async_trait::async_trait]
pub trait TaskSourceRepository: Send + Sync {
    async fn draft_source_preflight(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<DraftSourceEnrollmentPreflight, AppError>;
    async fn enroll_draft_source(
        &self,
        task: Uuid,
        actor: &Principal,
        command: EnrollDraftSource,
    ) -> Result<(DraftSourceEnrollmentReceipt, bool), AppError>;
    async fn source_policy_namespace(
        &self,
        project: Uuid,
        actor: &Principal,
    ) -> Result<NamespaceRef, AppError>;
    async fn set_source_policy(
        &self,
        project: Uuid,
        actor: &Principal,
        command: SetSourcePolicy,
        verified: VerifiedSource,
    ) -> Result<(ProjectSourcePolicy, bool), AppError>;
    async fn source_policy(
        &self,
        project: Uuid,
        actor: &Principal,
        revision: Option<i64>,
    ) -> Result<ProjectSourcePolicy, AppError>;
    async fn source_policy_operation(
        &self,
        project: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<ProjectSourcePolicy, AppError>;
    async fn task_source(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<TaskSourceReadback, AppError>;
}
