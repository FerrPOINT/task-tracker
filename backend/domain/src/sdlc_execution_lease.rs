use crate::sdlc::{MachineFence, safe_optional_version, safe_version};
use crate::sdlc_lifecycle::ArtifactReceipt;
use crate::sdlc_pm_draft::PmDraftBinding;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
use uuid::Uuid;

pub const LEASE_TTL_SECONDS: i64 = 30;
pub const LEASE_HEARTBEAT_SECONDS: i64 = 10;

fn canonical_uuid<'de, D: Deserializer<'de>>(d: D) -> Result<Uuid, D::Error> {
    let raw = String::deserialize(d)?;
    let id = Uuid::parse_str(&raw).map_err(D::Error::custom)?;
    if id.is_nil() || id.to_string() != raw {
        return Err(D::Error::custom("invalid lease reference"));
    }
    Ok(id)
}
fn canonical_fence<'de, D: Deserializer<'de>>(d: D) -> Result<MachineFence, D::Error> {
    let raw = serde_json::Value::deserialize(d)?;
    let fence: MachineFence = serde_json::from_value(raw.clone()).map_err(D::Error::custom)?;
    if [fence.assignment_id, fence.execution_id, fence.agent_id]
        .iter()
        .any(Uuid::is_nil)
        || serde_json::to_value(&fence).map_err(D::Error::custom)? != raw
    {
        return Err(D::Error::custom("invalid lease fence"));
    }
    Ok(fence)
}
pub fn utc_nanos<S: Serializer>(value: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&value.to_rfc3339_opts(SecondsFormat::Nanos, true))
}
fn no_dispatch<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    if bool::deserialize(d)? {
        Err(D::Error::custom(
            "execution lease cannot authorize dispatch",
        ))
    } else {
        Ok(false)
    }
}
fn false_schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
    utoipa::openapi::schema::ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::Boolean)
        .enum_values(Some([false]))
        .into()
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ClaimExecutionLease {
    #[serde(deserialize_with = "safe_version")]
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub expected_owner_version: i64,
    #[serde(deserialize_with = "canonical_fence")]
    pub fence: MachineFence,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct HeartbeatExecutionLease {
    #[serde(deserialize_with = "safe_version")]
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub expected_owner_version: i64,
    #[serde(deserialize_with = "canonical_fence")]
    pub fence: MachineFence,
    #[serde(deserialize_with = "canonical_uuid")]
    pub lease_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub expected_lease_version: i64,
    pub idempotency_key: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, utoipa::ToSchema, PartialEq, Eq)]
#[serde(tag = "input_kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum PmResumeInput {
    Answer {
        #[serde(deserialize_with = "canonical_uuid")]
        input_id: Uuid,
    },
    Feedback {
        #[serde(deserialize_with = "canonical_uuid")]
        input_id: Uuid,
    },
    Confirmation {
        #[serde(deserialize_with = "canonical_uuid")]
        input_id: Uuid,
    },
}
impl PmResumeInput {
    pub fn input_id(self) -> Uuid {
        match self {
            Self::Answer { input_id }
            | Self::Feedback { input_id }
            | Self::Confirmation { input_id } => input_id,
        }
    }
    pub fn kind(self) -> &'static str {
        match self {
            Self::Answer { .. } => "answer",
            Self::Feedback { .. } => "feedback",
            Self::Confirmation { .. } => "confirmation",
        }
    }
}
fn nullable_hash<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PmResumeCheckpoint {
    #[serde(deserialize_with = "canonical_uuid")]
    pub checkpoint_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub checkpoint_revision: i64,
    pub checkpoint_sha256: String,
    #[serde(deserialize_with = "canonical_fence")]
    pub fence: MachineFence,
    #[serde(deserialize_with = "safe_optional_version")]
    #[schema(
        required = true,
        nullable = true,
        minimum = 1,
        maximum = 9007199254740991i64
    )]
    pub requirement_revision: Option<i64>,
    #[serde(deserialize_with = "nullable_hash")]
    #[schema(required = true, nullable = true)]
    pub content_hash: Option<String>,
}

/// Trusted scheduler observation; artifact alone is not independently verified runtime proof.
#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ResumePmExecutionLease {
    #[serde(deserialize_with = "safe_version")]
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub expected_owner_version: i64,
    #[serde(deserialize_with = "canonical_uuid")]
    pub lease_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub expected_lease_version: i64,
    pub input: PmResumeInput,
    pub checkpoint: PmResumeCheckpoint,
    #[serde(deserialize_with = "canonical_uuid")]
    pub previous_run_id: Uuid,
    pub previous_run_terminal: ArtifactReceipt,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExecutionLease {
    pub lease_id: Uuid,
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub version: i64,
    pub holder_subject: String,
    #[serde(serialize_with = "utc_nanos")]
    pub claimed_at: DateTime<Utc>,
    #[serde(serialize_with = "utc_nanos")]
    pub heartbeat_at: DateTime<Utc>,
    #[serde(serialize_with = "utc_nanos")]
    pub expires_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecutionLeaseReceipt {
    #[schema(minimum = 1, maximum = 1)]
    pub contract_version: u8,
    pub binding: PmDraftBinding,
    pub owner_version: i64,
    pub fence: MachineFence,
    pub lease: ExecutionLease,
    #[schema(minimum = 30, maximum = 30)]
    pub ttl_seconds: i64,
    #[schema(minimum = 10, maximum = 10)]
    pub heartbeat_seconds: i64,
    #[serde(deserialize_with = "no_dispatch")]
    #[schema(schema_with = false_schema)]
    pub dispatch_allowed: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecutionLeaseOperation {
    pub idempotency_key: String,
    pub request_sha256: String,
    pub result: ExecutionLeaseReceipt,
}
#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionLeaseState {
    Unclaimed,
    Active,
    Expired,
}
#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecutionLeaseReadback {
    #[schema(minimum = 1, maximum = 1)]
    pub contract_version: u8,
    pub binding: PmDraftBinding,
    pub owner_version: i64,
    pub fence: MachineFence,
    #[serde(serialize_with = "utc_nanos")]
    pub observed_at: DateTime<Utc>,
    pub state: ExecutionLeaseState,
    #[schema(required = true, nullable = true)]
    pub current: Option<ExecutionLease>,
    #[schema(required = true, nullable = true)]
    pub operation: Option<ExecutionLeaseOperation>,
    #[schema(schema_with = false_schema)]
    pub dispatch_allowed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn claim() -> serde_json::Value {
        json!({"expected_owner_version":1,"fence":{
            "assignment_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            "execution_id":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            "agent_id":"cccccccc-cccc-4ccc-8ccc-cccccccccccc","assignment_version":1},
            "idempotency_key":"claim"})
    }
    #[test]
    fn lease_commands_are_strict_canonical_and_positive_safe_cas() {
        assert!(serde_json::from_value::<ClaimExecutionLease>(claim()).is_ok());
        for field in ["expected_owner_version", "fence", "idempotency_key"] {
            let mut c = claim();
            c.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<ClaimExecutionLease>(c).is_err());
        }
        for n in [
            json!(0),
            json!(-1),
            json!(9007199254740992i64),
            json!(1.0),
            json!("1"),
        ] {
            let mut c = claim();
            c["expected_owner_version"] = n;
            assert!(serde_json::from_value::<ClaimExecutionLease>(c).is_err());
        }
        for id in [
            "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
            "00000000-0000-0000-0000-000000000000",
        ] {
            let mut c = claim();
            c["fence"]["assignment_id"] = json!(id);
            assert!(serde_json::from_value::<ClaimExecutionLease>(c).is_err());
        }
        let mut c = claim();
        c["ttl_seconds"] = json!(60);
        assert!(serde_json::from_value::<ClaimExecutionLease>(c).is_err());
        assert!(no_dispatch(&mut serde_json::Deserializer::from_str("true")).is_err());
    }

    #[test]
    fn resume_inputs_are_closed_short_names_and_checkpoint_nulls_are_required() {
        let mut value = json!({"expected_owner_version":1,"lease_id":"dddddddd-dddd-4ddd-8ddd-dddddddddddd","expected_lease_version":1,
            "input":{"input_kind":"feedback","input_id":"eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee"},
            "checkpoint":{"checkpoint_id":"ffffffff-ffff-4fff-8fff-ffffffffffff","checkpoint_revision":1,
                "checkpoint_sha256":"a".repeat(64),"fence":claim()["fence"],"requirement_revision":null,"content_hash":null},
            "previous_run_id":"11111111-1111-4111-8111-111111111111","previous_run_terminal":{"reference":"fixture://terminal","sha256":"b".repeat(64)},
            "idempotency_key":"resume"});
        for kind in ["answer", "feedback", "confirmation"] {
            value["input"]["input_kind"] = json!(kind);
            let command: ResumePmExecutionLease = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(command.input.kind(), kind);
            assert_eq!(serde_json::to_value(command).unwrap(), value);
        }
        for kind in [
            "clarification_answer",
            "pm_feedback",
            "requirements_confirmation",
            "unknown",
        ] {
            let mut bad = value.clone();
            bad["input"]["input_kind"] = json!(kind);
            assert!(serde_json::from_value::<ResumePmExecutionLease>(bad).is_err());
        }
        for field in ["requirement_revision", "content_hash"] {
            let mut bad = value.clone();
            bad["checkpoint"].as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<ResumePmExecutionLease>(bad).is_err());
        }
        for field in ["terminal", "verified", "dispatch_allowed"] {
            let mut bad = value.clone();
            bad[field] = json!(true);
            assert!(serde_json::from_value::<ResumePmExecutionLease>(bad).is_err());
            for object in ["input", "checkpoint", "previous_run_terminal"] {
                let mut bad = value.clone();
                bad[object][field] = json!(true);
                assert!(serde_json::from_value::<ResumePmExecutionLease>(bad).is_err());
            }
        }
        for id in [
            "EEEEEEEE-EEEE-4EEE-8EEE-EEEEEEEEEEEE",
            "00000000-0000-0000-0000-000000000000",
        ] {
            let mut bad = value.clone();
            bad["input"]["input_id"] = json!(id);
            assert!(serde_json::from_value::<ResumePmExecutionLease>(bad).is_err());
        }
    }
}
