//! Exact human finding creation and intake receipts share the immutable Draft ledger.
use super::*;
use domain::background_intake::*;

impl PostgresSdlcRepository {
    pub(super) async fn finding_custom_values(
        tx: &DatabaseTransaction,
        project: Uuid,
        finding: &FindingIntake,
    ) -> Result<Vec<(shared::CustomFieldId, Json)>, AppError> {
        let rows = tx
            .query_all(statement(
                "SELECT id,project_id,name,field_type,options,is_required,created_at
            FROM custom_fields WHERE project_id=$1 ORDER BY id FOR SHARE",
                vec![project.into()],
            ))
            .await
            .map_err(map_db)?;
        let fields = rows
            .iter()
            .map(|row| {
                let options: Vec<String> =
                    serde_json::from_value(row.try_get::<Json>("", "options").map_err(map_db)?)
                        .map_err(|_| AppError::conflict("custom field options invalid"))?;
                Ok(domain::CustomField {
                    id: shared::CustomFieldId::from_uuid(row.try_get("", "id").map_err(map_db)?),
                    project_id: shared::ProjectId::from_uuid(project),
                    name: row.try_get::<String>("", "name").map_err(map_db)?.into(),
                    field_type: row
                        .try_get::<String>("", "field_type")
                        .map_err(map_db)?
                        .parse()
                        .map_err(AppError::invalid_input)?,
                    options: options.into_iter().map(Into::into).collect(),
                    is_required: row.try_get("", "is_required").map_err(map_db)?,
                    created_at: row
                        .try_get::<chrono::DateTime<chrono::FixedOffset>>("", "created_at")
                        .map_err(map_db)?,
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        app::services::custom_field::normalize_create_values(&fields, &finding.custom_fields)
    }

    pub(super) async fn read_finding_intake(
        &self,
        project: Uuid,
        actor: &Principal,
        operation: Uuid,
    ) -> Result<FindingIntakeReceipt, AppError> {
        if !actor.human_session || actor.subject.is_empty() || operation.is_nil() {
            return Err(AppError::Forbidden);
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        let key = format!("fleet-background:{operation}");
        let lock = json!(["sdlc.draft.create:v1", project, actor.subject, key]).to_string();
        exec(
            &tx,
            "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
            vec![lock.into()],
        )
        .await?;
        let (user, _) = Self::project_access(&tx, project, actor).await?;
        let row = tx
            .query_one(statement(
                "SELECT c.task_id,c.payload_hash,c.finding_intake,c.finding_intake_result,
            i.key,i.reporter_id FROM sdlc_draft_creations c JOIN issues i ON i.id=c.task_id
            AND i.project_id=c.project_id AND i.deleted_at IS NULL
            WHERE c.project_id=$1 AND c.actor_subject=$2 AND c.idempotency_key=$3 FOR SHARE OF i",
                vec![project.into(), actor.subject.clone().into(), key.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::not_found("finding intake operation", operation))?;
        let invalid = || AppError::conflict("finding intake receipt inconsistent");
        let command: FindingIntake =
            serde_json::from_value(row.try_get::<Json>("", "finding_intake").map_err(map_db)?)
                .map_err(|_| invalid())?;
        let receipt: FindingIntakeReceipt = serde_json::from_value(
            row.try_get::<Json>("", "finding_intake_result")
                .map_err(map_db)?,
        )
        .map_err(|_| invalid())?;
        let hash = app::sdlc::canonical_hash(
            &json!({"operation":"finding_intake","operation_id":operation,"payload":command}),
        )?;
        let state = self.load(&tx, receipt.task_id, actor).await?;
        if receipt.operation_id != operation
            || receipt.project_id != project
            || receipt.tracker_instance_id != self.config.instance_id
            || receipt.task_id != row.try_get::<Uuid>("", "task_id").map_err(map_db)?
            || receipt.task_key != row.try_get::<String>("", "key").map_err(map_db)?
            || receipt.destination != command.destination
            || receipt.request != command
            || state.owner_subject != actor.subject
            || state.root_task_id != state.task_id
            || state.project_id != project
            || row.try_get::<Uuid>("", "reporter_id").map_err(map_db)? != user
            || row.try_get::<String>("", "payload_hash").map_err(map_db)? != hash
        {
            return Err(invalid());
        }
        tx.commit().await.map_err(map_db)?;
        Ok(receipt)
    }

    pub(super) async fn queue_finding_analysis(
        &self,
        tx: &DatabaseTransaction,
        initial: &TaskState,
        actor: &Principal,
        operation: Uuid,
        finding: &FindingIntake,
    ) -> Result<(), AppError> {
        let task = initial.task_id;
        let now = chrono::Utc::now();
        let document = app::background_intake::requirements(finding)?;
        let revision = RequirementsRevision {
            revision: 1,
            content_hash: app::sdlc::canonical_hash(&document)?,
            document,
            author_subject: actor.subject.clone(),
            created_at: now,
        };
        let confirmation = Confirmation {
            id: Uuid::now_v7(),
            task_id: task,
            revision: 1,
            content_hash: revision.content_hash.clone(),
            owner_subject: actor.subject.clone(),
            created_at: now,
            stage: Stage::Draft,
        };
        let ordinal:i64=tx.query_one(statement("SELECT finding_workflow_ordinal AS ordinal FROM sdlc_draft_creations WHERE task_id=$1 AND finding_intake_result->>'operation_id'=$2",
            vec![task.into(),operation.to_string().into()])).await.map_err(map_db)?
            .ok_or_else(||AppError::conflict("finding creation origin missing"))?.try_get("","ordinal").map_err(map_db)?;
        let business = domain::sdlc_lifecycle::DeliveryExecution::from_ordinal(ordinal)?;
        let mut state = initial.clone();
        state
            .retained_fields
            .insert("workflow_task_key".into(), json!(business.key));
        state.revisions.push(revision.clone());
        state.confirmations.push(confirmation.clone());
        state.confirmation_revision = Some(1);
        state
            .retained_fields
            .insert("finding_intake_operation".into(), json!(operation));
        exec(tx,"INSERT INTO sdlc_requirements(task_id,revision,content_hash,payload) VALUES($1,1,$2,$3)",
            vec![task.into(),revision.content_hash.clone().into(),json_value(&revision)?]).await?;
        exec(
            tx,
            "UPDATE sdlc_tasks SET state=$2 WHERE task_id=$1",
            vec![task.into(), json_value(&state)?],
        )
        .await?;
        exec(tx,"INSERT INTO sdlc_confirmations(id,task_id,revision,content_hash,payload) VALUES($1,$2,1,$3,$4)",
            vec![confirmation.id.into(),task.into(),confirmation.content_hash.clone().into(),json_value(&confirmation)?]).await?;
        let routing = self.freeze_routing(tx, &state, &confirmation, None).await?;
        self.freeze_source(tx, &state, &confirmation, &routing)
            .await?;
        let intent =
            app::background_intake::analysis_intent(&state, &confirmation, Uuid::now_v7())?;
        let key = format!("finding-analysis:{operation}");
        let command = json!({"operation":"start_finding_analysis","intake_operation_id":operation,"idempotency_key":key});
        Self::queue_analysis(tx, &state, actor, routing, intent, command, &key).await?;
        Ok(())
    }
}
