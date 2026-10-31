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
        if self.proof.is_current() {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
    fn check(&self, write: bool) -> Result<i64, AppError> {
        let admission = self
            .proof
            .bounded_admission()
            .map_err(|_| AppError::Forbidden)?;
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
        crate::guarded_db::bind_commit_clock(tx, proof.check(write)?)
            .await
            .map_err(map_db)
    }
}
