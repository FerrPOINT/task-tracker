//! PM single-download shape and exact saved evidence only; never general attachment authority.
use domain::attachment_artifact::Seal;
use domain::sdlc::Principal;
use domain::sdlc_critical_blockers::{CriticalBlockerReceipt, CriticalBlockerState};
use shared::AppError;

pub fn route(actor: &Principal, method: &str, path: &str) -> Result<bool, AppError> {
    let Some(_) = crate::sdlc::pm_grant_task(actor)? else {
        return Ok(false);
    };
    if !actor.scopes.contains("task-tracker:read")
        || crate::sdlc_lifecycle::has_delivery_grant(&actor.scopes)
        || method != "GET"
        || !crate::attachment_artifacts::is_download_path(path)
    {
        return Err(AppError::Forbidden);
    }
    Ok(true)
}

/// Caller first loads the current saved review with fresh PM/owner/project/lease authority.
pub fn evidence(
    blocker: &CriticalBlockerReceipt,
    attachment: &domain::Attachment,
    seal: &Seal,
) -> Result<(), AppError> {
    seal.validate_attachment(attachment)?;
    if blocker.state != CriticalBlockerState::InPmReview
        || attachment.storage_key.as_ref() != seal.key()
        || blocker.task_id != blocker.root_task_id
        || attachment.issue_id.as_uuid() != blocker.task_id
        || seal.binding.assignment_id != blocker.proposal.fence.assignment_id
        || seal.binding.execution_id != blocker.proposal.fence.execution_id
        || seal.binding.role != blocker.role
        || seal.run_id != blocker.run_id
        || !blocker.proposal.evidence.contains(&seal.artifact())
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

#[cfg(test)]
#[path = "attachment_pm_review_tests.rs"]
mod tests;
