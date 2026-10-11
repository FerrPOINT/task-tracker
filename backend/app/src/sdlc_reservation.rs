use domain::{sdlc::*, sdlc_reservation::*};
use shared::AppError;

pub fn scheduler(actor: &Principal, config: &SdlcConfig) -> Result<(), AppError> {
    if config.reservation_scheduler_subject.trim().is_empty() {
        return Err(AppError::Unavailable(
            "reservation scheduler identity not configured".into(),
        ));
    }
    if actor.human_session
        || actor.subject != config.reservation_scheduler_subject
        || actor.subject == config.orchestrator_subject
        || actor.subject == config.verifier_subject
        || actor.scopes.len() != 1
        || !actor.scopes.contains("task-tracker:write")
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}
pub fn reader(actor: &Principal) -> Result<(), AppError> {
    if crate::sdlc::has_pm_grant(&actor.scopes)
        || (!actor.human_session
            && (actor.scopes.len() != 1 || !actor.scopes.contains("task-tracker:read")))
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}
pub fn assignment_hash(assignment: &PreparedAnalysisAssignment) -> Result<String, AppError> {
    let mut value = serde_json::to_value(assignment).map_err(AppError::internal)?;
    value
        .as_object_mut()
        .ok_or_else(|| AppError::internal("assignment is not an object"))?
        .remove("assignment_hash");
    crate::sdlc::canonical_hash(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn reservation_commands_reject_invented_proofs_and_unsafe_cas() {
        let command = json!({"intent_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", "routing_snapshot_id":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", "requirement_revision":1,"content_hash":"a".repeat(64),"expected_reservation_version":0,"idempotency_key":"reserve"});
        assert!(serde_json::from_value::<ReserveAnalysis>(command.clone()).is_ok());
        for field in ["stopped", "native_ready", "agent_id", "ttl_seconds"] {
            let mut c = command.clone();
            c[field] = json!(true);
            assert!(serde_json::from_value::<ReserveAnalysis>(c).is_err());
        }
        for n in [
            json!(0),
            json!(-1),
            json!(9007199254740992i64),
            json!(1.0),
            json!("1"),
        ] {
            let mut c = command.clone();
            c["requirement_revision"] = n;
            assert!(serde_json::from_value::<ReserveAnalysis>(c).is_err());
        }
        for n in [json!(1), json!(0.0), json!(null)] {
            let mut c = command.clone();
            c["expected_reservation_version"] = n;
            assert!(serde_json::from_value::<ReserveAnalysis>(c).is_err());
        }
        let config = SdlcConfig {
            reservation_scheduler_subject: "scheduler".into(),
            instance_id: "test".into(),
            orchestrator_subject: "pm".into(),
            verifier_subject: "verifier".into(),
        };
        let mut actor = Principal {
            subject: "scheduler".into(),
            human_session: false,
            trusted_human: false,
            scopes: ["task-tracker:write".into()].into(),
        };
        scheduler(&actor, &config).unwrap();
        actor.scopes.insert("task-tracker:read".into());
        assert!(scheduler(&actor, &config).is_err());
        actor.scopes = ["task-tracker:write".into()].into();
        actor.human_session = true;
        assert!(scheduler(&actor, &config).is_err());
    }
}
