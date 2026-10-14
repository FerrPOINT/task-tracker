//! Child of critical_blockers: the same current-PM read guard on both sides of blob I/O.
use super::*;
use shared::AttachmentId;

struct Snapshot {
    blocker: CriticalBlockerReceipt,
    item: domain::Attachment,
    seal: Seal,
}

async fn snapshot(
    repo: &PostgresSdlcRepository,
    actor: &Principal,
    id: AttachmentId,
) -> Result<Snapshot, AppError> {
    let task = app::sdlc::pm_grant_task(actor)?.ok_or(AppError::Forbidden)?;
    let tx = repo.db.begin().await.map_err(map_db)?;
    let state = repo.load_read(&tx, task, actor).await?;
    owner_access(&tx, &state).await?;
    if !repo
        .config
        .single_task_project_ids
        .contains(&state.project_id)
    {
        return Err(AppError::Unavailable(
            "single-task artifact policy disabled".into(),
        ));
    }
    let row = tx
        .query_one(statement(
            "SELECT v.after_blocker AS payload FROM sdlc_critical_blocker_heads h
         JOIN sdlc_critical_pm_reviews v ON v.task_id=h.task_id AND v.blocker_id=h.blocker_id
         WHERE h.task_id=$1",
            vec![task.into()],
        ))
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    let blocker: CriticalBlockerReceipt = decode(&row, "payload")?;
    scope(&state, &blocker)?;
    let model = attachment::Entity::find_by_id(id.as_uuid())
        .one(&tx)
        .await
        .map_err(map_db)?
        .ok_or(AppError::Forbidden)?;
    let item = crate::repos::map_attachment(model);
    let seal = Seal::parse(item.storage_key.as_ref())?.ok_or(AppError::Forbidden)?;
    app::attachment_pm_review::evidence(&blocker, &item, &seal)?;
    let (origin, binding) =
        crate::sdlc::attachment_artifacts::provenance(&tx, &item, &seal).await?;
    if origin.agent_id != blocker.agent_id
        || origin.owner_subject != blocker.owner_subject
        || origin.assignment_hash != blocker.assignment_hash
        || binding.machine_subject != blocker.producer_subject
        || binding.chat_id != blocker.chat_id
        || binding.run_id != blocker.run_id
    {
        return Err(AppError::conflict(
            "critical evidence origin differs from saved blocker",
        ));
    }
    // The actual lease clock is read last, after metadata and retained provenance reads.
    current_pm_reader(&tx, &state, actor, &blocker).await?;
    tx.commit().await.map_err(map_db)?;
    Ok(Snapshot {
        blocker,
        item,
        seal,
    })
}

pub(crate) async fn download(
    db: &DatabaseConnection,
    config: &SdlcConfig,
    actor: &Principal,
    id: AttachmentId,
    storage: &dyn domain::FileStorage,
) -> Result<(domain::Attachment, Vec<u8>), AppError> {
    // Reuse the existing pool/config without connect(), writes, new credentials or a new port.
    let repo = PostgresSdlcRepository {
        db: db.clone(),
        config: config.clone(),
        file_storage: None,
    };
    let before = snapshot(&repo, actor, id).await?;
    // No authorization/task/lease locks across storage I/O: revocation can commit meanwhile.
    let bytes = storage
        .get(
            &before.item.issue_id.to_string(),
            before.item.storage_key.as_ref(),
        )
        .await?;
    app::attachment_artifacts::verify_bytes(&before.item, &bytes)?;
    let after = snapshot(&repo, actor, id).await?;
    if before.blocker != after.blocker
        || before.seal != after.seal
        || app::sdlc::canonical_hash(&before.item)? != app::sdlc::canonical_hash(&after.item)?
    {
        return Err(AppError::conflict(
            "critical evidence read scope changed during storage read",
        ));
    }
    app::attachment_artifacts::verify_bytes(&after.item, &bytes)?;
    Ok((after.item, bytes))
}
