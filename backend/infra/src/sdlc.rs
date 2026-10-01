use async_trait::async_trait;
use domain::sdlc::*;
use sea_orm::{
    ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    QueryResult, Statement, TransactionTrait, Value,
};
use serde_json::{Value as Json, json};
use shared::AppError;
use uuid::Uuid;

pub struct PostgresSdlcRepository {
    db: DatabaseConnection,
    config: SdlcConfig,
}

fn statement(sql: &str, values: Vec<Value>) -> Statement {
    Statement::from_sql_and_values(DatabaseBackend::Postgres, sql, values)
}
async fn exec(db: &impl ConnectionTrait, sql: &str, values: Vec<Value>) -> Result<(), AppError> {
    db.execute(statement(sql, values)).await.map_err(map_db)?;
    Ok(())
}
pub(crate) fn map_db(error: sea_orm::DbErr) -> AppError {
    if error.to_string().contains("SDLC") {
        AppError::conflict("SDLC confirmation/binding gate rejected operation")
    } else {
        AppError::database(error)
    }
}
fn json_value<T: serde::Serialize>(value: &T) -> Result<Value, AppError> {
    Ok(serde_json::to_value(value)
        .map_err(AppError::internal)?
        .into())
}
fn state_from(row: QueryResult) -> Result<TaskState, AppError> {
    serde_json::from_value(row.try_get::<Json>("", "state").map_err(map_db)?)
        .map_err(AppError::internal)
}

impl PostgresSdlcRepository {
    pub async fn connect(url: &str, config: SdlcConfig) -> Result<Self, AppError> {
        if config.instance_id.trim().is_empty() || config.instance_id.len() > 128 {
            return Err(AppError::validation("stable SDLC instance ID is required"));
        }
        let db = Database::connect(url).await.map_err(map_db)?;
        exec(&db, "INSERT INTO sdlc_instance(singleton, instance_id) VALUES(true,$1) ON CONFLICT(singleton) DO NOTHING", vec![config.instance_id.clone().into()]).await?;
        let row = db
            .query_one(statement(
                "SELECT instance_id FROM sdlc_instance WHERE singleton",
                vec![],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::internal("SDLC instance missing"))?;
        if row.try_get::<String>("", "instance_id").map_err(map_db)? != config.instance_id {
            return Err(AppError::conflict(
                "configured SDLC instance differs from persisted identity",
            ));
        }
        Ok(Self { db, config })
    }

    async fn issue_access(
        &self,
        tx: &DatabaseTransaction,
        task: Uuid,
        actor: &Principal,
    ) -> Result<(Uuid, String), AppError> {
        // Issue is locked before aggregate, also fencing normal Tracker update/transition paths.
        let row = tx.query_one(statement(
            "SELECT i.project_id, u.central_sub AS owner_subject FROM issues i JOIN users u ON u.id=i.reporter_id WHERE i.id=$1 AND i.deleted_at IS NULL FOR UPDATE OF i",
            vec![task.into()])).await.map_err(map_db)?.ok_or_else(|| AppError::not_found("issue", task))?;
        let project: Uuid = row.try_get("", "project_id").map_err(map_db)?;
        // Lock authorization rows so membership/account revocation cannot race the commit.
        let user = tx
            .query_one(statement(
                "SELECT id FROM users WHERE central_sub=$1 AND is_active=true FOR SHARE",
                vec![actor.subject.clone().into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or(AppError::Forbidden)?;
        let user_id: Uuid = user.try_get("", "id").map_err(map_db)?;
        let project_row = tx
            .query_one(statement(
                "SELECT owner_id FROM projects WHERE id=$1 FOR SHARE",
                vec![project.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or(AppError::Forbidden)?;
        if project_row
            .try_get::<Uuid>("", "owner_id")
            .map_err(map_db)?
            != user_id
        {
            tx.query_one(statement(
                "SELECT user_id FROM project_members WHERE project_id=$1 AND user_id=$2 FOR SHARE",
                vec![project.into(), user_id.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or(AppError::Forbidden)?;
        }
        let owner: Option<String> = row.try_get("", "owner_subject").map_err(map_db)?;
        Ok((project, owner.unwrap_or_default()))
    }

    async fn load(
        &self,
        tx: &DatabaseTransaction,
        task: Uuid,
        actor: &Principal,
    ) -> Result<TaskState, AppError> {
        let (project, _) = self.issue_access(tx, task, actor).await?;
        let row = tx
            .query_one(statement(
                "SELECT state FROM sdlc_tasks WHERE task_id=$1 FOR UPDATE",
                vec![task.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::not_found("SDLC binding", task))?;
        let state = state_from(row)?;
        if state.tracker_instance_id != self.config.instance_id || state.project_id != project {
            return Err(AppError::conflict("SDLC binding mismatch"));
        }
        Ok(state)
    }

    async fn replay(
        tx: &DatabaseTransaction,
        task: Uuid,
        actor: &Principal,
        key: &str,
        hash: &str,
    ) -> Result<Option<Json>, AppError> {
        let row = tx.query_one(statement("SELECT payload_hash,result FROM sdlc_idempotency WHERE task_id=$1 AND actor_subject=$2 AND idempotency_key=$3", vec![task.into(), actor.subject.clone().into(), key.into()])).await.map_err(map_db)?;
        if let Some(row) = row {
            if row.try_get::<String>("", "payload_hash").map_err(map_db)? != hash {
                return Err(AppError::conflict(
                    "idempotency key reused with changed payload",
                ));
            }
            return Ok(Some(row.try_get("", "result").map_err(map_db)?));
        }
        Ok(None)
    }

    #[allow(clippy::too_many_arguments)]
    async fn finish(
        tx: &DatabaseTransaction,
        task: Uuid,
        actor: &Principal,
        key: &str,
        hash: &str,
        result: &Json,
        event_type: &str,
        state: &TaskState,
    ) -> Result<(), AppError> {
        exec(
            tx,
            "UPDATE sdlc_tasks SET state=$2 WHERE task_id=$1",
            vec![task.into(), json_value(state)?],
        )
        .await?;
        let status_name = match state.stage {
            Stage::Draft => "SDLC Draft",
            Stage::Clarification => "SDLC Clarification",
            Stage::Backlog => "Backlog",
        };
        if !matches!(state.stage, Stage::Backlog) || event_type == "requirements.confirmed" {
            exec(tx, "INSERT INTO issue_status_history(id,issue_id,from_status_id,to_status_id,changed_by_id,created_at) SELECT $1,i.id,i.status_id,s.id,u.id,now() FROM issues i JOIN statuses s ON s.id=(SELECT id FROM statuses WHERE name=$3 ORDER BY id LIMIT 1) JOIN users u ON u.central_sub=$4 WHERE i.id=$2 AND i.status_id<>s.id", vec![Uuid::new_v4().into(), task.into(), status_name.into(), actor.subject.clone().into()]).await?;
            exec(tx, "UPDATE issues SET status_id=(SELECT id FROM statuses WHERE name=$2 ORDER BY id LIMIT 1), sprint_id=NULL, updated_at=now() WHERE id=$1", vec![task.into(), status_name.into()]).await?;
        }
        exec(tx, "INSERT INTO sdlc_idempotency(task_id,actor_subject,idempotency_key,payload_hash,result) VALUES($1,$2,$3,$4,$5)", vec![task.into(), actor.subject.clone().into(), key.into(), hash.into(), result.clone().into()]).await?;
        let payload = json!({"contract_version":1,"tracker_instance_id":state.tracker_instance_id,"project_id":state.project_id,"task_id":task,"root_task_id":state.root_task_id,"owner_subject":state.owner_subject,"stage":state.stage,"requirement_revision":state.current_revision(),"result":result});
        exec(
            tx,
            "INSERT INTO sdlc_outbox(event_id,task_id,event_type,payload) VALUES($1,$2,$3,$4)",
            vec![
                Uuid::new_v4().into(),
                task.into(),
                event_type.into(),
                payload.into(),
            ],
        )
        .await
    }

    async fn persist_result(
        tx: &DatabaseTransaction,
        task: Uuid,
        command: &SdlcCommand,
        result: &Json,
        state: &TaskState,
    ) -> Result<&'static str, AppError> {
        let payload: Value = result.clone().into();
        let event = match command {
            SdlcCommand::Assign(c) => {
                exec(
                    tx,
                    "INSERT INTO sdlc_assignments(task_id,version,payload) VALUES($1,$2,$3)",
                    vec![task.into(), c.assignment.version.into(), payload],
                )
                .await?;
                exec(tx, "INSERT INTO sdlc_agent_bindings(tracker_instance_id,task_id,agent_id,binding_id) VALUES($1,$2,$3,$4) ON CONFLICT(tracker_instance_id,task_id,agent_id) DO NOTHING", vec![state.tracker_instance_id.clone().into(), task.into(), c.assignment.agent_id.into(), Uuid::new_v4().into()]).await?;
                "pm.assigned"
            }
            SdlcCommand::PublishQuestion(c) => {
                exec(tx, "INSERT INTO sdlc_requests(request_id,task_id) VALUES($1,$2) ON CONFLICT(request_id) DO NOTHING", vec![c.request_id.into(), task.into()]).await?;
                let q: Question =
                    serde_json::from_value(result.clone()).map_err(AppError::internal)?;
                exec(tx, "INSERT INTO sdlc_question_versions(task_id,question_id,version,request_id,payload) VALUES($1,$2,$3,$4,$5)", vec![task.into(), q.id.into(), q.version.into(), q.request_id.into(), payload]).await?;
                for option in &q.options {
                    let previous = tx.query_one(statement(
                        "SELECT payload FROM sdlc_options WHERE task_id=$1 AND question_id=$2 AND option_id=$3 ORDER BY question_version LIMIT 1",
                        vec![task.into(), q.id.into(), option.id.into()],
                    )).await.map_err(map_db)?;
                    if let Some(previous) = previous
                        && previous.try_get::<Json>("", "payload").map_err(map_db)?
                            != serde_json::to_value(option).map_err(AppError::internal)?
                    {
                        return Err(AppError::validation(
                            "changed historical option meaning requires a new stable ID",
                        ));
                    }
                    exec(tx, "INSERT INTO sdlc_options(task_id,question_id,question_version,option_id,payload) VALUES($1,$2,$3,$4,$5)", vec![task.into(), q.id.into(), q.version.into(), option.id.into(), json_value(option)?]).await?;
                }
                "clarification.published"
            }
            SdlcCommand::PublishRevision(_) => {
                let r = state
                    .revisions
                    .last()
                    .ok_or_else(|| AppError::internal("revision missing"))?;
                exec(tx, "INSERT INTO sdlc_requirements(task_id,revision,content_hash,payload) VALUES($1,$2,$3,$4)", vec![task.into(), r.revision.into(), r.content_hash.clone().into(), payload]).await?;
                "requirements.published"
            }
            SdlcCommand::Answer {
                question_id,
                command: c,
            } => {
                let answer: Answer =
                    serde_json::from_value(result.clone()).map_err(AppError::internal)?;
                exec(tx, "INSERT INTO sdlc_answers(id,task_id,question_id,question_version,requirement_revision,payload) VALUES($1,$2,$3,$4,$5,$6)", vec![answer.id.into(), task.into(), (*question_id).into(), c.expected_question_version.into(), c.requirement_revision.into(), payload]).await?;
                "clarification.answered"
            }
            SdlcCommand::Confirm {
                revision,
                command: c,
            } => {
                let confirmation: Confirmation =
                    serde_json::from_value(result.clone()).map_err(AppError::internal)?;
                exec(tx, "INSERT INTO sdlc_confirmations(id,task_id,revision,content_hash,payload) VALUES($1,$2,$3,$4,$5)", vec![confirmation.id.into(), task.into(), (*revision).into(), c.content_hash.clone().into(), payload]).await?;
                "requirements.confirmed"
            }
            SdlcCommand::Evidence(c) => {
                exec(tx, "INSERT INTO sdlc_evidence(id,task_id,revision,content_hash,check_id,payload) VALUES($1,$2,$3,$4,$5,$6)", vec![Uuid::new_v4().into(), task.into(), c.requirement_revision.into(), c.content_hash.clone().into(), c.check_id.clone().into(), payload]).await?;
                "requirements.evidence_recorded"
            }
            SdlcCommand::Cancel { .. } => "clarification.cancelled",
        };
        Ok(event)
    }
}

#[async_trait]
impl SdlcRepository for PostgresSdlcRepository {
    async fn read(&self, task: Uuid, actor: &Principal) -> Result<TaskState, AppError> {
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        tx.commit().await.map_err(map_db)?;
        Ok(state)
    }
    async fn bind(
        &self,
        task: Uuid,
        actor: &Principal,
        command: BindCommand,
    ) -> Result<TaskState, AppError> {
        app::sdlc::validate_key(&command.idempotency_key)?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let (project, owner) = self.issue_access(&tx, task, actor).await?;
        if !actor.human_session || owner.is_empty() || actor.subject != owner {
            return Err(AppError::Forbidden);
        }
        let root = tx
            .query_one(statement(
                "SELECT project_id FROM issues WHERE id=$1 AND deleted_at IS NULL",
                vec![command.root_task_id.into()],
            ))
            .await
            .map_err(map_db)?
            .ok_or_else(|| AppError::not_found("root task", command.root_task_id))?;
        if root.try_get::<Uuid>("", "project_id").map_err(map_db)? != project {
            return Err(AppError::validation("root must belong to the same project"));
        }
        let hash = app::sdlc::canonical_hash(&json!({"operation":"bind","payload":command}))?;
        if let Some(result) =
            Self::replay(&tx, task, actor, &command.idempotency_key, &hash).await?
        {
            tx.commit().await.map_err(map_db)?;
            return serde_json::from_value(result).map_err(AppError::internal);
        }
        if tx
            .query_one(statement(
                "SELECT task_id FROM sdlc_tasks WHERE task_id=$1",
                vec![task.into()],
            ))
            .await
            .map_err(map_db)?
            .is_some()
        {
            return Err(AppError::conflict("task already bound"));
        }
        let state = TaskState {
            tracker_instance_id: self.config.instance_id.clone(),
            project_id: project,
            task_id: task,
            root_task_id: command.root_task_id,
            owner_subject: owner,
            stage: Stage::Draft,
            confirmation_revision: None,
            assignment: None,
            questions: vec![],
            revisions: vec![],
            confirmations: vec![],
            evidence: vec![],
        };
        exec(&tx, "INSERT INTO sdlc_tasks(task_id,tracker_instance_id,project_id,root_task_id,owner_subject,state) VALUES($1,$2,$3,$4,$5,$6)", vec![task.into(), self.config.instance_id.clone().into(), project.into(), state.root_task_id.into(), state.owner_subject.clone().into(), json_value(&state)?]).await?;
        Self::finish(
            &tx,
            task,
            actor,
            &command.idempotency_key,
            &hash,
            &serde_json::to_value(&state).map_err(AppError::internal)?,
            "task.bound",
            &state,
        )
        .await?;
        tx.commit().await.map_err(map_db)?;
        Ok(state)
    }
    async fn execute(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SdlcCommand,
    ) -> Result<Json, AppError> {
        app::sdlc::validate_key(command.key())?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let mut state = self.load(&tx, task, actor).await?;
        app::sdlc::authorize(&state, actor, &self.config, &command)?;
        let hash = app::sdlc::command_hash(&command)?;
        if let Some(result) = Self::replay(&tx, task, actor, command.key(), &hash).await? {
            tx.commit().await.map_err(map_db)?;
            return Ok(result);
        }
        let result = app::sdlc::apply(&mut state, actor, &self.config, &command)?;
        let event = Self::persist_result(&tx, task, &command, &result, &state).await?;
        Self::finish(
            &tx,
            task,
            actor,
            command.key(),
            &hash,
            &result,
            event,
            &state,
        )
        .await?;
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }
    async fn outbox(
        &self,
        task: Uuid,
        actor: &Principal,
        after: i64,
    ) -> Result<Vec<OutboxEvent>, AppError> {
        if after < 0 {
            return Err(AppError::validation("cursor must be nonnegative"));
        }
        let tx = self.db.begin().await.map_err(map_db)?;
        self.load(&tx, task, actor).await?;
        let rows = tx.query_all(statement("SELECT sequence,event_id,task_id,event_type,payload,created_at FROM sdlc_outbox WHERE task_id=$1 AND sequence>$2 ORDER BY sequence LIMIT 100", vec![task.into(), after.into()])).await.map_err(map_db)?;
        let events = rows
            .into_iter()
            .map(|r| {
                Ok(OutboxEvent {
                    sequence: r.try_get("", "sequence").map_err(map_db)?,
                    event_id: r.try_get("", "event_id").map_err(map_db)?,
                    task_id: r.try_get("", "task_id").map_err(map_db)?,
                    event_type: r.try_get("", "event_type").map_err(map_db)?,
                    payload: r.try_get("", "payload").map_err(map_db)?,
                    created_at: r.try_get("", "created_at").map_err(map_db)?,
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        tx.commit().await.map_err(map_db)?;
        Ok(events)
    }
}
