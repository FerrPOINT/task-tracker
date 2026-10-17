use super::*;

impl PostgresSdlcRepository {
    async fn enrollment_draft(
        &self,
        tx: &DatabaseTransaction,
        task: Uuid,
        actor: &Principal,
    ) -> Result<TaskState, AppError> {
        app::sdlc_source::human_owner(actor)?;
        let state = self.load(tx, task, actor).await?;
        app::sdlc_draft_source_enrollment::authorize_draft(&state, actor)?;
        let row = tx
            .query_one(statement(
                "SELECT EXISTS(SELECT 1 FROM sdlc_confirmations WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_task_source_bindings WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_analysis_intents WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_analysis_reservations WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_analysis_results WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_delivery_heads WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_delivery_assignments WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_delivery_proposals WHERE task_id=$1)
             OR EXISTS(SELECT 1 FROM sdlc_delivery_results WHERE task_id=$1) AS published",
                vec![task.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::conflict("publication inventory unavailable"))?;
        if row.try_get::<bool>("", "published").map_err(map_db)? {
            return Err(AppError::conflict(
                "published/technical history blocks Draft enrollment",
            ));
        }
        Ok(state)
    }

    async fn enrollment_preflight(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
    ) -> Result<DraftSourceEnrollmentPreflight, AppError> {
        let task = state.task_id;
        let selected = tx.query_one(statement(
            "SELECT source_revision,policy_hash FROM sdlc_effective_task_source_selections WHERE task_id=$1",
            vec![task.into()],
        )).await.map_err(map_db)?;
        let original_selection = selected
            .map(|r| {
                let revision = r
                    .try_get::<Option<i64>>("", "source_revision")
                    .map_err(map_db)?;
                let hash = r
                    .try_get::<Option<String>>("", "policy_hash")
                    .map_err(map_db)?;
                if revision.is_some() || hash.is_some() {
                    return Err(AppError::conflict(
                        "Draft source is already pinned; rebind is closed",
                    ));
                }
                Ok(json!({"source_revision":revision,"policy_hash":hash}))
            })
            .transpose()?;
        let policy = Self::retained_source_policy(tx, state.project_id, None)
            .await
            .map_err(|error| match error {
                AppError::NotFound(_) => AppError::Unavailable(
                    "sdlc_source_project_policy_missing: owner/operator configuration required"
                        .into(),
                ),
                other => other,
            })?;
        if Self::source_namespace(tx, state.project_id).await? != policy.verification.namespace {
            return Err(AppError::conflict("source namespace changed"));
        }
        let mut result = DraftSourceEnrollmentPreflight {
            task_id: task,
            project_id: state.project_id,
            state_sha256: app::sdlc::canonical_hash(state)?,
            selection_sha256: app::sdlc::canonical_hash(&original_selection)?,
            policy,
            preflight_sha256: String::new(),
        };
        result.preflight_sha256 = app::sdlc_draft_source_enrollment::preflight_hash(&result)?;
        Ok(result)
    }

    async fn enrollment_replay(
        tx: &DatabaseTransaction,
        state: &TaskState,
        actor: &Principal,
        command: &EnrollDraftSource,
    ) -> Result<Option<DraftSourceEnrollmentReceipt>, AppError> {
        let row = tx.query_one(statement(
            "SELECT request_sha256,payload FROM sdlc_draft_source_enrollments WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![state.task_id.into(),actor.subject.clone().into(),command.idempotency_key.clone().into()],
        )).await.map_err(map_db)?;
        let Some(row) = row else { return Ok(None) };
        if row
            .try_get::<String>("", "request_sha256")
            .map_err(map_db)?
            != app::sdlc::canonical_hash(command)?
        {
            return Err(AppError::conflict(
                "Draft enrollment operation payload changed",
            ));
        }
        let receipt: DraftSourceEnrollmentReceipt = decode(&row)?;
        if receipt.task_id != state.task_id
            || receipt.project_id != state.project_id
            || receipt.owner_subject != actor.subject
            || receipt.source_hash != app::sdlc_source::sealed_hash(&receipt)?
        {
            return Err(AppError::conflict("Draft enrollment history inconsistent"));
        }
        Ok(Some(receipt))
    }

    pub(super) async fn prepare_draft_source_enrollment(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<DraftSourceEnrollmentPreflight, AppError> {
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.enrollment_draft(&tx, task, actor).await?;
        let result = self.enrollment_preflight(&tx, &state).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }

    pub(super) async fn write_draft_source_enrollment(
        &self,
        task: Uuid,
        actor: &Principal,
        command: EnrollDraftSource,
    ) -> Result<(DraftSourceEnrollmentReceipt, bool), AppError> {
        app::sdlc_draft_source_enrollment::validate_command(&command)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.enrollment_draft(&tx, task, actor).await?;
        if let Some(receipt) = Self::enrollment_replay(&tx, &state, actor, &command).await? {
            tx.commit().await.map_err(map_db)?;
            return Ok((receipt, true));
        }
        let before = self.enrollment_preflight(&tx, &state).await?;
        app::sdlc_draft_source_enrollment::verify_cas(&before, &command)?;
        let (creator, _) = Self::project_access(&tx, state.project_id, actor).await?;
        let policy = &before.policy;
        let link = domain::task_repositories::TaskRepositoryLink {
            namespace: policy.verification.namespace.clone(),
            forge_instance_id: policy.source.forge_instance_id,
            repository_id: policy.source.repository_id,
            public_name: policy.verification.public_name.clone(),
            kind: "hosted".into(),
        };
        exec(&tx,"INSERT INTO task_repository_links(issue_id,forge_instance_id,repository_id,snapshot,created_by) VALUES($1,$2,$3,$4,$5)
            ON CONFLICT(issue_id,forge_instance_id,repository_id) DO NOTHING",
            vec![task.into(),link.forge_instance_id.into(),link.repository_id.into(),json_value(&link)?,creator.into()]).await?;
        let saved_link = tx.query_one(statement("SELECT snapshot AS payload FROM task_repository_links WHERE issue_id=$1 AND forge_instance_id=$2 AND repository_id=$3 FOR SHARE",
            vec![task.into(),link.forge_instance_id.into(),link.repository_id.into()])).await.map_err(map_db)?.ok_or_else(||AppError::conflict("task repository link unavailable"))?;
        let saved_link: domain::task_repositories::TaskRepositoryLink = decode(&saved_link)?;
        if saved_link.namespace != link.namespace
            || saved_link.kind != "hosted"
            || saved_link.forge_instance_id != link.forge_instance_id
            || saved_link.repository_id != link.repository_id
        {
            return Err(AppError::conflict(
                "existing task repository identity differs",
            ));
        }
        let now = super::super::lifecycle::now(&tx).await?;
        let mut receipt = DraftSourceEnrollmentReceipt {
            enrollment_id: Uuid::new_v4(),
            task_id: task,
            project_id: state.project_id,
            owner_subject: actor.subject.clone(),
            source_revision: policy.source_revision,
            policy_hash: policy.source_hash.clone(),
            request_sha256: app::sdlc::canonical_hash(&command)?,
            source_hash: String::new(),
            created_at: now,
        };
        receipt.source_hash = app::sdlc_source::sealed_hash(&receipt)?;
        exec(&tx,"INSERT INTO sdlc_draft_source_enrollments(task_id,project_id,enrollment_id,actor_subject,idempotency_key,
            request_sha256,source_revision,policy_hash,source_hash,payload) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            vec![task.into(),state.project_id.into(),receipt.enrollment_id.into(),actor.subject.clone().into(),command.idempotency_key.into(),
                receipt.request_sha256.clone().into(),receipt.source_revision.into(),receipt.policy_hash.clone().into(),receipt.source_hash.clone().into(),json_value(&receipt)?]).await?;
        exec(&tx,"INSERT INTO audit_logs(id,actor_id,action,entity_type,entity_id,metadata,created_at)
            VALUES($1,$2,'sdlc.draft_source.enrolled','issue',$3,$4,$5)",
            vec![Uuid::new_v4().into(),creator.into(),task.into(),json!({"enrollment_id":receipt.enrollment_id,
                "source_revision":receipt.source_revision,"policy_hash":receipt.policy_hash,"source_hash":receipt.source_hash,
                "metadata_only":true}).into(),now.into()]).await?;
        let payload = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
            "project_id":state.project_id,"task_id":task,"root_task_id":state.root_task_id,
            "owner_subject":state.owner_subject,"stage":state.stage,
            "requirement_revision":state.current_revision(),"result":receipt});
        exec(
            &tx,
            "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload,created_at)
            VALUES($1,$2,'source.selected',$3,$4)",
            vec![
                receipt.enrollment_id.into(),
                task.into(),
                payload.into(),
                now.into(),
            ],
        )
        .await?;
        tx.commit().await.map_err(map_db)?;
        Ok((receipt, false))
    }
}
