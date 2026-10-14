//! Transport preflight only. Configured scheduler identity and fresh ACL stay in the repository.
use domain::sdlc::Principal;
use domain::sdlc_critical_blockers::valid_operation_key;
use domain::sdlc_critical_pm_reservation::{CriticalPmReservationResult, ReserveCriticalPmReview};
use shared::AppError;
use uuid::Uuid;

impl crate::sdlc::SdlcService {
    pub async fn reserve_critical_pm_review(
        &self,
        task: Uuid,
        actor: &Principal,
        command: ReserveCriticalPmReview,
    ) -> Result<(CriticalPmReservationResult, bool), AppError> {
        validate_request(task, actor, &command)?;
        self.lifecycle_repository()?
            .reserve_critical_pm_review(task, actor, command)
            .await
    }

    pub async fn critical_pm_review_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<CriticalPmReservationResult, AppError> {
        actor_shape(actor, false)?;
        validate_key(task, key)?;
        self.lifecycle_repository()?
            .critical_pm_review_operation(task, actor, key)
            .await
    }
}

fn actor_shape(actor: &Principal, write: bool) -> Result<(), AppError> {
    let scope = if write {
        "task-tracker:write"
    } else {
        "task-tracker:read"
    };
    if actor.human_session
        || actor.trusted_human
        || actor.scopes.len() != 1
        || !actor.scopes.contains(scope)
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

fn validate_key(task: Uuid, key: &str) -> Result<(), AppError> {
    if task.is_nil() || !valid_operation_key(key) {
        return Err(AppError::validation(
            "invalid critical PM review task or key",
        ));
    }
    Ok(())
}

fn validate_request(
    task: Uuid,
    actor: &Principal,
    command: &ReserveCriticalPmReview,
) -> Result<(), AppError> {
    actor_shape(actor, true)?;
    validate_key(task, &command.idempotency_key)?;
    command.validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::sdlc::SdlcConfig;
    use serde_json::json;

    fn actor(write: bool) -> Principal {
        Principal {
            subject: "scheduler".into(),
            human_session: false,
            trusted_human: false,
            scopes: [if write {
                "task-tracker:write"
            } else {
                "task-tracker:read"
            }
            .into()]
            .into(),
        }
    }

    fn command() -> ReserveCriticalPmReview {
        serde_json::from_value(
            json!({"blocker_id":Uuid::new_v4(),"blocker_sha256":"a".repeat(64),
            "expected_blocker_version":2,"expected_task_version":9,"expected_pm_owner_version":17,
            "expected_pm_assignment_version":17,"idempotency_key":"pm-review:original?x=+%#"}),
        )
        .unwrap()
    }

    #[test]
    fn preflight_preserves_original_command_and_arbitrary_positive_generation() {
        let command = command();
        let original = serde_json::to_value(&command).unwrap();
        validate_request(Uuid::new_v4(), &actor(true), &command).unwrap();
        assert_eq!(serde_json::to_value(&command).unwrap(), original);
        actor_shape(&actor(false), false).unwrap();
    }

    #[test]
    fn read_write_human_and_assignment_grants_cannot_be_substituted() {
        assert!(actor_shape(&actor(false), true).is_err());
        assert!(actor_shape(&actor(true), false).is_err());
        for write in [false, true] {
            for mismatch in 0..5 {
                let mut invalid = actor(write);
                match mismatch {
                    0 => invalid.human_session = true,
                    1 => invalid.trusted_human = true,
                    2 => {
                        invalid.scopes.insert("task-tracker:read".into());
                        invalid.scopes.insert("task-tracker:write".into());
                    }
                    3 => {
                        let old_pm = domain::sdlc::PmAssignment {
                            assignment_id: Uuid::new_v4(),
                            execution_id: Uuid::new_v4(),
                            agent_id: Uuid::new_v4(),
                            version: 17,
                            machine_subject: "previous-pm".into(),
                        };
                        invalid.scopes.insert(old_pm.scope(Uuid::new_v4()));
                    }
                    _ => {
                        invalid.scopes.clear();
                    }
                }
                assert!(matches!(
                    actor_shape(&invalid, write),
                    Err(AppError::Forbidden)
                ));
            }
        }
    }

    #[test]
    fn invalid_manual_command_and_unreadable_keys_are_rejected() {
        for key in ["", ".", "..", "a/b", "a\\b", "a b", "a\n", "\0"] {
            let mut bad = command();
            bad.idempotency_key = key.into();
            assert!(validate_request(Uuid::new_v4(), &actor(true), &bad).is_err());
            assert!(validate_key(Uuid::new_v4(), key).is_err());
        }
        assert!(validate_key(Uuid::nil(), "original").is_err());
        for mismatch in 0..6 {
            let mut bad = command();
            match mismatch {
                0 => bad.blocker_id = Uuid::nil(),
                1 => bad.blocker_sha256 = "A".repeat(64),
                2 => bad.expected_blocker_version = 0,
                3 => bad.expected_task_version = -1,
                4 => bad.expected_pm_owner_version = domain::sdlc::MAX_SAFE_VERSION + 1,
                _ => bad.expected_pm_assignment_version = 0,
            }
            assert!(validate_request(Uuid::new_v4(), &actor(true), &bad).is_err());
        }
    }

    #[test]
    fn shape_preflight_is_not_configured_scheduler_authority() {
        let config = SdlcConfig {
            reservation_scheduler_subject: "scheduler".into(),
            orchestrator_subject: "orchestrator".into(),
            verifier_subject: "verifier".into(),
            single_task_project_ids: Default::default(),
            instance_id: "synthetic-tracker".into(),
        };
        for subject in ["foreign", "orchestrator", "verifier"] {
            let mut writer = actor(true);
            writer.subject = subject.into();
            actor_shape(&writer, true).unwrap();
            assert!(crate::sdlc_reservation::scheduler(&writer, &config).is_err());
            let mut reader = actor(false);
            reader.subject = subject.into();
            actor_shape(&reader, false).unwrap();
            assert!(crate::sdlc_lifecycle::scheduler_read(&reader, &config).is_err());
        }
        crate::sdlc_reservation::scheduler(&actor(true), &config).unwrap();
        crate::sdlc_lifecycle::scheduler_read(&actor(false), &config).unwrap();
    }
}
