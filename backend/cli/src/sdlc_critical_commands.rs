use super::{Api, Result, Value, enc, json_input, sdlc_path};
use clap::Subcommand;
use uuid::Uuid;

fn readable_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !matches!(value, "." | "..")
        && !value
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '/' | '\\'))
}

#[derive(Subcommand)]
pub enum CriticalBlockerCommand {
    /// Scheduler only: return a held task to PM after actual native terminal readback
    ReturnToPm {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
    /// Scheduler only: read the original terminal return operation without retrying POST
    ReturnOperation {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        key: String,
    },
    /// Submit a critical business question using the original JSON fence and key
    Submit {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
    /// Read the current server-owned critical blocker receipt
    Read {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        blocker: Uuid,
    },
    /// Reconcile an unknown submission using its original idempotency key
    Operation {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        key: String,
    },
}

pub async fn critical_blocker(api: &Api, command: CriticalBlockerCommand) -> Result<Value> {
    match command {
        CriticalBlockerCommand::ReturnToPm { task, from_file } => {
            api.post(
                &sdlc_path(task, "/critical-blockers/returns"),
                json_input(&from_file)?,
            )
            .await
        }
        CriticalBlockerCommand::ReturnOperation { task, key } => {
            anyhow::ensure!(
                !matches!(key.as_str(), "." | "..")
                    && !key.is_empty()
                    && key.len() <= 128
                    && !key.chars().any(|c| c.is_whitespace() || c.is_control()),
                "invalid critical return operation key"
            );
            api.get(&sdlc_path(
                task,
                &format!("/critical-blockers/returns/operations/{}", enc(&key)),
            ))
            .await
        }
        CriticalBlockerCommand::Submit { task, from_file } => {
            let payload = json_input(&from_file)?;
            anyhow::ensure!(
                payload["idempotency_key"]
                    .as_str()
                    .is_some_and(readable_key),
                "invalid critical operation key"
            );
            api.post(&sdlc_path(task, "/critical-blockers"), payload)
                .await
        }
        CriticalBlockerCommand::Read { task, blocker } => {
            api.get(&sdlc_path(task, &format!("/critical-blockers/{blocker}")))
                .await
        }
        CriticalBlockerCommand::Operation { task, key } => {
            anyhow::ensure!(readable_key(&key), "invalid critical operation key");
            api.get(&sdlc_path(
                task,
                &format!("/critical-blockers/operations/{}", enc(&key)),
            ))
            .await
        }
    }
}
