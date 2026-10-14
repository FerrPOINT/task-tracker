//! Review purpose wraps the existing PM reservation without impersonating initial creation.
use crate::sdlc::{MAX_SAFE_VERSION, safe_version};
use crate::sdlc_critical_blockers::{sha256, valid_operation_key};
use crate::sdlc_pm_draft::PmDraftReservation;
use serde::{Deserialize, Serialize};
use shared::AppError;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReserveCriticalPmReview {
    pub blocker_id: Uuid,
    pub blocker_sha256: String,
    #[serde(deserialize_with = "safe_version")]
    pub expected_blocker_version: i64,
    #[serde(deserialize_with = "safe_version")]
    pub expected_task_version: i64,
    #[serde(deserialize_with = "safe_version")]
    pub expected_pm_owner_version: i64,
    #[serde(deserialize_with = "safe_version")]
    pub expected_pm_assignment_version: i64,
    pub idempotency_key: String,
}

impl ReserveCriticalPmReview {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.blocker_id.is_nil()
            || !sha256(&self.blocker_sha256)
            || [
                self.expected_blocker_version,
                self.expected_task_version,
                self.expected_pm_owner_version,
                self.expected_pm_assignment_version,
            ]
            .iter()
            .any(|version| !(1..=MAX_SAFE_VERSION).contains(version))
            || !valid_operation_key(&self.idempotency_key)
        {
            return Err(AppError::validation(
                "invalid critical PM reservation guard",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CriticalPmReservationPurpose {
    CriticalBusinessReview,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CriticalPmRequirementsRef {
    #[serde(deserialize_with = "safe_version")]
    pub revision: i64,
    pub content_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CriticalPmReviewContext {
    pub blocker_id: Uuid,
    pub blocker_sha256: String,
    #[serde(deserialize_with = "safe_version")]
    pub blocker_version: i64,
    pub requirements: CriticalPmRequirementsRef,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriticalPmReservationResult {
    pub purpose: CriticalPmReservationPurpose,
    pub reservation: PmDraftReservation,
    pub context: CriticalPmReviewContext,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn command() -> serde_json::Value {
        json!({"blocker_id":Uuid::new_v4(),"blocker_sha256":"a".repeat(64),
            "expected_blocker_version":2,"expected_task_version":8,
            "expected_pm_owner_version":1,"expected_pm_assignment_version":1,
            "idempotency_key":"critical-pm-review:original"})
    }

    #[test]
    fn command_has_closed_cas_and_no_arbitrary_pm_selector() {
        let value = command();
        let typed: ReserveCriticalPmReview = serde_json::from_value(value.clone()).unwrap();
        typed.validate().unwrap();
        assert_eq!(serde_json::to_value(typed).unwrap(), value);
        for key in ["requested_agent_id", "terminal", "dispatch_allowed"] {
            let mut bad = value.clone();
            bad[key] = json!(Uuid::new_v4());
            assert!(serde_json::from_value::<ReserveCriticalPmReview>(bad).is_err());
        }
    }

    #[test]
    fn missing_and_unsafe_cas_fields_are_rejected() {
        for key in [
            "expected_blocker_version",
            "expected_task_version",
            "expected_pm_owner_version",
            "expected_pm_assignment_version",
        ] {
            for invalid in [
                json!(null),
                json!(0),
                json!(-1),
                json!(MAX_SAFE_VERSION + 1),
            ] {
                let mut bad = command();
                bad[key] = invalid;
                assert!(serde_json::from_value::<ReserveCriticalPmReview>(bad).is_err());
            }
            let mut missing = command();
            missing.as_object_mut().unwrap().remove(key);
            assert!(serde_json::from_value::<ReserveCriticalPmReview>(missing).is_err());
        }
    }

    #[test]
    fn purpose_cannot_be_relabelled_initial_and_manual_values_still_validate() {
        assert_eq!(
            serde_json::to_value(CriticalPmReservationPurpose::CriticalBusinessReview).unwrap(),
            json!("critical_business_review")
        );
        for value in ["initial_draft", "resume", "completed"] {
            assert!(serde_json::from_value::<CriticalPmReservationPurpose>(json!(value)).is_err());
        }
        let typed: ReserveCriticalPmReview = serde_json::from_value(command()).unwrap();
        for change in 0..4 {
            let mut bad = typed.clone();
            match change {
                0 => bad.blocker_id = Uuid::nil(),
                1 => bad.blocker_sha256 = "G".repeat(64),
                2 => bad.idempotency_key.push('\0'),
                _ => bad.expected_task_version = 0,
            }
            assert!(bad.validate().is_err());
        }
    }

    #[test]
    fn reservation_keys_use_shared_critical_original_key_policy() {
        let mut typed: ReserveCriticalPmReview = serde_json::from_value(command()).unwrap();
        for key in ["", ".", "..", "a/b", "a\\b", "a b", "a\n", "a\0"] {
            typed.idempotency_key = key.into();
            assert!(!valid_operation_key(key));
            assert!(typed.validate().is_err());
        }
        typed.idempotency_key = "x".repeat(129);
        assert!(typed.validate().is_err());
        typed.idempotency_key = "critical:review-2.original".into();
        typed.validate().unwrap();
    }
}
