use super::*;
use domain::sdlc_discussion::*;

/// Real comment writes for explicit disposable-PG fixtures; no native execution proof.
#[cfg(test)]
pub(super) async fn test_pm_comment(repo: &PostgresSdlcRepository, task: Uuid) {
    let tx = repo.db.begin().await.unwrap();
    let row = tx
        .query_one(statement(
            "SELECT state FROM sdlc_tasks WHERE task_id=$1",
            vec![task.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    let state: TaskState =
        serde_json::from_value(row.try_get::<Json>("", "state").unwrap()).unwrap();
    if state.draft_flow().unwrap() != Some(DraftFlow::SingleTaskV1) {
        tx.rollback().await.unwrap();
        return;
    }
    let assignment = state.assignment.as_ref().unwrap();
    let reservation = PostgresSdlcRepository::reservation(&tx, &state, assignment.execution_id)
        .await
        .unwrap();
    let lease = PostgresSdlcRepository::lease_current(&tx, &reservation)
        .await
        .unwrap()
        .unwrap();
    let actor = Principal {
        subject: assignment.machine_subject.clone(),
        human_session: false,
        trusted_human: false,
        scopes: [
            "task-tracker:read".into(),
            "task-tracker:write".into(),
            assignment.scope(task),
        ]
        .into(),
    };
    let fence = CommentFence::Pm {
        fence: MachineFence {
            assignment_id: assignment.assignment_id,
            execution_id: assignment.execution_id,
            agent_id: assignment.agent_id,
            assignment_version: assignment.version,
        },
        lease_id: lease.lease.lease_id,
        expected_lease_version: lease.lease.version,
    };
    let key = format!(
        "fixture:pm-comment:{}:{}",
        assignment.execution_id, lease.lease.version
    );
    tx.rollback().await.unwrap();
    repo.publish_stage_comment(task,&actor,PublishStageComment { fence, body: "Synthetic PG fixture: PM work, checked inputs and requested handoff. Not native execution evidence.".into(), idempotency_key: key }).await.unwrap();
}

#[cfg(test)]
pub(super) async fn test_delivery_comment(repo: &PostgresSdlcRepository, task: Uuid) {
    let tx = repo.db.begin().await.unwrap();
    let row = tx
        .query_one(statement(
            "SELECT state FROM sdlc_tasks WHERE task_id=$1",
            vec![task.into()],
        ))
        .await
        .unwrap()
        .unwrap();
    let state: TaskState =
        serde_json::from_value(row.try_get::<Json>("", "state").unwrap()).unwrap();
    if state.draft_flow().unwrap() != Some(DraftFlow::SingleTaskV1) {
        tx.rollback().await.unwrap();
        return;
    }
    let head = super::lifecycle::current(&tx, task).await.unwrap().unwrap();
    let assignment = head.assignment.as_ref().unwrap();
    let actor = Principal {
        subject: head.binding.as_ref().unwrap().machine_subject.clone(),
        human_session: false,
        trusted_human: false,
        scopes: [
            "task-tracker:read".into(),
            "task-tracker:write".into(),
            assignment.scope(),
        ]
        .into(),
    };
    let fence = CommentFence::Delivery {
        fence: domain::sdlc_lifecycle::DeliveryFence {
            assignment_id: assignment.assignment_id,
            execution_id: assignment.execution_id,
            fencing_token: assignment.fencing_token,
            lease_id: assignment.lease_id,
            expected_lease_version: head.lease.as_ref().unwrap().version,
        },
    };
    let key = format!("fixture:delivery-comment:{}", assignment.execution_id);
    tx.rollback().await.unwrap();
    repo.publish_stage_comment(task,&actor,PublishStageComment { fence, body: "Synthetic PG fixture: role work, checks and requested handoff. Not native execution evidence.".into(), idempotency_key: key }).await.unwrap();
}

struct CurrentAuthor {
    user: Uuid,
    agent: Uuid,
    revision: Option<i64>,
}

impl PostgresSdlcRepository {
    async fn discussion_author(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
        expected: Option<&CommentFence>,
    ) -> Result<CurrentAuthor, AppError> {
        let task = state.task_id;
        if state.draft_flow()? != Some(DraftFlow::SingleTaskV1) {
            return Err(AppError::Forbidden);
        }
        if app::sdlc::pm_grant_task(actor)?.is_some() {
            app::sdlc::authorize_pm_read(state, actor)?;
            let reservation = Self::pm_lease_authority(tx, state, actor).await?;
            let lease = Self::lease_current(tx, &reservation)
                .await?
                .ok_or_else(|| AppError::conflict("PM discussion requires an active lease"))?;
            let now = super::execution_lease::clock(tx).await?;
            if lease.lease.holder_subject != actor.subject || lease.lease.expires_at <= now {
                return Err(AppError::conflict("PM discussion lease expired or foreign"));
            }
            let fence = CommentFence::Pm {
                fence: MachineFence {
                    assignment_id: reservation.assignment.assignment_id,
                    execution_id: reservation.assignment.execution_id,
                    agent_id: reservation.assignment.agent_id,
                    assignment_version: reservation.assignment.version,
                },
                lease_id: lease.lease.lease_id,
                expected_lease_version: lease.lease.version,
            };
            if expected.is_some_and(|expected| expected != &fence) {
                return Err(AppError::conflict("stale PM comment fence"));
            }
            let user = Self::project_access(tx, state.project_id, actor).await?.0;
            return Ok(CurrentAuthor {
                user,
                agent: reservation.assignment.agent_id,
                revision: state.current_revision(),
            });
        }
        // Reuse the same task/project/run/routing/ledger checks as sealed role artifacts.
        let authority = super::producer_access::authority(
            tx,
            &self.config,
            task,
            actor,
            false,
            super::producer_access::ProjectGate::CurrentAssignment,
        )
        .await?;
        if authority.scheduler {
            return Err(AppError::Forbidden);
        }
        super::producer_access::ensure_live(tx, &authority.head).await?;
        let assignment = authority
            .head
            .assignment
            .as_ref()
            .ok_or(AppError::Forbidden)?;
        let lease = authority.head.lease.as_ref().ok_or(AppError::Forbidden)?;
        if lease.holder_subject != self.config.reservation_scheduler_subject {
            return Err(AppError::Forbidden);
        }
        if let Some(expected) = expected {
            let CommentFence::Delivery { fence } = expected else {
                return Err(AppError::Forbidden);
            };
            super::lifecycle::exact_fence(
                &authority.head,
                fence,
                super::lifecycle::now(tx).await?,
                false,
            )?;
        }
        Ok(CurrentAuthor {
            user: authority.user,
            agent: assignment.agent_id,
            revision: authority.state.current_revision(),
        })
    }
}

fn decode_receipt(value: Json) -> Result<StageCommentReceipt, AppError> {
    serde_json::from_value(value)
        .map_err(|_| AppError::conflict("stage comment operation mismatch"))
}

fn receipt_authority(actor: &Principal, receipt: &StageCommentReceipt) -> Result<(), AppError> {
    if receipt.author_subject != actor.subject {
        return Err(AppError::Forbidden);
    }
    match &receipt.fence {
        CommentFence::Pm { fence, .. } => {
            let assignment = PmAssignment {
                assignment_id: fence.assignment_id,
                execution_id: fence.execution_id,
                agent_id: receipt.agent_id,
                version: fence.assignment_version,
                machine_subject: actor.subject.clone(),
            };
            if fence.agent_id != receipt.agent_id
                || !actor.scopes.contains(&assignment.scope(receipt.task_id))
            {
                return Err(AppError::Forbidden);
            }
        }
        CommentFence::Delivery { fence } => {
            let grant = app::sdlc_lifecycle::delivery_grant(actor)?.ok_or(AppError::Forbidden)?;
            if grant.task_id != receipt.task_id
                || grant.assignment_id != fence.assignment_id
                || grant.execution_id != fence.execution_id
                || grant.agent_id != receipt.agent_id
                || grant.fencing_token != fence.fencing_token
            {
                return Err(AppError::Forbidden);
            }
        }
    }
    Ok(())
}

async fn verify_comment(
    tx: &DatabaseTransaction,
    receipt: &StageCommentReceipt,
) -> Result<(), AppError> {
    let row = tx.query_one(statement(
        "SELECT body,created_at FROM comments WHERE id=$1 AND issue_id=$2 AND author_id=$3 FOR SHARE",
        vec![receipt.comment_id.into(), receipt.task_id.into(), receipt.author_id.into()],
    )).await.map_err(map_db)?.ok_or_else(|| AppError::conflict("stage comment was removed"))?;
    let body: String = row.try_get("", "body").map_err(map_db)?;
    if app::attachment_artifacts::raw_sha256(body.as_bytes()) != receipt.body_sha256
        || row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "created_at")
            .map_err(map_db)?
            != receipt.created_at
    {
        return Err(AppError::conflict("stage comment content changed"));
    }
    Ok(())
}

pub(super) async fn require_stage_comment(
    tx: &DatabaseTransaction,
    state: &TaskState,
    subject: &str,
    assignment: Uuid,
    execution: Uuid,
) -> Result<(), AppError> {
    if state.draft_flow()? != Some(DraftFlow::SingleTaskV1) {
        return Ok(());
    }
    let row = tx
        .query_one(statement(
            "SELECT result FROM sdlc_idempotency WHERE task_id=$1 AND actor_subject=$2
         AND result->'fence'->'fence'->>'assignment_id'=$3
         AND result->'fence'->'fence'->>'execution_id'=$4
         AND result->>'requirement_revision' IS NOT DISTINCT FROM $5::text
         AND result ? 'comment_id' ORDER BY (result->>'created_at')::timestamptz DESC LIMIT 1",
            vec![
                state.task_id.into(),
                subject.into(),
                assignment.to_string().into(),
                execution.to_string().into(),
                state.current_revision().map(|r| r.to_string()).into(),
            ],
        ))
        .await
        .map_err(map_db)?
        .ok_or_else(|| AppError::conflict("current stage completion comment required"))?;
    let receipt = decode_receipt(row.try_get("", "result").map_err(map_db)?)?;
    if receipt.task_id != state.task_id
        || receipt.author_subject != subject
        || receipt.fence.assignment_id() != assignment
        || receipt.fence.execution_id() != execution
        || receipt.requirement_revision != state.current_revision()
    {
        return Err(AppError::conflict("stage comment provenance mismatch"));
    }
    verify_comment(tx, &receipt).await
}

async fn cursor(
    tx: &DatabaseTransaction,
    task: Uuid,
    id: Uuid,
) -> Result<chrono::DateTime<chrono::Utc>, AppError> {
    tx.query_one(statement(
        "SELECT created_at FROM comments WHERE issue_id=$1 AND id=$2",
        vec![task.into(), id.into()],
    ))
    .await
    .map_err(map_db)?
    .ok_or_else(|| AppError::conflict("discussion cursor missing; restart the scan"))?
    .try_get("", "created_at")
    .map_err(map_db)
}

#[async_trait]
impl DiscussionRepository for PostgresSdlcRepository {
    async fn discussion(
        &self,
        task: Uuid,
        actor: &Principal,
        query: DiscussionQuery,
    ) -> Result<TaskDiscussion, AppError> {
        app::sdlc_discussion::authorize(actor, task, false)?;
        let limit = app::sdlc_discussion::validate_query(&query)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load_read(&tx, task, actor).await?;
        self.discussion_author(&tx, &state, actor, None).await?;
        let issue = tx
            .query_one(statement(
                "SELECT summary AS title,description FROM issues WHERE id=$1",
                vec![task.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or(AppError::Forbidden)?;
        let snapshot = match query.snapshot {
            Some(id) => Some(id),
            None => tx.query_one(statement(
                "SELECT id FROM comments WHERE issue_id=$1 ORDER BY created_at DESC,id DESC LIMIT 1",
                vec![task.into()],
            )).await.map_err(map_db)?.map(|r| r.try_get::<Uuid>("", "id").map_err(map_db)).transpose()?,
        };
        let mut comments = Vec::new();
        if let Some(upper) = snapshot {
            let through = cursor(&tx, task, upper).await?;
            let after_time = match query.after {
                Some(id) => {
                    let time = cursor(&tx, task, id).await?;
                    if (time, id) > (through, upper) {
                        return Err(AppError::validation("discussion cursor exceeds snapshot"));
                    }
                    Some(time)
                }
                None => None,
            };
            for row in tx.query_all(statement(
                "SELECT c.id,c.author_id,u.display_name AS author_name,c.body,c.created_at,c.updated_at
                 FROM comments c JOIN users u ON u.id=c.author_id
                 WHERE c.issue_id=$1 AND (c.created_at,c.id)<=($2,$3)
                   AND ($4::timestamptz IS NULL OR (c.created_at,c.id)>($4,$5))
                 ORDER BY c.created_at,c.id LIMIT $6",
                vec![task.into(), through.into(), upper.into(), after_time.into(), query.after.into(),
                    (i64::from(limit)+1).into()],
            )).await.map_err(map_db)? {
                comments.push(DiscussionComment { id: row.try_get("", "id").map_err(map_db)?,
                    author_id: row.try_get("", "author_id").map_err(map_db)?,
                    author_name: row.try_get("", "author_name").map_err(map_db)?,
                    body: row.try_get("", "body").map_err(map_db)?,
                    created_at: row.try_get("", "created_at").map_err(map_db)?,
                    updated_at: row.try_get("", "updated_at").map_err(map_db)? });
            }
        }
        let next_after = if comments.len() > usize::from(limit) {
            comments.truncate(usize::from(limit));
            comments.last().map(|c| c.id)
        } else {
            None
        };
        let result = TaskDiscussion {
            task_id: task,
            project_id: state.project_id,
            title: issue.try_get("", "title").map_err(map_db)?,
            description: issue
                .try_get::<Option<String>>("", "description")
                .map_err(map_db)?
                .unwrap_or_default(),
            comments,
            snapshot,
            next_after,
        };
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }

    async fn publish_stage_comment(
        &self,
        task: Uuid,
        actor: &Principal,
        command: PublishStageComment,
    ) -> Result<(StageCommentReceipt, bool), AppError> {
        app::sdlc_discussion::authorize(actor, task, true)?;
        app::sdlc_discussion::validate_comment(&command)?;
        let hash =
            app::sdlc::canonical_hash(&json!({"operation":"stage_comment","command":command}))?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        if let Some(saved) = Self::replay(&tx, task, actor, &command.idempotency_key, &hash).await?
        {
            let receipt = decode_receipt(saved)?;
            receipt_authority(actor, &receipt)?;
            verify_comment(&tx, &receipt).await?;
            tx.commit().await.map_err(map_db)?;
            return Ok((receipt, true));
        }
        let author = self
            .discussion_author(&tx, &state, actor, Some(&command.fence))
            .await?;
        let id = Uuid::now_v7();
        let row = tx
            .query_one(statement(
                "INSERT INTO comments(id,issue_id,author_id,body,created_at,updated_at)
             VALUES($1,$2,$3,$4,clock_timestamp(),clock_timestamp()) RETURNING created_at",
                vec![
                    id.into(),
                    task.into(),
                    author.user.into(),
                    command.body.clone().into(),
                ],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::conflict("stage comment insert missing"))?;
        let receipt = StageCommentReceipt {
            comment_id: id,
            task_id: task,
            author_id: author.user,
            author_subject: actor.subject.clone(),
            agent_id: author.agent,
            fence: command.fence,
            requirement_revision: author.revision,
            body_sha256: app::attachment_artifacts::raw_sha256(command.body.as_bytes()),
            created_at: row.try_get("", "created_at").map_err(map_db)?,
        };
        exec(&tx, "INSERT INTO sdlc_idempotency(task_id,actor_subject,idempotency_key,payload_hash,result)
            VALUES($1,$2,$3,$4,$5)", vec![task.into(), actor.subject.clone().into(),
                command.idempotency_key.into(), hash.into(), json_value(&receipt)?]).await?;
        tx.commit().await.map_err(map_db)?;
        Ok((receipt, false))
    }

    async fn stage_comment_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<StageCommentReceipt, AppError> {
        app::sdlc_discussion::authorize(actor, task, false)?;
        app::sdlc_discussion::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        self.load(&tx, task, actor).await?;
        let saved = tx.query_one(statement(
            "SELECT result FROM sdlc_idempotency WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![task.into(), actor.subject.clone().into(), key.into()],
        )).await.map_err(map_db)?.ok_or_else(|| AppError::not_found("stage comment operation", key))?;
        let receipt = decode_receipt(saved.try_get("", "result").map_err(map_db)?)?;
        receipt_authority(actor, &receipt)?;
        verify_comment(&tx, &receipt).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(receipt)
    }
}
