//! Discovery declares the existing next command; it never chooses roles or reserves capacity.
use domain::sdlc::{DraftFlow, Principal, SdlcConfig, Stage, TaskState};
use domain::sdlc_candidates::*;
use domain::sdlc_lifecycle::{LifecycleReceipt, LifecycleStage, LifecycleStatus, RuntimeState};
use shared::AppError;

impl crate::sdlc::SdlcService {
    pub async fn delivery_candidates(
        &self,
        actor: &Principal,
        query: DeliveryCandidatesQuery,
    ) -> Result<DeliveryCandidates, AppError> {
        query.validate()?;
        self.repository
            .lifecycle_repository()
            .ok_or_else(|| {
                AppError::Unavailable("single-task lifecycle repository not configured".into())
            })?
            .delivery_candidates(actor, query)
            .await
    }
}

pub fn authorize(actor: &Principal, config: &SdlcConfig) -> Result<(), AppError> {
    crate::sdlc_lifecycle::scheduler_read(actor, config)
}

/// Routing/revision/SQL owner predicates are checked by the repository in the same read.
pub fn candidate(
    state: &TaskState,
    current: Option<&LifecycleReceipt>,
    pm_completed: bool,
) -> Result<Option<DeliveryCandidate>, AppError> {
    if state.configured_cli() || state.draft_flow()? != Some(DraftFlow::SingleTaskV1) {
        return Ok(None);
    }
    state.require_root()?;
    let Some(revision) = state.current_revision() else {
        return Ok(None);
    };
    let (next_operation, task_version) = match current {
        None if pm_completed && matches!(state.stage, Stage::Analysis | Stage::Architecture) => {
            (CandidateOperation::EnsureQueued, None)
        }
        Some(receipt)
            if receipt.task.status == LifecycleStatus::Ready
                && receipt.task.runtime == RuntimeState::Queued
                && receipt.assignment.is_none()
                && receipt.task.stage.role().is_some()
                && receipt.task.stage == LifecycleStage::from_legacy(state.stage) =>
        {
            (CandidateOperation::Reserve, Some(receipt.task.version))
        }
        _ => return Ok(None),
    };
    let candidate = DeliveryCandidate {
        task_id: state.task_id,
        root_task_id: state.root_task_id,
        requirement_revision: revision,
        next_operation,
        task_version,
    };
    candidate.validate()?;
    Ok(Some(candidate))
}

pub fn page(
    query: &DeliveryCandidatesQuery,
    mut items: Vec<DeliveryCandidate>,
) -> Result<(Vec<DeliveryCandidate>, Option<uuid::Uuid>), AppError> {
    query.validate()?;
    let mut previous = query.after;
    for item in &items {
        item.validate()?;
        if previous.is_some_and(|id| item.task_id <= id) {
            return Err(AppError::conflict(
                "delivery candidate UUID order is inconsistent",
            ));
        }
        previous = Some(item.task_id);
    }
    if items.len() > usize::from(query.limit) + 1 {
        return Err(AppError::conflict(
            "delivery candidate query exceeded its bound",
        ));
    }
    let next_cursor = if items.len() > usize::from(query.limit) {
        items.truncate(usize::from(query.limit));
        items.last().map(|item| item.task_id)
    } else {
        None
    };
    Ok((items, next_cursor))
}

#[cfg(test)]
#[path = "sdlc_candidates_tests.rs"]
mod tests;
