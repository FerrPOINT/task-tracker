use super::*;
use domain::sdlc_native_admission::{CommandAdmission, NativeAdmission};

impl PostgresSdlcRepository {
    pub(super) async fn native_command_preflight(
        &self,
        task: Uuid,
        actor: &Principal,
        command: &SdlcCommand,
    ) -> Result<CommandAdmission, AppError> {
        app::sdlc::validate_key(command.key())?;
        let tx = self.db.begin().await.map_err(map_db)?;
        let state = self.load(&tx, task, actor).await?;
        app::sdlc::authorize(&state, actor, &self.config, command)?;
        let hash = app::sdlc::command_hash(command)?;
        let result = if let Some(value) =
            Self::replay(&tx, task, actor, command.key(), &hash).await?
        {
            CommandAdmission::Replay(value)
        } else if !Self::enrolled(&tx, task).await? {
            CommandAdmission::Legacy
        } else {
            if matches!(command, SdlcCommand::Assign(_)) {
                return Err(AppError::conflict(
                    "Reserved PM assignment cannot be replaced by a generic command",
                ));
            }
            let assignment = state
                .assignment
                .as_ref()
                .ok_or_else(|| AppError::conflict("PM assignment missing"))?;
            let reservation = Self::reservation(&tx, &state, assignment.execution_id).await?;
            if matches!(
                command,
                SdlcCommand::Answer { .. } | SdlcCommand::Confirm { .. } | SdlcCommand::Evidence(_)
            ) {
                CommandAdmission::History(reservation)
            } else {
                CommandAdmission::Native(reservation)
            }
        };
        tx.commit().await.map_err(map_db)?;
        Ok(result)
    }

    pub(super) async fn verify_native_command(
        &self,
        tx: &DatabaseTransaction,
        state: &TaskState,
        expected: &PmDraftReservation,
        observation: Option<&NativeAdmission>,
        command: &SdlcCommand,
    ) -> Result<(), AppError> {
        let assignment = state
            .assignment
            .as_ref()
            .ok_or_else(|| AppError::conflict("PM assignment missing"))?;
        let current = Self::reservation(tx, state, assignment.execution_id).await?;
        if json_value(&current)? != json_value(expected)? {
            return Err(AppError::conflict(
                "PM reservation changed during native observation",
            ));
        }
        if let Some(observed) = observation {
            observed.validate(&current)?;
            let latest = tx
                .query_one(statement(
                    "SELECT session_run_id,fence FROM sdlc_pm_native_admissions
                  WHERE execution_id=$1 ORDER BY fence DESC LIMIT 1",
                    vec![assignment.execution_id.into()],
                ))
                .await
                .map_err(map_db)?;
            if let Some(latest) = latest {
                let fence: i64 = latest.try_get("", "fence").map_err(map_db)?;
                let run: Uuid = latest.try_get("", "session_run_id").map_err(map_db)?;
                if observed
                    .fence
                    .checked_sub(fence)
                    .is_none_or(|delta| !(0..=1).contains(&delta))
                    || observed.fence == fence && observed.session_run_id != run
                {
                    return Err(AppError::conflict(
                        "Native admission fence is stale or discontinuous",
                    ));
                }
            } else if observed.fence != 1 {
                return Err(AppError::conflict(
                    "Initial native admission fence must be one",
                ));
            }
            let lease = Self::lease_current(tx, &current)
                .await?
                .ok_or_else(|| AppError::conflict("PM execution lease missing"))?;
            if lease.lease.holder_subject != assignment.machine_subject
                || lease.lease.expires_at <= chrono::Utc::now()
            {
                return Err(AppError::conflict("PM execution lease expired or foreign"));
            }
            let previous=tx.query_one(statement(
                "SELECT proof FROM sdlc_pm_native_admissions WHERE execution_id=$1 AND session_run_id=$2",
                vec![assignment.execution_id.into(),observed.session_run_id.into()])).await.map_err(map_db)?;
            if let Some(previous) = previous {
                let original: NativeAdmission =
                    serde_json::from_value(previous.try_get("", "proof").map_err(map_db)?)
                        .map_err(AppError::internal)?;
                if json_value(&original.identity)? != json_value(&observed.identity)?
                    || original.fence != observed.fence
                    || original.native_run_ref != observed.native_run_ref
                    || original.native_session_ref != observed.native_session_ref
                    || original.binding_ref != observed.binding_ref
                    || original.effective_config_revision != observed.effective_config_revision
                    || original.configuration_sha256 != observed.configuration_sha256
                    || json_value(&original.native_configuration)?
                        != json_value(&observed.native_configuration)?
                {
                    return Err(AppError::conflict(
                        "Native PM run/configuration identity changed",
                    ));
                }
            } else {
                exec(tx,"INSERT INTO sdlc_pm_native_admissions(execution_id,session_run_id,observation_ref,
                      assignment_id,assignment_version,fence,proof) VALUES($1,$2,$3,$4,$5,$6,$7)",
                      vec![assignment.execution_id.into(),observed.session_run_id.into(),observed.observation_ref.into(),
                        assignment.assignment_id.into(),assignment.version.into(),observed.fence.into(),json_value(observed)?]).await?;
            }
        } else {
            if !matches!(
                command,
                SdlcCommand::Answer { .. } | SdlcCommand::Confirm { .. } | SdlcCommand::Evidence(_)
            ) {
                return Err(AppError::conflict(
                    "Current native observation is required for a PM machine write",
                ));
            }
            let row=tx.query_one(statement("SELECT EXISTS(SELECT 1 FROM sdlc_pm_native_admissions
                  WHERE execution_id=$1 AND assignment_id=$2 AND assignment_version=$3) AS admitted",
                  vec![assignment.execution_id.into(),assignment.assignment_id.into(),assignment.version.into()]))
                  .await.map_err(map_db)?.ok_or_else(||AppError::internal("Native admission history missing"))?;
            if !row.try_get::<bool>("", "admitted").map_err(map_db)? {
                return Err(AppError::conflict(
                    "PM execution has no verified native admission history",
                ));
            }
        }
        Ok(())
    }
    pub(super) async fn execute_with_native(
        &self,
        task: Uuid,
        actor: &Principal,
        command: SdlcCommand,
        expected: Option<PmDraftReservation>,
        observed: Option<domain::sdlc_native_admission::NativeAdmission>,
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
        if Self::enrolled(&tx, task).await? {
            let expected = expected.ok_or_else(|| {
                AppError::conflict("reserved PM execution requires verified admission")
            })?;
            if matches!(command, SdlcCommand::Assign(_)) {
                return Err(AppError::conflict(
                    "Reserved PM assignment cannot be replaced by a generic command",
                ));
            }
            self.verify_native_command(&tx, &state, &expected, observed.as_ref(), &command)
                .await?;
        }
        let result = app::sdlc::apply(&mut state, actor, &self.config, &command)?;
        let event = self
            .persist_result(&tx, task, &command, &result, &state)
            .await?;
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
}

#[cfg(test)]
mod owned_history_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires explicit owned native Tracker QA database with accepted history"]
    async fn history_does_not_authorize_new_machine_writes_or_foreign_reservations() {
        let db = Database::connect(std::env::var("TRACKER_NATIVE_ADMISSION_TEST_URL").unwrap())
            .await
            .unwrap();
        let tx = db.begin().await.unwrap();
        let state = state_from(tx.query_one(statement(
            "SELECT t.state FROM sdlc_tasks t WHERE t.pm_execution_id IN (SELECT execution_id FROM sdlc_pm_native_admissions) ORDER BY t.task_id LIMIT 1 FOR UPDATE",
            vec![],
        )).await.unwrap().unwrap()).unwrap();
        let repo = PostgresSdlcRepository {
            db,
            config: SdlcConfig {
                instance_id: state.tracker_instance_id.clone(),
                orchestrator_subject: String::new(),
                verifier_subject: String::new(),
                reservation_scheduler_subject: String::new(),
            },
        };
        let assignment = state.assignment.as_ref().unwrap();
        let reservation = PostgresSdlcRepository::reservation(&tx, &state, assignment.execution_id)
            .await
            .unwrap();
        let command = SdlcCommand::PublishRevision(PublishRevision {
            fence: MachineFence {
                assignment_id: assignment.assignment_id,
                execution_id: assignment.execution_id,
                agent_id: assignment.agent_id,
                assignment_version: assignment.version,
            },
            expected_requirement_revision: state.current_revision(),
            document: state.revisions.last().unwrap().document.clone(),
            idempotency_key: "owned-negative-machine".into(),
        });
        let error = repo
            .verify_native_command(&tx, &state, &reservation, None, &command)
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Current native observation is required")
        );
        let owner = SdlcCommand::Confirm {
            revision: state.current_revision().unwrap(),
            command: ConfirmCommand {
                content_hash: state.revisions.last().unwrap().content_hash.clone(),
                idempotency_key: "owned-negative-owner".into(),
                expected_routing_policy_version: None,
            },
        };
        repo.verify_native_command(&tx, &state, &reservation, None, &owner)
            .await
            .unwrap();
        let mut foreign = reservation.clone();
        foreign.assignment.agent_id = Uuid::new_v4();
        assert!(
            repo.verify_native_command(&tx, &state, &foreign, None, &owner)
                .await
                .is_err()
        );
        tx.rollback().await.unwrap();
    }
}
