//! Durable clarification aggregate and command contract. Identity is always a central subject.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::AppError;
use std::collections::HashSet;
use uuid::Uuid;

pub const MAX_SAFE_VERSION: i64 = 9_007_199_254_740_991;

pub fn safe_version<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    let value = i64::deserialize(deserializer)?;
    if (1..=MAX_SAFE_VERSION).contains(&value) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(
            "version/revision must be in 1..=Number.MAX_SAFE_INTEGER",
        ))
    }
}

pub fn safe_optional_version<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<i64>, D::Error> {
    let value = Option::<i64>::deserialize(deserializer)?;
    if value.is_none_or(|value| (1..=MAX_SAFE_VERSION).contains(&value)) {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(
            "version/revision must be in 1..=Number.MAX_SAFE_INTEGER",
        ))
    }
}

#[derive(Clone, Debug)]
pub struct Principal {
    pub subject: String,
    pub human_session: bool,
    pub scopes: HashSet<String>,
}

#[derive(Clone, Debug)]
pub struct SdlcConfig {
    pub instance_id: String,
    pub orchestrator_subject: String,
    pub verifier_subject: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PmAssignment {
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
    pub agent_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    pub version: i64,
    pub machine_subject: String,
}

impl PmAssignment {
    pub fn scope(&self, task_id: Uuid) -> String {
        format!(
            "task-tracker:sdlc:pm:{task_id}:{}:{}:{}:{}",
            self.assignment_id, self.execution_id, self.agent_id, self.version
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum QuestionMode {
    Single,
    Multiple,
    Text,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum QuestionState {
    Open,
    Answered,
    Superseded,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub enum Stage {
    Draft,
    Clarification,
    Backlog,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct QuestionOption {
    pub id: Uuid,
    pub label: String,
    pub consequences: String,
    #[serde(default)]
    pub is_custom: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RequirementsDocument {
    pub goal: String,
    pub scope: Vec<String>,
    pub exclusions: Vec<String>,
    pub scenarios: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub constraints: Vec<String>,
    pub dependencies: Vec<String>,
    pub assumptions: Vec<String>,
    pub checklist: Vec<String>,
    pub prerequisites: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequirementsRevision {
    #[serde(deserialize_with = "safe_version")]
    pub revision: i64,
    pub content_hash: String,
    #[serde(flatten)]
    pub document: RequirementsDocument,
    pub author_subject: String,
    pub created_at: DateTime<Utc>,
}

impl utoipa::ToSchema for RequirementsRevision {}

impl utoipa::PartialSchema for RequirementsRevision {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        // A strict document allOf would reject revision metadata as extra properties.
        // Merge the two objects so OpenAPI describes the actual flat wire document.
        #[derive(utoipa::ToSchema)]
        #[allow(dead_code)]
        struct Metadata {
            #[schema(minimum = 1, maximum = 9007199254740991i64)]
            revision: i64,
            content_hash: String,
            author_subject: String,
            created_at: DateTime<Utc>,
        }
        use utoipa::openapi::{RefOr, schema::Schema};
        let RefOr::T(Schema::Object(mut document)) =
            <RequirementsDocument as utoipa::PartialSchema>::schema()
        else {
            unreachable!("document schema is an object")
        };
        let RefOr::T(Schema::Object(metadata)) = <Metadata as utoipa::PartialSchema>::schema()
        else {
            unreachable!("metadata schema is an object")
        };
        document.properties.extend(metadata.properties);
        document.required.extend(metadata.required);
        RefOr::T(Schema::Object(document))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AnswerCommand {
    #[serde(deserialize_with = "safe_version")]
    pub expected_question_version: i64,
    #[serde(deserialize_with = "safe_version")]
    pub requirement_revision: i64,
    pub selected_option_ids: Vec<Uuid>,
    pub text: Option<String>,
    pub comment: Option<String>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfirmCommand {
    pub content_hash: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MachineFence {
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
    pub agent_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    pub assignment_version: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PublishQuestion {
    pub fence: MachineFence,
    pub request_id: Uuid,
    pub question_id: Uuid,
    #[serde(default, deserialize_with = "safe_optional_version")]
    pub expected_question_version: Option<i64>,
    #[serde(deserialize_with = "safe_version")]
    pub requirement_revision: i64,
    pub checkpoint_id: Uuid,
    pub requirement_reference: Option<String>,
    pub text: String,
    pub rationale: String,
    pub required: bool,
    pub mode: QuestionMode,
    pub options: Vec<QuestionOption>,
    pub recommended_option_id: Option<Uuid>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PublishRevision {
    pub fence: MachineFence,
    #[serde(default, deserialize_with = "safe_optional_version")]
    pub expected_requirement_revision: Option<i64>,
    pub document: RequirementsDocument,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AssignCommand {
    pub assignment: PmAssignment,
    #[serde(default, deserialize_with = "safe_optional_version")]
    pub expected_assignment_version: Option<i64>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BindCommand {
    pub root_task_id: Uuid,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateDraftCommand {
    #[schema(min_length = 1, max_length = 500)]
    pub title: String,
    #[schema(max_length = 100000)]
    pub description: String,
    #[schema(min_length = 1, max_length = 128)]
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq, Eq)]
pub enum DraftStage {
    Draft,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreatedDraft {
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub root_task_id: Uuid,
    pub task_key: String,
    pub owner_subject: String,
    pub stage: DraftStage,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PmDraftInput {
    pub snapshot_ref: Uuid,
    #[schema(min_length = 1, max_length = 500)]
    pub title: String,
    #[schema(max_length = 100000)]
    pub description: String,
    #[schema(pattern = "^[0-9a-f]{64}$")]
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PmDraftInputResponse {
    #[schema(minimum = 1, maximum = 1)]
    pub contract_version: u8,
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub root_task_id: Uuid,
    pub owner_subject: String,
    pub input: PmDraftInput,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceCommand {
    pub fence: MachineFence,
    #[serde(deserialize_with = "safe_version")]
    pub requirement_revision: i64,
    pub content_hash: String,
    pub check_id: String,
    pub evidence_reference: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CancelQuestion {
    pub fence: MachineFence,
    #[serde(deserialize_with = "safe_version")]
    pub expected_question_version: i64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Answer {
    pub id: Uuid,
    pub question_id: Uuid,
    pub question_version: i64,
    pub requirement_revision: i64,
    pub selected_option_ids: Vec<Uuid>,
    pub text: Option<String>,
    pub comment: Option<String>,
    pub author_subject: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Question {
    pub id: Uuid,
    pub request_id: Uuid,
    pub task_id: Uuid,
    pub root_task_id: Uuid,
    pub version: i64,
    pub requirement_revision: i64,
    pub requirement_reference: Option<String>,
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
    pub agent_id: Uuid,
    pub assignment_version: i64,
    pub checkpoint_id: Uuid,
    pub text: String,
    pub rationale: String,
    pub required: bool,
    pub mode: QuestionMode,
    pub options: Vec<QuestionOption>,
    pub recommended_option_id: Option<Uuid>,
    pub state: QuestionState,
    pub answer: Option<Answer>,
    pub author_subject: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Confirmation {
    pub id: Uuid,
    pub task_id: Uuid,
    pub revision: i64,
    pub content_hash: String,
    pub owner_subject: String,
    pub created_at: DateTime<Utc>,
    pub stage: Stage,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Evidence {
    pub requirement_revision: i64,
    pub content_hash: String,
    pub check_id: String,
    pub evidence_reference: String,
    pub verifier_subject: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Permissions {
    pub can_answer: bool,
    pub can_confirm: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SdlcContext {
    pub contract_version: u8,
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub root_task_id: Uuid,
    pub owner_subject: String,
    pub stage: Stage,
    pub requirement_revision: Option<i64>,
    pub waiting_reason: Option<String>,
    pub permissions: Permissions,
    pub assignment: Option<PmAssignment>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskState {
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub root_task_id: Uuid,
    pub owner_subject: String,
    pub stage: Stage,
    pub confirmation_revision: Option<i64>,
    pub assignment: Option<PmAssignment>,
    pub questions: Vec<Question>,
    pub revisions: Vec<RequirementsRevision>,
    pub confirmations: Vec<Confirmation>,
    pub evidence: Vec<Evidence>,
}

impl TaskState {
    pub fn current_revision(&self) -> Option<i64> {
        self.revisions.last().map(|r| r.revision)
    }
    pub fn owner(&self, actor: &Principal) -> bool {
        actor.human_session && actor.subject == self.owner_subject
    }
    pub fn ready(&self) -> bool {
        let Some(revision) = self.revisions.last() else {
            return false;
        };
        !matches!(self.stage, Stage::Backlog)
            && self.confirmation_revision == Some(revision.revision)
            && !self
                .questions
                .iter()
                .any(|q| q.required && q.state == QuestionState::Open)
            && revision
                .document
                .checklist
                .iter()
                .chain(&revision.document.prerequisites)
                .all(|check| {
                    self.evidence.iter().any(|e| {
                        e.requirement_revision == revision.revision
                            && e.content_hash == revision.content_hash
                            && &e.check_id == check
                    })
                })
    }
    pub fn context(&self, actor: &Principal) -> SdlcContext {
        let can_answer = self.owner(actor)
            && self
                .questions
                .iter()
                .any(|q| q.state == QuestionState::Open);
        SdlcContext {
            contract_version: 1,
            tracker_instance_id: self.tracker_instance_id.clone(),
            project_id: self.project_id,
            task_id: self.task_id,
            root_task_id: self.root_task_id,
            owner_subject: self.owner_subject.clone(),
            stage: self.stage.clone(),
            requirement_revision: self.current_revision(),
            waiting_reason: match self.stage {
                Stage::Draft => Some("waiting_for_pm".into()),
                Stage::Backlog => None,
                Stage::Clarification
                    if self
                        .questions
                        .iter()
                        .any(|q| q.state == QuestionState::Open) =>
                {
                    Some("waiting_for_owner_answers".into())
                }
                Stage::Clarification if !self.ready() => {
                    Some("waiting_for_requirements_or_evidence".into())
                }
                Stage::Clarification => Some("waiting_for_owner_confirmation".into()),
            },
            permissions: Permissions {
                can_answer,
                can_confirm: self.owner(actor) && self.ready(),
            },
            assignment: self.assignment.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", content = "payload", rename_all = "snake_case")]
pub enum SdlcCommand {
    Assign(AssignCommand),
    PublishQuestion(PublishQuestion),
    PublishRevision(PublishRevision),
    Answer {
        question_id: Uuid,
        command: AnswerCommand,
    },
    Confirm {
        revision: i64,
        command: ConfirmCommand,
    },
    Evidence(EvidenceCommand),
    Cancel {
        question_id: Uuid,
        command: CancelQuestion,
    },
}

impl SdlcCommand {
    pub fn key(&self) -> &str {
        match self {
            Self::Assign(c) => &c.idempotency_key,
            Self::PublishQuestion(c) => &c.idempotency_key,
            Self::PublishRevision(c) => &c.idempotency_key,
            Self::Answer { command, .. } => &command.idempotency_key,
            Self::Confirm { command, .. } => &command.idempotency_key,
            Self::Evidence(c) => &c.idempotency_key,
            Self::Cancel { command, .. } => &command.idempotency_key,
        }
    }
}

#[async_trait]
pub trait SdlcRepository: Send + Sync {
    async fn claim_execution_lease(
        &self,
        task: Uuid,
        actor: &Principal,
        command: crate::sdlc_execution_lease::ClaimExecutionLease,
    ) -> Result<(crate::sdlc_execution_lease::ExecutionLeaseReceipt, bool), AppError>;
    async fn heartbeat_execution_lease(
        &self,
        task: Uuid,
        actor: &Principal,
        command: crate::sdlc_execution_lease::HeartbeatExecutionLease,
    ) -> Result<(crate::sdlc_execution_lease::ExecutionLeaseReceipt, bool), AppError>;
    async fn execution_lease(
        &self,
        task: Uuid,
        actor: &Principal,
        key: Option<&str>,
    ) -> Result<crate::sdlc_execution_lease::ExecutionLeaseReadback, AppError>;
    async fn reserve_pm_draft(
        &self,
        task: Uuid,
        actor: &Principal,
        command: crate::sdlc_pm_draft::ReservePmDraft,
    ) -> Result<(crate::sdlc_pm_draft::PmDraftReservation, bool), AppError>;
    async fn pm_draft_assignment(
        &self,
        task: Uuid,
        actor: &Principal,
        key: Option<&str>,
    ) -> Result<crate::sdlc_pm_draft::PmDraftReadback, AppError>;
    async fn project_access_scope(&self, actor: &Principal) -> Result<ProjectAccess, AppError>;
    async fn project_directory(
        &self,
        actor: &Principal,
        after: Option<Uuid>,
        limit: u16,
    ) -> Result<ProjectDirectory, AppError>;
    async fn create_draft(
        &self,
        project: Uuid,
        actor: &Principal,
        command: CreateDraftCommand,
    ) -> Result<(CreatedDraft, bool), AppError>;
    async fn draft_creation_operation(
        &self,
        project: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<CreatedDraft, AppError>;
    async fn read(&self, task: Uuid, actor: &Principal) -> Result<TaskState, AppError>;
    async fn pm_draft_input(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<PmDraftInputResponse, AppError>;
    async fn bind(
        &self,
        task: Uuid,
        actor: &Principal,
        command: BindCommand,
    ) -> Result<TaskState, AppError>;
    async fn execute(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SdlcCommand,
    ) -> Result<serde_json::Value, AppError>;
    async fn outbox(
        &self,
        task: Uuid,
        actor: &Principal,
        after: i64,
    ) -> Result<Vec<OutboxEvent>, AppError>;
    async fn metadata_outbox(
        &self,
        task: Uuid,
        actor: &Principal,
        after: i64,
        limit: u16,
    ) -> Result<Vec<crate::sdlc_metadata::MetadataCandidate>, AppError>;
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectAccess {
    pub contract_version: u32,
    pub tracker_instance_id: String,
    pub project_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectDirectoryEntry {
    pub id: Uuid,
    pub key: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectDirectory {
    #[schema(minimum = 1, maximum = 1)]
    pub contract_version: u32,
    pub tracker_instance_id: String,
    pub projects: Vec<ProjectDirectoryEntry>,
    #[serde(deserialize_with = "directory_cursor")]
    #[schema(required = true, nullable = true)]
    pub next_cursor: Option<Uuid>,
}

fn directory_cursor<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Uuid>, D::Error> {
    Option::<Uuid>::deserialize(deserializer)
}

impl ProjectDirectory {
    pub fn from_prefix(
        tracker_instance_id: String,
        mut projects: Vec<ProjectDirectoryEntry>,
        after: Option<Uuid>,
        limit: u16,
    ) -> Result<Self, AppError> {
        if !(1..=100).contains(&limit) || after.is_some_and(|id| id.is_nil()) {
            return Err(AppError::validation("invalid project directory query"));
        }
        // Check the lookahead too: a corrupt source row must not become an unseen cursor gap.
        if projects.len() > usize::from(limit) + 1
            || projects.iter().any(|p| p.id.is_nil())
            || projects.windows(2).any(|pair| pair[0].id >= pair[1].id)
            || projects
                .first()
                .is_some_and(|p| after.is_some_and(|id| p.id <= id))
        {
            return Err(AppError::conflict("project directory source invalid"));
        }
        let more = projects.len() > usize::from(limit);
        projects.truncate(usize::from(limit));
        let next_cursor = more.then(|| projects.last().expect("positive page limit").id);
        Ok(Self {
            contract_version: 1,
            tracker_instance_id,
            projects,
            next_cursor,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OutboxEvent {
    pub sequence: i64,
    pub event_id: Uuid,
    pub task_id: Uuid,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod project_directory_tests {
    use super::*;
    use serde_json::json;

    fn entry(n: u128) -> ProjectDirectoryEntry {
        ProjectDirectoryEntry {
            id: Uuid::from_u128(n),
            key: format!("P{n}"),
            name: format!("Project {n}"),
        }
    }

    #[test]
    fn directory_prefix_uses_lookahead_and_required_null_without_other_metadata() {
        for (rows, limit, expected_len, cursor) in [
            (0, 1, 0, None),
            (1, 1, 1, None),
            (2, 1, 1, Some(1)),
            (2, 2, 2, None),
            (3, 2, 2, Some(2)),
            (100, 100, 100, None),
            (101, 100, 100, Some(100)),
        ] {
            let page = ProjectDirectory::from_prefix(
                "tracker".into(),
                (1..=rows).map(entry).collect(),
                None,
                limit,
            )
            .unwrap();
            assert_eq!(page.projects.len(), expected_len);
            assert_eq!(page.next_cursor, cursor.map(Uuid::from_u128));
            let wire = serde_json::to_value(&page).unwrap();
            assert_eq!(wire.as_object().unwrap().len(), 4);
            assert!(wire.get("next_cursor").is_some());
            for project in wire["projects"].as_array().unwrap() {
                assert_eq!(project.as_object().unwrap().len(), 3);
            }
            serde_json::from_value::<ProjectDirectory>(wire.clone()).unwrap();
            let mut missing = wire;
            missing.as_object_mut().unwrap().remove("next_cursor");
            assert!(serde_json::from_value::<ProjectDirectory>(missing).is_err());
        }
        let mut wire = serde_json::to_value(
            ProjectDirectory::from_prefix("tracker".into(), vec![entry(1)], None, 1).unwrap(),
        )
        .unwrap();
        wire["count"] = json!(1);
        assert!(serde_json::from_value::<ProjectDirectory>(wire).is_err());
        let mut wire = serde_json::to_value(entry(1)).unwrap();
        wire["description"] = json!("not exposed");
        assert!(serde_json::from_value::<ProjectDirectoryEntry>(wire).is_err());
    }

    #[test]
    fn directory_rejects_invalid_bounds_and_corrupt_or_noncontiguous_prefix() {
        for limit in [0, 101, u16::MAX] {
            assert!(ProjectDirectory::from_prefix("tracker".into(), vec![], None, limit).is_err());
        }
        assert!(
            ProjectDirectory::from_prefix("tracker".into(), vec![], Some(Uuid::nil()), 1).is_err()
        );
        for rows in [
            vec![entry(0)],
            vec![entry(1), entry(1)],
            vec![entry(2), entry(1)],
            vec![entry(1), entry(2), entry(3)],
        ] {
            assert!(ProjectDirectory::from_prefix("tracker".into(), rows, None, 1).is_err());
        }
        assert!(
            ProjectDirectory::from_prefix(
                "tracker".into(),
                vec![entry(1)],
                Some(Uuid::from_u128(1)),
                1
            )
            .is_err()
        );
        assert!(
            ProjectDirectory::from_prefix(
                "tracker".into(),
                vec![entry(2)],
                Some(Uuid::from_u128(1)),
                1
            )
            .is_ok()
        );
    }
}
