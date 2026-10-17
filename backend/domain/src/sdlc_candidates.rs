//! Read-only discovery of existing single-task lifecycle work, never a reservation.
use crate::sdlc::{MAX_SAFE_VERSION, safe_optional_version, safe_version};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::AppError;
use uuid::Uuid;

pub const DEFAULT_LIMIT: u16 = 50;
pub const MAX_LIMIT: u16 = 100;

fn default_limit() -> u16 {
    DEFAULT_LIMIT
}

#[derive(Clone, Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
pub struct DeliveryCandidatesQuery {
    pub project_id: Uuid,
    pub after: Option<Uuid>,
    #[serde(default = "default_limit")]
    #[param(value_type = Option<u16>, minimum = 1, maximum = 100, default = 50)]
    pub limit: u16,
}

impl DeliveryCandidatesQuery {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.project_id.is_nil()
            || self.after.is_some_and(|id| id.is_nil())
            || !(1..=MAX_LIMIT).contains(&self.limit)
        {
            return Err(AppError::validation("invalid delivery candidates query"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CandidateOperation {
    EnsureQueued,
    Reserve,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeliveryCandidate {
    pub task_id: Uuid,
    pub root_task_id: Uuid,
    #[serde(deserialize_with = "safe_version")]
    #[schema(minimum = 1, maximum = 9007199254740991i64)]
    pub requirement_revision: i64,
    pub next_operation: CandidateOperation,
    #[serde(deserialize_with = "safe_optional_version")]
    #[schema(
        required = true,
        nullable = true,
        minimum = 1,
        maximum = 9007199254740991i64
    )]
    pub task_version: Option<i64>,
}

impl DeliveryCandidate {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.task_id.is_nil()
            || self.root_task_id != self.task_id
            || !(1..=MAX_SAFE_VERSION).contains(&self.requirement_revision)
            || match (self.next_operation, self.task_version) {
                (CandidateOperation::EnsureQueued, None) => false,
                (CandidateOperation::Reserve, Some(version)) => {
                    !(1..=MAX_SAFE_VERSION).contains(&version)
                }
                _ => true,
            }
        {
            return Err(AppError::conflict(
                "invalid delivery candidate identity/version",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeliveryCandidates {
    #[schema(minimum = 1, maximum = 1)]
    pub contract_version: u8,
    pub tracker_instance_id: String,
    pub project_id: Uuid,
    #[serde(serialize_with = "crate::sdlc_execution_lease::utc_nanos")]
    pub observed_at: DateTime<Utc>,
    pub items: Vec<DeliveryCandidate>,
    #[schema(required = true, nullable = true)]
    pub next_cursor: Option<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn query_defaults_bounds_and_closed_shape() {
        let project = Uuid::new_v4();
        let query: DeliveryCandidatesQuery =
            serde_json::from_value(json!({"project_id":project})).unwrap();
        assert_eq!(query.limit, DEFAULT_LIMIT);
        query.validate().unwrap();
        for limit in [0, 101, u16::MAX] {
            assert!(
                DeliveryCandidatesQuery {
                    limit,
                    ..query.clone()
                }
                .validate()
                .is_err()
            );
        }
        for limit in [1, MAX_LIMIT] {
            DeliveryCandidatesQuery {
                limit,
                ..query.clone()
            }
            .validate()
            .unwrap();
        }
        assert!(
            DeliveryCandidatesQuery {
                project_id: Uuid::nil(),
                ..query.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            DeliveryCandidatesQuery {
                after: Some(Uuid::nil()),
                ..query
            }
            .validate()
            .is_err()
        );
        assert!(
            serde_json::from_value::<DeliveryCandidatesQuery>(
                json!({"project_id":project,"claim":true})
            )
            .is_err()
        );
    }

    #[test]
    fn wire_is_closed_nullable_and_never_claims_an_assignment() {
        let task = Uuid::new_v4();
        let item = DeliveryCandidate {
            task_id: task,
            root_task_id: task,
            requirement_revision: 1,
            next_operation: CandidateOperation::EnsureQueued,
            task_version: None,
        };
        let value = serde_json::to_value(&item).unwrap();
        assert_eq!(
            value,
            json!({"task_id":task,"root_task_id":task,"requirement_revision":1,
            "next_operation":"ensure_queued","task_version":null})
        );
        item.validate().unwrap();
        for (operation, version) in [
            (CandidateOperation::EnsureQueued, Some(1)),
            (CandidateOperation::Reserve, None),
            (CandidateOperation::Reserve, Some(0)),
        ] {
            assert!(
                DeliveryCandidate {
                    next_operation: operation,
                    task_version: version,
                    ..item.clone()
                }
                .validate()
                .is_err()
            );
        }
        assert!(
            DeliveryCandidate {
                root_task_id: Uuid::new_v4(),
                ..item.clone()
            }
            .validate()
            .is_err()
        );
        let mut unknown = value.clone();
        unknown["assignment_id"] = json!(Uuid::new_v4());
        assert!(serde_json::from_value::<DeliveryCandidate>(unknown).is_err());
        let mut missing = value;
        missing.as_object_mut().unwrap().remove("task_version");
        assert!(serde_json::from_value::<DeliveryCandidate>(missing).is_err());
        let page = DeliveryCandidates {
            contract_version: 1,
            tracker_instance_id: "fixture".into(),
            project_id: Uuid::new_v4(),
            observed_at: Utc::now(),
            items: vec![item],
            next_cursor: None,
        };
        let wire = serde_json::to_value(page).unwrap();
        assert_eq!(wire.as_object().unwrap().len(), 6);
        assert!(wire["next_cursor"].is_null());
        assert!(wire["observed_at"].as_str().unwrap().ends_with('Z'));
        assert_eq!(
            wire["observed_at"]
                .as_str()
                .unwrap()
                .split('.')
                .next_back()
                .unwrap()
                .len(),
            10
        );
        let mut unknown = wire;
        unknown["dispatch_allowed"] = json!(true);
        assert!(serde_json::from_value::<DeliveryCandidates>(unknown).is_err());
    }
}
