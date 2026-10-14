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
