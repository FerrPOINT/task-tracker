use async_trait::async_trait;
use std::sync::Arc;

use domain::{WorkflowTransition, WorkflowTransitionRepository};
use shared::{AppError, StatusId};

pub struct WorkflowServiceImpl {
    transitions: Arc<dyn WorkflowTransitionRepository>,
}

impl WorkflowServiceImpl {
    pub fn new(transitions: Arc<dyn WorkflowTransitionRepository>) -> Self {
        Self { transitions }
    }
}

#[async_trait]
impl crate::context::WorkflowService for WorkflowServiceImpl {
    async fn list_transitions(&self) -> Result<Vec<WorkflowTransition>, AppError> {
        self.transitions.list_all().await
    }

    async fn is_transition_allowed(
        &self,
        from_status_id: StatusId,
        to_status_id: StatusId,
    ) -> Result<bool, AppError> {
        self.transitions
            .is_allowed(from_status_id, to_status_id)
            .await
    }
}
