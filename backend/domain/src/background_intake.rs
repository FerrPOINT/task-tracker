//! A human's exact finding snapshot; it carries no assignment or execution authority.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FindingApplication {
    FleetControl,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FindingOrigin {
    pub application: FindingApplication,
    pub finding_id: Uuid,
    #[serde(deserialize_with = "crate::sdlc::safe_version")]
    pub finding_revision: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub enum FindingDestination {
    Draft,
    Backlog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum FindingKind {
    Bug,
    Refactoring,
}
impl FindingKind {
    pub fn issue_type(self) -> shared::IssueType {
        match self {
            Self::Bug => shared::IssueType::Bug,
            Self::Refactoring => shared::IssueType::Task,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FindingIntake {
    pub source: FindingOrigin,
    pub destination: FindingDestination,
    pub kind: FindingKind,
    pub title: String,
    pub description: String,
    pub expected: String,
    pub criteria: String,
    pub evidence: String,
    #[serde(default)]
    pub custom_fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FindingIntakeReceipt {
    pub operation_id: Uuid,
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    pub task_id: Uuid,
    pub task_key: String,
    pub destination: FindingDestination,
    pub request: FindingIntake,
}
