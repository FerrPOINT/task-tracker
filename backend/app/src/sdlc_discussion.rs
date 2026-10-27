use crate::sdlc::SdlcService;
use domain::sdlc::Principal;
use domain::sdlc_discussion::*;
use shared::AppError;
use uuid::Uuid;

pub fn authorize(actor: &Principal, task: Uuid, _write: bool) -> Result<(), AppError> {
    if task.is_nil() {
        return Err(AppError::Forbidden);
    }
    // The repository checks current assignment and project ACL in the transaction.
    crate::sdlc_configured_cli::authorize_configured_identity(actor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor() -> Principal {
        Principal {
            subject: Uuid::new_v4().to_string(),
            human_session: false,
            trusted_human: false,
            scopes: ["task-tracker:read".into(), "task-tracker:write".into()].into(),
        }
    }

    #[test]
    fn discussion_does_not_turn_human_or_task_tokens_into_permanent_identity() {
        let task = Uuid::new_v4();
        let actor = actor();
        assert!(authorize(&actor, task, false).is_ok());
        assert!(authorize(&actor, task, true).is_ok());
        assert!(authorize(&actor, Uuid::nil(), false).is_err());
        for scope in ["task-tracker:read", "task-tracker:write"] {
            let mut invalid = actor.clone();
            invalid.scopes.remove(scope);
            assert!(authorize(&invalid, task, true).is_err());
        }
        for human_session in [false, true] {
            let mut invalid = actor.clone();
            invalid.human_session = human_session;
            invalid.trusted_human = !human_session;
            assert!(authorize(&invalid, task, false).is_err());
        }
        let mut invalid = actor;
        invalid
            .scopes
            .insert(format!("task-tracker:sdlc:pm:{task}:obsolete"));
        assert!(authorize(&invalid, task, true).is_err());
    }

    #[test]
    fn pagination_requires_a_bounded_same_snapshot_scan() {
        assert_eq!(validate_query(&DiscussionQuery::default()).unwrap(), 100);
        let id = Uuid::new_v4();
        assert_eq!(
            validate_query(&DiscussionQuery {
                limit: Some(1),
                after: Some(id),
                snapshot: Some(id)
            })
            .unwrap(),
            1
        );
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
                after: Some(id),
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
    fn comment_is_closed_business_assignment_wire_without_native_files_or_lease() {
        let value = serde_json::json!({"fence":{"assignment_id":Uuid::new_v4(),
            "execution_id":Uuid::new_v4(),"agent_id":Uuid::new_v4(),"assignment_version":1},
            "body":"Actual findings and evidence","idempotency_key":"comment:actual"});
        let command: PublishStageComment = serde_json::from_value(value.clone()).unwrap();
        validate_comment(&command).unwrap();
        for (key, field) in [
            ("body", serde_json::json!("  ")),
            ("idempotency_key", serde_json::json!("unsafe/key")),
        ] {
            let mut invalid = value.clone();
            invalid[key] = field;
            assert!(validate_comment(&serde_json::from_value(invalid).unwrap()).is_err());
        }
        let mut invalid = value;
        invalid["native_run_id"] = serde_json::json!(Uuid::new_v4());
        assert!(serde_json::from_value::<PublishStageComment>(invalid).is_err());
    }
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
    let fence = &command.fence;
    if fence.assignment_id.is_nil()
        || fence.execution_id.is_nil()
        || fence.agent_id.is_nil()
        || !(1..=domain::sdlc::MAX_SAFE_VERSION).contains(&fence.assignment_version)
    {
        return Err(AppError::validation("invalid comment assignment fence"));
    }
    Ok(())
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
