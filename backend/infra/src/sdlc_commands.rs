//! Commands authorize and commit against the current task in one transaction.
use super::*;

impl PostgresSdlcRepository {
    pub(super) async fn execute_command(
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
        if Self::enrolled(&tx, task).await? && matches!(command, SdlcCommand::Assign(_)) {
            return Err(AppError::conflict(
                "Reserved PM assignment cannot be replaced by a generic command",
            ));
        }
        if let Some(result) = self
            .execute_critical_pm_review_command(&tx, &state, actor, &command, &hash)
            .await?
        {
            tx.commit().await.map_err(map_db)?;
            return Ok(result);
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
