//! Human SDLC reads use the same canonical space proof as the ordinary Tracker API.
use super::*;
use app::space_access::{CentralRequestContext, SpaceActor};
use shared::{ProjectId, space_access::SpaceRef};

pub(super) async fn request_scope(
    db: &impl ConnectionTrait,
    actor: &Principal,
) -> Result<Option<Arc<CentralRequestContext>>, AppError> {
    // Native capabilities remain subject to their own exact task/lease protocol.
    if !actor.trusted_human {
        return Ok(None);
    }
    let row = db
        .query_one(statement(
            "SELECT active FROM tracker_space_admission_state WHERE singleton",
            vec![],
        ))
        .await
        .map_err(map_db)?
        .ok_or_else(|| AppError::Unavailable("space_admission_state_missing".into()))?;
    if !row.try_get::<bool>("", "active").map_err(map_db)? {
        return Ok(None);
    }
    let frame = app::space_access::current_request()?.ok_or(AppError::Forbidden)?;
    let spaces = frame.spaces().ok_or(AppError::Forbidden)?;
    if !frame.is_current() || spaces.identity().subject != actor.subject {
        return Err(AppError::Forbidden);
    }
    Ok(Some(frame))
}
pub(super) async fn require_project(
    tx: &DatabaseTransaction,
    project: Uuid,
    user: Uuid,
    actor: &Principal,
) -> Result<bool, AppError> {
    let Some(frame) = request_scope(tx, actor).await? else {
        return Ok(false);
    };
    if frame.user().as_uuid() != user {
        return Err(AppError::Forbidden);
    }
    let proof = frame.spaces().ok_or(AppError::Forbidden)?;
    let links = crate::project_spaces::get(tx, ProjectId::from_uuid(project)).await?;
    proof.require_read(&links)?;
    if frame.is_mutation() {
        if !frame.allows_mutation() {
            return Err(AppError::Forbidden);
        }
        proof.require_project_write(&links)?;
        // The caller still enforces native project ownership/membership for writes.
        return Ok(false);
    }
    Ok(true)
}
/// Owner-specific protocols retain their native owner check, but an old native
/// membership never restores a human namespace grant that has been revoked.
pub(super) async fn require_owner_project(
    tx: &DatabaseTransaction,
    project: Uuid,
    _owner: Uuid,
) -> Result<(), AppError> {
    if let Some(frame) = app::space_access::current_request()? {
        if let Some(proof) = frame.spaces() {
            let links = crate::project_spaces::get(tx, ProjectId::from_uuid(project)).await?;
            proof.require_read(&links)?;
            if frame.is_mutation() {
                if !frame.allows_mutation() {
                    return Err(AppError::Forbidden);
                }
                proof.require_project_write(&links)?;
            }
        }
        if !frame.is_current() {
            return Err(AppError::Forbidden);
        }
    }
    Ok(())
}
fn arrays(spaces: &[SpaceRef]) -> Vec<Value> {
    let registry = spaces
        .iter()
        .map(|s| Value::Uuid(Some(Box::new(s.registry_instance_id))))
        .collect();
    let ids = spaces
        .iter()
        .map(|s| Value::Uuid(Some(Box::new(s.space_id))))
        .collect();
    vec![
        Value::Array(
            sea_orm::sea_query::ArrayType::Uuid,
            Some(Box::new(registry)),
        ),
        Value::Array(sea_orm::sea_query::ArrayType::Uuid, Some(Box::new(ids))),
    ]
}
fn params(actor: &Principal, proof: &SpaceActor) -> Result<Vec<Value>, AppError> {
    let mut values = vec![actor.subject.clone().into()];
    values.extend(arrays(&proof.readable_spaces()?));
    Ok(values)
}
const ALLOWED: &str = "SELECT p.id FROM projects p WHERE EXISTS(SELECT 1 FROM tracker_project_spaces l JOIN unnest($2::uuid[],$3::uuid[]) s(registry,id) ON l.registry_instance_id=s.registry AND l.space_id=s.id WHERE l.project_id=p.id)";
pub(super) async fn access(
    db: &impl ConnectionTrait,
    actor: &Principal,
    frame: &CentralRequestContext,
    instance: &str,
) -> Result<ProjectAccess, AppError> {
    let proof = frame.spaces().ok_or(AppError::Forbidden)?;
    let sql = format!(
        "WITH active AS MATERIALIZED(SELECT id FROM users WHERE central_sub=$1 AND is_active=true),allowed AS ({ALLOWED}) SELECT EXISTS(SELECT 1 FROM active) AS authorized,COALESCE((SELECT jsonb_agg(id ORDER BY id) FROM allowed),'[]'::jsonb) AS project_ids"
    );
    let row = db
        .query_one(statement(&sql, params(actor, proof)?))
        .await
        .map_err(map_db)?
        .ok_or_else(|| AppError::Unavailable("project_access_missing".into()))?;
    if !row.try_get::<bool>("", "authorized").map_err(map_db)? || !frame.is_current() {
        return Err(AppError::Forbidden);
    }
    Ok(ProjectAccess {
        contract_version: 1,
        tracker_instance_id: instance.into(),
        project_ids: serde_json::from_value(
            row.try_get::<Json>("", "project_ids").map_err(map_db)?,
        )
        .map_err(AppError::internal)?,
    })
}
pub(super) async fn directory(
    db: &impl ConnectionTrait,
    actor: &Principal,
    frame: &CentralRequestContext,
    instance: &str,
    after: Option<Uuid>,
    limit: u16,
) -> Result<ProjectDirectory, AppError> {
    let proof = frame.spaces().ok_or(AppError::Forbidden)?;
    let mut values = params(actor, proof)?;
    values.extend([after.into(), (i64::from(limit) + 1).into()]);
    let sql = format!(
        "WITH active AS MATERIALIZED(SELECT id FROM users WHERE central_sub=$1 AND is_active=true),allowed AS ({ALLOWED}),page AS(SELECT p.id,p.key,p.name FROM projects p JOIN allowed a ON a.id=p.id WHERE($4::uuid IS NULL OR p.id>$4::uuid) ORDER BY p.id LIMIT $5) SELECT EXISTS(SELECT 1 FROM active) AS authorized,COALESCE((SELECT jsonb_agg(jsonb_build_object('id',id,'key',key,'name',name) ORDER BY id) FROM page),'[]'::jsonb) AS projects"
    );
    let row = db
        .query_one(statement(&sql, values))
        .await
        .map_err(map_db)?
        .ok_or_else(|| AppError::Unavailable("project_directory_missing".into()))?;
    if !row.try_get::<bool>("", "authorized").map_err(map_db)? || !frame.is_current() {
        return Err(AppError::Forbidden);
    }
    let projects = serde_json::from_value(row.try_get::<Json>("", "projects").map_err(map_db)?)
        .map_err(AppError::internal)?;
    ProjectDirectory::from_prefix(instance.into(), projects, after, limit)
}

/// A linked project does not confer access to a different repository's owning space.
pub(super) fn require_resource_namespace(
    namespace: &shared::resource_context::NamespaceRef,
) -> Result<(), AppError> {
    if let Some(frame) = app::space_access::current_request()? {
        if let Some(actor) = frame.spaces() {
            let space = SpaceRef {
                registry_instance_id: namespace.registry_instance_id,
                space_id: namespace.namespace_id,
            };
            let allowed = if frame.is_mutation() {
                actor.active_spaces()?
            } else {
                actor.readable_spaces()?
            };
            if !allowed.contains(&space) {
                return Err(AppError::Forbidden);
            }
        }
        if !frame.is_current() {
            return Err(AppError::Forbidden);
        }
    }
    Ok(())
}

/// Native service access is independent of human membership. A live service
/// identity still needs its registered product protocol and the existing current
/// project ACL; the caller has already locked those rows in issue_access.
/// This value is transaction-local and can name only the aggregate actually read.
pub(super) async fn bind_task(
    tx: &DatabaseTransaction,
    config: &SdlcConfig,
    state: &TaskState,
    actor: &Principal,
) -> Result<(), AppError> {
    let row = tx.query_one(statement(
        "SELECT active OR EXISTS(SELECT 1 FROM tracker_space_catalog WHERE metadata IS NOT NULL) AS enforced FROM tracker_space_admission_state WHERE singleton",
        vec![],
    )).await.map_err(map_db)?.ok_or_else(|| AppError::Unavailable("space_admission_state_missing".into()))?;
    let space_policy = row.try_get::<bool>("", "enforced").map_err(map_db)?;
    let application_policy = app::native_space_access::application_policy_required().await?;
    if !space_policy && !application_policy {
        return Ok(());
    }
    if actor.trusted_human {
        return Ok(());
    }
    let native = app::native_space_access::current()?.ok_or(AppError::Forbidden)?;
    if native.identity().subject != actor.subject
        || (!app::sdlc::has_pm_grant(&actor.scopes)
            && !app::sdlc_lifecycle::has_delivery_grant(&actor.scopes)
            && actor.subject != config.orchestrator_subject
            && actor.subject != config.verifier_subject
            && actor.subject != config.reservation_scheduler_subject)
    {
        return Err(AppError::Forbidden);
    }
    // A task-scoped PAT cannot borrow a generic machine's native project role.
    app::sdlc::authorize_pm_read(state, actor)?;
    super::lifecycle::authorize_read(tx, state, actor).await?;
    if application_policy {
        let owner = Uuid::parse_str(&state.owner_subject).map_err(|_| AppError::Forbidden)?;
        if owner.to_string() != state.owner_subject {
            return Err(AppError::Forbidden);
        }
        // The confirmed aggregate owns this identity. A machine credential and
        // its exact task/lease grant remain separate from human admission.
        native
            .require_owner(owner, &["task-tracker", "fleet-control"])
            .await?;
        crate::guarded_db::bind_commit_clock(tx, native.validity()?.valid_until)
            .await
            .map_err(map_db)?;
    }
    if !space_policy {
        return Ok(());
    }
    let scope = serde_json::json!({
        "schema_version":1,"task_id":state.task_id,"project_id":state.project_id,
        "identity":native.identity(),"validity":native.validity()?,
        "writable":native.may_write()
    });
    tx.execute(statement(
        "SELECT tracker_bind_native_task($1::jsonb)",
        vec![scope.into()],
    ))
    .await
    .map_err(map_db)?;
    if !native.is_current() {
        return Err(AppError::Forbidden);
    }
    Ok(())
}
