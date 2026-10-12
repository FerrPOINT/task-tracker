use crate::sdlc::{MachineFence, Stage};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const METADATA_MAX_BYTES: usize = 1_048_576;
pub const METADATA_DEFAULT_BYTES: usize = 262_144;
pub const METADATA_MIN_BYTES: usize = 1024;

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataPayload<R> {
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    pub root_task_id: Uuid,
    pub owner_subject: String,
    pub stage: Stage,
    #[schema(
        required = true,
        nullable = true,
        minimum = 1,
        maximum = 9007199254740991i64
    )]
    pub current_requirement_revision: Option<i64>,
    #[schema(inline)]
    pub resource: R,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataInputRef {
    pub snapshot_ref: Uuid,
    #[schema(pattern = "^[0-9a-f]{64}$")]
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatedResource {
    #[schema(required = true, nullable = true)]
    pub input: Option<MetadataInputRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BoundResource {}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum PublishedQuestionState {
    Open,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CancelledQuestionState {
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct QuestionResource<S> {
    pub question_id: Uuid,
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub question_version: i64,
    pub request_id: Uuid,
    pub checkpoint_id: Uuid,
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub requirement_revision: i64,
    #[schema(inline)]
    pub state: S,
    pub fence: MachineFence,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AnswerResource {
    pub answer_id: Uuid,
    pub question_id: Uuid,
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub question_version: i64,
    pub request_id: Uuid,
    pub checkpoint_id: Uuid,
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub requirement_revision: i64,
    pub fence: MachineFence,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RevisionResource {
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub requirement_revision: i64,
    #[schema(pattern = "^[0-9a-f]{64}$")]
    pub content_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceResource {
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub requirement_revision: i64,
    #[schema(pattern = "^[0-9a-f]{64}$")]
    pub content_hash: String,
    #[schema(pattern = "^[0-9a-f]{64}$")]
    pub check_id_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfirmationResource {
    pub confirmation_id: Uuid,
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub requirement_revision: i64,
    #[schema(pattern = "^[0-9a-f]{64}$")]
    pub content_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AnalysisReservationResource {
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
    pub workflow_task_ref: String,
    pub intent_id: Uuid,
    pub routing_snapshot_id: Uuid,
    pub agent_id: Uuid,
    pub fencing_token: i64,
    pub assignment_hash: String,
}

macro_rules! metadata_events {
    ($($variant:ident, $name:literal, $resource:ty);+ $(;)?) => {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        #[serde(tag = "event_type", deny_unknown_fields)]
        pub enum MetadataEvent {
            $(#[serde(rename = $name)]
            $variant {
                sequence: String,
                event_id: Uuid,
                task_id: Uuid,
                created_at: String,
                metadata_sha256: String,
                payload: MetadataPayload<$resource>,
            }),+
        }
        impl MetadataEvent {
            pub fn sequence(&self) -> &str {
                match self { $(Self::$variant { sequence, .. } => sequence),+ }
            }
        }
        impl utoipa::ToSchema for MetadataEvent {
            fn schemas(schemas: &mut Vec<(String, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>)>) {
                $(<$resource as utoipa::ToSchema>::schemas(schemas);)+
            }
        }
        impl utoipa::PartialSchema for MetadataEvent {
            fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
                use utoipa::openapi::{RefOr, schema::{Schema, ObjectBuilder, OneOfBuilder}};
                let mut union = OneOfBuilder::new();
                $(let RefOr::T(Schema::Object(mut object)) =
                    <MetadataEventFields<$resource> as utoipa::PartialSchema>::schema()
                    else { unreachable!("metadata event fields are an object") };
                  object.properties.insert("event_type".into(), ObjectBuilder::new()
                    .schema_type(utoipa::openapi::schema::Type::String)
                    .enum_values(Some([$name])).into());
                  object.required.push("event_type".into());
                  union = union.item(object);)+
                union.into()
            }
        }
    };
}

// Enum derive does not preserve strict keys or macro variant wire names in OpenAPI.
#[derive(utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct MetadataEventFields<R> {
    #[schema(pattern = "^[1-9][0-9]{0,18}$")]
    sequence: String,
    event_id: Uuid,
    task_id: Uuid,
    #[schema(format = DateTime)]
    created_at: String,
    #[schema(pattern = "^[0-9a-f]{64}$")]
    metadata_sha256: String,
    #[schema(inline)]
    payload: MetadataPayload<R>,
}

metadata_events! {
    TaskCreated, "task.created", CreatedResource;
    TaskBound, "task.bound", BoundResource;
    PmAssigned, "pm.assigned", MachineFence;
    ClarificationPublished, "clarification.published", QuestionResource<PublishedQuestionState>;
    ClarificationCancelled, "clarification.cancelled", QuestionResource<CancelledQuestionState>;
    ClarificationAnswered, "clarification.answered", AnswerResource;
    RequirementsPublished, "requirements.published", RevisionResource;
    EvidenceRecorded, "requirements.evidence_recorded", EvidenceResource;
    RequirementsConfirmed, "requirements.confirmed", ConfirmationResource;
    AnalysisIntentCreated, "analysis.intent_created", crate::sdlc::AnalysisIntent;
    AnalysisAssignmentReserved, "analysis.assignment_reserved", AnalysisReservationResource;
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub enum MetadataProjection {
    #[serde(rename = "metadata_v1")]
    MetadataV1,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataPage {
    #[schema(minimum = 1, maximum = 1)]
    pub contract_version: u8,
    pub projection: MetadataProjection,
    #[schema(pattern = "^(0|[1-9][0-9]{0,18})$")]
    pub after: String,
    #[schema(pattern = "^(0|[1-9][0-9]{0,18})$")]
    pub next_after: String,
    pub has_more: bool,
    pub events: Vec<MetadataEvent>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetadataErrorCode {
    MetadataBudgetTooSmall,
    MetadataEventUnrepresentable,
    MetadataSourceInvalid,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataError {
    #[schema(minimum = 1, maximum = 1)]
    pub contract_version: u8,
    pub projection: MetadataProjection,
    pub code: MetadataErrorCode,
    pub after: String,
    pub blocked_sequence: String,
    pub event_id: Uuid,
    #[schema(required = true, nullable = true)]
    pub required_bytes: Option<u64>,
    pub max_bytes: u32,
}

pub struct MetadataCandidate {
    pub sequence: i64,
    pub event_id: Uuid,
    pub event: Result<MetadataEvent, MetadataErrorCode>,
}
