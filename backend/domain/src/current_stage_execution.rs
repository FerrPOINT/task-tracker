//! Current-stage configured execution uses an explicit genuine human basis.
//! Historical PM/Delivery envelopes keep their original schemas and meaning.
use crate::{
    sdlc::InteractionProtocol,
    sdlc_execution_lease::ExecutionLease,
    sdlc_lifecycle::{
        ArtifactReceipt, DeliveryExecution, DeliveryResult, LifecycleStage, LifecycleTask,
        RunBinding,
    },
    sdlc_routing::TaskRoutingSnapshot,
    sdlc_source::TaskSourceBinding,
    task_handoff::{TaskExecutionRole, TaskHandoffReceipt},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Same owner leases, execution ordinals and fences; all seven existing professions.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CurrentStageAssignment {
    pub handoff_id: Uuid,
    pub assignment_id: Uuid,
    pub execution_id: Uuid,
    pub execution: DeliveryExecution,
    pub task_id: Uuid,
    pub owner_subject: String,
    pub role: TaskExecutionRole,
    pub stage: LifecycleStage,
    pub cycle: u8,
    pub attempt: u8,
    pub agent_id: Uuid,
    pub machine_subject: String,
    pub routing: TaskRoutingSnapshot,
    pub previous_result_id: Option<Uuid>,
    pub fencing_token: i64,
    pub lease_id: Uuid,
    pub assignment_hash: String,
}

/// This result does not manufacture requirements or impersonate an owner confirmation.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CurrentStageResult {
    ProjectManager { report: ArtifactReceipt },
    Technical { result: DeliveryResult },
}

/// Versioned separately from historical strict SdlcContext/LifecycleReceipt.
/// The business state remains LifecycleTask projected by its Tracker owner.
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CurrentStageExecution {
    pub contract_version: u8,
    pub interaction_protocol: InteractionProtocol,
    pub consent: TaskHandoffReceipt,
    pub task: LifecycleTask,
    pub source_binding: TaskSourceBinding,
    pub assignment: Option<CurrentStageAssignment>,
    pub lease: Option<ExecutionLease>,
    pub binding: Option<RunBinding>,
    pub result: Option<CurrentStageResult>,
}
