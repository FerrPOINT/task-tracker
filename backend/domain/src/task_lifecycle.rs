//! Common task state; execution authority remains in the SDLC execution ports.
use crate::StatusCategory;
use crate::sdlc_lifecycle::{LifecycleStage as Stage, LifecycleStatus as Status};
use serde::{Deserialize, Serialize};
use shared::AppError;

pub const STAGES: [Stage; 9] = [
    Stage::Draft,
    Stage::Backlog,
    Stage::Analysis,
    Stage::Architecture,
    Stage::Development,
    Stage::Review,
    Stage::Testing,
    Stage::Rework,
    Stage::Deployment,
];
pub const STATUSES: [Status; 3] = [Status::Ready, Status::InProgress, Status::Completed];

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskLifecycleAction {
    Start,
    Complete,
    RequestRework,
    Reopen,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskLifecycleState {
    pub stage: Stage,
    pub status: Status,
    pub revision: i64,
}
impl Default for State {
    fn default() -> Self {
        Self {
            stage: Stage::Draft,
            status: Status::Ready,
            revision: 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskLifecycleTransitionCommand {
    pub action: Action,
    pub expected_revision: i64,
    pub idempotency_key: String,
}
impl TransitionCommand {
    pub fn validate(&self) -> Result<(), AppError> {
        if !(1..=9_007_199_254_740_991).contains(&self.expected_revision)
            || self.idempotency_key.is_empty()
            || self.idempotency_key.len() > 128
            || self.idempotency_key.chars().any(char::is_control)
        {
            return Err(AppError::validation("invalid task lifecycle command"));
        }
        Ok(())
    }
}

pub fn category(stage: Stage, status: Status) -> StatusCategory {
    if stage == Stage::Deployment && status == Status::Completed {
        StatusCategory::Done
    } else if matches!(stage, Stage::Draft | Stage::Backlog) && status == Status::Ready {
        StatusCategory::Todo
    } else {
        StatusCategory::InProgress
    }
}

pub fn transition(state: State, action: Action) -> Result<State, AppError> {
    let next = match (state.stage, state.status, action) {
        (stage, Status::Ready, Action::Start) => (stage, Status::InProgress),
        (Stage::Draft, Status::InProgress | Status::Completed, Action::Complete) => {
            (Stage::Backlog, Status::Ready)
        }
        (Stage::Backlog, Status::InProgress | Status::Completed, Action::Complete) => {
            (Stage::Analysis, Status::Ready)
        }
        (Stage::Analysis, Status::InProgress | Status::Completed, Action::Complete) => {
            (Stage::Architecture, Status::Ready)
        }
        (Stage::Architecture, Status::InProgress | Status::Completed, Action::Complete) => {
            (Stage::Development, Status::Ready)
        }
        (
            Stage::Development | Stage::Rework,
            Status::InProgress | Status::Completed,
            Action::Complete,
        ) => (Stage::Review, Status::Ready),
        (Stage::Review, Status::InProgress | Status::Completed, Action::Complete) => {
            (Stage::Testing, Status::Ready)
        }
        (Stage::Testing, Status::InProgress | Status::Completed, Action::Complete) => {
            (Stage::Deployment, Status::Ready)
        }
        (Stage::Deployment, Status::InProgress, Action::Complete) => {
            (Stage::Deployment, Status::Completed)
        }
        (
            Stage::Review | Stage::Testing,
            Status::Ready | Status::InProgress,
            Action::RequestRework,
        ) => (Stage::Rework, Status::Ready),
        (Stage::Deployment, Status::Completed, Action::Reopen) => (Stage::Rework, Status::Ready),
        _ => return Err(AppError::conflict("task_lifecycle_transition_not_allowed")),
    };
    let revision = state
        .revision
        .checked_add(1)
        .filter(|v| *v <= 9_007_199_254_740_991)
        .ok_or_else(|| AppError::conflict("task_lifecycle_revision_exhausted"))?;
    Ok(State {
        stage: next.0,
        status: next.1,
        revision,
    })
}

pub fn allowed_actions(state: State, may_edit: bool, execution_busy: bool) -> Vec<Action> {
    if !may_edit || execution_busy {
        return Vec::new();
    }
    [
        Action::Start,
        Action::Complete,
        Action::RequestRework,
        Action::Reopen,
    ]
    .into_iter()
    .filter(|action| transition(state, *action).is_ok())
    .collect()
}

/// Only immutable baseline IDs qualify for ordinary legacy migration.
pub fn import_baseline_status(id: uuid::Uuid) -> Option<State> {
    let (stage, status) = match id.as_u128() {
        1 => (Stage::Backlog, Status::Ready),
        2 => (Stage::Development, Status::InProgress),
        4 => (Stage::Review, Status::Ready),
        3 => (Stage::Deployment, Status::Completed),
        _ => return None,
    };
    Some(State {
        stage,
        status,
        revision: 1,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskLifecycleTransitionOption {
    pub action: Action,
    pub target: State,
}

pub fn transition_options(
    state: State,
    may_edit: bool,
    execution_busy: bool,
) -> Vec<TaskLifecycleTransitionOption> {
    allowed_actions(state, may_edit, execution_busy)
        .into_iter()
        .filter_map(|action| {
            transition(state, action)
                .ok()
                .map(|target| TaskLifecycleTransitionOption { action, target })
        })
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskLifecycleReadback {
    pub task_id: uuid::Uuid,
    pub state: State,
    pub allowed_actions: Vec<TaskLifecycleTransitionOption>,
    pub execution_busy: bool,
    pub draft_submode: Option<crate::sdlc::DraftSubmode>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_task_does_not_need_execution_configuration() {
        assert_eq!(
            State::default(),
            State {
                stage: Stage::Draft,
                status: Status::Ready,
                revision: 1
            }
        );
    }
    #[test]
    fn manual_path_and_rework_preserve_revision_and_completion_meaning() {
        let mut s = State::default();
        for stage in [
            Stage::Draft,
            Stage::Backlog,
            Stage::Analysis,
            Stage::Architecture,
            Stage::Development,
            Stage::Review,
            Stage::Testing,
            Stage::Deployment,
        ] {
            assert_eq!(s.stage, stage);
            assert_ne!(category(s.stage, s.status), StatusCategory::Done);
            s = transition(s, Action::Start).unwrap();
            s = transition(s, Action::Complete).unwrap();
        }
        assert_eq!(
            (s.stage, s.status, s.revision),
            (Stage::Deployment, Status::Completed, 17)
        );
        assert_eq!(category(s.stage, s.status), StatusCategory::Done);
        s = transition(s, Action::Reopen).unwrap();
        assert_eq!((s.stage, s.status), (Stage::Rework, Status::Ready));
        s = transition(transition(s, Action::Start).unwrap(), Action::Complete).unwrap();
        assert_eq!((s.stage, s.status), (Stage::Review, Status::Ready));
    }
    #[test]
    fn access_or_unknown_execution_cannot_be_bypassed_by_a_human_action() {
        assert!(allowed_actions(State::default(), false, false).is_empty());
        assert!(allowed_actions(State::default(), true, true).is_empty());
        assert!(transition(State::default(), Action::Reopen).is_err());
    }
    #[test]
    fn ready_technical_stage_is_work_not_new_backlog() {
        for stage in STAGES {
            for status in STATUSES {
                assert_eq!(
                    category(stage, status) == StatusCategory::Done,
                    stage == Stage::Deployment && status == Status::Completed
                );
            }
        }
        assert_eq!(
            category(Stage::Analysis, Status::Ready),
            StatusCategory::InProgress
        );
        assert_eq!(
            category(Stage::Draft, Status::InProgress),
            StatusCategory::InProgress
        );
    }
    #[test]
    fn import_uses_ids_and_never_guesses_foreign_status_names() {
        let done = import_baseline_status(uuid::Uuid::from_u128(3)).unwrap();
        assert_eq!(
            (done.stage, done.status),
            (Stage::Deployment, Status::Completed)
        );
        assert!(import_baseline_status(uuid::Uuid::new_v4()).is_none());
        assert!(import_baseline_status(uuid::Uuid::nil()).is_none());
    }
}

pub type Action = TaskLifecycleAction;
pub type State = TaskLifecycleState;
pub type TransitionCommand = TaskLifecycleTransitionCommand;
pub type Readback = TaskLifecycleReadback;

/// Read-only compatibility column identity; it never authorizes a status write.
#[derive(Clone, Debug)]
pub struct StageProjection {
    pub stage: Stage,
    pub projection_status_id: shared::StatusId,
    pub wip_limit: Option<i64>,
}

/// Read projection for historical, unmarked Draft state. Never rewrites receipts.
pub fn effective_state(snapshot: &serde_json::Value, revision: i64) -> Result<State, AppError> {
    let stage = match snapshot.get("stage").and_then(serde_json::Value::as_str) {
        Some("Clarification") => Stage::Draft,
        _ => serde_json::from_value(snapshot["stage"].clone()).map_err(AppError::internal)?,
    };
    let status = if snapshot
        .get("status")
        .is_none_or(serde_json::Value::is_null)
        && stage == Stage::Draft
        && snapshot.get("flow").is_none()
    {
        Status::Ready
    } else {
        serde_json::from_value(snapshot["status"].clone())
            .map_err(|_| AppError::Unavailable("task_lifecycle_import_required".into()))?
    };
    Ok(State {
        stage,
        status,
        revision,
    })
}
#[cfg(test)]
mod legacy_projection_tests {
    use super::*;
    #[test]
    fn clarification_is_a_draft_read_projection_and_missing_technical_status_is_not_guessed() {
        let snapshot = serde_json::json!({"stage":"Clarification","questions":[{"id":"retained"}]});
        let original = snapshot.clone();
        assert_eq!(
            effective_state(&snapshot, 7).unwrap(),
            State {
                stage: Stage::Draft,
                status: Status::Ready,
                revision: 7
            }
        );
        assert_eq!(snapshot, original);
        assert!(effective_state(&serde_json::json!({"stage":"Analysis"}), 1).is_err());
        assert!(
            effective_state(
                &serde_json::json!({"stage":"Draft","flow":{},"status":null}),
                1
            )
            .is_err()
        );
    }
}

/// Stable compatibility row identity shared with the additive PostgreSQL migration.
/// MD5 names these internal rows; it is not a credential or security proof.
pub fn projection_status_id(stage: Stage, status: Status) -> shared::StatusId {
    use md5::{Digest, Md5};
    let key = format!("pdlc.task-lifecycle.v1/{stage:?}/{status:?}");
    let bytes: [u8; 16] = Md5::digest(key.as_bytes()).into();
    shared::StatusId::from_uuid(uuid::Uuid::from_bytes(bytes))
}

pub fn default_board_columns() -> Vec<crate::BoardColumn> {
    STAGES
        .into_iter()
        .enumerate()
        .map(|(position, stage)| crate::BoardColumn {
            id: projection_status_id(stage, Status::Ready),
            name: format!("{stage:?}").into(),
            category: category(stage, Status::Ready),
            wip_limit: if stage == Stage::Development {
                Some(5)
            } else {
                None
            },
            position: position as i32,
        })
        .collect()
}

#[cfg(test)]
mod board_defaults_tests {
    use super::*;
    #[test]
    fn every_fresh_project_gets_the_same_nine_stages_without_execution_config() {
        let columns = default_board_columns();
        assert_eq!(columns.len(), STAGES.len());
        for (column, stage) in columns.iter().zip(STAGES) {
            assert_eq!(column.id, projection_status_id(stage, Status::Ready));
            assert_eq!(column.name.as_ref(), format!("{stage:?}"));
        }
        assert_eq!(columns.iter().filter(|c| c.wip_limit.is_some()).count(), 1);
        assert_eq!(
            columns
                .iter()
                .map(|c| c.id)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            9
        );
        assert_eq!(
            default_board_columns()
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>(),
            columns.iter().map(|c| c.id).collect::<Vec<_>>()
        );
    }
}

/// Owner read model: issue content and lifecycle come from one database snapshot.
pub struct StageBoardIssue {
    pub issue: crate::Issue,
    pub lifecycle: Readback,
    pub assignee_name: Option<String>,
    pub reporter_name: Option<String>,
    pub labels: Vec<String>,
}
pub struct StageBoardSnapshot {
    pub cards: Vec<StageBoardIssue>,
    pub columns: Vec<StageProjection>,
    pub sprint: Option<crate::Sprint>,
}

#[cfg(test)]
mod retained_completion_tests {
    use super::*;
    #[test]
    fn retained_intermediate_completion_can_advance_but_final_completion_is_not_recompleted() {
        for stage in STAGES
            .into_iter()
            .filter(|stage| *stage != Stage::Deployment)
        {
            let state = State {
                stage,
                status: Status::Completed,
                revision: 9,
            };
            let next = transition(state, Action::Complete).unwrap();
            assert_eq!(next.status, Status::Ready);
            assert_eq!(next.revision, 10);
            assert_ne!(next.stage, stage);
            assert!(transition(state, Action::Start).is_err());
        }
        let terminal = State {
            stage: Stage::Deployment,
            status: Status::Completed,
            revision: 9,
        };
        assert!(transition(terminal, Action::Complete).is_err());
        assert_eq!(
            transition(terminal, Action::Reopen).unwrap().stage,
            Stage::Rework
        );
    }
}
