use domain::sdlc::{Confirmation, MAX_SAFE_VERSION, Principal, Stage, TaskState};
use shared::AppError;
use uuid::Uuid;

impl crate::sdlc::SdlcService {
    pub async fn confirmation_read(
        &self,
        task: Uuid,
        actor: &Principal,
        revision: i64,
    ) -> Result<Confirmation, AppError> {
        validate_revision(revision)?;
        self.repository
            .confirmation_read(task, actor, revision)
            .await
    }
}

pub fn validate_revision(revision: i64) -> Result<(), AppError> {
    if !(1..=MAX_SAFE_VERSION).contains(&revision) {
        return Err(AppError::validation("invalid confirmation revision"));
    }
    Ok(())
}

/// Called only after the PM grant's own-task prefix has been validated.
pub fn is_confirmation_read(method: &str, resource: &[&str]) -> bool {
    matches!((method,resource), ("GET",["requirements",revision,"confirmation"])
        if crate::sdlc::canonical_pm_version(revision))
}

pub fn authorize_read(state: &TaskState, actor: &Principal) -> Result<(), AppError> {
    crate::sdlc::authorize_pm_read(state, actor)?;
    if crate::sdlc::pm_grant_task(actor)?.is_some()
        || crate::sdlc_configured_cli::configured_agent(actor)?.is_some()
    {
        if !actor.scopes.contains("task-tracker:read") {
            return Err(AppError::Forbidden);
        }
        Ok(())
    } else {
        crate::sdlc_reservation::reader(actor)
    }
}

pub fn exact_receipt(
    state: &TaskState,
    revision: i64,
    persisted: Option<&Confirmation>,
    requirement_hash: Option<&str>,
) -> Result<Confirmation, AppError> {
    validate_revision(revision)?;
    state.require_root()?;
    let history: Vec<_> = state
        .confirmations
        .iter()
        .filter(|c| c.revision == revision)
        .collect();
    let inconsistent = || AppError::conflict("owner_confirmation_history_inconsistent");
    let Some(receipt) = persisted else {
        return if history.is_empty() {
            Err(AppError::not_found("owner confirmation", revision))
        } else {
            Err(inconsistent())
        };
    };
    let revisions: Vec<_> = state
        .revisions
        .iter()
        .filter(|r| r.revision == revision)
        .collect();
    if history.len() != 1 || revisions.len() != 1 {
        return Err(inconsistent());
    }
    let requirement = revisions[0];
    if history[0] != receipt
        || receipt.id.is_nil()
        || receipt.task_id != state.task_id
        || receipt.revision != revision
        || receipt.stage != Stage::Backlog
        || receipt.owner_subject.trim().is_empty()
        || receipt.owner_subject != state.owner_subject
        || receipt.content_hash != requirement.content_hash
        || requirement_hash != Some(receipt.content_hash.as_str())
        || crate::sdlc::canonical_hash(&requirement.document)? != receipt.content_hash
    {
        return Err(inconsistent());
    }
    Ok(receipt.clone())
}
