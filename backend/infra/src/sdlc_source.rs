use super::*;

/// Synthetic observation for disposable PostgreSQL tests; never native Forge evidence.
#[cfg(test)]
pub(super) async fn configure_test_source(
    repo: &PostgresSdlcRepository,
    project: Uuid,
    owner: &Principal,
) {
    let namespace = NamespaceRef {
        registry_instance_id: Uuid::new_v4(),
        namespace_id: Uuid::new_v4(),
    };
    let binding = json!({"schema_version":1,"namespace":namespace,
        "resource":{"kind":"tracker_project","instance_id":Uuid::new_v4(),"resource_id":project},
        "operation_id":Uuid::new_v4(),"generation":1,"state":"active"});
    exec(&repo.db,"INSERT INTO tracker_namespace_bindings(resource_id,registry_instance_id,namespace_id,generation,state,command)
        VALUES($1,$2,$3,1,'active',$4)",vec![project.into(),namespace.registry_instance_id.into(),namespace.namespace_id.into(),binding.into()]).await.unwrap();
    let source = SourcePolicySpec {
        forge_instance_id: Uuid::new_v4(),
        repository_id: Uuid::new_v4(),
        base_ref: "refs/heads/main".into(),
        base_commit: "a".repeat(40),
    };
    let verified = VerifiedSource {
        namespace,
        forge_instance_id: source.forge_instance_id,
        repository_id: source.repository_id,
        public_name: "fixture/source".into(),
        base_ref: source.base_ref.clone(),
        base_commit: source.base_commit.clone(),
        default_ref: "refs/heads/main".into(),
        protected_refs: vec!["refs/heads/release".into()],
        observed_at: chrono::Utc::now(),
    };
    repo.set_source_policy(
        project,
        owner,
        SetSourcePolicy {
            expected_source_revision: None,
            source,
            idempotency_key: "fixture-source-policy".into(),
        },
        verified,
    )
    .await
    .unwrap();
}
use domain::sdlc_source::*;
use shared::resource_context::NamespaceRef;

#[path = "sdlc_draft_source_enrollment.rs"]
mod enrollment;
#[cfg(test)]
#[path = "sdlc_source_pg_tests.rs"]
mod tests;

fn decode<T: serde::de::DeserializeOwned>(row: &QueryResult) -> Result<T, AppError> {
    serde_json::from_value(row.try_get::<Json>("", "payload").map_err(map_db)?)
        .map_err(|_| AppError::conflict("source history inconsistent"))
}

impl PostgresSdlcRepository {
    pub(super) async fn select_task_source(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        creator: Uuid,
    ) -> Result<(), AppError> {
        // Called only for a newly inserted single-task Draft, never generic load/replay.
        let head = tx
            .query_one(statement(
                "SELECT source_revision FROM sdlc_source_policy_heads WHERE project_id=$1",
                vec![state.project_id.into()],
            ))
            .await
            .map_err(map_db)?;
        let revision = head
            .as_ref()
            .map(|r| r.try_get::<i64>("", "source_revision").map_err(map_db))
            .transpose()?;
        let policy = match revision {
            Some(revision) => {
                Some(Self::retained_source_policy(tx, state.project_id, Some(revision)).await?)
            }
            None => None,
        };
        exec(tx,"INSERT INTO sdlc_task_source_selections(task_id,project_id,source_revision,policy_hash) VALUES($1,$2,$3,$4)",
            vec![state.task_id.into(),state.project_id.into(),revision.into(),policy.as_ref().map(|p|p.source_hash.clone()).into()]).await?;
        if let Some(policy) = policy {
            if Self::source_namespace(tx, state.project_id).await? != policy.verification.namespace
            {
                return Err(AppError::conflict("source policy namespace changed"));
            }
            let link = domain::task_repositories::TaskRepositoryLink {
                namespace: policy.verification.namespace,
                forge_instance_id: policy.source.forge_instance_id,
                repository_id: policy.source.repository_id,
                public_name: policy.verification.public_name,
                kind: "hosted".into(),
            };
            exec(tx,"INSERT INTO task_repository_links(issue_id,forge_instance_id,repository_id,snapshot,created_by) VALUES($1,$2,$3,$4,$5)",
                vec![state.task_id.into(),link.forge_instance_id.into(),link.repository_id.into(),json_value(&link)?,creator.into()]).await?;
        }
        Ok(())
    }

    async fn source_namespace(
        tx: &DatabaseTransaction,
        project: Uuid,
    ) -> Result<NamespaceRef, AppError> {
        let row = tx.query_one(statement(
            "SELECT registry_instance_id,namespace_id FROM tracker_namespace_bindings WHERE resource_id=$1 AND state='active' FOR SHARE",
            vec![project.into()],
        )).await.map_err(map_db)?.ok_or_else(|| AppError::conflict("active project namespace required for source binding"))?;
        let namespace = NamespaceRef {
            registry_instance_id: row.try_get("", "registry_instance_id").map_err(map_db)?,
            namespace_id: row.try_get("", "namespace_id").map_err(map_db)?,
        };
        super::spaces::require_resource_namespace(&namespace)?;
        Ok(namespace)
    }

    async fn retained_source_policy(
        tx: &DatabaseTransaction,
        project: Uuid,
        revision: Option<i64>,
    ) -> Result<ProjectSourcePolicy, AppError> {
        let row = tx.query_one(statement(
            "SELECT r.payload FROM sdlc_source_policy_revisions r JOIN sdlc_source_policy_heads h USING(project_id)
             WHERE r.project_id=$1 AND r.source_revision=COALESCE($2,h.source_revision)",
            vec![project.into(),revision.into()],
        )).await.map_err(map_db)?.ok_or_else(|| AppError::not_found("source policy", project))?;
        let policy: ProjectSourcePolicy = decode(&row)?;
        app::sdlc_source::validate_policy(&policy)?;
        super::spaces::require_resource_namespace(&policy.verification.namespace)?;
        if policy.project_id != project || revision.is_some_and(|r| r != policy.source_revision) {
            return Err(AppError::conflict("source policy identity differs"));
        }
        Ok(policy)
    }

    pub(super) async fn freeze_source(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        confirmation: &Confirmation,
        routing: &domain::sdlc_routing::TaskRoutingSnapshot,
    ) -> Result<(), AppError> {
        let Some(selection) = tx.query_one(statement(
            "SELECT source_revision,policy_hash FROM sdlc_effective_task_source_selections WHERE task_id=$1 AND project_id=$2",
            vec![state.task_id.into(),state.project_id.into()],
        )).await.map_err(map_db)? else {
            // Preserve the original Draft for explicit safe enrollment, not a new task/rebind.
            return Err(AppError::Unavailable(
                "sdlc_source_binding_missing: explicit owner Draft enrollment required".into(),
            ));
        };
        let revision = selection
            .try_get::<Option<i64>>("", "source_revision")
            .map_err(map_db)?
            .ok_or_else(|| {
                AppError::Unavailable(
                    "sdlc_source_project_policy_missing: operator configuration/Draft enrollment required"
                        .into(),
                )
            })?;
        let policy = Self::retained_source_policy(tx, state.project_id, Some(revision)).await?;
        if selection
            .try_get::<Option<String>>("", "policy_hash")
            .map_err(map_db)?
            .as_deref()
            != Some(policy.source_hash.as_str())
        {
            return Err(AppError::conflict("task source selection differs"));
        }
        if Self::source_namespace(tx, state.project_id).await? != policy.verification.namespace {
            return Err(AppError::conflict("source policy namespace changed"));
        }
        let binding = app::sdlc_source::freeze_binding(state, confirmation, routing, &policy)?;
        let link = tx.query_one(statement(
            "SELECT snapshot AS payload FROM task_repository_links WHERE issue_id=$1 AND forge_instance_id=$2 AND repository_id=$3 FOR SHARE",
            vec![state.task_id.into(),binding.forge_instance_id.into(),binding.repository_id.into()],
        )).await.map_err(map_db)?.ok_or_else(|| AppError::conflict("exact task repository link required"))?;
        let link: domain::task_repositories::TaskRepositoryLink = decode(&link)?;
        if link.namespace != binding.namespace
            || link.forge_instance_id != binding.forge_instance_id
            || link.repository_id != binding.repository_id
            || link.kind != "hosted"
        {
            return Err(AppError::conflict("task source repository link differs"));
        }
        exec(tx,"INSERT INTO sdlc_source_ref_owners(project_id,forge_instance_id,repository_id,candidate_ref,task_id)
            VALUES($1,$2,$3,$4,$5) ON CONFLICT(forge_instance_id,repository_id,candidate_ref) DO NOTHING",
            vec![state.project_id.into(),binding.forge_instance_id.into(),binding.repository_id.into(),binding.candidate_ref.clone().into(),state.task_id.into()]).await?;
        let owner = tx.query_one(statement(
            "SELECT task_id,project_id FROM sdlc_source_ref_owners WHERE forge_instance_id=$1 AND repository_id=$2 AND candidate_ref=$3 FOR SHARE",
            vec![binding.forge_instance_id.into(),binding.repository_id.into(),binding.candidate_ref.clone().into()],
        )).await.map_err(map_db)?.ok_or_else(|| AppError::conflict("source ref reservation missing"))?;
        if owner.try_get::<Uuid>("", "task_id").map_err(map_db)? != state.task_id
            || owner.try_get::<Uuid>("", "project_id").map_err(map_db)? != state.project_id
        {
            return Err(AppError::conflict(
                "candidate ref belongs to another root task",
            ));
        }
        exec(tx,"INSERT INTO sdlc_task_source_bindings(task_id,project_id,confirmation_id,requirement_revision,content_hash,routing_snapshot_id,
            forge_instance_id,repository_id,candidate_ref,source_revision,policy_hash,source_hash,payload)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
            vec![state.task_id.into(),state.project_id.into(),confirmation.id.into(),confirmation.revision.into(),confirmation.content_hash.clone().into(),routing.snapshot_id.into(),
                binding.forge_instance_id.into(),binding.repository_id.into(),binding.candidate_ref.clone().into(),binding.source_revision.into(),binding.policy_hash.clone().into(),binding.source_hash.clone().into(),json_value(&binding)?]).await?;
        Ok(())
    }
}

#[async_trait]
impl TaskSourceRepository for PostgresSdlcRepository {
    async fn draft_source_preflight(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<DraftSourceEnrollmentPreflight, AppError> {
        self.prepare_draft_source_enrollment(task, actor).await
    }
    async fn enroll_draft_source(
        &self,
        task: Uuid,
        actor: &Principal,
        command: EnrollDraftSource,
    ) -> Result<(DraftSourceEnrollmentReceipt, bool), AppError> {
        self.write_draft_source_enrollment(task, actor, command)
            .await
    }
    async fn source_policy_namespace(
        &self,
        project: Uuid,
        actor: &Principal,
    ) -> Result<NamespaceRef, AppError> {
        app::sdlc_source::human_owner(actor)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        Self::routing_owner(&tx, project, actor, false).await?;
        let namespace = Self::source_namespace(&tx, project).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(namespace)
    }

    async fn set_source_policy(
        &self,
        project: Uuid,
        actor: &Principal,
        command: SetSourcePolicy,
        verified: VerifiedSource,
    ) -> Result<(ProjectSourcePolicy, bool), AppError> {
        app::sdlc_source::human_owner(actor)?;
        app::sdlc_source::validate_spec(&command.source)?;
        app::sdlc::validate_key(&command.idempotency_key)?;
        if command
            .expected_source_revision
            .is_some_and(|v| !(1..=MAX_SAFE_VERSION).contains(&v))
        {
            return Err(AppError::validation("invalid source CAS revision"));
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        Self::routing_owner(&tx, project, actor, true).await?;
        let hash = app::sdlc::canonical_hash(&command)?;
        if let Some(row) = tx.query_one(statement(
            "SELECT payload_hash,source_revision FROM sdlc_source_policy_operations WHERE project_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![project.into(),actor.subject.clone().into(),command.idempotency_key.clone().into()],
        )).await.map_err(map_db)? {
            if row.try_get::<String>("", "payload_hash").map_err(map_db)? != hash {
                return Err(AppError::conflict("source operation payload changed"));
            }
            let policy = Self::retained_source_policy(&tx, project, Some(row.try_get("", "source_revision").map_err(map_db)?)).await?;
            tx.commit().await.map_err(map_db)?;
            return Ok((policy, true));
        }
        let namespace = Self::source_namespace(&tx, project).await?;
        let now = super::lifecycle::now(&tx).await?;
        app::sdlc_source::validate_verification(&command.source, &namespace, &verified, now)?;
        let current = tx
            .query_one(statement(
                "SELECT source_revision FROM sdlc_source_policy_heads WHERE project_id=$1",
                vec![project.into()],
            ))
            .await
            .map_err(map_db)?
            .map(|r| r.try_get::<i64>("", "source_revision").map_err(map_db))
            .transpose()?;
        if current != command.expected_source_revision {
            return Err(AppError::conflict("stale project source policy revision"));
        }
        let revision = current
            .unwrap_or(0)
            .checked_add(1)
            .filter(|r| *r <= MAX_SAFE_VERSION)
            .ok_or_else(|| AppError::conflict("source policy revision exhausted"))?;
        let mut policy = ProjectSourcePolicy {
            contract_version: 1,
            tracker_instance_id: self.config.instance_id.clone(),
            project_id: project,
            source_revision: revision,
            source_hash: String::new(),
            source: command.source,
            verification: verified,
            author_subject: actor.subject.clone(),
            created_at: now,
        };
        policy.source_hash = app::sdlc_source::sealed_hash(&policy)?;
        exec(&tx,"INSERT INTO sdlc_source_policy_revisions(project_id,source_revision,source_hash,payload) VALUES($1,$2,$3,$4)",
            vec![project.into(),revision.into(),policy.source_hash.clone().into(),json_value(&policy)?]).await?;
        exec(
            &tx,
            if current.is_none() {
                "INSERT INTO sdlc_source_policy_heads(project_id,source_revision) VALUES($1,$2)"
            } else {
                "UPDATE sdlc_source_policy_heads SET source_revision=$2 WHERE project_id=$1"
            },
            vec![project.into(), revision.into()],
        )
        .await?;
        exec(&tx,"INSERT INTO sdlc_source_policy_operations(project_id,actor_subject,idempotency_key,payload_hash,source_revision) VALUES($1,$2,$3,$4,$5)",
            vec![project.into(),actor.subject.clone().into(),command.idempotency_key.into(),hash.into(),revision.into()]).await?;
        tx.commit().await.map_err(map_db)?;
        Ok((policy, false))
    }

    async fn source_policy(
        &self,
        project: Uuid,
        actor: &Principal,
        revision: Option<i64>,
    ) -> Result<ProjectSourcePolicy, AppError> {
        app::sdlc_reservation::reader(actor)?;
        if revision.is_some_and(|v| !(1..=MAX_SAFE_VERSION).contains(&v)) {
            return Err(AppError::validation("invalid source revision"));
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        Self::project_access(&tx, project, actor).await?;
        let result = Self::retained_source_policy(&tx, project, revision).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }

    async fn source_policy_operation(
        &self,
        project: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<ProjectSourcePolicy, AppError> {
        app::sdlc_source::human_owner(actor)?;
        app::sdlc::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        Self::routing_owner(&tx, project, actor, false).await?;
        let row = tx.query_one(statement(
            "SELECT source_revision FROM sdlc_source_policy_operations WHERE project_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![project.into(),actor.subject.clone().into(),key.into()],
        )).await.map_err(map_db)?.ok_or_else(|| AppError::not_found("source operation", key))?;
        let result = Self::retained_source_policy(
            &tx,
            project,
            Some(row.try_get("", "source_revision").map_err(map_db)?),
        )
        .await?;
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }

    async fn task_source(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<TaskSourceReadback, AppError> {
        if app::sdlc_lifecycle::has_delivery_grant(&actor.scopes) {
            app::sdlc_lifecycle::producer(actor, false)?;
        } else if actor.scopes.contains("task-tracker:write") && !actor.trusted_human {
            // load_read below verifies current configured assignment and project
            // ACL. Ordinary agent credentials do not carry a native grant.
            app::sdlc_configured_cli::authorize_configured_identity(actor)?;
        } else {
            app::sdlc_reservation::reader(actor)?;
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load_read(&tx, task, actor).await?;
        let current = super::lifecycle::current(&tx, task).await?;
        if let Some(current) = &current {
            if self.review_snapshot(&tx, &state, current).await?.is_none() {
                super::lifecycle::exact_revision(&state, current)?;
            }
        }
        let confirmation = current
            .as_ref()
            .map(|c| c.task.routing.confirmation_id)
            .or_else(|| state.confirmations.last().map(|c| c.id));
        let binding: Option<TaskSourceBinding> = tx.query_one(statement(
            "SELECT payload FROM sdlc_task_source_bindings WHERE task_id=$1 AND confirmation_id=$2",
            vec![task.into(),confirmation.into()],
        )).await.map_err(map_db)?.as_ref().map(decode).transpose()?;
        if let Some(binding) = &binding {
            if binding.task_id != task
                || binding.project_id != state.project_id
                || binding.owner_subject != state.owner_subject
                || binding.tracker_instance_id != self.config.instance_id
            {
                return Err(AppError::conflict("task source identity inconsistent"));
            }
        }
        let observed_at = super::lifecycle::now(&tx).await?;
        let current = current.map(|receipt| domain::sdlc_lifecycle::LifecycleReadback {
            observed_at,
            runtime_state: if receipt
                .lease
                .as_ref()
                .is_some_and(|l| l.expires_at <= observed_at)
            {
                domain::sdlc_lifecycle::RuntimeState::Unknown
            } else {
                receipt.task.runtime
            },
            receipt,
        });
        let missing_policy = tx.query_one(statement(
            "SELECT source_revision IS NULL AS missing FROM sdlc_effective_task_source_selections WHERE task_id=$1",
            vec![task.into()],
        )).await.map_err(map_db)?.map(|r|r.try_get::<bool>("", "missing").map_err(map_db)).transpose()?.unwrap_or(false);
        let result = if missing_policy && binding.is_none() {
            TaskSourceReadback::Blocked {
                reason: SourceBlockReason::MissingProjectSourcePolicy,
                binding: None,
                current,
            }
        } else {
            app::sdlc_source::readback(binding, current)?
        };
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }
}
