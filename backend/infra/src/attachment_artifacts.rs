//! Child module of `sdlc`: existing attachment rows/blobs and owner transaction only.
use super::*;
use crate::entities::attachment;
use domain::attachment_artifact::{ArtifactBinding, ArtifactUpload, Seal};
use domain::sdlc_lifecycle::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use shared::{AttachmentId, IssueId, UserId};

fn decode<T: serde::de::DeserializeOwned>(row: &QueryResult, field: &str) -> Result<T, AppError> {
    serde_json::from_value(row.try_get::<Json>("", field).map_err(map_db)?)
        .map_err(|_| AppError::conflict("artifact owner history invalid"))
}

pub(crate) async fn human_access(
    db: &DatabaseConnection,
    issue: IssueId,
    user: UserId,
) -> Result<(), AppError> {
    let tx = db.begin().await.map_err(map_db)?;
    let row = tx
        .query_one(statement(
            "SELECT project_id FROM issues WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",
            vec![issue.as_uuid().into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    local_access(
        &tx,
        row.try_get("", "project_id").map_err(map_db)?,
        user.as_uuid(),
    )
    .await?;
    tx.commit().await.map_err(map_db)
}

async fn local_access(tx: &DatabaseTransaction, project: Uuid, user: Uuid) -> Result<(), AppError> {
    tx.query_one(statement(
        "SELECT id FROM users WHERE id=$1 AND is_active FOR SHARE",
        vec![user.into()],
    ))
    .await
    .map_err(map_db)?
    .ok_or(AppError::Forbidden)?;
    let row = tx
        .query_one(statement(
            "SELECT owner_id FROM projects WHERE id=$1 FOR SHARE",
            vec![project.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    if row.try_get::<Uuid>("", "owner_id").map_err(map_db)? != user {
        tx.query_one(statement(
            "SELECT user_id FROM project_members WHERE project_id=$1 AND user_id=$2 FOR SHARE",
            vec![project.into(), user.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    }
    Ok(())
}

struct Authority {
    user: Uuid,
    head: LifecycleReceipt,
    scheduler: bool,
}

async fn authority(
    tx: &DatabaseTransaction,
    config: &SdlcConfig,
    task: Uuid,
    actor: &Principal,
    write: bool,
) -> Result<Authority, AppError> {
    let scheduler = app::sdlc_lifecycle::scheduler_read(actor, config).is_ok();
    if scheduler && write {
        return Err(AppError::Forbidden);
    }
    if !scheduler && !app::sdlc_lifecycle::has_delivery_grant(&actor.scopes) {
        app::sdlc_configured_cli::authorize_configured_identity(actor)?;
    }
    let issue = tx
        .query_one(statement(
            "SELECT project_id,status_id FROM issues WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",
            vec![task.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    let project: Uuid = issue.try_get("", "project_id").map_err(map_db)?;
    let user = tx
        .query_one(statement(
            "SELECT id FROM users WHERE central_sub=$1 AND is_active FOR SHARE",
            vec![actor.subject.clone().into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?
        .try_get::<Uuid>("", "id")
        .map_err(map_db)?;
    local_access(tx, project, user).await?;
    if !config.single_task_project_ids.contains(&project) {
        return Err(AppError::Unavailable(
            "single-task artifact policy disabled".into(),
        ));
    }
    let state: TaskState = decode(
        &tx.query_one(statement(
            "SELECT state FROM sdlc_tasks WHERE task_id=$1 FOR UPDATE",
            vec![task.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?,
        "state",
    )?;
    state.require_root()?;
    if state.task_id != task
        || state.project_id != project
        || state.tracker_instance_id != config.instance_id
        || state.draft_flow()? != Some(DraftFlow::SingleTaskV1)
    {
        return Err(AppError::Forbidden);
    }
    let head: LifecycleReceipt = decode(
        &tx.query_one(statement(
            "SELECT receipt FROM sdlc_delivery_tasks WHERE task_id=$1 FOR UPDATE",
            vec![task.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?,
        "receipt",
    )?;
    super::lifecycle::exact_revision(&state, &head)?;
    let grant = if scheduler {
        None
    } else {
        Some(app::sdlc_lifecycle::assigned_producer(actor, &head, write)?)
    };
    if grant.as_ref().is_some_and(|grant| grant.task_id != task) {
        return Err(AppError::Forbidden);
    }
    super::spaces::bind_task(tx, config, &state, actor).await?;
    if head.task.routing.contract_version != 2 {
        return Err(AppError::conflict("single-task artifact routing required"));
    }
    app::sdlc_routing::validate_routes(&head.task.routing.policy.routes)
        .map_err(|_| AppError::conflict("artifact routing history invalid"))?;
    if issue.try_get::<Uuid>("", "status_id").map_err(map_db)?
        != super::lifecycle::mapped_issue_status(tx, head.task.stage.canonical(), head.task.status)
            .await?
    {
        return Err(AppError::conflict("artifact lifecycle projection drift"));
    }
    let assignment = head.assignment.as_ref().ok_or(AppError::Forbidden)?;
    let binding = head.binding.as_ref().ok_or(AppError::Forbidden)?;
    if assignment.task_id != task
        || assignment.owner_subject != head.task.owner_subject
        || assignment.stage != head.task.stage
        || assignment.cycle != head.task.cycle
        || assignment.routing != head.task.routing
        || assignment.stage.role() != Some(assignment.role)
        || assignment.assignment_hash != app::sdlc_lifecycle::assignment_hash(assignment)?
    {
        return Err(AppError::conflict("artifact assignment drift"));
    }
    let saved: DeliveryAssignment = decode(
        &tx.query_one(statement(
            "SELECT payload FROM sdlc_delivery_assignments WHERE assignment_id=$1 AND task_id=$2",
            vec![assignment.assignment_id.into(), task.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?,
        "payload",
    )?;
    let run: RunBinding = decode(
        &tx.query_one(statement(
            "SELECT payload FROM sdlc_delivery_run_bindings WHERE assignment_id=$1 AND run_id=$2",
            vec![assignment.assignment_id.into(), binding.run_id.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?,
        "payload",
    )?;
    if &saved != assignment || &run != binding {
        return Err(AppError::conflict("artifact binding history drift"));
    }
    if let Some(grant) = &grant {
        app::sdlc_lifecycle::authorize_delivery_assignment(grant, assignment)?;
        if binding.machine_subject != actor.subject {
            return Err(AppError::Forbidden);
        }
    }
    if write {
        if head
            .lease
            .as_ref()
            .is_none_or(|lease| lease.holder_subject != config.reservation_scheduler_subject)
        {
            return Err(AppError::Forbidden);
        }
        ensure_live(tx, &head).await?;
    }
    Ok(Authority {
        user,
        head,
        scheduler: grant.is_none(),
    })
}

async fn ensure_live(tx: &DatabaseTransaction, head: &LifecycleReceipt) -> Result<(), AppError> {
    let assignment = head.assignment.as_ref().ok_or(AppError::Forbidden)?;
    let lease = head.lease.as_ref().ok_or(AppError::Forbidden)?;
    let now: chrono::DateTime<chrono::Utc> = tx
        .query_one(statement("SELECT clock_timestamp() AS now", vec![]))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?
        .try_get("", "now")
        .map_err(map_db)?;
    if head.task.runtime != RuntimeState::Running
        || head.task.status != LifecycleStatus::InProgress
        || lease.lease_id != assignment.lease_id
        || lease.version < 1
        || lease.expires_at <= now
    {
        return Err(AppError::conflict(
            "artifact publisher not running or lease expired",
        ));
    }
    // The scheduler owns this lease. Agent identity is the run binding, never lease.holder_subject.
    Ok(())
}

pub(super) async fn provenance(
    tx: &DatabaseTransaction,
    attachment: &domain::Attachment,
    seal: &Seal,
) -> Result<(DeliveryAssignment, RunBinding), AppError> {
    seal.validate_attachment(attachment)?;
    let assignment: DeliveryAssignment = decode(
        &tx.query_one(statement(
            "SELECT payload FROM sdlc_delivery_assignments WHERE assignment_id=$1 AND task_id=$2",
            vec![
                seal.binding.assignment_id.into(),
                attachment.issue_id.as_uuid().into(),
            ],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?,
        "payload",
    )?;
    let binding: RunBinding = decode(
        &tx.query_one(statement(
            "SELECT payload FROM sdlc_delivery_run_bindings WHERE assignment_id=$1 AND run_id=$2",
            vec![seal.binding.assignment_id.into(), seal.run_id.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?,
        "payload",
    )?;
    if assignment.execution_id != seal.binding.execution_id
        || assignment.assignment_id != seal.binding.assignment_id
        || assignment.role != seal.binding.role
        || assignment.task_id != attachment.issue_id.as_uuid()
        || binding.run_id != seal.run_id
        || assignment.assignment_hash != app::sdlc_lifecycle::assignment_hash(&assignment)?
    {
        return Err(AppError::conflict("artifact provenance mismatch"));
    }
    tx.query_one(statement(
        "SELECT id FROM users WHERE id=$1 AND central_sub=$2",
        vec![
            attachment.author_id.as_uuid().into(),
            binding.machine_subject.clone().into(),
        ],
    ))
    .await
    .map_err(map_db)?
    .ok_or_else(|| AppError::conflict("artifact author differs from actual producer"))?;
    Ok((assignment, binding))
}

async fn allowed(
    tx: &DatabaseTransaction,
    auth: &Authority,
    attachment: &domain::Attachment,
) -> Result<bool, AppError> {
    let Some(seal) = Seal::parse(attachment.storage_key.as_ref())? else {
        return Ok(false);
    };
    provenance(tx, attachment, &seal).await?;
    let current = auth.head.assignment.as_ref().ok_or(AppError::Forbidden)?;
    if !auth.scheduler && current.assignment_id == seal.binding.assignment_id {
        return Ok(true);
    }
    let wanted = seal.artifact().receipt;
    if auth.scheduler {
        if current.assignment_id != seal.binding.assignment_id
            || current.execution_id != seal.binding.execution_id
            || current.role != seal.binding.role
        {
            return Ok(false);
        }
        let Some(row) = tx
            .query_one(statement(
                "SELECT payload FROM sdlc_delivery_proposals WHERE assignment_id=$1 AND task_id=$2",
                vec![current.assignment_id.into(), current.task_id.into()],
            ))
            .await
            .map_err(map_db)?
        else {
            return Ok(false);
        };
        let proposal: DeliveryProposalReceipt = decode(&row, "payload")?;
        return Ok(proposal.task_id == current.task_id
            && proposal.assignment_id == current.assignment_id
            && proposal.execution_id == current.execution_id
            && proposal.agent_id == current.agent_id
            && auth
                .head
                .binding
                .as_ref()
                .is_some_and(|binding| binding.machine_subject == proposal.producer_subject)
            && proposal.result.role() == current.role
            && proposal.result.artifacts().contains(&&wanted));
    }
    let mut previous = current.previous_result_id;
    let mut visited = std::collections::HashSet::new();
    while let Some(id) = previous {
        if !visited.insert(id) || visited.len() > 128 {
            return Err(AppError::conflict("artifact predecessor history invalid"));
        }
        let Some(row) = tx
            .query_one(statement(
                "SELECT payload FROM sdlc_delivery_results WHERE result_id=$1 AND task_id=$2",
                vec![id.into(), current.task_id.into()],
            ))
            .await
            .map_err(map_db)?
        else {
            return Ok(false);
        }; // Legacy Analysis is not retroactively a sealed artifact.
        let result: AcceptedDeliveryResult = decode(&row, "payload")?;
        let assignment: DeliveryAssignment = decode(&tx.query_one(statement("SELECT payload FROM sdlc_delivery_assignments WHERE assignment_id=$1 AND task_id=$2",vec![result.assignment_id.into(),current.task_id.into()]))
            .await.map_err(map_db)?.ok_or(AppError::Forbidden)?,"payload")?;
        if result.result_id != id
            || result.task_id != current.task_id
            || assignment.assignment_id != result.assignment_id
            || assignment.task_id != current.task_id
            || result.output.role() != assignment.role
            || assignment.assignment_hash != app::sdlc_lifecycle::assignment_hash(&assignment)?
        {
            return Err(AppError::conflict("artifact accepted history drift"));
        }
        if result.assignment_id == seal.binding.assignment_id
            && result.output.artifacts().contains(&&wanted)
        {
            return Ok(true);
        }
        previous = assignment.previous_result_id;
    }
    Ok(false)
}

pub(crate) async fn publish(
    db: &DatabaseConnection,
    config: &SdlcConfig,
    actor: &Principal,
    issue: IssueId,
    upload: ArtifactUpload,
    storage: &dyn domain::FileStorage,
) -> Result<domain::Attachment, AppError> {
    if upload.bytes.is_empty() {
        return Err(AppError::invalid_input("file is empty"));
    }
    let tx = db.begin().await.map_err(map_db)?;
    let auth = authority(&tx, config, issue.as_uuid(), actor, true).await?;
    let assignment = auth.head.assignment.as_ref().ok_or(AppError::Forbidden)?;
    let seal = Seal::new(
        issue.as_uuid(),
        ArtifactBinding {
            assignment_id: assignment.assignment_id,
            execution_id: assignment.execution_id,
            role: assignment.role,
        },
        auth.head
            .binding
            .as_ref()
            .ok_or(AppError::Forbidden)?
            .run_id,
        &upload.file_name,
        &upload.content_type,
        app::attachment_artifacts::raw_sha256(&upload.bytes),
    )?;
    if let Some(model) = attachment::Entity::find_by_id(seal.artifact_id)
        .one(&tx)
        .await
        .map_err(map_db)?
    {
        let attachment = crate::repos::map_attachment(model);
        provenance(&tx, &attachment, &seal).await?;
        if attachment.author_id.as_uuid() != auth.user
            || attachment.storage_key.as_ref() != seal.key()
        {
            return Err(AppError::conflict("artifact replay identity conflict"));
        }
        let bytes = storage.get(&issue.to_string(), &seal.key()).await?;
        app::attachment_artifacts::verify_bytes(&attachment, &bytes)?;
        tx.commit().await.map_err(map_db)?;
        return Ok(attachment);
    }
    let size_bytes = upload.bytes.len() as i64;
    storage
        .put(&issue.to_string(), &seal.key(), upload.bytes)
        .await?;
    let item = domain::Attachment {
        id: AttachmentId::from_uuid(seal.artifact_id),
        issue_id: issue,
        author_id: UserId::from_uuid(auth.user),
        file_name: upload.file_name.into(),
        content_type: upload.content_type.into(),
        size_bytes,
        storage_key: seal.key().into(),
        created_at: shared::now(),
    };
    let retained = storage
        .get(&issue.to_string(), item.storage_key.as_ref())
        .await?;
    app::attachment_artifacts::verify_bytes(&item, &retained)?;
    // Locks retain assignment/ACL throughout I/O; check the actual clock again before metadata commit.
    ensure_live(&tx, &auth.head).await?;
    let saved = attachment::ActiveModel {
        id: Set(item.id.as_uuid()),
        issue_id: Set(issue.as_uuid()),
        author_id: Set(auth.user),
        file_name: Set(item.file_name.to_string()),
        content_type: Set(item.content_type.to_string()),
        size_bytes: Set(item.size_bytes),
        storage_key: Set(item.storage_key.to_string()),
        created_at: Set(item.created_at),
    }
    .insert(&tx)
    .await
    .map_err(map_db)?;
    tx.commit().await.map_err(map_db)?;
    // Never delete a sealed blob on DB error: exact retry safely adopts the same bytes; no loser deletes a winner.
    Ok(crate::repos::map_attachment(saved))
}

pub(crate) async fn list(
    db: &DatabaseConnection,
    config: &SdlcConfig,
    actor: &Principal,
    issue: IssueId,
) -> Result<Vec<domain::Attachment>, AppError> {
    let tx = db.begin().await.map_err(map_db)?;
    let auth = authority(&tx, config, issue.as_uuid(), actor, false).await?;
    let models = attachment::Entity::find()
        .filter(attachment::Column::IssueId.eq(issue.as_uuid()))
        .all(&tx)
        .await
        .map_err(map_db)?;
    let mut items = Vec::new();
    for model in models {
        let item = crate::repos::map_attachment(model);
        if allowed(&tx, &auth, &item).await? {
            items.push(item);
        }
    }
    items.sort_by_key(|item| item.id.as_uuid());
    tx.commit().await.map_err(map_db)?;
    Ok(items)
}

pub(crate) async fn download(
    db: &DatabaseConnection,
    config: &SdlcConfig,
    actor: &Principal,
    id: AttachmentId,
    storage: &dyn domain::FileStorage,
) -> Result<(domain::Attachment, Vec<u8>), AppError> {
    if app::sdlc::has_pm_grant(&actor.scopes) {
        return super::critical_blockers::pm_evidence::download(db, config, actor, id, storage)
            .await;
    }
    if app::sdlc_configured_cli::authorize_configured_identity(actor).is_ok() {
        // Select the read policy from current business state, not a display role
        // or a failed technical read. The PM policy rechecks full custody and
        // fresh ACL on both sides of blob I/O before returning any data.
        let row = db
            .query_one(statement(
                "SELECT EXISTS(SELECT 1 FROM attachments a
            JOIN sdlc_tasks t ON t.task_id=a.issue_id WHERE a.id=$1
            AND t.state->>'interaction_protocol'='configured-cli-v1' AND t.state->>'stage'='Draft'
            AND t.state->'assignment'->>'machine_subject'=$2) AS pm",
                vec![id.as_uuid().into(), actor.subject.clone().into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or(AppError::Forbidden)?;
        if row.try_get::<bool>("", "pm").map_err(map_db)? {
            return super::critical_blockers::pm_evidence::download(db, config, actor, id, storage)
                .await;
        }
    }
    let tx = db.begin().await.map_err(map_db)?;
    let model = attachment::Entity::find_by_id(id.as_uuid())
        .one(&tx)
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    let item = crate::repos::map_attachment(model);
    let auth = authority(&tx, config, item.issue_id.as_uuid(), actor, false).await?;
    if !allowed(&tx, &auth, &item).await? {
        return Err(AppError::Forbidden);
    }
    let bytes = storage
        .get(&item.issue_id.to_string(), item.storage_key.as_ref())
        .await?;
    app::attachment_artifacts::verify_bytes(&item, &bytes)?;
    tx.commit().await.map_err(map_db)?;
    Ok((item, bytes))
}
