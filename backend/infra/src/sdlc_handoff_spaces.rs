//! Fresh owner visibility restricts an independently verified native capability.
//! It is never a human session, product edit role or native execution permit.
use super::*;
use shared::{resource_context::NamespaceRef, space_access::SpaceRef};
#[derive(PartialEq, Eq)]
struct Basis {
    owner: String,
    project: Uuid,
    source: NamespaceRef,
    links: Vec<SpaceRef>,
}
pub(super) struct OwnerDataAdmission {
    basis: Basis,
    proof: app::native_space_access::OwnerSpaceRead,
}
impl OwnerDataAdmission {
    pub(super) fn current(&self) -> Result<(), AppError> {
        self.check(false).map(|_| ())
    }
    fn active_dependency_spaces(&self) -> Result<Vec<SpaceRef>, AppError> {
        self.check(true)?;
        let admission = self.proof.admission().map_err(|_| AppError::Forbidden)?;
        let now = chrono::Utc::now().timestamp();
        Ok(self
            .basis
            .links
            .iter()
            .filter(|space| {
                admission.has_active_space(
                    &admission.identity,
                    space,
                    now,
                    admission.validity.valid_until,
                )
            })
            .cloned()
            .collect())
    }
    fn check(&self, write: bool) -> Result<i64, AppError> {
        let admission = self.proof.admission().map_err(|_| AppError::Forbidden)?;
        if admission.identity.subject != self.basis.owner {
            return Err(AppError::Forbidden);
        }
        let now = chrono::Utc::now().timestamp();
        let expiry = admission.validity.valid_until;
        let permits = |space: &SpaceRef| {
            if write {
                admission.has_active_space(&admission.identity, space, now, expiry)
            } else {
                admission.can_read_space(&admission.identity, space, now, expiry)
            }
        };
        let source = SpaceRef {
            registry_instance_id: self.basis.source.registry_instance_id,
            space_id: self.basis.source.namespace_id,
        };
        if !permits(&source)
            || (!self.basis.links.is_empty() && !self.basis.links.iter().any(permits))
        {
            return Err(AppError::Forbidden);
        }
        Ok(expiry)
    }
}
impl PostgresSdlcRepository {
    async fn owner_data_basis(
        tx: &DatabaseTransaction,
        state: &TaskState,
        source: Option<&NamespaceRef>,
    ) -> Result<Option<Basis>, AppError> {
        let active = tx
            .query_one(statement(
                "SELECT active FROM tracker_space_admission_state WHERE singleton FOR SHARE",
                vec![],
            ))
            .await
            .map_err(map_db)?
            .ok_or(AppError::Forbidden)?
            .try_get::<bool>("", "active")
            .map_err(map_db)?;
        if !active {
            return Ok(None);
        }
        let source = if let Some(source) = source {
            source.clone()
        } else {
            let origin = state
                .retained_fields
                .get("execution_handoff_id")
                .or_else(|| state.retained_fields.get("human_handoff_id"))
                .ok_or(AppError::Forbidden)?;
            let id: Uuid =
                serde_json::from_value(origin.clone()).map_err(|_| AppError::Forbidden)?;
            let row=tx.query_one(statement("SELECT source_binding->'namespace' AS namespace FROM task_handoff_materials WHERE task_id=$1 AND handoff_id=$2 AND actor_subject=$3",vec![state.task_id.into(),id.into(),state.owner_subject.clone().into()])).await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
            serde_json::from_value(row.try_get::<Json>("", "namespace").map_err(map_db)?)
                .map_err(AppError::internal)?
        };
        // Link management locks this same project header. Empty links retain the
        // independent project's native membership requirement; source access remains explicit.
        tx.query_one(statement(
            "SELECT project_id FROM tracker_project_space_sets WHERE project_id=$1 FOR SHARE",
            vec![state.project_id.into()],
        ))
        .await
        .map_err(map_db)?;
        let rows=tx.query_all(statement("SELECT registry_instance_id,space_id FROM tracker_project_spaces WHERE project_id=$1 ORDER BY registry_instance_id,space_id",vec![state.project_id.into()])).await.map_err(map_db)?;
        let links = rows
            .into_iter()
            .map(|row| {
                Ok(SpaceRef {
                    registry_instance_id: row
                        .try_get("", "registry_instance_id")
                        .map_err(map_db)?,
                    space_id: row.try_get("", "space_id").map_err(map_db)?,
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        Ok(Some(Basis {
            owner: state.owner_subject.clone(),
            project: state.project_id,
            source,
            links,
        }))
    }
    pub(super) async fn owner_data_admission(
        &self,
        task: Uuid,
        actor: &Principal,
        source: Option<&NamespaceRef>,
    ) -> Result<Option<OwnerDataAdmission>, AppError> {
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = Self::load_binding(&tx, &self.config, task, actor).await?;
        let basis = Self::owner_data_basis(&tx, &state, source).await?;
        tx.commit().await.map_err(map_db)?;
        let Some(basis) = basis else { return Ok(None) };
        let native = app::native_space_access::current()?.ok_or(AppError::Forbidden)?;
        if native.identity().subject != actor.subject {
            return Err(AppError::Forbidden);
        }
        let owner = Uuid::parse_str(&basis.owner).map_err(|_| AppError::Forbidden)?;
        if owner.to_string() != basis.owner {
            return Err(AppError::Forbidden);
        }
        let mut spaces = basis.links.clone();
        spaces.push(SpaceRef {
            registry_instance_id: basis.source.registry_instance_id,
            space_id: basis.source.namespace_id,
        });
        spaces.sort_by_key(|s| (s.registry_instance_id, s.space_id));
        spaces.dedup();
        let proof = native.owner_space_read(owner, &spaces).await?;
        if proof
            .admission()
            .map_err(|_| AppError::Forbidden)?
            .identity
            .issuer
            != native.identity().issuer
        {
            return Err(AppError::Forbidden);
        }
        Ok(Some(OwnerDataAdmission { basis, proof }))
    }
    /// Freeze actual dependency identities in the existing execution ledger. The
    /// renewable owner read proof never becomes a native write/launch capability.
    pub(super) async fn freeze_handoff_dependencies(
        tx: &DatabaseTransaction,
        pending: &app::task_handoff::PendingHandoffMaterials,
        assignment: Uuid,
        actor: &Principal,
        proof: Option<&OwnerDataAdmission>,
        observed: &app::task_handoff_observations::MatchedHandoffInputs,
    ) -> Result<(), AppError> {
        let until: chrono::DateTime<chrono::Utc> =
            serde_json::from_value(observed.audit()["valid_until"].clone())
                .map_err(AppError::internal)?;
        Self::freeze_delivery_dependencies(
            tx,
            &pending.source,
            assignment,
            actor,
            proof,
            Some(until),
        )
        .await
    }
    /// A later real assignment retains the same human origin, not a new consent
    /// or a fictitious Fleet observation. Fresh owner admission is already bound.
    pub(super) async fn freeze_current_handoff_dependencies(
        tx: &DatabaseTransaction,
        state: &TaskState,
        assignment: Uuid,
        actor: &Principal,
        proof: Option<&OwnerDataAdmission>,
    ) -> Result<(), AppError> {
        let row=tx.query_one(statement("SELECT m.source_binding FROM task_handoff_materials m JOIN sdlc_tasks t USING(task_id) WHERE t.task_id=$1 AND m.handoff_id::text=t.state->>'execution_handoff_id' AND m.actor_subject=t.owner_subject AND task_handoff_current_delivery_origin(t.task_id,t.state)",vec![state.task_id.into()])).await.map_err(map_db)?.ok_or(AppError::Forbidden)?;
        let source: domain::sdlc_source::TaskSourceBinding =
            serde_json::from_value(row.try_get::<Json>("", "source_binding").map_err(map_db)?)
                .map_err(AppError::internal)?;
        if source.task_id != state.task_id
            || source.project_id != state.project_id
            || source.owner_subject != state.owner_subject
        {
            return Err(AppError::Forbidden);
        }
        Self::freeze_delivery_dependencies(tx, &source, assignment, actor, proof, None).await
    }
    async fn freeze_delivery_dependencies(
        tx: &DatabaseTransaction,
        source: &domain::sdlc_source::TaskSourceBinding,
        assignment: Uuid,
        actor: &Principal,
        proof: Option<&OwnerDataAdmission>,
        observed_until: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), AppError> {
        let Some(native) = app::native_space_access::current()? else {
            return Ok(());
        };
        if native.identity().subject != actor.subject || !native.may_write() {
            return Err(AppError::Forbidden);
        }
        let allowed = proof
            .map(OwnerDataAdmission::active_dependency_spaces)
            .transpose()?;
        tx.query_one(statement(
            "SELECT project_id FROM tracker_project_space_sets WHERE project_id=$1 FOR SHARE",
            vec![source.project_id.into()],
        ))
        .await
        .map_err(map_db)?;
        let rows=tx.query_all(statement("SELECT c.registry_instance_id,c.space_id,c.state FROM tracker_project_spaces l JOIN tracker_space_catalog c USING(registry_instance_id,space_id) WHERE l.project_id=$1 ORDER BY c.registry_instance_id,c.space_id FOR SHARE OF c",vec![source.project_id.into()])).await.map_err(map_db)?;
        let linked = !rows.is_empty();
        let mut spaces = Vec::new();
        for row in rows {
            let space = SpaceRef {
                registry_instance_id: row.try_get("", "registry_instance_id").map_err(map_db)?,
                space_id: row.try_get("", "space_id").map_err(map_db)?,
            };
            if row.try_get::<String>("", "state").map_err(map_db)? == "active"
                && allowed
                    .as_ref()
                    .is_none_or(|allowed| allowed.contains(&space))
            {
                spaces.push(space)
            }
        }
        if linked && spaces.is_empty() {
            return Err(AppError::Forbidden);
        }
        spaces.push(SpaceRef {
            registry_instance_id: source.namespace.registry_instance_id,
            space_id: source.namespace.namespace_id,
        });
        spaces.sort_by_key(|s| (s.registry_instance_id, s.space_id));
        spaces.dedup();
        let mut expiry = native.validity()?.valid_until;
        if let Some(until) = observed_until {
            expiry = expiry.min(until.timestamp());
        }
        if let Some(proof) = proof {
            expiry = expiry.min(proof.check(true)?);
        }
        crate::guarded_db::bind_commit_clock(tx, expiry)
            .await
            .map_err(map_db)?;
        exec(tx,"INSERT INTO tracker_execution_space_grants(assignment_id,task_id,project_id,origin,assignment_version,spaces,required_wiki_space,original_actor,initial_valid_until) VALUES($1,$2,$3,'delivery',NULL,$4,NULL,$5,$6)",vec![assignment.into(),source.task_id.into(),source.project_id.into(),json_value(&spaces)?,json!({"issuer":native.identity().issuer,"subject":source.owner_subject}).into(),expiry.into()]).await
    }
    pub(super) async fn bind_owner_data(
        tx: &DatabaseTransaction,
        state: &TaskState,
        proof: Option<&OwnerDataAdmission>,
        source: Option<&NamespaceRef>,
        write: bool,
    ) -> Result<(), AppError> {
        let Some(basis) = Self::owner_data_basis(tx, state, source).await? else {
            return Ok(());
        };
        let proof = proof.ok_or(AppError::Forbidden)?;
        if proof.basis != basis {
            return Err(AppError::conflict("current owner data scope changed"));
        }
        let expiry = proof.check(write)?;
        // Wire export rounds to whole seconds; a local proof retains its exact
        // original monotonic deadline, including its last partial second.
        let wall = chrono::Utc::now();
        let remaining = proof
            .proof
            .monotonic_deadline()
            .checked_duration_since(std::time::Instant::now())
            .ok_or(AppError::Forbidden)?;
        let deadline = wall + chrono::Duration::from_std(remaining).map_err(AppError::internal)?;
        let original = chrono::DateTime::from_timestamp(expiry, 0).ok_or(AppError::Forbidden)?;
        crate::guarded_db::bind_commit_deadline(tx, deadline.min(original))
            .await
            .map_err(map_db)
    }
}
