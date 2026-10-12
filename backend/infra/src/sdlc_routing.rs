use super::*;
use domain::sdlc_routing::*;

fn reader(actor: &Principal) -> Result<(), AppError> {
    if app::sdlc::has_pm_grant(&actor.scopes)
        || (!actor.human_session
            && (actor.scopes.len() != 1 || !actor.scopes.contains("task-tracker:read")))
    {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

fn decode<T: serde::de::DeserializeOwned>(row: &QueryResult) -> Result<T, AppError> {
    serde_json::from_value(row.try_get::<Json>("", "payload").map_err(map_db)?)
        .map_err(|_| AppError::conflict("SDLC routing history inconsistent"))
}

impl PostgresSdlcRepository {
    async fn routing_owner(
        tx: &DatabaseTransaction,
        project: Uuid,
        actor: &Principal,
        lock: bool,
    ) -> Result<(), AppError> {
        if !actor.human_session || app::sdlc::has_pm_grant(&actor.scopes) {
            return Err(AppError::Forbidden);
        }
        let user = tx
            .query_one(statement(
                "SELECT id FROM users WHERE central_sub=$1 AND is_active FOR SHARE",
                vec![actor.subject.clone().into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or(AppError::Forbidden)?;
        // Acquire the exclusive project lock directly, never upgrade a shared lock.
        let sql = if lock {
            "SELECT owner_id FROM projects WHERE id=$1 FOR UPDATE"
        } else {
            "SELECT owner_id FROM projects WHERE id=$1 FOR SHARE"
        };
        let row = tx
            .query_one(statement(sql, vec![project.into()]))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::not_found("project", project))?;
        if row.try_get::<Uuid>("", "owner_id").map_err(map_db)?
            != user.try_get::<Uuid>("", "id").map_err(map_db)?
        {
            return Err(AppError::Forbidden);
        }
        Ok(())
    }

    async fn policy(
        tx: &DatabaseTransaction,
        project: Uuid,
        version: Option<i64>,
    ) -> Result<RoutingPolicy, AppError> {
        let row = tx.query_one(statement(
            "SELECT r.payload FROM sdlc_project_routing_revisions r JOIN sdlc_project_routing_heads h USING(project_id) WHERE r.project_id=$1 AND r.version=COALESCE($2,h.version)",
            vec![project.into(), version.into()],
        )).await.map_err(map_db)?.ok_or_else(|| AppError::not_found("SDLC routing policy", project))?;
        let policy: RoutingPolicy = decode(&row)?;
        app::sdlc_routing::validate_routes(&policy.routes)?;
        if policy.project_id != project
            || policy.native_ready
            || policy.dispatch_allowed
            || policy.routing_hash != app::sdlc::canonical_hash(&policy.routes)?
        {
            return Err(AppError::conflict("SDLC routing policy inconsistent"));
        }
        Ok(policy)
    }

    pub(super) async fn write_routing_policy(
        &self,
        project: Uuid,
        actor: &Principal,
        command: SetRoutingPolicy,
    ) -> Result<(RoutingPolicy, bool), AppError> {
        app::sdlc::validate_key(&command.idempotency_key)?;
        app::sdlc_routing::validate_routes(&command.routes)?;
        if command
            .expected_version
            .is_some_and(|v| !bounded_policy_version(v))
        {
            return Err(AppError::validation("invalid expected routing version"));
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        Self::routing_owner(&tx, project, actor, true).await?;
        let hash = app::sdlc::canonical_hash(&command)?;
        let replay = tx.query_one(statement(
            "SELECT payload_hash,version FROM sdlc_project_routing_operations WHERE project_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![project.into(), actor.subject.clone().into(), command.idempotency_key.clone().into()],
        )).await.map_err(map_db)?;
        if let Some(row) = replay {
            if row.try_get::<String>("", "payload_hash").map_err(map_db)? != hash {
                return Err(AppError::conflict("routing idempotency payload changed"));
            }
            let policy = Self::policy(
                &tx,
                project,
                Some(row.try_get("", "version").map_err(map_db)?),
            )
            .await?;
            tx.commit().await.map_err(map_db)?;
            return Ok((policy, true));
        }
        let current = tx
            .query_one(statement(
                "SELECT version FROM sdlc_project_routing_heads WHERE project_id=$1",
                vec![project.into()],
            ))
            .await
            .map_err(map_db)?
            .map(|r| r.try_get::<i64>("", "version").map_err(map_db))
            .transpose()?;
        if current != command.expected_version {
            return Err(AppError::conflict("stale project routing policy version"));
        }
        let version = current
            .unwrap_or(0)
            .checked_add(1)
            .filter(|v| bounded_policy_version(*v))
            .ok_or_else(|| AppError::conflict("routing version exhausted"))?;
        let policy = RoutingPolicy {
            contract_version: 1,
            tracker_instance_id: self.config.instance_id.clone(),
            project_id: project,
            version,
            routing_hash: app::sdlc::canonical_hash(&command.routes)?,
            routes: command.routes,
            verification: RoutingVerification::Declared,
            native_ready: false,
            dispatch_allowed: false,
            author_subject: actor.subject.clone(),
            created_at: shared::now().into(),
        };
        exec(&tx, "INSERT INTO sdlc_project_routing_revisions(project_id,version,tracker_instance_id,routing_hash,payload) VALUES($1,$2,$3,$4,$5)",
            vec![project.into(), version.into(), self.config.instance_id.clone().into(), policy.routing_hash.clone().into(), json_value(&policy)?]).await?;
        let head_sql = if current.is_some() {
            "UPDATE sdlc_project_routing_heads SET version=$2 WHERE project_id=$1"
        } else {
            "INSERT INTO sdlc_project_routing_heads(project_id,version) VALUES($1,$2)"
        };
        exec(&tx, head_sql, vec![project.into(), version.into()]).await?;
        exec(&tx, "INSERT INTO sdlc_project_routing_operations(project_id,actor_subject,idempotency_key,payload_hash,version) VALUES($1,$2,$3,$4,$5)",
            vec![project.into(), actor.subject.clone().into(), command.idempotency_key.into(), hash.into(), version.into()]).await?;
        tx.commit().await.map_err(map_db)?;
        Ok((policy, false))
    }

    pub(super) async fn read_routing_policy(
        &self,
        project: Uuid,
        actor: &Principal,
        version: Option<i64>,
    ) -> Result<RoutingPolicy, AppError> {
        reader(actor)?;
        if version.is_some_and(|v| !bounded_policy_version(v)) {
            return Err(AppError::validation("invalid routing version"));
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        self.project_access(&tx, project, actor).await?;
        let result = Self::policy(&tx, project, version).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }

    pub(super) async fn read_routing_operation(
        &self,
        project: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<RoutingPolicy, AppError> {
        app::sdlc::validate_key(key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        Self::routing_owner(&tx, project, actor, false).await?;
        let row = tx.query_one(statement("SELECT version FROM sdlc_project_routing_operations WHERE project_id=$1 AND actor_subject=$2 AND idempotency_key=$3",
            vec![project.into(), actor.subject.clone().into(), key.into()])).await.map_err(map_db)?
            .ok_or_else(|| AppError::not_found("routing operation", key))?;
        let result = Self::policy(
            &tx,
            project,
            Some(row.try_get("", "version").map_err(map_db)?),
        )
        .await?;
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }

    pub(super) async fn freeze_routing(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        confirmation: &Confirmation,
        expected: i64,
    ) -> Result<TaskRoutingSnapshot, AppError> {
        let policy = Self::policy(tx, state.project_id, None).await?;
        if policy.version != expected || policy.tracker_instance_id != state.tracker_instance_id {
            return Err(AppError::conflict(
                "stale publication routing policy version",
            ));
        }
        let snapshot = TaskRoutingSnapshot {
            contract_version: 1,
            snapshot_id: Uuid::new_v4(),
            tracker_instance_id: state.tracker_instance_id.clone(),
            project_id: state.project_id,
            task_id: state.task_id,
            root_task_id: state.root_task_id,
            confirmation_id: confirmation.id,
            requirement_revision: confirmation.revision,
            content_hash: confirmation.content_hash.clone(),
            policy,
            created_at: confirmation.created_at,
        };
        exec(tx, "INSERT INTO sdlc_task_routing_snapshots(snapshot_id,task_id,project_id,policy_version,confirmation_id,requirement_revision,content_hash,payload) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
            vec![snapshot.snapshot_id.into(), state.task_id.into(), state.project_id.into(), expected.into(), confirmation.id.into(), confirmation.revision.into(), confirmation.content_hash.clone().into(), json_value(&snapshot)?]).await?;
        Ok(snapshot)
    }

    pub(super) async fn read_task_routing(
        &self,
        task: Uuid,
        actor: &Principal,
    ) -> Result<TaskRoutingSnapshot, AppError> {
        reader(actor)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        let row = tx
            .query_one(statement(
                "SELECT payload FROM sdlc_task_routing_snapshots WHERE task_id=$1",
                vec![task.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::not_found("task routing snapshot", task))?;
        let snapshot: TaskRoutingSnapshot = decode(&row)?;
        let retained = Self::policy(&tx, state.project_id, Some(snapshot.policy.version)).await?;
        if snapshot.task_id != task
            || snapshot.root_task_id != state.root_task_id
            || snapshot.tracker_instance_id != state.tracker_instance_id
            || snapshot.project_id != state.project_id
            || snapshot.policy != retained
            || !state.confirmations.iter().any(|c| {
                c.id == snapshot.confirmation_id
                    && c.revision == snapshot.requirement_revision
                    && c.content_hash == snapshot.content_hash
            })
        {
            return Err(AppError::conflict("task routing snapshot inconsistent"));
        }
        tx.commit().await.map_err(map_db)?;
        Ok(snapshot)
    }
}
