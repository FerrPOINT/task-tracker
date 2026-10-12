//! Owner-declared routing references. These are not configuration or native admission proofs.
use crate::sdlc::{MAX_SAFE_VERSION, safe_optional_version, safe_version};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PACKAGE_COMMIT: &str = "4b9b4c9297a13fb28a6ba2039af2f7cb719f2f58";

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RoleRoute {
    pub agent_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub fleet_config_revision: i64,
    pub package_commit: String,
    pub package_manifest_sha256: String,
    pub namespace_id: String,
    pub namespace_name: String,
    pub workflow_id: String,
    pub workflow_key: String,
    pub profile: String,
    pub workflow_catalog_version: u32,
    pub workflow_catalog_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RoleRoutes {
    pub project_manager: RoleRoute,
    pub analyst: RoleRoute,
    pub architect: RoleRoute,
    pub developer: RoleRoute,
    pub reviewer: RoleRoute,
    pub tester: RoleRoute,
    pub devops: RoleRoute,
}

impl RoleRoutes {
    pub fn entries(&self) -> [(&'static str, &RoleRoute); 7] {
        [
            ("project_manager", &self.project_manager),
            ("analyst", &self.analyst),
            ("architect", &self.architect),
            ("developer", &self.developer),
            ("reviewer", &self.reviewer),
            ("tester", &self.tester),
            ("devops", &self.devops),
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SetRoutingPolicy {
    #[serde(deserialize_with = "safe_optional_version")]
    #[schema(
        required = true,
        nullable = true,
        minimum = 1,
        maximum = 9007199254740991i64
    )]
    pub expected_version: Option<i64>,
    pub routes: RoleRoutes,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoutingVerification {
    Declared,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RoutingPolicy {
    pub contract_version: u8,
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub version: i64,
    pub routing_hash: String,
    pub routes: RoleRoutes,
    pub verification: RoutingVerification,
    pub native_ready: bool,
    pub dispatch_allowed: bool,
    pub author_subject: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskRoutingSnapshot {
    pub contract_version: u8,
    pub snapshot_id: Uuid,
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub root_task_id: Uuid,
    pub confirmation_id: Uuid,
    pub requirement_revision: i64,
    pub content_hash: String,
    pub policy: RoutingPolicy,
    pub created_at: DateTime<Utc>,
}

pub fn bounded_policy_version(version: i64) -> bool {
    (1..=MAX_SAFE_VERSION).contains(&version)
}
