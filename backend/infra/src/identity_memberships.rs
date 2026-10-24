//! A bounded reverse projection; reads never create or infer a local user binding.
use domain::identity_memberships::{
    IdentityMembershipPage, IdentityProjectMembership, validate_page,
};
use sea_orm::{
    AccessMode, ConnectionTrait, DbBackend, IsolationLevel, Statement, TransactionTrait,
};
use shared::AppError;
use uuid::Uuid;

const LINKS: &str = "WITH memberships AS (SELECT p.id,p.key,p.name,'owner'::text AS role,NULL::timestamptz AS joined_at FROM projects p JOIN users u ON u.id=p.owner_id WHERE u.central_sub=$1 UNION ALL SELECT p.id,p.key,p.name,m.role,m.joined_at FROM project_members m JOIN users u ON u.id=m.user_id JOIN projects p ON p.id=m.project_id WHERE u.central_sub=$1 AND p.owner_id<>m.user_id)";

pub(crate) async fn page(
    db: &crate::guarded_db::GuardedDb,
    subject: Uuid,
    limit: u64,
    offset: u64,
) -> Result<IdentityMembershipPage, AppError> {
    validate_page(subject, limit, offset)?;
    let tx = db
        .begin_with_config(
            Some(IsolationLevel::RepeatableRead),
            Some(AccessMode::ReadOnly),
        )
        .await
        .map_err(crate::db_error::map)?;
    let profiles: i64 = tx
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT count(*) AS total FROM users WHERE central_sub=$1",
            [subject.to_string().into()],
        ))
        .await
        .map_err(crate::db_error::map)?
        .ok_or_else(|| AppError::Unavailable("membership_identity_unavailable".into()))?
        .try_get("", "total")
        .map_err(crate::db_error::map)?;
    if profiles > 1 {
        return Err(AppError::Unavailable(
            "membership_identity_ambiguous".into(),
        ));
    }
    let total: i64 = tx
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!("{LINKS} SELECT count(*) AS total FROM memberships"),
            [subject.to_string().into()],
        ))
        .await
        .map_err(crate::db_error::map)?
        .ok_or_else(|| AppError::Unavailable("membership_count_unavailable".into()))?
        .try_get("", "total")
        .map_err(crate::db_error::map)?;
    let query = format!(
        "{LINKS} SELECT m.id,m.key,m.name,m.role,m.joined_at,COALESCE((SELECT jsonb_agg(jsonb_build_object('registry_instance_id',s.registry_instance_id,'space_id',s.space_id) ORDER BY s.registry_instance_id,s.space_id) FROM tracker_project_spaces s WHERE s.project_id=m.id),'[]'::jsonb) AS spaces FROM memberships m ORDER BY m.key COLLATE \"C\",m.id LIMIT $2 OFFSET $3"
    );
    let rows = tx
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            query,
            [
                subject.to_string().into(),
                (limit as i64).into(),
                (offset as i64).into(),
            ],
        ))
        .await
        .map_err(crate::db_error::map)?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let spaces: serde_json::Value = row.try_get("", "spaces").map_err(crate::db_error::map)?;
        items.push(IdentityProjectMembership {
            project_id: row.try_get("", "id").map_err(crate::db_error::map)?,
            project_key: row.try_get("", "key").map_err(crate::db_error::map)?,
            project_name: row.try_get("", "name").map_err(crate::db_error::map)?,
            role: row.try_get("", "role").map_err(crate::db_error::map)?,
            joined_at: row.try_get("", "joined_at").map_err(crate::db_error::map)?,
            spaces: serde_json::from_value(spaces)
                .map_err(|_| AppError::Unavailable("membership_spaces_invalid".into()))?,
        });
    }
    tx.commit().await.map_err(crate::db_error::map)?;
    Ok(IdentityMembershipPage {
        items,
        total: u64::try_from(total)
            .map_err(|_| AppError::Unavailable("membership_count_invalid".into()))?,
        limit,
        offset,
    })
}
