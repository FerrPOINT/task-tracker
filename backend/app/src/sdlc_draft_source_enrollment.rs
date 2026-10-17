//! Initial Draft source metadata, never a runtime or Git permission.
use domain::{
    sdlc::{DraftFlow, MAX_SAFE_VERSION, Principal, Stage, TaskState},
    sdlc_source::*,
};
use shared::AppError;

pub fn validate_command(command: &EnrollDraftSource) -> Result<(), AppError> {
    crate::sdlc::validate_key(&command.idempotency_key)?;
    if !crate::sdlc_source::lower_hex(&command.expected_preflight_sha256, 64)
        || !crate::sdlc_source::lower_hex(&command.policy_hash, 64)
        || !(1..=MAX_SAFE_VERSION).contains(&command.source_revision)
    {
        return Err(AppError::validation("invalid Draft source enrollment CAS"));
    }
    Ok(())
}

pub fn authorize_draft(state: &TaskState, actor: &Principal) -> Result<(), AppError> {
    crate::sdlc_source::human_owner(actor)?;
    state.require_root()?;
    if !state.owner(actor) {
        return Err(AppError::Forbidden);
    }
    if state.stage != Stage::Draft
        || state.draft_flow()? != Some(DraftFlow::SingleTaskV1)
        || state.confirmation_revision.is_some()
        || !state.confirmations.is_empty()
    {
        return Err(AppError::conflict(
            "source enrollment is only for unconfirmed one-task Draft",
        ));
    }
    Ok(())
}

pub fn preflight_hash(value: &DraftSourceEnrollmentPreflight) -> Result<String, AppError> {
    let mut body = serde_json::to_value(value).map_err(AppError::internal)?;
    body.as_object_mut()
        .ok_or_else(|| AppError::conflict("invalid enrollment preflight"))?
        .remove("preflight_sha256");
    crate::sdlc::canonical_hash(&body)
}

pub fn verify_cas(
    before: &DraftSourceEnrollmentPreflight,
    command: &EnrollDraftSource,
) -> Result<(), AppError> {
    validate_command(command)?;
    crate::sdlc_source::validate_policy(&before.policy)?;
    if before.preflight_sha256 != preflight_hash(before)?
        || before.preflight_sha256 != command.expected_preflight_sha256
        || before.policy.source_revision != command.source_revision
        || before.policy.source_hash != command.policy_hash
        || before.project_id != before.policy.project_id
    {
        return Err(AppError::conflict("Draft source enrollment CAS changed"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "sdlc_draft_source_enrollment_tests.rs"]
mod tests;
