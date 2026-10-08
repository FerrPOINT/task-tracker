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
        if snapshot.contract_version != 1
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
        let row = tx.query_one(statement("SELECT r.payload,r.pool_slot,l.version,l.holder_subject,l.claimed_at,l.heartbeat_at,l.expires_at FROM sdlc_analysis_reservations r JOIN sdlc_analysis_reservation_leases l USING(assignment_id) WHERE r.task_id=$1 FOR UPDATE OF l",vec![state.task_id.into()])).await.map_err(map_db)?;
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
        let prior = tx.query_one(statement("SELECT EXISTS(SELECT 1 FROM sdlc_assignments WHERE task_id=$1) OR EXISTS(SELECT 1 FROM sdlc_pm_executions WHERE task_id=$1) AS pm",vec![task.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if state.assignment.is_some() || prior.try_get::<bool>("", "pm").map_err(map_db)? {
            return Err(AppError::conflict("pm_quiescence_unverified"));
        }
        let agent = snapshot.policy.routes.analyst.agent_id;
        let blocked = tx.query_one(statement("SELECT EXISTS(SELECT 1 FROM sdlc_analysis_reservations WHERE root_task_id=$1 OR agent_id=$2) AS busy, EXISTS(SELECT 1 FROM sdlc_assignments WHERE payload->>'agent_id'=$2::text) AS pm",vec![state.root_task_id.into(),agent.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?;
        if blocked.try_get::<bool>("", "pm").map_err(map_db)? {
            return Err(AppError::conflict("pm_quiescence_unverified"));
        }
        if blocked.try_get::<bool>("", "busy").map_err(map_db)? {
            return Err(AppError::conflict("root_or_agent_capacity_held"));
        }
        let slot = tx.query_one(statement("SELECT s::smallint AS slot FROM generate_series(1,2) s WHERE NOT EXISTS(SELECT 1 FROM sdlc_analysis_reservations WHERE pool_slot=s) ORDER BY s LIMIT 1",vec![])).await.map_err(map_db)?.ok_or_else(||AppError::conflict("technical_pool_capacity_held"))?.try_get::<i16>("","slot").map_err(map_db)?;
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
            route: snapshot.policy.routes.analyst,
            stage: AnalysisStage::Analysis,
            role_key: "analyst".into(),
            workflow_key: "hermes-sdlc:analyst".into(),
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
        let pm = tx.query_one(statement("SELECT EXISTS(SELECT 1 FROM sdlc_assignments WHERE task_id=$1) OR EXISTS(SELECT 1 FROM sdlc_pm_executions WHERE task_id=$1) AS pm",vec![task.into()])).await.map_err(map_db)?.ok_or_else(inconsistent)?.try_get::<bool>("","pm").map_err(map_db)? || state.assignment.is_some();
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
        let current = Self::reservation_current(&tx, &state)
            .await?
            .ok_or_else(|| AppError::not_found("Analysis reservation", task))?;
        let op = Self::reservation_operation(&tx, task, key)
            .await?
            .ok_or_else(|| AppError::not_found("reservation operation", key))?;
        if op.result.assignment != current.assignment
            || op.result.lease.version > current.lease.version
        {
            return Err(inconsistent());
        }
        tx.commit().await.map_err(map_db)?;
        Ok(op)
    }
}
