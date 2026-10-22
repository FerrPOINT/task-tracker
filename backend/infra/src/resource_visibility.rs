//! Historical messages retain data, not authority. Resolve their owning resource
//! and apply canonical project visibility before counts or pagination.
use super::{audit_log, map_audit_log, map_notification, notification};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, Statement, TransactionTrait, Value,
};
use shared::{AppError, UserId, space_access::SpaceRef};
fn params(spaces: &[SpaceRef]) -> Result<Vec<Value>, AppError> {
    if spaces.len() > 10000 || spaces.iter().any(|s| !s.valid()) {
        return Err(AppError::invalid_input("invalid_space_filter"));
    }
    let regs = spaces
        .iter()
        .map(|s| Value::Uuid(Some(Box::new(s.registry_instance_id))))
        .collect();
    let ids = spaces
        .iter()
        .map(|s| Value::Uuid(Some(Box::new(s.space_id))))
        .collect();
    Ok(vec![
        Value::Array(sea_orm::sea_query::ArrayType::Uuid, Some(Box::new(regs))),
        Value::Array(sea_orm::sea_query::ArrayType::Uuid, Some(Box::new(ids))),
    ])
}
fn number(n: u64) -> Result<Value, AppError> {
    Ok(i64::try_from(n)
        .map_err(|_| AppError::invalid_input("invalid_page"))?
        .into())
}
// e is a fixed SQL alias, never a client-provided identifier. Unknown, deleted,
// and unbound resource types are denied rather than treated as global/public.
const PROJECT_VISIBLE: &str = "EXISTS(SELECT 1 FROM tracker_project_spaces l JOIN unnest($1::uuid[],$2::uuid[]) admitted(registry,id) ON l.registry_instance_id=admitted.registry AND l.space_id=admitted.id WHERE (e.entity_type='project' AND e.entity_id=l.project_id) OR (e.entity_type='issue' AND EXISTS(SELECT 1 FROM issues i WHERE i.id=e.entity_id AND i.project_id=l.project_id)))";
pub async fn audit(
    db: &DatabaseConnection,
    spaces: &[SpaceRef],
    limit: u64,
    offset: u64,
) -> Result<Vec<domain::AuditLog>, AppError> {
    let mut args = params(spaces)?;
    args.push(number(limit.clamp(1, 100))?);
    args.push(number(offset)?);
    // Existing user-management and whitelisted-system-setting audit categories
    // are global; this does not grant any management capability.
    let sql = format!(
        "SELECT e.* FROM audit_logs e WHERE e.entity_type IN ('user','system_setting') OR {PROJECT_VISIBLE} ORDER BY e.created_at DESC,e.id DESC LIMIT $3 OFFSET $4"
    );
    let rows = audit_log::Entity::find()
        .from_raw_sql(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            args,
        ))
        .all(db)
        .await
        .map_err(AppError::database)?;
    Ok(rows.into_iter().map(map_audit_log).collect())
}
pub async fn notifications(
    db: &DatabaseConnection,
    recipient: UserId,
    spaces: &[SpaceRef],
    include_read: bool,
    limit: u64,
    offset: u64,
) -> Result<(Vec<domain::Notification>, u64), AppError> {
    if !(1..=50).contains(&limit) {
        return Err(AppError::invalid_input("invalid_notification_limit"));
    }
    let tx = db
        .begin_with_config(
            Some(sea_orm::IsolationLevel::RepeatableRead),
            Some(sea_orm::AccessMode::ReadOnly),
        )
        .await
        .map_err(AppError::database)?;
    let mut args = params(spaces)?;
    args.push(recipient.as_uuid().into());
    let query = format!(
        "SELECT count(*) AS n FROM notifications e WHERE e.recipient_id=$3 AND NOT e.is_read AND {PROJECT_VISIBLE}"
    );
    let row = tx
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            query,
            args.clone(),
        ))
        .await
        .map_err(AppError::database)?
        .ok_or_else(|| AppError::Unavailable("notification_count_missing".into()))?;
    let count: i64 = row.try_get("", "n").map_err(AppError::database)?;
    args.extend([include_read.into(), number(limit)?, number(offset)?]);
    let query = format!(
        "SELECT e.* FROM notifications e WHERE e.recipient_id=$3 AND ($4 OR NOT e.is_read) AND {PROJECT_VISIBLE} ORDER BY e.created_at DESC,e.id DESC LIMIT $5 OFFSET $6"
    );
    let rows = notification::Entity::find()
        .from_raw_sql(Statement::from_sql_and_values(
            DbBackend::Postgres,
            query,
            args,
        ))
        .all(&tx)
        .await
        .map_err(AppError::database)?;
    tx.commit().await.map_err(AppError::database)?;
    Ok((
        rows.into_iter().map(map_notification).collect(),
        u64::try_from(count)
            .map_err(|_| AppError::Unavailable("notification_count_invalid".into()))?,
    ))
}
