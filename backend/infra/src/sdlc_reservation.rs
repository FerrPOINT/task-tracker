use super::*;
use chrono::{DateTime, Duration, Utc};
use domain::{sdlc_execution_lease::*, sdlc_reservation::*, sdlc_routing::TaskRoutingSnapshot};

fn inconsistent() -> AppError {
    AppError::conflict("Analysis reservation source inconsistent; reconciliation required")
}
async fn clock(tx: &DatabaseTransaction) -> Result<DateTime<Utc>, AppError> {
    tx.query_one(statement("SELECT clock_timestamp() AS t", vec![]))
        .await
        .map_err(map_db)?
        .ok_or_else(inconsistent)?
        .try_get("", "t")
        .map_err(map_db)
}

impl PostgresSdlcRepository {
    async fn reservation_inputs(
        tx: &DatabaseTransaction,
        state: &TaskState,
    ) -> Result<(AnalysisIntent, TaskRoutingSnapshot), AppError> {
        let intent = Self::current_analysis_intent(tx, state).await?;
        let row = tx.query_one(statement("SELECT s.payload FROM sdlc_task_routing_snapshots s JOIN sdlc_analysis_intents i ON i.routing_snapshot_id=s.snapshot_id WHERE i.intent_id=$1 AND s.task_id=$2", vec![intent.intent_id.into(),state.task_id.into()])).await.map_err(map_db)?
            .ok_or_else(|| AppError::conflict("explicit frozen routing snapshot required"))?;
        let snapshot: TaskRoutingSnapshot =
            serde_json::from_value(row.try_get("", "payload").map_err(map_db)?)
                .map_err(|_| inconsistent())?;
        app::sdlc_routing::validate_routes(&snapshot.policy.routes)?;
        let policy_row = tx.query_one(statement("SELECT payload FROM sdlc_project_routing_revisions WHERE project_id=$1 AND version=$2", vec![state.project_id.into(), snapshot.policy.version.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if snapshot.contract_version != snapshot.policy.routes.contract_version()
            || snapshot.policy.contract_version != snapshot.contract_version
            || snapshot.tracker_instance_id != state.tracker_instance_id
            || snapshot.project_id != state.project_id
            || snapshot.task_id != state.task_id
            || snapshot.root_task_id != state.root_task_id
            || snapshot.confirmation_id != intent.confirmation_id
            || snapshot.requirement_revision != intent.requirement_revision
            || snapshot.content_hash != intent.content_hash
            || snapshot.policy.native_ready
            || snapshot.policy.dispatch_allowed
            || snapshot.policy.routing_hash != app::sdlc::canonical_hash(&snapshot.policy.routes)?
            || policy_row.try_get::<Json>("", "payload").map_err(map_db)?
                != serde_json::to_value(&snapshot.policy).map_err(AppError::internal)?
        {
            return Err(inconsistent());
        }
        Ok((intent, snapshot))
    }
    async fn reservation_current(
        tx: &DatabaseTransaction,
        state: &TaskState,
    ) -> Result<Option<AnalysisReservationReceipt>, AppError> {
        let row = tx.query_one(statement("SELECT r.payload,h.pool_slot,l.version,l.holder_subject,l.claimed_at,l.heartbeat_at,l.expires_at FROM sdlc_analysis_reservations r JOIN sdlc_analysis_reservation_leases l USING(assignment_id) JOIN sdlc_analysis_capacity_holds h USING(assignment_id) WHERE r.task_id=$1 FOR UPDATE OF l",vec![state.task_id.into()])).await.map_err(map_db)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let assignment: PreparedAnalysisAssignment =
            serde_json::from_value(row.try_get("", "payload").map_err(map_db)?)
                .map_err(|_| inconsistent())?;
        let (intent, snapshot) = Self::reservation_inputs(tx, state).await?;
        if assignment.assignment_hash != app::sdlc_reservation::assignment_hash(&assignment)?
            || assignment.task_id != state.task_id
            || assignment.owner_subject != state.owner_subject
            || assignment.project_id != state.project_id
            || assignment.tracker_instance_id != state.tracker_instance_id
            || assignment.root_task_id != state.root_task_id
            || assignment.intent_id != intent.intent_id
            || assignment.confirmation_id != intent.confirmation_id
            || assignment.requirement_revision != intent.requirement_revision
            || assignment.content_hash != intent.content_hash
            || assignment.routing_snapshot_id != snapshot.snapshot_id
            || assignment.routing_policy_version != snapshot.policy.version
            || assignment.routing_hash != snapshot.policy.routing_hash
            || assignment.route != snapshot.policy.routes.analyst
            || assignment.agent_id != assignment.route.agent_id
        {
            return Err(inconsistent());
        }
        let current = AnalysisReservationReceipt {
            lease: ExecutionLease {
                lease_id: assignment.lease_id,
                version: row.try_get("", "version").map_err(map_db)?,
                holder_subject: row.try_get("", "holder_subject").map_err(map_db)?,
                claimed_at: row.try_get("", "claimed_at").map_err(map_db)?,
                heartbeat_at: row.try_get("", "heartbeat_at").map_err(map_db)?,
                expires_at: row.try_get("", "expires_at").map_err(map_db)?,
            },
            assignment,
            technical_pool_slot: row.try_get::<i16>("", "pool_slot").map_err(map_db)? as u8,
            ttl_seconds: LEASE_TTL_SECONDS,
            heartbeat_interval_seconds: LEASE_HEARTBEAT_SECONDS,
            admission_state: AnalysisAdmissionState::AwaitingAdmission,
            dispatch_allowed: false,
            capacity_held: true,
        };
        let operation = tx.query_one(statement("SELECT result FROM sdlc_analysis_reservation_operations WHERE assignment_id=$1 AND lease_version=$2",vec![current.assignment.assignment_id.into(),current.lease.version.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if operation.try_get::<Json>("", "result").map_err(map_db)?
            != serde_json::to_value(&current).map_err(AppError::internal)?
        {
            return Err(inconsistent());
        }
        Ok(Some(current))
    }
    async fn reservation_operation(
        tx: &DatabaseTransaction,
        task: Uuid,
        key: &str,
    ) -> Result<Option<AnalysisReservationOperation>, AppError> {
        let row = tx.query_one(statement("SELECT payload_hash,result FROM sdlc_analysis_reservation_operations WHERE task_id=$1 AND idempotency_key=$2",vec![task.into(),key.into()])).await.map_err(map_db)?;
        row.map(|r| {
            Ok(AnalysisReservationOperation {
                idempotency_key: key.into(),
                request_sha256: r.try_get("", "payload_hash").map_err(map_db)?,
                result: serde_json::from_value(r.try_get("", "result").map_err(map_db)?)
                    .map_err(|_| inconsistent())?,
            })
        })
        .transpose()
    }
    async fn save_reservation_operation(
        tx: &DatabaseTransaction,
        actor: &Principal,
        key: &str,
        hash: &str,
        result: &AnalysisReservationReceipt,
    ) -> Result<(), AppError> {
        exec(tx,"INSERT INTO sdlc_analysis_reservation_operations(task_id,idempotency_key,actor_subject,payload_hash,assignment_id,lease_version,result) VALUES($1,$2,$3,$4,$5,$6,$7)",vec![result.assignment.task_id.into(),key.into(),actor.subject.clone().into(),hash.into(),result.assignment.assignment_id.into(),result.lease.version.into(),json_value(result)?]).await
    }
    async fn result_operation(
        tx: &DatabaseTransaction,
        task: Uuid,
        key: &str,
    ) -> Result<Option<AnalysisResultOperation>, AppError> {
        let row = tx.query_one(statement("SELECT payload_hash,result FROM sdlc_analysis_result_operations WHERE task_id=$1 AND idempotency_key=$2",vec![task.into(),key.into()])).await.map_err(map_db)?;
        row.map(|r| {
            Ok(AnalysisResultOperation {
                idempotency_key: key.into(),
                request_sha256: r.try_get("", "payload_hash").map_err(map_db)?,
                result: serde_json::from_value(r.try_get("", "result").map_err(map_db)?)
                    .map_err(|_| inconsistent())?,
            })
        })
        .transpose()
    }
    async fn save_result_operation(
        tx: &DatabaseTransaction,
        actor: &Principal,
        key: &str,
        hash: &str,
        result: &AnalysisResultReceipt,
    ) -> Result<(), AppError> {
        exec(tx,"INSERT INTO sdlc_analysis_result_operations(task_id,idempotency_key,actor_subject,payload_hash,result_id,result) VALUES($1,$2,$3,$4,$5,$6)",vec![result.task_id.into(),key.into(),actor.subject.clone().into(),hash.into(),result.result_id.into(),json_value(result)?]).await
    }
    async fn pm_completion_current(
        tx: &DatabaseTransaction,
        task: Uuid,
    ) -> Result<Option<PmDraftCompletionReceipt>, AppError> {
        let row = tx.query_one(statement(
            "SELECT c.result FROM sdlc_tasks t JOIN sdlc_pm_completions c ON c.task_id=t.task_id AND c.execution_id=t.pm_execution_id WHERE t.task_id=$1",
            vec![task.into()],
        )).await.map_err(map_db)?;
        row.map(|r| {
            serde_json::from_value(r.try_get("", "result").map_err(map_db)?)
                .map_err(|_| AppError::conflict("PM completion receipt invalid"))
        })
        .transpose()
    }
    async fn pm_completion_operation(
        tx: &DatabaseTransaction,
        task: Uuid,
        key: &str,
    ) -> Result<Option<PmDraftCompletionOperation>, AppError> {
        let row = tx.query_one(statement("SELECT payload_hash,result FROM sdlc_pm_completion_operations WHERE task_id=$1 AND idempotency_key=$2",vec![task.into(),key.into()])).await.map_err(map_db)?;
        row.map(|r| {
            Ok(PmDraftCompletionOperation {
                idempotency_key: key.into(),
                request_sha256: r.try_get("", "payload_hash").map_err(map_db)?,
                result: serde_json::from_value(r.try_get("", "result").map_err(map_db)?)
                    .map_err(|_| AppError::conflict("PM completion operation invalid"))?,
            })
        })
        .transpose()
    }
    async fn pm_completed_for_current_revision(
        tx: &DatabaseTransaction,
        state: &TaskState,
    ) -> Result<bool, AppError> {
        let Some(assignment) = state.assignment.as_ref() else {
            return Ok(false);
        };
        let Some(revision) = state.revisions.last() else {
            return Ok(false);
        };
        let row = tx.query_one(statement("SELECT t.pm_execution_id,t.pm_admission_state,c.result FROM sdlc_tasks t LEFT JOIN sdlc_pm_completions c ON c.task_id=t.task_id AND c.execution_id=t.pm_execution_id WHERE t.task_id=$1",vec![state.task_id.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        let id: Option<Uuid> = row.try_get("", "pm_execution_id").map_err(map_db)?;
        let status: Option<String> = row.try_get("", "pm_admission_state").map_err(map_db)?;
        if id != Some(assignment.execution_id) || status.as_deref() != Some("completed") {
            return Ok(false);
        }
        let Some(value) = row.try_get::<Option<Json>>("", "result").map_err(map_db)? else {
            return Ok(false);
        };
        let receipt: PmDraftCompletionReceipt =
            serde_json::from_value(value).map_err(|_| inconsistent())?;
        Ok(receipt.fence.assignment_id == assignment.assignment_id
            && receipt.fence.execution_id == assignment.execution_id
            && receipt.fence.agent_id == assignment.agent_id
            && receipt.fence.assignment_version == assignment.version
            && receipt.requirement_revision == revision.revision
            && receipt.content_hash == revision.content_hash)
    }
    pub(super) async fn reserve_analysis_intent(
        &self,
        task: Uuid,
        actor: &Principal,
        command: ReserveAnalysis,
    ) -> Result<(AnalysisReservationReceipt, bool), AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        app::sdlc::validate_key(&command.idempotency_key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        if actor.subject == state.owner_subject {
            return Err(AppError::Forbidden);
        }
        let (intent, snapshot) = Self::reservation_inputs(&tx, &state).await?;
        if command.intent_id != intent.intent_id
            || command.routing_snapshot_id != snapshot.snapshot_id
            || command.requirement_revision != intent.requirement_revision
            || command.content_hash != intent.content_hash
            || command.expected_reservation_version != 0
        {
            return Err(AppError::conflict("stale Analysis intent/routing/CAS"));
        }
        let hash =
            app::sdlc::canonical_hash(&json!({"operation":"reserve_analysis","payload":command}))?;
        if let Some(op) = Self::reservation_operation(&tx, task, &command.idempotency_key).await? {
            if op.request_sha256 != hash {
                return Err(AppError::conflict("reservation key payload conflict"));
            }
            let current = Self::reservation_current(&tx, &state)
                .await?
                .ok_or_else(inconsistent)?;
            if op.result.assignment != current.assignment
                || op.result.lease.holder_subject != actor.subject
            {
                return Err(AppError::Forbidden);
            }
            tx.commit().await.map_err(map_db)?;
            return Ok((op.result, true));
        }
        if Self::reservation_current(&tx, &state).await?.is_some() {
            return Err(AppError::conflict(
                "Analysis already reserved; reconciliation required",
            ));
        }
        // All reservation and PM assignment writers take this one capacity lock after their task lock.
        tx.query_one(statement(
            "SELECT singleton FROM sdlc_reservation_capacity WHERE singleton FOR UPDATE",
            vec![],
        ))
        .await
        .map_err(map_db)?
        .ok_or_else(inconsistent)?;
        let pm_completed = Self::pm_completed_for_current_revision(&tx, &state).await?;
        let prior = tx.query_one(statement("SELECT EXISTS(SELECT 1 FROM sdlc_assignments WHERE task_id=$1) OR EXISTS(SELECT 1 FROM sdlc_pm_executions WHERE task_id=$1) AS pm",vec![task.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if (state.assignment.is_some() || prior.try_get::<bool>("", "pm").map_err(map_db)?)
            && !pm_completed
        {
            return Err(AppError::conflict("pm_quiescence_unverified"));
        }
        let agent = snapshot.policy.routes.analyst.agent_id;
        let blocked = tx.query_one(statement("SELECT EXISTS(SELECT 1 FROM sdlc_shared_capacity WHERE task_id=$1 OR agent_id=$2) AS busy, EXISTS(SELECT 1 FROM sdlc_assignments a JOIN sdlc_tasks t ON t.task_id=a.task_id AND t.pm_execution_id=(a.payload->>'execution_id')::uuid WHERE a.payload->>'agent_id'=$2::text AND t.pm_admission_state IS DISTINCT FROM 'completed') AS pm",vec![state.root_task_id.into(),agent.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if blocked.try_get::<bool>("", "pm").map_err(map_db)? {
            return Err(AppError::conflict("pm_quiescence_unverified"));
        }
        if blocked.try_get::<bool>("", "busy").map_err(map_db)? {
            return Err(AppError::conflict("root_or_agent_capacity_held"));
        }
        let slot = tx.query_one(statement("SELECT s::smallint AS slot FROM generate_series(1,2) s WHERE NOT EXISTS(SELECT 1 FROM sdlc_shared_capacity WHERE pool_slot=s) ORDER BY s LIMIT 1",vec![])).await.map_err(map_db)?.ok_or_else(||AppError::conflict("technical_pool_capacity_held"))?.try_get::<i16>("","slot").map_err(map_db)?;
        let fence = tx
            .query_one(statement(
                "SELECT nextval('sdlc_analysis_reservation_fence') AS n",
                vec![],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(inconsistent)?
            .try_get::<i64>("", "n")
            .map_err(map_db)?;
        let now = clock(&tx).await?;
        let workflow_ordinal = tx
            .query_one(statement(
                "SELECT nextval('sdlc_analysis_workflow_task_ordinal') AS n",
                vec![],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(inconsistent)?
            .try_get::<i64>("", "n")
            .map_err(map_db)?;
        let id = Uuid::now_v7();
        let lease_id = Uuid::now_v7();
        let mut assignment = PreparedAnalysisAssignment {
            contract_version: 1,
            tracker_instance_id: state.tracker_instance_id,
            project_id: state.project_id,
            task_id: task,
            root_task_id: state.root_task_id,
            owner_subject: state.owner_subject,
            intent_id: intent.intent_id,
            confirmation_id: intent.confirmation_id,
            requirement_revision: intent.requirement_revision,
            content_hash: intent.content_hash,
            routing_snapshot_id: snapshot.snapshot_id,
            routing_policy_version: snapshot.policy.version,
            routing_hash: snapshot.policy.routing_hash,
            assignment_id: id,
            execution_id: Uuid::now_v7(),
            reservation_version: 1,
            workflow_task_ref: format!("SDLC-{workflow_ordinal}"),
            agent_id: agent,
            route: snapshot.policy.routes.analyst.clone(),
            stage: AnalysisStage::Analysis,
            role_key: "analyst".into(),
            workflow_key: snapshot.policy.routes.analyst.workflow_key,
            mode_key: "analysis".into(),
            scope: "business".into(),
            cycle_number: 0,
            attempt_number: 1,
            fencing_token: fence,
            lease_id,
            assignment_operation_key: format!("analysis-reserve:{id}"),
            assignment_hash: String::new(),
            created_at: now,
        };
        assignment.assignment_hash = app::sdlc_reservation::assignment_hash(&assignment)?;
        let result = AnalysisReservationReceipt {
            assignment,
            lease: ExecutionLease {
                lease_id,
                version: 1,
                holder_subject: actor.subject.clone(),
                claimed_at: now,
                heartbeat_at: now,
                expires_at: now + Duration::seconds(LEASE_TTL_SECONDS),
            },
            technical_pool_slot: slot as u8,
            ttl_seconds: LEASE_TTL_SECONDS,
            heartbeat_interval_seconds: LEASE_HEARTBEAT_SECONDS,
            admission_state: AnalysisAdmissionState::AwaitingAdmission,
            dispatch_allowed: false,
            capacity_held: true,
        };
        let a = &result.assignment;
        exec(&tx,"INSERT INTO sdlc_analysis_reservations(assignment_id,execution_id,task_id,root_task_id,agent_id,intent_id,routing_snapshot_id,fencing_token,lease_id,pool_slot,payload,workflow_task_ordinal) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",vec![id.into(),a.execution_id.into(),task.into(),a.root_task_id.into(),agent.into(),a.intent_id.into(),a.routing_snapshot_id.into(),fence.into(),lease_id.into(),slot.into(),json_value(a)?,workflow_ordinal.into()]).await?;
        exec(&tx,"INSERT INTO sdlc_analysis_capacity_holds(assignment_id,root_task_id,agent_id,pool_slot) VALUES($1,$2,$3,$4)",vec![id.into(),a.root_task_id.into(),agent.into(),slot.into()]).await?;
        exec(&tx,"INSERT INTO sdlc_analysis_reservation_leases(assignment_id,version,holder_subject,claimed_at,heartbeat_at,expires_at) VALUES($1,1,$2,$3,$3,$4)",vec![id.into(),actor.subject.clone().into(),now.into(),result.lease.expires_at.into()]).await?;
        Self::save_reservation_operation(&tx, actor, &command.idempotency_key, &hash, &result)
            .await?;
        let payload = json!({"contract_version":1,"tracker_instance_id":a.tracker_instance_id,"project_id":a.project_id,"task_id":task,"root_task_id":a.root_task_id,"owner_subject":a.owner_subject,"stage":Stage::Analysis,"requirement_revision":a.requirement_revision,"result":result});
        exec(&tx,"INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,'analysis.assignment_reserved',$3)",vec![id.into(),task.into(),payload.into()]).await?;
        tx.commit().await.map_err(map_db)?;
        Ok((result, false))
    }
    pub(super) async fn heartbeat_analysis_reservation(
        &self,
        task: Uuid,
        actor: &Principal,
        command: HeartbeatAnalysis,
    ) -> Result<(AnalysisReservationReceipt, bool), AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        app::sdlc::validate_key(&command.idempotency_key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        let mut current = Self::reservation_current(&tx, &state)
            .await?
            .ok_or_else(|| AppError::conflict("Analysis reservation absent"))?;
        let a = &current.assignment;
        if current.lease.holder_subject != actor.subject {
            return Err(AppError::Forbidden);
        }
        if command.assignment_id != a.assignment_id
            || command.execution_id != a.execution_id
            || command.fencing_token != a.fencing_token
            || command.lease_id != a.lease_id
        {
            return Err(AppError::conflict("stale Analysis reservation fence"));
        }
        let hash = app::sdlc::canonical_hash(
            &json!({"operation":"heartbeat_analysis","payload":command}),
        )?;
        if let Some(op) = Self::reservation_operation(&tx, task, &command.idempotency_key).await? {
            if op.request_sha256 != hash || op.result.assignment != *a {
                return Err(AppError::conflict("heartbeat key payload conflict"));
            }
            tx.commit().await.map_err(map_db)?;
            return Ok((op.result, true));
        }
        let now = clock(&tx).await?;
        if current.lease.expires_at <= now
            || current.lease.version != command.expected_lease_version
            || current.lease.version >= MAX_SAFE_VERSION
        {
            return Err(AppError::conflict(
                "lease_expired_or_stale; reconciliation_needed",
            ));
        }
        current.lease.version += 1;
        current.lease.heartbeat_at = now;
        current.lease.expires_at = now + Duration::seconds(LEASE_TTL_SECONDS);
        exec(&tx,"UPDATE sdlc_analysis_reservation_leases SET version=$2,heartbeat_at=$3,expires_at=$4 WHERE assignment_id=$1 AND version=$5",vec![current.assignment.assignment_id.into(),current.lease.version.into(),now.into(),current.lease.expires_at.into(),command.expected_lease_version.into()]).await?;
        Self::save_reservation_operation(&tx, actor, &command.idempotency_key, &hash, &current)
            .await?;
        tx.commit().await.map_err(map_db)?;
        Ok((current, false))
    }
    pub(super) async fn read_analysis_reservation(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<AnalysisReservationReadback, AppError> {
        app::sdlc_reservation::reader(actor)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        let current = Self::reservation_current(&tx, &state).await?;
        let now = clock(&tx).await?;
        let expired = current.as_ref().is_some_and(|r| r.lease.expires_at <= now);
        let pm_completed = Self::pm_completed_for_current_revision(&tx, &state).await?;
        let pm = (tx.query_one(statement("SELECT EXISTS(SELECT 1 FROM sdlc_assignments WHERE task_id=$1) OR EXISTS(SELECT 1 FROM sdlc_pm_executions WHERE task_id=$1) AS pm",vec![task.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?.try_get::<bool>("","pm").map_err(map_db)? || state.assignment.is_some()) && !pm_completed;
        let result = AnalysisReservationReadback {
            contract_version: 1,
            observed_at: now,
            lease_state: if expired {
                AnalysisReservationLeaseState::Expired
            } else if current.is_some() {
                AnalysisReservationLeaseState::Active
            } else {
                AnalysisReservationLeaseState::Unreserved
            },
            reconciliation_needed: expired || pm,
            reason: if expired {
                Some("lease_expired_stop_unverified".into())
            } else if pm {
                Some("pm_quiescence_unverified".into())
            } else {
                None
            },
            capacity_held: current.is_some(),
            current,
            dispatch_allowed: false,
        };
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }
    pub(super) async fn read_analysis_reservation_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<AnalysisReservationOperation, AppError> {
        app::sdlc_reservation::reader(actor)?;
        app::sdlc::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        let op = Self::reservation_operation(&tx, task, key)
            .await?
            .ok_or_else(|| AppError::not_found("reservation operation", key))?;
        if let Some(current) = Self::reservation_current(&tx, &state).await? {
            if op.result.assignment != current.assignment
                || op.result.lease.version > current.lease.version
            {
                return Err(inconsistent());
            }
        } else {
            let row = tx.query_one(statement("SELECT r.payload,EXISTS(SELECT 1 FROM sdlc_analysis_results ar WHERE ar.assignment_id=r.assignment_id) AS released FROM sdlc_analysis_reservations r WHERE r.task_id=$1 AND r.assignment_id=$2",
                vec![task.into(), op.result.assignment.assignment_id.into()])).await.map_err(map_db)?
                .ok_or_else(inconsistent)?;
            let assignment: PreparedAnalysisAssignment =
                serde_json::from_value(row.try_get("", "payload").map_err(map_db)?)
                    .map_err(|_| inconsistent())?;
            if assignment != op.result.assignment
                || !row.try_get::<bool>("", "released").map_err(map_db)?
            {
                return Err(inconsistent());
            }
        }
        tx.commit().await.map_err(map_db)?;
        Ok(op)
    }

    pub(super) async fn complete_pm_draft_execution(
        &self,
        task: Uuid,
        actor: &Principal,
        command: CompletePmDraft,
    ) -> Result<(PmDraftCompletionReceipt, bool), AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        app::sdlc_reservation::validate_pm_completion(&command)?;
        let hash = app::sdlc::canonical_hash(
            &json!({"operation":"complete_pm_draft","payload":&command}),
        )?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        if let Some(op) = Self::pm_completion_operation(&tx, task, &command.idempotency_key).await?
        {
            if op.request_sha256 != hash {
                return Err(AppError::conflict("PM completion key payload conflict"));
            }
            tx.commit().await.map_err(map_db)?;
            return Ok((op.result, true));
        }
        let assignment = state
            .assignment
            .as_ref()
            .ok_or_else(|| AppError::conflict("current PM assignment absent"))?;
        let revision = state
            .revisions
            .last()
            .ok_or_else(|| AppError::conflict("current requirements revision absent"))?;
        if command.fence.assignment_id != assignment.assignment_id
            || command.fence.execution_id != assignment.execution_id
            || command.fence.agent_id != assignment.agent_id
            || command.fence.assignment_version != assignment.version
            || command.requirement_revision != revision.revision
            || command.content_hash != revision.content_hash
        {
            return Err(AppError::conflict("stale PM completion fence"));
        }
        let row = tx.query_one(statement(
            "SELECT pm_owner_version,pm_execution_id,pm_admission_state FROM sdlc_tasks WHERE task_id=$1 FOR UPDATE",
            vec![task.into()],
        )).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        let owner_version: i64 = row.try_get("", "pm_owner_version").map_err(map_db)?;
        let execution_id: Option<Uuid> = row.try_get("", "pm_execution_id").map_err(map_db)?;
        let status: Option<String> = row.try_get("", "pm_admission_state").map_err(map_db)?;
        if execution_id != Some(assignment.execution_id)
            || status.as_deref() != Some("reserved")
            || Self::pm_completion_current(&tx, task).await?.is_some()
        {
            return Err(AppError::conflict(
                "PM completion already accepted or not current",
            ));
        }
        let reservation = Self::reservation(&tx, &state, assignment.execution_id).await?;
        if reservation.assignment != *assignment || reservation.owner_cas.version != owner_version {
            return Err(inconsistent());
        }
        let lease = tx
            .query_one(statement(
                "SELECT lease_id,version FROM sdlc_pm_execution_leases WHERE execution_id=$1",
                vec![assignment.execution_id.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::conflict("PM execution lease absent"))?;
        if lease.try_get::<Uuid>("", "lease_id").map_err(map_db)? != command.lease_id
            || lease.try_get::<i64>("", "version").map_err(map_db)? != command.lease_version
        {
            return Err(AppError::conflict("stale PM completion lease"));
        }
        let now = clock(&tx).await?;
        let result = PmDraftCompletionReceipt {
            contract_version: 1,
            completion_id: Uuid::now_v7(),
            tracker_instance_id: state.tracker_instance_id.clone(),
            project_id: state.project_id,
            task_id: state.task_id,
            root_task_id: state.root_task_id,
            owner_subject: state.owner_subject.clone(),
            fence: command.fence.clone(),
            owner_version,
            lease_id: command.lease_id,
            lease_version: command.lease_version,
            requirement_revision: command.requirement_revision,
            content_hash: command.content_hash.clone(),
            terminal_receipt_ref: command.terminal_receipt_ref.clone(),
            terminal_receipt_sha256: command.terminal_receipt_sha256.clone(),
            workflow_receipt_ref: command.workflow_receipt_ref.clone(),
            workflow_receipt_sha256: command.workflow_receipt_sha256.clone(),
            previous_pm_admission_state: "reserved".into(),
            next_pm_admission_state: "completed".into(),
            idempotency_key: command.idempotency_key.clone(),
            created_at: now,
        };
        if self
            .finalize_critical_pm_review(&tx, &state, actor, &command, &hash, &result)
            .await?
            .is_some()
        {
            tx.commit().await.map_err(map_db)?;
            return Ok((result, false));
        }
        exec(&tx,"INSERT INTO sdlc_pm_completions(completion_id,task_id,execution_id,assignment_id,assignment_version,lease_id,lease_version,requirement_revision,content_hash,terminal_receipt_ref,terminal_receipt_sha256,workflow_receipt_ref,workflow_receipt_sha256,result) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
            vec![result.completion_id.into(),task.into(),assignment.execution_id.into(),assignment.assignment_id.into(),assignment.version.into(),result.lease_id.into(),result.lease_version.into(),result.requirement_revision.into(),result.content_hash.clone().into(),result.terminal_receipt_ref.clone().into(),result.terminal_receipt_sha256.clone().into(),result.workflow_receipt_ref.clone().into(),result.workflow_receipt_sha256.clone().into(),json_value(&result)?]).await?;
        exec(&tx,"INSERT INTO sdlc_pm_completion_operations(task_id,idempotency_key,actor_subject,payload_hash,completion_id,result) VALUES($1,$2,$3,$4,$5,$6)",
            vec![task.into(),command.idempotency_key.into(),actor.subject.clone().into(),hash.into(),result.completion_id.into(),json_value(&result)?]).await?;
        exec(&tx,"UPDATE sdlc_tasks SET pm_admission_state='completed' WHERE task_id=$1 AND pm_admission_state='reserved'",vec![task.into()]).await?;
        let payload = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,
            "project_id":state.project_id,"task_id":task,"root_task_id":state.root_task_id,
            "owner_subject":state.owner_subject,"stage":state.stage,
            "requirement_revision":state.current_revision(),"result":&result});
        exec(&tx,"INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,'pm.completed',$3)",
            vec![result.completion_id.into(),task.into(),payload.into()]).await?;
        tx.commit().await.map_err(map_db)?;
        Ok((result, false))
    }

    pub(super) async fn read_pm_draft_completion(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<PmDraftCompletionReadback, AppError> {
        app::sdlc_reservation::reader(actor)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        self.load(&tx, task, actor).await?;
        let current = Self::pm_completion_current(&tx, task).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(PmDraftCompletionReadback {
            contract_version: 1,
            current,
        })
    }

    pub(super) async fn read_pm_draft_completion_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<PmDraftCompletionOperation, AppError> {
        app::sdlc_reservation::reader(actor)?;
        app::sdlc::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        self.load(&tx, task, actor).await?;
        let op = Self::pm_completion_operation(&tx, task, key)
            .await?
            .ok_or_else(|| AppError::not_found("PM completion operation", key))?;
        tx.commit().await.map_err(map_db)?;
        Ok(op)
    }

    pub(super) async fn submit_analysis_terminal_result(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SubmitAnalysisResult,
    ) -> Result<(AnalysisResultReceipt, bool), AppError> {
        app::sdlc_reservation::scheduler(actor, &self.config)?;
        app::sdlc_reservation::validate_analysis_result(&command)?;
        let hash = app::sdlc::canonical_hash(
            &json!({"operation":"submit_analysis_result","payload":&command}),
        )?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let mut state = self.load(&tx, task, actor).await?;
        if let Some(op) = Self::result_operation(&tx, task, &command.idempotency_key).await? {
            if op.request_sha256 != hash {
                return Err(AppError::conflict("Analysis result key payload conflict"));
            }
            tx.commit().await.map_err(map_db)?;
            return Ok((op.result, true));
        }
        if !matches!(state.stage, Stage::Analysis) {
            return Err(AppError::conflict("active Analysis stage required"));
        }
        let current = Self::reservation_current(&tx, &state)
            .await?
            .ok_or_else(|| AppError::conflict("Analysis reservation absent or released"))?;
        let now = clock(&tx).await?;
        let a = &current.assignment;
        if current.lease.holder_subject != actor.subject {
            return Err(AppError::Forbidden);
        }
        if current.lease.expires_at <= now
            || current.lease.version != command.expected_lease_version
            || command.assignment_id != a.assignment_id
            || command.execution_id != a.execution_id
            || command.agent_id != a.agent_id
            || command.intent_id != a.intent_id
            || command.routing_snapshot_id != a.routing_snapshot_id
            || command.requirement_revision != a.requirement_revision
            || command.content_hash != a.content_hash
            || command.fencing_token != a.fencing_token
            || command.lease_id != a.lease_id
            || command.assignment_hash != a.assignment_hash
        {
            return Err(AppError::conflict("stale Analysis terminal fence"));
        }
        let duplicate = tx.query_one(statement(
            "SELECT result_id FROM sdlc_analysis_results WHERE task_id=$1 OR assignment_id=$2 OR intent_id=$3",
            vec![task.into(), a.assignment_id.into(), a.intent_id.into()],
        )).await.map_err(map_db)?;
        if duplicate.is_some() {
            return Err(AppError::conflict("Analysis result already accepted"));
        }
        let result = AnalysisResultReceipt {
            contract_version: 1,
            result_id: Uuid::now_v7(),
            tracker_instance_id: a.tracker_instance_id.clone(),
            project_id: a.project_id,
            task_id: a.task_id,
            root_task_id: a.root_task_id,
            owner_subject: a.owner_subject.clone(),
            assignment_id: a.assignment_id,
            execution_id: a.execution_id,
            agent_id: a.agent_id,
            intent_id: a.intent_id,
            routing_snapshot_id: a.routing_snapshot_id,
            requirement_revision: a.requirement_revision,
            content_hash: a.content_hash.clone(),
            workflow_task_ref: a.workflow_task_ref.clone(),
            fencing_token: a.fencing_token,
            lease_id: a.lease_id,
            lease_version: current.lease.version,
            previous_stage: Stage::Analysis,
            next_stage: Stage::Architecture,
            previous_status: "SDLC Analysis".into(),
            next_status: "SDLC Architecture".into(),
            terminal_receipt_ref: command.terminal_receipt_ref.clone(),
            terminal_receipt_sha256: command.terminal_receipt_sha256.clone(),
            architecture_input_sha256: command.architecture_input_sha256.clone(),
            summary: command.summary.clone(),
            idempotency_key: command.idempotency_key.clone(),
            created_at: now,
        };
        exec(&tx,"INSERT INTO sdlc_analysis_results(result_id,assignment_id,execution_id,task_id,intent_id,lease_version,terminal_receipt_ref,terminal_receipt_sha256,architecture_input_sha256,payload) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
            vec![result.result_id.into(),result.assignment_id.into(),result.execution_id.into(),task.into(),result.intent_id.into(),result.lease_version.into(),
                result.terminal_receipt_ref.clone().into(),result.terminal_receipt_sha256.clone().into(),result.architecture_input_sha256.clone().into(),json_value(&result)?]).await?;
        Self::save_result_operation(&tx, actor, &command.idempotency_key, &hash, &result).await?;
        state.stage = Stage::Architecture;
        let next_state = json_value(&state)?;
        if Self::pm_completed_for_current_revision(&tx, &state).await? {
            exec(&tx,"INSERT INTO sdlc_pm_state_write_permits(task_id,transaction_id,next_state) VALUES($1,pg_current_xact_id(),$2)",
                vec![task.into(),next_state.clone()]).await?;
        }
        exec(&tx,"INSERT INTO sdlc_analysis_state_write_permits(task_id,transaction_id,next_state) VALUES($1,pg_current_xact_id(),$2)",
            vec![task.into(),next_state.clone()]).await?;
        exec(
            &tx,
            "UPDATE sdlc_tasks SET state=$2 WHERE task_id=$1",
            vec![task.into(), next_state],
        )
        .await?;
        exec(&tx, "INSERT INTO issue_status_history(id,issue_id,from_status_id,to_status_id,changed_by_id,created_at)
            SELECT $1,i.id,i.status_id,s.id,u.id,now() FROM issues i
            JOIN statuses s ON s.id=(SELECT id FROM statuses WHERE name=$3 ORDER BY id LIMIT 1)
            JOIN users u ON u.central_sub=$4
            WHERE i.id=$2 AND i.status_id<>s.id",
            vec![Uuid::new_v4().into(), task.into(), result.next_status.clone().into(), actor.subject.clone().into()]).await?;
        exec(&tx, "UPDATE issues SET status_id=(SELECT id FROM statuses WHERE name=$2 ORDER BY id LIMIT 1), sprint_id=NULL, updated_at=now() WHERE id=$1", vec![task.into(), result.next_status.clone().into()]).await?;
        let payload = json!({"contract_version":1,"tracker_instance_id":&result.tracker_instance_id,
            "project_id":result.project_id,"task_id":task,"root_task_id":result.root_task_id,
            "owner_subject":&result.owner_subject,"stage":Stage::Architecture,
            "requirement_revision":result.requirement_revision,"result":&result});
        exec(&tx,"INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,'analysis.result_accepted',$3)",
            vec![result.result_id.into(), task.into(), payload.into()]).await?;
        tx.commit().await.map_err(map_db)?;
        Ok((result, false))
    }

    pub(super) async fn read_analysis_result_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<AnalysisResultOperation, AppError> {
        app::sdlc_reservation::reader(actor)?;
        app::sdlc::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        self.load(&tx, task, actor).await?;
        let op = Self::result_operation(&tx, task, key)
            .await?
            .ok_or_else(|| AppError::not_found("Analysis result operation", key))?;
        tx.commit().await.map_err(map_db)?;
        Ok(op)
    }
}
