use crate::sdlc::SdlcService;
use domain::{
    decomposition::*,
    sdlc::{MAX_SAFE_VERSION, Principal},
};
use shared::AppError;
use uuid::Uuid;

pub fn authorize(actor: &Principal, task: Uuid) -> Result<(), AppError> {
    if task.is_nil() || actor.subject.is_empty() {
        return Err(AppError::Forbidden);
    }
    if !actor.trusted_human {
        crate::sdlc_configured_cli::authorize_configured_identity(actor)?;
    }
    Ok(())
}
pub fn validate(command: &SavePlan) -> Result<(), AppError> {
    crate::sdlc_discussion::validate_key(&command.idempotency_key)?;
    let text =
        |s: &str, max: usize| !s.trim().is_empty() && s.chars().count() <= max && !s.contains('\0');
    if !(0..MAX_SAFE_VERSION).contains(&command.expected_revision)
        || !text(&command.content.overview, 50_000)
        || command.reason.chars().count() > 2000
        || command.reason.contains('\0')
        || command.content.sections.is_empty()
        || command.content.sections.len() > 100
        || command.content.sections.iter().any(|s| {
            s.id.is_nil()
                || !text(&s.title, 300)
                || !text(&s.description, 20_000)
                || !text(&s.criteria, 20_000)
                || !text(&s.requirements, 10_000)
        })
        || command
            .content
            .sections
            .iter()
            .map(|s| s.id)
            .collect::<std::collections::HashSet<_>>()
            .len()
            != command.content.sections.len()
        || serde_json::to_vec(&command.content)
            .map_err(AppError::internal)?
            .len()
            > 1_000_000
    {
        return Err(AppError::validation(
            "invalid decomposition content, IDs or revision",
        ));
    }
    if let Some(source) = &command.content.source {
        if !(1..=MAX_SAFE_VERSION).contains(&source.requirements_revision)
            || source.requirements_hash.len() != 64
            || !source
                .requirements_hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || source.analysis_result_id.is_some_and(|id| id.is_nil())
        {
            return Err(AppError::validation("invalid decomposition source"));
        }
    }
    if command.fence.as_ref().is_some_and(|f| {
        f.assignment_id.is_nil()
            || f.execution_id.is_nil()
            || f.agent_id.is_nil()
            || !(1..=MAX_SAFE_VERSION).contains(&f.assignment_version)
    }) {
        return Err(AppError::validation("invalid Architect fence"));
    }
    Ok(())
}
pub fn history_limit(query: &PlanHistoryQuery) -> Result<u16, AppError> {
    let limit = query.limit.unwrap_or(20);
    if !(1..=100).contains(&limit)
        || query
            .before
            .is_some_and(|v| !(1..=MAX_SAFE_VERSION).contains(&v))
    {
        return Err(AppError::validation("invalid decomposition history cursor"));
    }
    Ok(limit)
}
impl SdlcService {
    pub fn decomposition(&self) -> Result<&dyn DecompositionRepository, AppError> {
        self.repository
            .decomposition_repository()
            .ok_or_else(|| AppError::Unavailable("decomposition repository not configured".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn command() -> SavePlan {
        SavePlan {
            expected_revision: 0,
            idempotency_key: "plan:first".into(),
            reason: String::new(),
            fence: None,
            content: PlanContent {
                overview: "Use the existing profile".into(),
                source: None,
                sections: vec![PlanSection {
                    id: Uuid::new_v4(),
                    title: "Save preferences".into(),
                    description: "Persist preferences".into(),
                    criteria: "Read after write, deny foreign profile".into(),
                    requirements: "R1, R2".into(),
                }],
            },
        }
    }
    #[test]
    fn meaningful_flat_plan_has_unique_stable_ids_and_bounded_content() {
        let valid = command();
        validate(&valid).unwrap();
        let mut bad = valid.clone();
        bad.content.sections.push(bad.content.sections[0].clone());
        assert!(validate(&bad).is_err());
        let mut bad = valid.clone();
        bad.content.sections[0].criteria = " ".into();
        assert!(validate(&bad).is_err());
        let mut bad = valid.clone();
        bad.content.sections.clear();
        assert!(validate(&bad).is_err());
        let mut bad = valid.clone();
        bad.expected_revision = MAX_SAFE_VERSION;
        assert!(validate(&bad).is_err());
        let mut bad = valid;
        bad.content.source = Some(PlanSource {
            requirements_revision: 1,
            requirements_hash: "X".repeat(64),
            analysis_result_id: None,
        });
        assert!(validate(&bad).is_err());
    }
    #[test]
    fn wire_rejects_child_lifecycle_fields() {
        let mut value = serde_json::to_value(command()).unwrap();
        value["content"]["sections"][0]["status"] = "done".into();
        assert!(serde_json::from_value::<SavePlan>(value).is_err());
    }
}
