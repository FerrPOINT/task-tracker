//! Canonical project/space storage. Catalog barriers and link changes share database locks.
use domain::project_spaces::{ReplaceProjectSpaces, SpaceWritePermit};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value,
};
use shared::{AppError, ProjectId, space_access::*};
use uuid::Uuid;

fn statement(sql: &str, values: impl IntoIterator<Item = Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, sql, values)
}
fn tracker_instance() -> Result<Uuid, AppError> {
    let tracker_instance_id = std::env::var("TT_NAMESPACE__INSTANCE_ID")
        .ok()
        .and_then(|value| value.parse::<Uuid>().ok())
        .filter(|id| !id.is_nil())
        .ok_or_else(|| AppError::Unavailable("project_space_owner_not_configured".into()))?;
    Ok(tracker_instance_id)
}
fn project_ref(project: ProjectId) -> Result<ProjectRef, AppError> {
    Ok(ProjectRef {
        tracker_instance_id: tracker_instance()?,
        project_id: project.as_uuid(),
    })
}

pub async fn get<C: ConnectionTrait>(
    db: &C,
    project: ProjectId,
) -> Result<ConfirmedProjectSpaces, AppError> {
    let observed_at = chrono::Utc::now().timestamp();
    let row = db.query_one(statement(
        "SELECT h.revision,COALESCE(jsonb_agg(jsonb_build_object('registry_instance_id',l.registry_instance_id,'space_id',l.space_id) ORDER BY l.registry_instance_id,l.space_id) FILTER (WHERE l.space_id IS NOT NULL),'[]'::jsonb) AS spaces FROM tracker_project_space_sets h LEFT JOIN tracker_project_spaces l ON l.project_id=h.project_id WHERE h.project_id=$1 GROUP BY h.project_id,h.revision",
        [project.as_uuid().into()],
    )).await.map_err(crate::db_error::map)?.ok_or_else(|| AppError::not_found("project", project))?;
    let value: serde_json::Value = row.try_get("", "spaces").map_err(crate::db_error::map)?;
    let spaces = serde_json::from_value(value)
        .map_err(|_| AppError::Unavailable("invalid_project_space_projection".into()))?;
    let validity = AdmissionValidity::confirmed(observed_at, i64::MAX, i64::MAX)
        .ok_or_else(|| AppError::Unavailable("project_space_clock_unavailable".into()))?;
    let result = ConfirmedProjectSpaces {
        project: project_ref(project)?,
        revision: row.try_get("", "revision").map_err(crate::db_error::map)?,
        spaces,
        validity,
    };
    if !result.valid() || !validity.valid_at(chrono::Utc::now().timestamp(), i64::MAX) {
        return Err(AppError::Unavailable("project_spaces_not_confirmed".into()));
    }
    Ok(result)
}

pub(crate) async fn lock_catalog(
    tx: &DatabaseTransaction,
    spaces: &[SpaceRef],
) -> Result<std::collections::HashSet<SpaceRef>, AppError> {
    let mut active = std::collections::HashSet::new();
    let mut ordered: Vec<_> = spaces.iter().collect();
    ordered.sort_by_key(|space| (space.registry_instance_id, space.space_id));
    for space in ordered {
        let row = tx.query_one(statement("SELECT admin_revision,state FROM tracker_space_catalog WHERE registry_instance_id=$1 AND space_id=$2 FOR SHARE", [space.registry_instance_id.into(),space.space_id.into()]))
            .await.map_err(crate::db_error::map)?.ok_or_else(|| AppError::invalid_input("unknown_space_identity"))?;
        let revision: Option<i64> = row
            .try_get("", "admin_revision")
            .map_err(crate::db_error::map)?;
        let state: String = row.try_get("", "state").map_err(crate::db_error::map)?;
        if revision.is_some_and(|revision| revision > 0) && state == "active" {
            active.insert(space.clone());
        }
    }
    Ok(active)
}

/// This function is called inside the same transaction that creates the project and board.
pub async fn insert_initial(
    tx: &DatabaseTransaction,
    project: ProjectId,
    spaces: &[SpaceRef],
    permit: &SpaceWritePermit,
) -> Result<(), AppError> {
    permit.require_set(spaces)?;
    let active = lock_catalog(tx, spaces).await?;
    if !spaces.iter().all(|space| active.contains(space)) {
        return Err(AppError::conflict("space_not_active_or_confirmed"));
    }
    permit.require_set(spaces)?;
    for space in spaces {
        tx.execute(statement("INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id) VALUES ($1,$2,$3)", [project.as_uuid().into(),space.registry_instance_id.into(),space.space_id.into()]))
            .await.map_err(crate::db_error::map)?;
    }
    let links = get(tx, project).await?;
    crate::project_space_outbox::enqueue(tx, &links, permit.identity(), Uuid::new_v4()).await?;
    Ok(())
}

/// The application must verify project/link management authority before entering this port.
pub async fn replace(
    db: &DatabaseConnection,
    project: ProjectId,
    change: &ReplaceProjectSpaces,
    permit: &SpaceWritePermit,
) -> Result<ConfirmedProjectSpaces, AppError> {
    change.validate()?;
    if !permit.is_current() {
        return Err(AppError::Forbidden);
    }
    let tx = db.begin().await.map_err(crate::db_error::map)?;
    tx.query_one(statement(
        "SELECT singleton FROM tracker_space_admission_state WHERE singleton FOR SHARE",
        [],
    ))
    .await
    .map_err(crate::db_error::map)?;
    // Original request key is serialized before the project header, including first use.
    tx.execute(statement(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,0))",
        [format!("project-space-request:{}", change.operation_id).into()],
    ))
    .await
    .map_err(crate::db_error::map)?;

    tx.query_one(statement(
        "SELECT id FROM projects WHERE id=$1 FOR UPDATE",
        [project.as_uuid().into()],
    ))
    .await
    .map_err(crate::db_error::map)?
    .ok_or_else(|| AppError::not_found("project", project))?;
    let row = tx
        .query_one(statement(
            "SELECT h.revision,p.owner_id,(p.archived_at IS NOT NULL) AS archived FROM tracker_project_space_sets h JOIN projects p ON p.id=h.project_id WHERE h.project_id=$1 FOR UPDATE OF h",
            [project.as_uuid().into()],
        ))
        .await
        .map_err(crate::db_error::map)?
        .ok_or_else(|| AppError::not_found("project", project))?;
    let owner: Uuid = row.try_get("", "owner_id").map_err(crate::db_error::map)?;
    if owner != permit.actor().as_uuid() {
        return Err(AppError::Forbidden);
    }
    let profile = tx
        .query_one(statement(
            "SELECT is_active,central_sub FROM users WHERE id=$1 FOR SHARE",
            [permit.actor().as_uuid().into()],
        ))
        .await
        .map_err(crate::db_error::map)?
        .ok_or(AppError::Forbidden)?;
    if !profile
        .try_get::<bool>("", "is_active")
        .map_err(crate::db_error::map)?
        || profile
            .try_get::<Option<String>>("", "central_sub")
            .map_err(crate::db_error::map)?
            .as_deref()
            != Some(permit.identity().subject.as_str())
    {
        return Err(AppError::Forbidden);
    }
    let revision: i64 = row.try_get("", "revision").map_err(crate::db_error::map)?;
    let previous = get(&tx, project).await?;
    let mut all = previous.spaces.clone();
    all.extend(change.spaces.iter().cloned());
    all.sort_by_key(|space| (space.registry_instance_id, space.space_id));
    all.dedup();
    let active = lock_catalog(&tx, &all).await?;
    if !previous
        .spaces
        .iter()
        .any(|space| active.contains(space) && permit.allows(space))
    {
        return Err(AppError::Forbidden);
    }
    let request = serde_json::to_value(change).map_err(|e| AppError::Internal(e.to_string()))?;
    let identity =
        serde_json::to_value(permit.identity()).map_err(|e| AppError::Internal(e.to_string()))?;
    if let Some(row)=tx.query_one(statement("SELECT project_id,actor_identity,requested_change FROM tracker_project_space_operations WHERE operation_id=$1",[change.operation_id.into()])).await.map_err(crate::db_error::map)?{
        if row.try_get::<Uuid>("","project_id").map_err(crate::db_error::map)?!=project.as_uuid() || row.try_get::<serde_json::Value>("","actor_identity").map_err(crate::db_error::map)?!=identity || row.try_get::<serde_json::Value>("","requested_change").map_err(crate::db_error::map)?!=request{return Err(AppError::conflict("project_space_operation_conflict"));}
        // A replay reads the actual current set. A historical command is never
        // reissued as a newly valid canonical admission proof.
        if !permit.is_current(){return Err(AppError::Forbidden);}
        tx.commit().await.map_err(crate::db_error::map)?;return Ok(previous);
    }
    if row
        .try_get::<bool>("", "archived")
        .map_err(crate::db_error::map)?
    {
        return Err(AppError::conflict("project_archived_read_only"));
    }
    if revision != change.expected_revision {
        return Err(AppError::conflict("project_space_revision_conflict"));
    }
    // Retained associations are valid data, including archived or hidden spaces.
    // Only additions need fresh active admission; current project management still
    // requires an allowed active space from the previous set.
    let added: Vec<_> = change
        .spaces
        .iter()
        .filter(|space| !previous.spaces.contains(space))
        .collect();
    if !added
        .iter()
        .all(|space| active.contains(*space) && permit.allows(space))
    {
        return Err(AppError::Forbidden);
    }
    for space in previous
        .spaces
        .iter()
        .filter(|space| !change.spaces.contains(space))
    {
        tx.execute(statement("DELETE FROM tracker_project_spaces WHERE project_id=$1 AND registry_instance_id=$2 AND space_id=$3",[project.as_uuid().into(),space.registry_instance_id.into(),space.space_id.into()])).await.map_err(crate::db_error::map)?;
    }
    for space in added {
        tx.execute(statement("INSERT INTO tracker_project_spaces(project_id,registry_instance_id,space_id) VALUES($1,$2,$3)",[project.as_uuid().into(),space.registry_instance_id.into(),space.space_id.into()])).await.map_err(crate::db_error::map)?;
    }
    let result = get(&tx, project).await?;
    if !permit.is_current() {
        return Err(AppError::Forbidden);
    }
    let projection_operation =
        crate::project_space_outbox::enqueue(&tx, &result, permit.identity(), change.operation_id)
            .await?;
    tx.execute(statement("INSERT INTO tracker_project_space_operations(operation_id,project_id,actor_identity,requested_change,projection_operation_id,authority_valid_until) VALUES($1,$2,$3,$4,$5,$6)",[change.operation_id.into(),project.as_uuid().into(),identity.into(),request.into(),projection_operation.into(),permit.valid_until().into()])).await.map_err(crate::db_error::map)?;
    if !permit.is_current() {
        return Err(AppError::Forbidden);
    }
    tx.commit().await.map_err(crate::db_error::map)?;
    Ok(result)
}

/// One SQL snapshot for all requested project identities and complete association sets.
pub async fn contexts<C: ConnectionTrait>(
    db: &C,
    projects: &[ProjectRef],
) -> Result<Vec<shared::project_spaces::ProjectSpaceReadContext>, AppError> {
    if projects.len() > 1000 {
        return Err(AppError::invalid_input("too_many_project_selectors"));
    }
    let instance = tracker_instance()?;
    let mut seen = std::collections::HashSet::new();
    if projects
        .iter()
        .any(|p| !p.valid() || p.tracker_instance_id != instance || !seen.insert(p.project_id))
    {
        return Err(AppError::invalid_input("invalid_project_selectors"));
    }
    if projects.is_empty() {
        return Ok(Vec::new());
    }
    let observed_at = chrono::Utc::now().timestamp();
    let ids: Vec<Uuid> = projects.iter().map(|p| p.project_id).collect();
    let rows=db.query_all(statement("SELECT p.id,p.key,p.name,p.archived_at,p.project_lifecycle_revision,h.revision,coalesce(jsonb_agg(jsonb_build_object('registry_instance_id',l.registry_instance_id,'space_id',l.space_id) ORDER BY l.registry_instance_id,l.space_id) FILTER(WHERE l.space_id IS NOT NULL),'[]'::jsonb) AS spaces FROM projects p JOIN tracker_project_space_sets h ON h.project_id=p.id LEFT JOIN tracker_project_spaces l ON l.project_id=p.id WHERE p.id=ANY($1) GROUP BY p.id,p.key,p.name,h.revision ORDER BY p.id",[sea_orm::Value::Array(sea_orm::sea_query::ArrayType::Uuid,Some(Box::new(ids.into_iter().map(|id|sea_orm::Value::Uuid(Some(Box::new(id)))).collect())))])).await.map_err(crate::db_error::map)?;
    let validity = AdmissionValidity::confirmed(observed_at, i64::MAX, i64::MAX)
        .ok_or_else(|| AppError::Unavailable("project_context_clock_unavailable".into()))?;
    let mut result = Vec::new();
    for row in rows {
        let value = shared::project_spaces::ProjectSpaceReadContext {
            key: row.try_get("", "key").map_err(crate::db_error::map)?,
            name: row.try_get("", "name").map_err(crate::db_error::map)?,
            lifecycle: shared::project_spaces::ProjectLifecycle {
                schema_version: 1,
                project: ProjectRef {
                    tracker_instance_id: instance,
                    project_id: row.try_get("", "id").map_err(crate::db_error::map)?,
                },
                revision: row
                    .try_get("", "project_lifecycle_revision")
                    .map_err(crate::db_error::map)?,
                archived: row
                    .try_get::<Option<chrono::DateTime<chrono::FixedOffset>>>("", "archived_at")
                    .map_err(crate::db_error::map)?
                    .is_some(),
            },
            links: ConfirmedProjectSpaces {
                project: ProjectRef {
                    tracker_instance_id: instance,
                    project_id: row.try_get("", "id").map_err(crate::db_error::map)?,
                },
                revision: row.try_get("", "revision").map_err(crate::db_error::map)?,
                spaces: serde_json::from_value(
                    row.try_get("", "spaces").map_err(crate::db_error::map)?,
                )
                .map_err(|_| AppError::Unavailable("invalid_project_space_set".into()))?,
                validity,
            },
        };
        if !value.valid() {
            return Err(AppError::Unavailable("project_context_unconfirmed".into()));
        }
        result.push(value);
    }
    if !validity.valid_at(chrono::Utc::now().timestamp(), i64::MAX) {
        return Err(AppError::Unavailable("project_context_expired".into()));
    }
    Ok(result)
}

/// Space admission is checked by the owner API first. Every selected project has
/// this exact Space, so visibility is filtered before count, LIMIT and OFFSET.
pub async fn context_page(
    db: &DatabaseConnection,
    space: &SpaceRef,
    limit: u32,
    offset: u32,
) -> Result<domain::project_spaces::CanonicalProjectContextPage, AppError> {
    if !space.valid() || !(1..=100).contains(&limit) {
        return Err(AppError::invalid_input("invalid_project_context_page"));
    }
    let tx = db.begin().await.map_err(crate::db_error::map)?;
    tx.execute_unprepared("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .await
        .map_err(crate::db_error::map)?;
    let predicate = "EXISTS(SELECT 1 FROM tracker_project_spaces l WHERE l.project_id=p.id AND l.registry_instance_id=$1 AND l.space_id=$2)";
    let total = tx
        .query_one(statement(
            &format!("SELECT count(*)::bigint AS total FROM projects p WHERE {predicate}"),
            [space.registry_instance_id.into(), space.space_id.into()],
        ))
        .await
        .map_err(crate::db_error::map)?
        .ok_or_else(|| AppError::Unavailable("project_context_count_unavailable".into()))?
        .try_get::<i64>("", "total")
        .map_err(crate::db_error::map)?;
    let ids=tx.query_all(statement(&format!("SELECT p.id FROM projects p WHERE {predicate} ORDER BY p.name,p.id LIMIT $3 OFFSET $4"),[space.registry_instance_id.into(),space.space_id.into(),i64::from(limit).into(),i64::from(offset).into()])).await.map_err(crate::db_error::map)?;
    let tracker = tracker_instance()?;
    let mut selectors = Vec::new();
    for row in ids {
        selectors.push(ProjectRef {
            tracker_instance_id: tracker,
            project_id: row.try_get("", "id").map_err(crate::db_error::map)?,
        });
    }
    let found = contexts(&tx, &selectors).await?;
    let mut found: std::collections::HashMap<_, _> = found
        .into_iter()
        .map(|p| (p.links.project.project_id, p))
        .collect();
    let mut projects = Vec::new();
    for selector in selectors {
        projects.push(
            found
                .remove(&selector.project_id)
                .ok_or_else(|| AppError::Unavailable("project_context_page_incomplete".into()))?,
        );
    }
    tx.commit().await.map_err(crate::db_error::map)?;
    Ok(domain::project_spaces::CanonicalProjectContextPage {
        projects,
        total: u64::try_from(total)
            .map_err(|_| AppError::Unavailable("project_context_count_invalid".into()))?,
    })
}

pub async fn catalog_refs<C: ConnectionTrait>(db: &C) -> Result<Vec<SpaceRef>, AppError> {
    let rows=db.query_all(statement("SELECT registry_instance_id,space_id FROM tracker_space_catalog WHERE admin_revision IS NOT NULL ORDER BY registry_instance_id,space_id LIMIT 10001",[])).await.map_err(crate::db_error::map)?;
    if rows.len() > 10000 {
        return Err(AppError::Unavailable("space_catalog_limit_exceeded".into()));
    }
    let mut result = Vec::new();
    for row in rows {
        let space = SpaceRef {
            registry_instance_id: row
                .try_get("", "registry_instance_id")
                .map_err(crate::db_error::map)?,
            space_id: row.try_get("", "space_id").map_err(crate::db_error::map)?,
        };
        if !space.valid() {
            return Err(AppError::Unavailable(
                "space_catalog_identity_invalid".into(),
            ));
        }
        result.push(space);
    }
    Ok(result)
}
const VISIBLE_PROJECT_PREDICATE: &str = "EXISTS(SELECT 1 FROM tracker_project_spaces l JOIN unnest($1::uuid[],$2::uuid[]) allowed(registry,id) ON l.registry_instance_id=allowed.registry AND l.space_id=allowed.id WHERE l.project_id=p.id)";
fn visibility_values(spaces: &[SpaceRef]) -> Result<Vec<Value>, AppError> {
    if spaces.len() > 10000 || spaces.iter().any(|s| !s.valid()) {
        return Err(AppError::invalid_input("invalid_space_filter"));
    }
    let registries = spaces
        .iter()
        .map(|s| Value::Uuid(Some(Box::new(s.registry_instance_id))))
        .collect();
    let ids = spaces
        .iter()
        .map(|s| Value::Uuid(Some(Box::new(s.space_id))))
        .collect();
    Ok(vec![
        Value::Array(
            sea_orm::sea_query::ArrayType::Uuid,
            Some(Box::new(registries)),
        ),
        Value::Array(sea_orm::sea_query::ArrayType::Uuid, Some(Box::new(ids))),
    ])
}
/// Count and page share one SQL snapshot, including an empty page after offset.
pub async fn visible_project_catalog_ids<C: ConnectionTrait>(
    db: &C,
    spaces: &[SpaceRef],
    query: &domain::project_spaces::ProjectCatalogQuery,
) -> Result<(u64, Vec<ProjectId>), AppError> {
    query.validate()?;
    let mut values = visibility_values(spaces)?;
    if spaces.is_empty() {
        return Ok((0, Vec::new()));
    }
    values.extend([
        query.search.clone().into(),
        (query.limit as i64).into(),
        (query.offset as i64).into(),
    ]);
    values.push(query.project_key.as_ref().map(|key| key.to_string()).into());
    let sql = format!(
        "WITH visible AS (SELECT p.id,p.key FROM projects p WHERE {VISIBLE_PROJECT_PREDICATE} AND ($3='' OR strpos(lower(p.key || ' ' || p.name),lower($3))>0) AND ($6::text IS NULL OR p.key=$6)), page AS (SELECT id,key FROM visible ORDER BY key,id LIMIT $4 OFFSET $5) SELECT (SELECT count(*) FROM visible) AS total,coalesce((SELECT jsonb_agg(id ORDER BY key,id) FROM page),'[]'::jsonb) AS ids"
    );
    let row = db
        .query_one(statement(&sql, values))
        .await
        .map_err(crate::db_error::map)?
        .ok_or_else(|| AppError::Unavailable("project_catalog_snapshot_missing".into()))?;
    let total: i64 = row.try_get("", "total").map_err(crate::db_error::map)?;
    let ids: serde_json::Value = row.try_get("", "ids").map_err(crate::db_error::map)?;
    let ids: Vec<Uuid> = serde_json::from_value(ids)
        .map_err(|_| AppError::Unavailable("invalid_project_catalog_page".into()))?;
    let total = u64::try_from(total)
        .map_err(|_| AppError::Unavailable("invalid_project_catalog_total".into()))?;
    Ok((total, ids.into_iter().map(ProjectId::from_uuid).collect()))
}

/// Paired registry/space predicates prevent accidental cross-registry combinations.
pub async fn visible_project_ids<C: ConnectionTrait>(
    db: &C,
    spaces: &[SpaceRef],
    limit: u64,
    offset: u64,
) -> Result<Vec<ProjectId>, AppError> {
    let mut values = visibility_values(spaces)?;
    if spaces.is_empty() {
        return Ok(Vec::new());
    }
    let mut query = format!(
        "SELECT p.id FROM projects p WHERE {VISIBLE_PROJECT_PREDICATE} ORDER BY p.key,p.id"
    );
    if limit > 0 {
        values.push(
            i64::try_from(limit)
                .map_err(|_| AppError::invalid_input("invalid_project_limit"))?
                .into(),
        );
        query.push_str(" LIMIT $3");
    }
    if offset > 0 {
        values.push(
            i64::try_from(offset)
                .map_err(|_| AppError::invalid_input("invalid_project_offset"))?
                .into(),
        );
        query.push_str(&format!(" OFFSET ${}", values.len()));
    }
    let rows = db
        .query_all(statement(&query, values))
        .await
        .map_err(crate::db_error::map)?;
    rows.into_iter()
        .map(|r| {
            r.try_get::<Uuid>("", "id")
                .map(ProjectId::from_uuid)
                .map_err(crate::db_error::map)
        })
        .collect()
}

/// Receipt existence proves the canonical transaction committed. Absence is not a rejection proof.
pub async fn operation<C: ConnectionTrait>(
    db: &C,
    project: ProjectId,
    operation: Uuid,
    actor: &CentralIdentity,
) -> Result<Option<domain::project_spaces::StoredProjectSpaceOperation>, AppError> {
    if operation.is_nil() || !actor.valid() {
        return Err(AppError::invalid_input("invalid_project_space_operation"));
    }
    let identity = serde_json::to_value(actor).map_err(|e| AppError::Internal(e.to_string()))?;
    let row = db.query_one(statement(
        "SELECT o.requested_change,b.revision AS applied_revision FROM tracker_project_space_operations o JOIN tracker_project_space_outbox b ON b.operation_id=o.projection_operation_id AND b.project_id=o.project_id WHERE o.project_id=$1 AND o.operation_id=$2 AND o.actor_identity=$3::jsonb",
        [project.as_uuid().into(),operation.into(),identity.into()],
    )).await.map_err(crate::db_error::map)?;
    let Some(row) = row else { return Ok(None) };
    let change: domain::project_spaces::ReplaceProjectSpaces = serde_json::from_value(
        row.try_get("", "requested_change")
            .map_err(crate::db_error::map)?,
    )
    .map_err(|_| AppError::Unavailable("invalid_project_space_receipt".into()))?;
    let revision: i64 = row
        .try_get("", "applied_revision")
        .map_err(crate::db_error::map)?;
    if change.operation_id != operation
        || change.validate().is_err()
        || revision < change.expected_revision
    {
        return Err(AppError::Unavailable(
            "invalid_project_space_receipt".into(),
        ));
    }
    Ok(Some(domain::project_spaces::StoredProjectSpaceOperation {
        project: project_ref(project)?,
        operation_id: operation,
        applied_revision: revision,
        change,
    }))
}
