//! Read-only retained PM lease decoding; no claim/heartbeat/resume writer.
use super::*;
use chrono::{DateTime, Duration, Utc};
use domain::sdlc_execution_lease::*;

impl PostgresSdlcRepository {
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
        let saved = tx.query_all(statement("SELECT result FROM sdlc_pm_lease_operations WHERE execution_id=$1 AND lease_version=$2
            UNION ALL SELECT result FROM sdlc_pm_resume_inputs WHERE execution_id=$1 AND lease_version=$2",
            vec![id.into(),result.lease.version.into()])).await.map_err(map_db)?;
        if saved.len() != 1 || saved[0].try_get::<Json>("", "result").map_err(map_db)? != value {
            return Err(invalid());
        }
        Ok(Some(result))
    }
}

fn invalid() -> AppError {
    AppError::conflict("execution lease source inconsistent; reconcile before continuing")
}
pub(super) async fn clock(tx: &DatabaseTransaction) -> Result<DateTime<Utc>, AppError> {
    tx.query_one(statement("SELECT clock_timestamp() AS observed_at", vec![]))
        .await
        .map_err(map_db)?
        .ok_or_else(invalid)?
        .try_get("", "observed_at")
        .map_err(map_db)
}

pub(super) fn check_fence(
    r: &PmDraftReservation,
    owner: i64,
    f: &MachineFence,
) -> Result<(), AppError> {
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
pub(super) fn valid_receipt(
    c: &ExecutionLeaseReceipt,
    r: &PmDraftReservation,
) -> Result<(), AppError> {
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
