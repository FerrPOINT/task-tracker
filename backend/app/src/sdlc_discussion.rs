use crate::sdlc::SdlcService;
use domain::sdlc::Principal;
use domain::sdlc_discussion::*;
use shared::AppError;
use uuid::Uuid;

pub fn authorize(actor: &Principal, task: Uuid, write: bool) -> Result<(), AppError> {
    let pm = crate::sdlc::pm_grant_task(actor)?;
    let delivery = crate::sdlc_lifecycle::delivery_grant(actor)?;
    if actor.human_session
        || pm.is_some() == delivery.is_some()
        || pm.or_else(|| delivery.as_ref().map(|g| g.task_id)) != Some(task)
        || !actor.scopes.contains(if write {
            "task-tracker:write"
        } else {
            "task-tracker:read"
        })
        || (write && !actor.scopes.contains("task-tracker:read"))
    {
        return Err(AppError::Forbidden);
    }
    if pm.is_some()
        && actor.scopes.iter().any(|s| {
            s != "task-tracker:read"
                && s != "task-tracker:write"
                && !s.starts_with("task-tracker:sdlc:pm:")
        })
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

pub fn validate_query(query: &DiscussionQuery) -> Result<u16, AppError> {
    let limit = query.limit.unwrap_or(100);
    if !(1..=500).contains(&limit)
        || (query.after.is_some() && query.snapshot.is_none())
        || query.after.is_some_and(|id| id.is_nil())
        || query.snapshot.is_some_and(|id| id.is_nil())
    {
        return Err(AppError::validation("invalid discussion page or cursor"));
    }
    Ok(limit)
}

pub fn validate_comment(command: &PublishStageComment) -> Result<(), AppError> {
    validate_key(&command.idempotency_key)?;
    if command.body.trim().is_empty()
        || command.body.chars().count() > 100_000
        || command.body.contains('\0')
    {
        return Err(AppError::validation(
            "comment body must contain 1..100000 characters",
        ));
    }
    if command.fence.assignment_id().is_nil() || command.fence.execution_id().is_nil() {
        return Err(AppError::validation("invalid comment assignment fence"));
    }
    match &command.fence {
        CommentFence::Delivery { fence } => crate::sdlc_lifecycle::validate_fence(fence),
        CommentFence::Pm {
            fence,
            lease_id,
            expected_lease_version,
        } => {
            if fence.agent_id.is_nil()
                || lease_id.is_nil()
                || !(1..=domain::sdlc::MAX_SAFE_VERSION).contains(&fence.assignment_version)
                || !(1..=domain::sdlc::MAX_SAFE_VERSION).contains(expected_lease_version)
            {
                return Err(AppError::validation("invalid PM comment lease fence"));
            }
            Ok(())
        }
    }
}

pub fn validate_key(key: &str) -> Result<(), AppError> {
    if !domain::sdlc_critical_blockers::valid_operation_key(key) {
        return Err(AppError::validation(
            "invalid route-safe stage comment operation key",
        ));
    }
    Ok(())
}

impl SdlcService {
    fn discussion_repository(&self) -> Result<&dyn DiscussionRepository, AppError> {
        self.repository.discussion_repository().ok_or_else(|| {
            AppError::Unavailable("task discussion repository not configured".into())
        })
    }
    pub async fn discussion(
        &self,
        task: Uuid,
        actor: &Principal,
        query: DiscussionQuery,
    ) -> Result<TaskDiscussion, AppError> {
        authorize(actor, task, false)?;
        validate_query(&query)?;
        self.discussion_repository()?
            .discussion(task, actor, query)
            .await
    }
    pub async fn publish_stage_comment(
        &self,
        task: Uuid,
        actor: &Principal,
        command: PublishStageComment,
    ) -> Result<(StageCommentReceipt, bool), AppError> {
        authorize(actor, task, true)?;
        validate_comment(&command)?;
        self.discussion_repository()?
            .publish_stage_comment(task, actor, command)
            .await
    }
    pub async fn stage_comment_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<StageCommentReceipt, AppError> {
        authorize(actor, task, false)?;
        validate_key(key)?;
        self.discussion_repository()?
            .stage_comment_operation(task, actor, key)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::sdlc::{MachineFence, PmAssignment};
    use domain::sdlc_lifecycle::{DeliveryFence, DeliveryGrant};

    fn actors() -> (Uuid, Principal, Principal) {
        let task = Uuid::new_v4();
        let assignment = PmAssignment {
            assignment_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            agent_id: Uuid::new_v4(),
            version: 1,
            machine_subject: "pm-runtime".into(),
        };
        let pm = Principal {
            subject: assignment.machine_subject.clone(),
            human_session: false,
            trusted_human: false,
            scopes: [
                assignment.scope(task),
                "task-tracker:read".into(),
                "task-tracker:write".into(),
            ]
            .into(),
        };
        let grant = DeliveryGrant {
            task_id: task,
            assignment_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            agent_id: Uuid::new_v4(),
            fencing_token: 7,
        };
        let delivery = Principal {
            subject: "review-runtime".into(),
            scopes: [
                format!(
                    "task-tracker:sdlc:delivery:{}:{}:{}:{}:{}",
                    grant.task_id,
                    grant.assignment_id,
                    grant.execution_id,
                    grant.agent_id,
                    grant.fencing_token
                ),
                "task-tracker:read".into(),
                "task-tracker:write".into(),
            ]
            .into(),
            ..pm.clone()
        };
        (task, pm, delivery)
    }

    #[test]
    fn only_one_exact_machine_grant_may_read_and_write_its_task() {
        let (task, pm, delivery) = actors();
        for actor in [&pm, &delivery] {
            for write in [false, true] {
                authorize(actor, task, write).unwrap();
                assert!(authorize(actor, Uuid::new_v4(), write).is_err());
                let mut changed = actor.clone();
                changed.human_session = true;
                assert!(authorize(&changed, task, write).is_err());
                let mut changed = actor.clone();
                changed.scopes.insert("task-tracker:admin".into());
                assert!(authorize(&changed, task, write).is_err());
            }
            let mut read_only = actor.clone();
            read_only.scopes.remove("task-tracker:write");
            authorize(&read_only, task, false).unwrap();
            assert!(authorize(&read_only, task, true).is_err());
            let mut write_only = actor.clone();
            write_only.scopes.remove("task-tracker:read");
            assert!(authorize(&write_only, task, false).is_err());
            assert!(authorize(&write_only, task, true).is_err());
        }
        let mut mixed = pm.clone();
        mixed.scopes.extend(delivery.scopes);
        assert!(authorize(&mixed, task, true).is_err());
        let generic = Principal {
            scopes: ["task-tracker:read".into(), "task-tracker:write".into()].into(),
            ..pm
        };
        assert!(authorize(&generic, task, false).is_err());
    }

    #[test]
    fn pagination_is_bounded_and_requires_the_original_snapshot() {
        assert_eq!(validate_query(&DiscussionQuery::default()).unwrap(), 100);
        for limit in [1, 100, 500] {
            validate_query(&DiscussionQuery {
                limit: Some(limit),
                after: Some(Uuid::new_v4()),
                snapshot: Some(Uuid::new_v4()),
            })
            .unwrap();
        }
        for query in [
            DiscussionQuery {
                limit: Some(0),
                ..Default::default()
            },
            DiscussionQuery {
                limit: Some(501),
                ..Default::default()
            },
            DiscussionQuery {
                after: Some(Uuid::new_v4()),
                ..Default::default()
            },
            DiscussionQuery {
                snapshot: Some(Uuid::nil()),
                ..Default::default()
            },
        ] {
            assert!(validate_query(&query).is_err());
        }
    }

    #[test]
    fn comment_keys_remain_addressable_and_delivery_routes_remain_own_task_only() {
        let (task, _, actor) = actors();
        for key in [
            "",
            ".",
            "..",
            "with/slash",
            "with\\slash",
            "two words",
            "\n",
        ] {
            assert!(validate_key(key).is_err(), "{key:?}");
        }
        validate_key("fleet-stage-comment:actual-run").unwrap();
        let base = format!("/api/v1/issues/{task}/sdlc/discussion");
        for (method, path) in [
            ("GET", base.clone()),
            ("POST", base.clone()),
            ("GET", format!("{base}/operations/fixture:comment")),
        ] {
            crate::sdlc_lifecycle::delivery_request_policy(&actor, method, &path).unwrap();
        }
        for (method, path) in [
            ("DELETE", base.clone()),
            ("HEAD", base.clone()),
            ("POST", format!("{base}/operations/fixture:comment")),
            ("GET", format!("{base}/")),
            ("GET", format!("{base}/operations/..")),
            (
                "GET",
                format!("/api/v1/issues/{}/sdlc/discussion", Uuid::new_v4()),
            ),
        ] {
            assert!(crate::sdlc_lifecycle::delivery_request_policy(&actor, method, &path).is_err());
        }
    }

    #[test]
    fn comments_preserve_unicode_newlines_and_reject_empty_oversized_or_invalid_fences() {
        let fence = CommentFence::Delivery {
            fence: DeliveryFence {
                assignment_id: Uuid::new_v4(),
                execution_id: Uuid::new_v4(),
                fencing_token: 1,
                lease_id: Uuid::new_v4(),
                expected_lease_version: 1,
            },
        };
        let mut command = PublishStageComment {
            fence,
            body: "Выполнено: анализ\nПроверки и ограничения.".into(),
            idempotency_key: "stage-comment:run-1".into(),
        };
        validate_comment(&command).unwrap();
        for body in [" ".into(), "\0bad".into(), "я".repeat(100_001)] {
            command.body = body;
            assert!(validate_comment(&command).is_err());
        }
        command.body = "Actual result".into();
        command.fence = CommentFence::Pm {
            fence: MachineFence {
                assignment_id: Uuid::new_v4(),
                execution_id: Uuid::new_v4(),
                agent_id: Uuid::new_v4(),
                assignment_version: 1,
            },
            lease_id: Uuid::new_v4(),
            expected_lease_version: 1,
        };
        validate_comment(&command).unwrap();
        if let CommentFence::Pm {
            expected_lease_version,
            ..
        } = &mut command.fence
        {
            *expected_lease_version = 0;
        }
        assert!(validate_comment(&command).is_err());
        let mut raw = serde_json::to_value(command).unwrap();
        raw["owner_subject"] = serde_json::json!("borrowed-owner");
        assert!(serde_json::from_value::<PublishStageComment>(raw).is_err());
    }
}
