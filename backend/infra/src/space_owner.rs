//! Admin-owned Space catalog barrier. No Tracker project is created or renamed here.
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value,
};
use shared::{AppError, space_lifecycle::*};
use uuid::Uuid;
fn sql(query: &str, values: impl IntoIterator<Item = Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, query, values)
}
fn current(proof: &SpaceOwnerAuthority) -> Result<(), AppError> {
    if proof.is_current() {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}
fn configured(key: &str) -> Result<Uuid, AppError> {
    std::env::var(key)
        .ok()
        .and_then(|v| Uuid::parse_str(&v).ok())
        .filter(|v| !v.is_nil())
        .ok_or_else(|| AppError::Unavailable("space_owner_not_configured".into()))
}
async fn drained(
    tx: &DatabaseTransaction,
    command: &SpaceOwnerCommandV2,
) -> Result<bool, AppError> {
    // Frozen grants survive link removal. Legacy work uses its immutable source
    // snapshot; an unclassified legacy execution blocks rather than guessing a space.
    // Only accepted terminal receipts / released analysis holds establish quiescence.
    let query = r#"SELECT
 NOT EXISTS(
   SELECT 1 FROM sdlc_assignments a
   LEFT JOIN tracker_execution_space_grants g ON g.assignment_id::text=a.payload->>'assignment_id'
   WHERE ((g.assignment_id IS NOT NULL AND g.spaces @> jsonb_build_array(jsonb_build_object('registry_instance_id',$1::uuid,'space_id',$2::uuid)))
     OR (g.assignment_id IS NULL AND (
       NOT EXISTS(SELECT 1 FROM sdlc_task_source_bindings b WHERE b.task_id=a.task_id)
       OR EXISTS(SELECT 1 FROM sdlc_task_source_bindings b WHERE b.task_id=a.task_id AND b.payload->'namespace'->>'registry_instance_id'=$1::text AND b.payload->'namespace'->>'namespace_id'=$2::text))))
   AND NOT EXISTS(SELECT 1 FROM sdlc_pm_completions c WHERE c.task_id=a.task_id
     AND c.assignment_id::text=a.payload->>'assignment_id' AND c.execution_id::text=a.payload->>'execution_id' AND c.assignment_version=a.version)
 ) AND NOT EXISTS(
   SELECT 1 FROM sdlc_analysis_capacity_holds h
   LEFT JOIN tracker_execution_space_grants g ON g.assignment_id=h.assignment_id
   WHERE (g.assignment_id IS NOT NULL AND g.spaces @> jsonb_build_array(jsonb_build_object('registry_instance_id',$1::uuid,'space_id',$2::uuid)))
     OR (g.assignment_id IS NULL AND (
       NOT EXISTS(SELECT 1 FROM sdlc_task_source_bindings b WHERE b.task_id=h.root_task_id)
       OR EXISTS(SELECT 1 FROM sdlc_task_source_bindings b WHERE b.task_id=h.root_task_id AND b.payload->'namespace'->>'registry_instance_id'=$1::text AND b.payload->'namespace'->>'namespace_id'=$2::text)))
 ) AS drained"#;
    let row = tx
        .query_one(sql(
            query,
            [
                command.space.registry_instance_id.into(),
                command.space.space_id.into(),
            ],
        ))
        .await
        .map_err(AppError::database)?
        .ok_or_else(|| AppError::Unavailable("space_drain_readback_missing".into()))?;
    row.try_get("", "drained").map_err(AppError::database)
}
pub async fn apply(
    db: &DatabaseConnection,
    command: &SpaceOwnerCommandV2,
    authority: &SpaceOwnerAuthority,
) -> Result<SpaceOwnerReadbackV2, AppError> {
    current(authority)?;
    let registry = configured("TT_NAMESPACE__REGISTRY_INSTANCE_ID")?;
    let instance = configured("TT_NAMESPACE__INSTANCE_ID")?;
    if !command.valid_for(registry, instance) {
        return Err(AppError::invalid_input("invalid_space_owner_v2_command"));
    }
    let tx = db.begin().await.map_err(AppError::database)?;
    tx.execute(sql("SET TRANSACTION ISOLATION LEVEL READ COMMITTED", []))
        .await
        .map_err(AppError::database)?;
    tx.execute(sql(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        [format!("space-owner-operation:{}", command.operation_id).into()],
    ))
    .await
    .map_err(AppError::database)?;
    tx.query_one(sql(
        "SELECT singleton FROM tracker_space_admission_state WHERE singleton FOR SHARE",
        [],
    ))
    .await
    .map_err(AppError::database)?;
    tx.execute(sql(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        [format!("shared-space:{registry}:{}", command.space.space_id).into()],
    ))
    .await
    .map_err(AppError::database)?;
    current(authority)?;
    let mut replay = false;
    if let Some(row) = tx
        .query_one(sql(
            "SELECT command,readback FROM tracker_space_owner_operations WHERE operation_id=$1",
            [command.operation_id.into()],
        ))
        .await
        .map_err(AppError::database)?
    {
        let old: SpaceOwnerCommandV2 =
            serde_json::from_value(row.try_get("", "command").map_err(AppError::database)?)
                .map_err(|_| AppError::Unavailable("invalid_space_owner_receipt".into()))?;
        let ack: SpaceOwnerReadbackV2 =
            serde_json::from_value(row.try_get("", "readback").map_err(AppError::database)?)
                .map_err(|_| AppError::Unavailable("invalid_space_owner_receipt".into()))?;
        let mut checked = ack.clone();
        checked.drained = true;
        if old != *command || !checked.matches(command) || ack.local_resource.is_some() {
            return Err(AppError::conflict("space_operation_payload_conflict"));
        }
        if ack.matches(command) {
            current(authority)?;
            tx.commit().await.map_err(AppError::database)?;
            return Ok(ack);
        }
        replay = true;
    }
    let old=tx.query_one(sql("SELECT admin_revision,operation_id FROM tracker_space_catalog WHERE registry_instance_id=$1 AND space_id=$2 FOR UPDATE",[registry.into(),command.space.space_id.into()])).await.map_err(AppError::database)?;
    current(authority)?;
    if let Some(row) = old {
        let revision: Option<i64> = row
            .try_get("", "admin_revision")
            .map_err(AppError::database)?;
        let operation: Option<Uuid> = row
            .try_get("", "operation_id")
            .map_err(AppError::database)?;
        if revision.is_some_and(|r| r > command.revision)
            || (replay && operation != Some(command.operation_id))
        {
            return Err(AppError::conflict("space_owner_revision_conflict"));
        }
        if let Some(row)=tx.query_one(sql("SELECT command,readback FROM tracker_space_owner_operations WHERE registry_instance_id=$1 AND space_id=$2 ORDER BY revision DESC LIMIT 1",[registry.into(),command.space.space_id.into()])).await.map_err(AppError::database)? {
   let prior:SpaceOwnerCommandV2=serde_json::from_value(row.try_get("","command").map_err(AppError::database)?).map_err(|_|AppError::Unavailable("invalid_space_owner_receipt".into()))?;
   let ack:SpaceOwnerReadbackV2=serde_json::from_value(row.try_get("","readback").map_err(AppError::database)?).map_err(|_|AppError::Unavailable("invalid_space_owner_receipt".into()))?;
   if prior.operation_id!=command.operation_id && (!ack.matches(&prior) || !command.follows(&prior)) {return Err(AppError::conflict("space_owner_operation_pending_or_stale"));}
  }
    } else {
        if command.desired_state != DesiredSpaceState::Active {
            return Err(AppError::conflict("space_catalog_entry_required"));
        }
        tx.execute(sql("INSERT INTO tracker_space_catalog(registry_instance_id,space_id,state) VALUES($1,$2,'provisioning')",[registry.into(),command.space.space_id.into()])).await.map_err(AppError::database)?;
    }
    let state = if command.desired_state == DesiredSpaceState::Active {
        "active"
    } else {
        "archiving"
    };
    tx.execute(sql("UPDATE tracker_space_catalog SET admin_revision=$3,state=$4,operation_id=$5,metadata=$6 WHERE registry_instance_id=$1 AND space_id=$2",[registry.into(),command.space.space_id.into(),command.revision.into(),state.into(),command.operation_id.into(),serde_json::to_value(&command.metadata).map_err(|_|AppError::Internal("invalid_space_metadata".into()))?.into()])).await.map_err(AppError::database)?;
    let is_drained =
        command.desired_state == DesiredSpaceState::Active || drained(&tx, command).await?;
    let ack = SpaceOwnerReadbackV2 {
        schema_version: 2,
        space: command.space.clone(),
        participant_instance_id: instance,
        operation_id: command.operation_id,
        revision: command.revision,
        state: command.desired_state,
        drained: is_drained,
        local_resource: None,
    };
    if command.desired_state == DesiredSpaceState::Archived && is_drained {
        tx.execute(sql("UPDATE tracker_space_catalog SET state='archived' WHERE registry_instance_id=$1 AND space_id=$2 AND operation_id=$3",[registry.into(),command.space.space_id.into(),command.operation_id.into()])).await.map_err(AppError::database)?;
    }
    let command_value: Value = serde_json::to_value(command)
        .map_err(|_| AppError::Internal("invalid_space_owner_command".into()))?
        .into();
    let readback: Value = serde_json::to_value(&ack)
        .map_err(|_| AppError::Internal("invalid_space_owner_readback".into()))?
        .into();
    if replay {
        tx.execute(sql(
            "UPDATE tracker_space_owner_operations SET readback=$2 WHERE operation_id=$1",
            [command.operation_id.into(), readback],
        ))
        .await
        .map_err(AppError::database)?;
    } else {
        tx.execute(sql("INSERT INTO tracker_space_owner_operations(operation_id,registry_instance_id,space_id,revision,command,readback,owner_issuer,owner_subject) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",[command.operation_id.into(),registry.into(),command.space.space_id.into(),command.revision.into(),command_value,readback,authority.identity().issuer.clone().into(),authority.identity().subject.clone().into()])).await.map_err(AppError::database)?;
    }
    current(authority)?;
    tx.commit().await.map_err(AppError::database)?;
    Ok(ack)
}
