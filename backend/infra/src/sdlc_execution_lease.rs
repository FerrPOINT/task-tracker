use super::*;
use chrono::{DateTime, Duration, Utc};
use domain::sdlc_execution_lease::*;

impl PostgresSdlcRepository {
    async fn lease_authority(
        &self,
        tx: &DatabaseTransaction,
        task: Uuid,
        actor: &Principal,
    ) -> Result<PmDraftReservation, AppError> {
        if actor.human_session {
            return Err(AppError::Forbidden);
        }
        let state = self.load_read(tx, task, actor).await?;
        let assignment = state
            .assignment
            .as_ref()
            .ok_or_else(|| AppError::conflict("reserved PM assignment absent"))?;
        if actor.subject != assignment.machine_subject
            || !actor.scopes.contains(&assignment.scope(task))
        {
            return Err(AppError::Forbidden);
        }
        let row = tx.query_one(statement(
            "SELECT pm_owner_version,pm_execution_id,pm_admission_state FROM sdlc_tasks WHERE task_id=$1",
            vec![task.into()])).await.map_err(map_db)?.ok_or_else(invalid)?;
        let id: Option<Uuid> = row.try_get("", "pm_execution_id").map_err(map_db)?;
        let version: i64 = row.try_get("", "pm_owner_version").map_err(map_db)?;
        let status: Option<String> = row.try_get("", "pm_admission_state").map_err(map_db)?;
        if id != Some(assignment.execution_id) || status.as_deref() != Some("reserved") {
            return Err(AppError::conflict("PM reservation is not current"));
        }
        let result = Self::reservation(tx, &state, assignment.execution_id).await?;
        if result.assignment != *assignment
            || result.owner_cas.version != version
            || result.contract_version != 1
            || result.dispatch_allowed
        {
            return Err(invalid());
        }
        Ok(result)
    }
    pub(super) async fn lease_current(
        tx: &DatabaseTransaction,
        reservation: &PmDraftReservation,
    ) -> Result<Option<ExecutionLeaseReceipt>, AppError> {
        let id = reservation.assignment.execution_id;
        let row = tx.query_one(statement(
            "SELECT lease_id,version,holder_subject,claimed_at,heartbeat_at,expires_at,result FROM sdlc_pm_execution_leases WHERE execution_id=$1 FOR UPDATE",
            vec![id.into()])).await.map_err(map_db)?;
        let Some(row) = row else {
            let history = tx.query_one(statement("SELECT EXISTS(SELECT 1 FROM sdlc_pm_lease_operations WHERE execution_id=$1) AS retained", vec![id.into()]))
                .await.map_err(map_db)?.ok_or_else(invalid)?;
            if history.try_get::<bool>("", "retained").map_err(map_db)? {
                return Err(invalid());
            }
            return Ok(None);
        };
        let value: Json = row.try_get("", "result").map_err(map_db)?;
        let result: ExecutionLeaseReceipt =
            serde_json::from_value(value.clone()).map_err(|_| invalid())?;
        valid_receipt(&result, reservation)?;
        if serde_json::to_value(&result).map_err(AppError::internal)? != value
            || result.lease.lease_id != row.try_get::<Uuid>("", "lease_id").map_err(map_db)?
            || result.lease.version != row.try_get::<i64>("", "version").map_err(map_db)?
            || result.lease.holder_subject
                != row
                    .try_get::<String>("", "holder_subject")
                    .map_err(map_db)?
            || result.lease.claimed_at
                != row
                    .try_get::<DateTime<Utc>>("", "claimed_at")
                    .map_err(map_db)?
            || result.lease.heartbeat_at
                != row
                    .try_get::<DateTime<Utc>>("", "heartbeat_at")
                    .map_err(map_db)?
            || result.lease.expires_at
                != row
                    .try_get::<DateTime<Utc>>("", "expires_at")
                    .map_err(map_db)?
        {
            return Err(invalid());
        }
        let saved = tx.query_one(statement("SELECT result FROM sdlc_pm_lease_operations WHERE execution_id=$1 AND lease_version=$2",
            vec![id.into(),result.lease.version.into()])).await.map_err(map_db)?.ok_or_else(invalid)?;
        if saved.try_get::<Json>("", "result").map_err(map_db)? != value {
            return Err(invalid());
        }
        Ok(Some(result))
    }
    async fn lease_operation(
        tx: &DatabaseTransaction,
        reservation: &PmDraftReservation,
        current: Option<&ExecutionLeaseReceipt>,
        actor: &Principal,
        key: &str,
    ) -> Result<Option<ExecutionLeaseOperation>, AppError> {
        let row = tx.query_one(statement(
            "SELECT payload_hash,lease_version,result FROM sdlc_pm_lease_operations WHERE execution_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![reservation.assignment.execution_id.into(),actor.subject.clone().into(),key.into()])).await.map_err(map_db)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let value: Json = row.try_get("", "result").map_err(map_db)?;
        let result: ExecutionLeaseReceipt =
            serde_json::from_value(value.clone()).map_err(|_| invalid())?;
        valid_receipt(&result, reservation)?;
        let current = current.ok_or_else(invalid)?;
        if serde_json::to_value(&result).map_err(AppError::internal)? != value
            || result.lease.version != row.try_get::<i64>("", "lease_version").map_err(map_db)?
            || result.lease.lease_id != current.lease.lease_id
            || result.lease.claimed_at != current.lease.claimed_at
            || result.lease.version > current.lease.version
            || result.lease.heartbeat_at > current.lease.heartbeat_at
        {
            return Err(invalid());
        }
        Ok(Some(ExecutionLeaseOperation {
            idempotency_key: key.into(),
            request_sha256: row.try_get("", "payload_hash").map_err(map_db)?,
            result,
        }))
    }
    async fn save_lease_operation(
        tx: &DatabaseTransaction,
        reservation: &PmDraftReservation,
        actor: &Principal,
        key: &str,
        hash: &str,
        receipt: &ExecutionLeaseReceipt,
    ) -> Result<(), AppError> {
        exec(tx, "INSERT INTO sdlc_pm_lease_operations(execution_id,actor_subject,idempotency_key,payload_hash,lease_version,result) VALUES($1,$2,$3,$4,$5,$6)",
            vec![reservation.assignment.execution_id.into(),actor.subject.clone().into(),key.into(),hash.into(),receipt.lease.version.into(),json_value(receipt)?]).await
    }
    pub(super) async fn claim_pm_lease(
        &self,
        task: Uuid,
        actor: &Principal,
        command: ClaimExecutionLease,
    ) -> Result<(ExecutionLeaseReceipt, bool), AppError> {
        app::sdlc::validate_key(&command.idempotency_key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let reservation = self.lease_authority(&tx, task, actor).await?;
        check_fence(&reservation, command.expected_owner_version, &command.fence)?;
        let current = Self::lease_current(&tx, &reservation).await?;
        let hash = app::sdlc::canonical_hash(
            &json!({"operation":"claim_pm_execution_lease","payload":command}),
        )?;
        if let Some(saved) = Self::lease_operation(
            &tx,
            &reservation,
            current.as_ref(),
            actor,
            &command.idempotency_key,
        )
        .await?
        {
            if saved.request_sha256 != hash {
                return Err(AppError::conflict("lease key reused with changed payload"));
            }
            tx.commit().await.map_err(map_db)?;
            return Ok((saved.result, true));
        }
        if current.is_some() {
            return Err(AppError::conflict(
                "execution lease already claimed; quiescence recovery required",
            ));
        }
        let now = clock(&tx).await?;
        let receipt = receipt(
            &reservation,
            ExecutionLease {
                lease_id: Uuid::now_v7(),
                version: 1,
                holder_subject: actor.subject.clone(),
                claimed_at: now,
                heartbeat_at: now,
                expires_at: now + Duration::seconds(LEASE_TTL_SECONDS),
            },
        );
        exec(&tx, "INSERT INTO sdlc_pm_execution_leases(execution_id,lease_id,version,holder_subject,claimed_at,heartbeat_at,expires_at,result) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
            vec![reservation.assignment.execution_id.into(),receipt.lease.lease_id.into(),1i64.into(),actor.subject.clone().into(),now.into(),now.into(),receipt.lease.expires_at.into(),json_value(&receipt)?]).await?;
        Self::save_lease_operation(
            &tx,
            &reservation,
            actor,
            &command.idempotency_key,
            &hash,
            &receipt,
        )
        .await?;
        tx.commit().await.map_err(map_db)?;
        Ok((receipt, false))
    }
    pub(super) async fn heartbeat_pm_lease(
        &self,
        task: Uuid,
        actor: &Principal,
        command: HeartbeatExecutionLease,
    ) -> Result<(ExecutionLeaseReceipt, bool), AppError> {
        app::sdlc::validate_key(&command.idempotency_key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let reservation = self.lease_authority(&tx, task, actor).await?;
        check_fence(&reservation, command.expected_owner_version, &command.fence)?;
        let mut current = Self::lease_current(&tx, &reservation)
            .await?
            .ok_or_else(|| {
                AppError::conflict("execution lease unknown; reconcile before continuing")
            })?;
        let hash = app::sdlc::canonical_hash(
            &json!({"operation":"heartbeat_pm_execution_lease","payload":command}),
        )?;
        if let Some(saved) = Self::lease_operation(
            &tx,
            &reservation,
            Some(&current),
            actor,
            &command.idempotency_key,
        )
        .await?
        {
            if saved.request_sha256 != hash {
                return Err(AppError::conflict("lease key reused with changed payload"));
            }
            tx.commit().await.map_err(map_db)?;
            return Ok((saved.result, true));
        }
        let now = clock(&tx).await?;
        if current.lease.expires_at <= now
            || current.lease.lease_id != command.lease_id
            || current.lease.version != command.expected_lease_version
            || current.lease.version >= MAX_SAFE_VERSION
        {
            return Err(AppError::conflict(
                "execution lease expired or stale; quiescence recovery required",
            ));
        }
        current.lease.version += 1;
        current.lease.heartbeat_at = now;
        current.lease.expires_at = now + Duration::seconds(LEASE_TTL_SECONDS);
        exec(&tx, "UPDATE sdlc_pm_execution_leases SET version=$2,heartbeat_at=$3,expires_at=$4,result=$5 WHERE execution_id=$1 AND version=$6",
            vec![reservation.assignment.execution_id.into(),current.lease.version.into(),now.into(),current.lease.expires_at.into(),json_value(&current)?,command.expected_lease_version.into()]).await?;
        Self::save_lease_operation(
            &tx,
            &reservation,
            actor,
            &command.idempotency_key,
            &hash,
            &current,
        )
        .await?;
        tx.commit().await.map_err(map_db)?;
        Ok((current, false))
    }
    pub(super) async fn read_pm_lease(
        &self,
        task: Uuid,
        actor: &Principal,
        key: Option<&str>,
    ) -> Result<ExecutionLeaseReadback, AppError> {
        if let Some(key) = key {
            app::sdlc::validate_key(key)?;
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        let reservation = self.lease_authority(&tx, task, actor).await?;
        let current = Self::lease_current(&tx, &reservation).await?;
        let operation = match key {
            Some(key) => {
                Self::lease_operation(&tx, &reservation, current.as_ref(), actor, key).await?
            }
            None => None,
        };
        let observed_at = clock(&tx).await?;
        let state = match &current {
            None => ExecutionLeaseState::Unclaimed,
            Some(c) if c.lease.expires_at > observed_at => ExecutionLeaseState::Active,
            Some(_) => ExecutionLeaseState::Expired,
        };
        let result = ExecutionLeaseReadback {
            contract_version: 1,
            binding: reservation.binding.clone(),
            owner_version: reservation.owner_cas.version,
            fence: fence(&reservation),
            observed_at,
            state,
            current: current.map(|c| c.lease),
            operation,
            dispatch_allowed: false,
        };
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }
}
fn invalid() -> AppError {
    AppError::conflict("execution lease source inconsistent; reconcile before continuing")
}
async fn clock(tx: &DatabaseTransaction) -> Result<DateTime<Utc>, AppError> {
    tx.query_one(statement("SELECT clock_timestamp() AS observed_at", vec![]))
        .await
        .map_err(map_db)?
        .ok_or_else(invalid)?
        .try_get("", "observed_at")
        .map_err(map_db)
}
fn fence(r: &PmDraftReservation) -> MachineFence {
    MachineFence {
        assignment_id: r.assignment.assignment_id,
        execution_id: r.assignment.execution_id,
        agent_id: r.assignment.agent_id,
        assignment_version: r.assignment.version,
    }
}
fn check_fence(r: &PmDraftReservation, owner: i64, f: &MachineFence) -> Result<(), AppError> {
    if owner != r.owner_cas.version
        || f.assignment_id != r.assignment.assignment_id
        || f.execution_id != r.assignment.execution_id
        || f.agent_id != r.assignment.agent_id
        || f.assignment_version != r.assignment.version
    {
        return Err(AppError::conflict("stale PM owner/assignment fence"));
    }
    Ok(())
}
fn receipt(r: &PmDraftReservation, lease: ExecutionLease) -> ExecutionLeaseReceipt {
    ExecutionLeaseReceipt {
        contract_version: 1,
        binding: r.binding.clone(),
        owner_version: r.owner_cas.version,
        fence: fence(r),
        lease,
        ttl_seconds: LEASE_TTL_SECONDS,
        heartbeat_seconds: LEASE_HEARTBEAT_SECONDS,
        dispatch_allowed: false,
    }
}
fn valid_receipt(c: &ExecutionLeaseReceipt, r: &PmDraftReservation) -> Result<(), AppError> {
    check_fence(r, c.owner_version, &c.fence)?;
    if c.contract_version != 1
        || c.binding != r.binding
        || c.dispatch_allowed
        || c.ttl_seconds != LEASE_TTL_SECONDS
        || c.heartbeat_seconds != LEASE_HEARTBEAT_SECONDS
        || c.lease.lease_id.is_nil()
        || !(1..=MAX_SAFE_VERSION).contains(&c.lease.version)
        || c.lease.holder_subject != r.assignment.machine_subject
        || c.lease.heartbeat_at < c.lease.claimed_at
        || (c.lease.version == 1 && c.lease.heartbeat_at != c.lease.claimed_at)
        || c.lease.expires_at != c.lease.heartbeat_at + Duration::seconds(LEASE_TTL_SECONDS)
    {
        return Err(invalid());
    }
    Ok(())
}
