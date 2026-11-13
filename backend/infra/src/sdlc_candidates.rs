//! Bounded SELECT-only discovery on existing Tracker owner tables.
use super::*;
use domain::sdlc_candidates::*;
use domain::sdlc_lifecycle::LifecycleReceipt;
use domain::sdlc_routing::TaskRoutingSnapshot;

// Filter business eligibility before LIMIT, including current PM completion, not consent alone.
const CANDIDATES_SQL: &str = r#"
SELECT t.state, s.payload AS routing, d.receipt,
       (started.task_id IS NOT NULL OR reviewed.operation_id IS NOT NULL) AS configured_started,
       (finding_start.task_id IS NOT NULL) AS human_finding_started,
       COALESCE(sdlc_current_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state),false) AS pm_completed
FROM sdlc_tasks t
JOIN issues issue ON issue.id=t.task_id AND issue.project_id=t.project_id AND issue.deleted_at IS NULL
JOIN sdlc_routing_snapshot_inventory s ON s.task_id=t.task_id AND s.project_id=t.project_id
JOIN sdlc_confirmations c ON c.task_id=t.task_id AND c.id=s.confirmation_id
JOIN sdlc_analysis_intents intent ON intent.task_id=t.task_id
 AND COALESCE(intent.routing_snapshot_id,intent.review_routing_snapshot_id)=s.snapshot_id
LEFT JOIN sdlc_delivery_tasks d ON d.task_id=t.task_id
LEFT JOIN sdlc_delivery_operations started ON started.task_id=t.task_id
 AND started.command->>'operation'='start_analysis' AND s.generation=1
 AND EXISTS(SELECT 1 FROM sdlc_configured_pm_completions completed
     WHERE completed.task_id=t.task_id AND completed.completion_id=(started.command->>'pm_completion_id')::uuid
       AND completed.confirmation_id=c.id AND completed.requirement_revision=s.requirement_revision
       AND completed.content_hash=s.content_hash
       AND (started.actor_subject=t.owner_subject OR
            (started.actor_subject=completed.actor_subject AND started.transaction_id=completed.transaction_id
             AND started.idempotency_key='pm-analysis:'||completed.completion_id::text)))
LEFT JOIN sdlc_delivery_operations finding_start ON finding_start.task_id=t.task_id
 AND finding_start.command->>'operation'='start_finding_analysis' AND s.generation=1
 AND finding_start.result->'task'->'routing'=s.payload AND finding_start.actor_subject=t.owner_subject
 AND sdlc_finding_intake_origin(t.task_id,t.state)
 AND EXISTS(SELECT 1 FROM sdlc_draft_creations creation WHERE creation.task_id=t.task_id
    AND creation.project_id=t.project_id AND creation.actor_subject=t.owner_subject
    AND creation.intake_transaction_id=finding_start.transaction_id
    AND creation.finding_intake_result->>'operation_id'=finding_start.command->>'intake_operation_id')
LEFT JOIN sdlc_task_routing_generations generation ON generation.task_id=t.task_id AND generation.snapshot_id=s.snapshot_id
LEFT JOIN sdlc_critical_pm_lifecycle_operations reviewed ON reviewed.operation_id=generation.operation_id
 AND reviewed.kind='complete' AND reviewed.configured_completion_id=generation.configured_completion_id
 AND reviewed.result->>'confirmation_id'=c.id::text AND reviewed.after_state->'lifecycle'->'routing'=s.payload
WHERE t.project_id=$1 AND t.tracker_instance_id=$2
  AND t.root_task_id=t.task_id AND t.state->>'root_task_id'=t.task_id::text
  AND t.state->'flow'='"single-task-v1"'::jsonb
  AND (t.state->>'interaction_protocol' IS DISTINCT FROM 'configured-cli-v1'
       OR (d.task_id IS NOT NULL AND (finding_start.task_id IS NOT NULL OR
           ((started.task_id IS NOT NULL OR reviewed.operation_id IS NOT NULL)
            AND sdlc_current_configured_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state)))))
  AND ($3::uuid IS NULL OR t.task_id>$3)
  AND s.payload->'contract_version'='2'::jsonb AND s.payload->'policy'->'contract_version'='2'::jsonb
  AND s.requirement_revision::text=t.state->'revisions'->-1->>'revision'
  AND s.requirement_revision BETWEEN 1 AND 9007199254740991
  AND s.content_hash=t.state->'revisions'->-1->>'content_hash'
  AND c.revision=s.requirement_revision AND c.content_hash=s.content_hash
  AND c.payload->>'owner_subject'=t.owner_subject
  AND (d.task_id IS NULL AND s.generation=1 AND t.state->>'stage' IN ('Analysis','Architecture')
       AND sdlc_current_pm_completed(t.task_id,t.pm_execution_id,t.state,t.pm_admission_state)
       AND NOT EXISTS(SELECT 1 FROM sdlc_analysis_capacity_holds h WHERE h.root_task_id=t.task_id)
       AND (t.state->>'stage'='Analysis' OR EXISTS(
           SELECT 1 FROM sdlc_analysis_results r WHERE r.task_id=t.task_id
             AND r.payload->>'requirement_revision'=s.requirement_revision::text
             AND r.payload->>'content_hash'=s.content_hash))
       OR d.task_id IS NOT NULL AND d.receipt->'task'->>'status'='Ready'
       AND d.receipt->'task'->>'runtime'='queued' AND d.receipt->'assignment'='null'::jsonb
       AND d.receipt->'task'->'routing'=s.payload
       AND d.receipt->'task'->>'stage' IN ('Analysis','Architecture','Development','Review','Testing','Rework','Deployment')
       AND EXISTS(SELECT 1 FROM sdlc_issue_status_mapping m
           WHERE m.stage=d.receipt->'task'->>'stage' AND m.status='Ready' AND m.status_id=issue.status_id))
ORDER BY t.task_id LIMIT $4
"#;

impl PostgresSdlcRepository {
    pub(super) async fn read_delivery_candidates(
        &self,
        actor: &Principal,
        query: DeliveryCandidatesQuery,
    ) -> Result<DeliveryCandidates, AppError> {
        app::sdlc_candidates::authorize(actor, &self.config)?;
        query.validate()?;
        let tx = self.db.begin().await.map_err(map_db)?;
        Self::project_access(&tx, query.project_id, actor).await?;
        self.single_task_policy(&tx, query.project_id).await?;
        let rows = tx
            .query_all(statement(
                CANDIDATES_SQL,
                vec![
                    query.project_id.into(),
                    self.config.instance_id.clone().into(),
                    query.after.into(),
                    (i64::from(query.limit) + 1).into(),
                ],
            ))
            .await
            .map_err(map_db)?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let state: TaskState =
                serde_json::from_value(row.try_get::<Json>("", "state").map_err(map_db)?)
                    .map_err(|_| AppError::conflict("delivery candidate state inconsistent"))?;
            let snapshot: TaskRoutingSnapshot =
                serde_json::from_value(row.try_get::<Json>("", "routing").map_err(map_db)?)
                    .map_err(|_| AppError::conflict("delivery candidate routing inconsistent"))?;
            let current: Option<LifecycleReceipt> = row
                .try_get::<Option<Json>>("", "receipt")
                .map_err(map_db)?
                .map(serde_json::from_value)
                .transpose()
                .map_err(|_| AppError::conflict("delivery candidate head inconsistent"))?;
            app::sdlc_draft_flow::validate_policy(
                &snapshot.policy,
                query.project_id,
                &self.config.instance_id,
            )
            .map_err(|_| AppError::conflict("delivery candidate routing incompatible"))?;
            super::lifecycle::exact_snapshot(&state, &snapshot)?;
            if let Some(receipt) = &current {
                super::lifecycle::exact_revision(&state, receipt)?;
                if receipt.task.routing != snapshot {
                    return Err(AppError::conflict(
                        "delivery candidate frozen routing differs",
                    ));
                }
            }
            let item = app::sdlc_candidates::candidate(
                &state,
                current.as_ref(),
                if row
                    .try_get::<bool>("", "human_finding_started")
                    .map_err(map_db)?
                {
                    app::sdlc_candidates::CandidateOrigin::HumanFinding
                } else {
                    app::sdlc_candidates::CandidateOrigin::Pm {
                        completed: row.try_get::<bool>("", "pm_completed").map_err(map_db)?,
                        configured_started: row
                            .try_get::<bool>("", "configured_started")
                            .map_err(map_db)?,
                    }
                },
            )?
            .ok_or_else(|| AppError::conflict("delivery candidate projection inconsistent"))?;
            items.push(item);
        }
        let (items, next_cursor) = app::sdlc_candidates::page(&query, items)?;
        let observed_at = tx
            .query_one(statement("SELECT clock_timestamp() AS observed_at", vec![]))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::internal("candidate clock unavailable"))?
            .try_get("", "observed_at")
            .map_err(map_db)?;
        tx.commit().await.map_err(map_db)?;
        Ok(DeliveryCandidates {
            contract_version: 1,
            tracker_instance_id: self.config.instance_id.clone(),
            project_id: query.project_id,
            observed_at,
            items,
            next_cursor,
        })
    }
}

#[cfg(test)]
#[path = "sdlc_candidates_pg_tests.rs"]
mod tests;
