use crate::sdlc::{AnalysisStage, safe_version};
use crate::sdlc_execution_lease::ExecutionLease;
use crate::sdlc_routing::RoleRoute;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, de::Error};
use uuid::Uuid;

fn uuid<'de, D: Deserializer<'de>>(d: D) -> Result<Uuid, D::Error> {
    let raw = String::deserialize(d)?;
    let id = Uuid::parse_str(&raw).map_err(D::Error::custom)?;
    if id.is_nil() || id.to_string() != raw {
        return Err(D::Error::custom("invalid canonical reservation UUID"));
    }
    Ok(id)
}
fn zero<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    let n = i64::deserialize(d)?;
    if n != 0 {
        return Err(D::Error::custom("initial reservation CAS must be zero"));
    }
    Ok(n)
}
fn no_dispatch<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    if bool::deserialize(d)? {
        return Err(D::Error::custom("reservation cannot allow dispatch"));
    }
    Ok(false)
}
fn false_schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
    utoipa::openapi::schema::ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::Boolean)
        .enum_values(Some([false]))
        .into()
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReserveAnalysis {
    #[serde(deserialize_with = "uuid")]
    pub intent_id: Uuid,
    #[serde(deserialize_with = "uuid")]
    pub routing_snapshot_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub requirement_revision: i64,
    pub content_hash: String,
    #[serde(deserialize_with = "zero")]
    #[schema(minimum = 0, maximum = 0)]
    pub expected_reservation_version: i64,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct HeartbeatAnalysis {
    #[serde(deserialize_with = "uuid")]
    pub assignment_id: Uuid,
    #[serde(deserialize_with = "uuid")]
    pub execution_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub fencing_token: i64,
    #[serde(deserialize_with = "uuid")]
    pub lease_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub expected_lease_version: i64,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PreparedAnalysisAssignment {
    #[schema(minimum = 1, maximum = 1)]
    pub contract_version: u8,
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub root_task_id: Uuid,
    pub owner_subject: String,
    pub intent_id: Uuid,
    pub confirmation_id: Uuid,
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub requirement_revision: i64,
    pub content_hash: String,
    pub routing_snapshot_id: Uuid,
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub routing_policy_version: i64,
    pub routing_hash: String,
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
    #[schema(minimum = 1, maximum = 1)]
    pub reservation_version: i64,
    #[schema(pattern = "^SDLC-[1-9][0-9]{0,18}$")]
    pub workflow_task_ref: String,
    pub agent_id: Uuid,
    pub route: RoleRoute,
    pub stage: AnalysisStage,
    pub role_key: String,
    pub workflow_key: String,
    pub mode_key: String,
    pub scope: String,
    #[schema(minimum = 0, maximum = 0)]
    pub cycle_number: u8,
    #[schema(minimum = 1, maximum = 1)]
    pub attempt_number: u8,
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub fencing_token: i64,
    pub lease_id: Uuid,
    pub assignment_operation_key: String,
    #[schema(pattern = "^[0-9a-f]{64}$")]
    pub assignment_hash: String,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisAdmissionState {
    AwaitingAdmission,
}
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AnalysisReservationReceipt {
    pub assignment: PreparedAnalysisAssignment,
    pub lease: ExecutionLease,
    #[schema(minimum = 1, maximum = 2)]
    pub technical_pool_slot: u8,
    #[schema(minimum = 30, maximum = 30)]
    pub ttl_seconds: i64,
    #[schema(minimum = 10, maximum = 10)]
    pub heartbeat_interval_seconds: i64,
    pub admission_state: AnalysisAdmissionState,
    #[serde(deserialize_with = "no_dispatch")]
    #[schema(schema_with = false_schema)]
    pub dispatch_allowed: bool,
    pub capacity_held: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AnalysisReservationOperation {
    pub idempotency_key: String,
    pub request_sha256: String,
    pub result: AnalysisReservationReceipt,
}
#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisReservationLeaseState {
    Unreserved,
    Active,
    Expired,
}
#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AnalysisReservationReadback {
    pub contract_version: u8,
    pub observed_at: DateTime<Utc>,
    pub lease_state: AnalysisReservationLeaseState,
    pub reconciliation_needed: bool,
    #[schema(required = true, nullable = true)]
    pub reason: Option<String>,
    #[schema(required = true, nullable = true)]
    pub current: Option<AnalysisReservationReceipt>,
    #[schema(schema_with = false_schema)]
    pub dispatch_allowed: bool,
    pub capacity_held: bool,
}
