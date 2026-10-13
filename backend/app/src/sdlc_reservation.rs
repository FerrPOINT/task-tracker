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

pub fn validate_analysis_result(command: &SubmitAnalysisResult) -> Result<(), AppError> {
    crate::sdlc::validate_key(&command.idempotency_key)?;
    let hex = |value: &str| {
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if command.contract_version != 1
        || !hex(&command.content_hash)
        || !hex(&command.assignment_hash)
        || !hex(&command.terminal_receipt_sha256)
        || !hex(&command.architecture_input_sha256)
        || command.terminal_receipt_ref.trim().is_empty()
        || command.terminal_receipt_ref.len() > 512
        || command.summary.trim().is_empty()
        || command.summary.len() > 100_000
        || command
            .terminal_receipt_ref
            .chars()
            .chain(command.summary.chars())
            .any(|c| c == '\0')
    {
        return Err(AppError::validation("invalid Analysis terminal result"));
    }
    Ok(())
}

pub fn validate_pm_completion(command: &CompletePmDraft) -> Result<(), AppError> {
    crate::sdlc::validate_key(&command.idempotency_key)?;
    let hex = |value: &str| {
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if command.contract_version != 1
        || !hex(&command.content_hash)
        || !hex(&command.terminal_receipt_sha256)
        || !hex(&command.workflow_receipt_sha256)
        || command.terminal_receipt_ref.trim().is_empty()
        || command.workflow_receipt_ref.trim().is_empty()
        || command.terminal_receipt_ref.len() > 512
        || command.workflow_receipt_ref.len() > 512
        || command.terminal_receipt_ref.chars().any(|c| c == '\0')
        || command.workflow_receipt_ref.chars().any(|c| c == '\0')
    {
        return Err(AppError::validation("invalid PM completion receipt"));
    }
    Ok(())
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
            single_task_project_ids: std::collections::HashSet::new(),
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

    #[test]
    fn analysis_result_command_is_strict_and_requires_terminal_hashes() {
        let command = json!({
            "contract_version":1,
            "assignment_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            "execution_id":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            "agent_id":"cccccccc-cccc-4ccc-8ccc-cccccccccccc",
            "intent_id":"dddddddd-dddd-4ddd-8ddd-dddddddddddd",
            "routing_snapshot_id":"eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
            "requirement_revision":1,
            "content_hash":"a".repeat(64),
            "fencing_token":1,
            "lease_id":"ffffffff-ffff-4fff-8fff-ffffffffffff",
            "expected_lease_version":1,
            "assignment_hash":"b".repeat(64),
            "terminal_receipt_ref":"workflow/terminal/1",
            "terminal_receipt_sha256":"c".repeat(64),
            "architecture_input_sha256":"d".repeat(64),
            "summary":"ready for architecture",
            "idempotency_key":"analysis-result-1"
        });
        let parsed = serde_json::from_value::<SubmitAnalysisResult>(command.clone()).unwrap();
        validate_analysis_result(&parsed).unwrap();
        for field in [
            "contract_version",
            "assignment_id",
            "terminal_receipt_sha256",
        ] {
            let mut c = command.clone();
            c.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<SubmitAnalysisResult>(c).is_err());
        }
        let mut c = command.clone();
        c["contract_version"] = json!(2);
        assert!(serde_json::from_value::<SubmitAnalysisResult>(c).is_err());
        let mut c = command.clone();
        c["terminal_receipt_sha256"] = json!("not-a-hash");
        let parsed = serde_json::from_value::<SubmitAnalysisResult>(c).unwrap();
        assert!(validate_analysis_result(&parsed).is_err());
        let mut c = command;
        c["summary"] = json!("");
        let parsed = serde_json::from_value::<SubmitAnalysisResult>(c).unwrap();
        assert!(validate_analysis_result(&parsed).is_err());
    }

    #[test]
    fn pm_completion_command_is_strict_and_requires_exact_receipts() {
        let command = json!({
            "contract_version":1,
            "fence":{
                "assignment_id":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                "execution_id":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
                "agent_id":"cccccccc-cccc-4ccc-8ccc-cccccccccccc",
                "assignment_version":1
            },
            "lease_id":"dddddddd-dddd-4ddd-8ddd-dddddddddddd",
            "lease_version":1,
            "requirement_revision":1,
            "content_hash":"a".repeat(64),
            "terminal_receipt_ref":"fleet/pm/terminal.json",
            "terminal_receipt_sha256":"b".repeat(64),
            "workflow_receipt_ref":"workflow/pm/terminal.json",
            "workflow_receipt_sha256":"c".repeat(64),
            "idempotency_key":"pm-complete-1"
        });
        let parsed = serde_json::from_value::<CompletePmDraft>(command.clone()).unwrap();
        validate_pm_completion(&parsed).unwrap();
        for field in ["contract_version", "fence", "lease_id"] {
            let mut c = command.clone();
            c.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<CompletePmDraft>(c).is_err());
        }
        for field in ["native_ready", "ttl_seconds", "holder_subject"] {
            let mut c = command.clone();
            c[field] = json!(true);
            assert!(serde_json::from_value::<CompletePmDraft>(c).is_err());
        }
        let mut c = command.clone();
        c["fence"]["assignment_id"] = json!("00000000-0000-0000-0000-000000000000");
        assert!(serde_json::from_value::<CompletePmDraft>(c).is_err());
        let mut c = command.clone();
        c["workflow_receipt_sha256"] = json!("not-a-hash");
        let parsed = serde_json::from_value::<CompletePmDraft>(c).unwrap();
        assert!(validate_pm_completion(&parsed).is_err());
        let mut c = command;
        c["terminal_receipt_ref"] = json!("");
        let parsed = serde_json::from_value::<CompletePmDraft>(c).unwrap();
        assert!(validate_pm_completion(&parsed).is_err());
    }
}
