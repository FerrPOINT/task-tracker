use super::*;
use domain::sdlc_discussion::*;

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
        app::sdlc_configured_cli::authorize_configured_identity(actor)?;
        let assignment = super::lifecycle::configured_assignment(tx, state, actor)
            .await?
            .ok_or(AppError::Forbidden)?;
        let fence = MachineFence {
            assignment_id: assignment.assignment_id,
            execution_id: assignment.execution_id,
            agent_id: assignment.agent_id,
            assignment_version: assignment.version,
        };
        if !state.configured_cli()
            || assignment.machine_subject != actor.subject
            || expected.is_some_and(|expected| expected != &fence)
        {
            return Err(AppError::conflict("current comment assignment changed"));
        }
        let user = Self::project_access(tx, state.project_id, actor).await?.0;
        Ok(CurrentAuthor {
            user,
            agent: assignment.agent_id,
            revision: state.current_revision(),
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
    app::sdlc_configured_cli::authorize_configured_identity(actor)
}

async fn verify_comment(
    tx: &DatabaseTransaction,
    receipt: &StageCommentReceipt,
) -> Result<(), AppError> {
    let row = tx.query_one(statement(
        "SELECT body,created_at,updated_at FROM comments WHERE id=$1 AND issue_id=$2 AND author_id=$3 FOR SHARE",
        vec![receipt.comment_id.into(), receipt.task_id.into(), receipt.author_id.into()],
    )).await.map_err(map_db)?.ok_or_else(|| AppError::conflict("stage comment was removed"))?;
    let body: String = row.try_get("", "body").map_err(map_db)?;
    if app::attachment_artifacts::raw_sha256(body.as_bytes()) != receipt.body_sha256
        || row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "created_at")
            .map_err(map_db)?
            != receipt.created_at
        || row
            .try_get::<chrono::DateTime<chrono::Utc>>("", "updated_at")
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
    if !state.configured_cli() {
        return Ok(());
    }
    let row = tx
        .query_one(statement(
            "SELECT result FROM sdlc_idempotency WHERE task_id=$1 AND actor_subject=$2
         AND result->'fence'->>'assignment_id'=$3
         AND result->'fence'->>'execution_id'=$4
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
        || receipt.fence.assignment_id != assignment
        || receipt.fence.execution_id != execution
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
            self.discussion_author(&tx, &state, actor, Some(&receipt.fence))
                .await?;
            if receipt.requirement_revision != state.current_revision() {
                return Err(AppError::conflict("comment revision changed"));
            }
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
             SELECT $1,$2,$3,$4,stamp,stamp FROM (SELECT clock_timestamp() AS stamp) t RETURNING created_at",
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
        let state = self.load(&tx, task, actor).await?;
        let saved = tx.query_one(statement(
            "SELECT result FROM sdlc_idempotency WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![task.into(), actor.subject.clone().into(), key.into()],
        )).await.map_err(map_db)?.ok_or_else(|| AppError::not_found("stage comment operation", key))?;
        let receipt = decode_receipt(saved.try_get("", "result").map_err(map_db)?)?;
        receipt_authority(actor, &receipt)?;
        self.discussion_author(&tx, &state, actor, Some(&receipt.fence))
            .await?;
        if receipt.requirement_revision != state.current_revision() {
            return Err(AppError::conflict("comment revision changed"));
        }
        verify_comment(&tx, &receipt).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(receipt)
    }
}
