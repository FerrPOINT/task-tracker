use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AnalysisConfigurationPreflight {
    pub contract_version: u8,
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
    pub assignment_hash: String,
    pub agent_id: Uuid,
    pub effective_config_revision: i64,
    pub observation_ref: Uuid,
    pub configuration_observed_at: DateTime<Utc>,
    pub lease_version: i64,
    pub fencing_token: i64,
    pub configuration_matched: bool,
    #[schema(schema_with = crate::sdlc_reservation::false_schema)]
    pub runtime_ready: bool,
    #[schema(schema_with = crate::sdlc_reservation::false_schema)]
    pub dispatch_allowed: bool,
    pub blockers: Vec<String>,
}
