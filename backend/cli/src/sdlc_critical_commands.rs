use super::{Api, Result, Value, enc, json_input, sdlc_path};
use clap::Subcommand;
use uuid::Uuid;

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
            api.post(
                &sdlc_path(task, "/critical-blockers"),
                json_input(&from_file)?,
            )
            .await
        }
        CriticalBlockerCommand::Read { task, blocker } => {
            api.get(&sdlc_path(task, &format!("/critical-blockers/{blocker}")))
                .await
        }
        CriticalBlockerCommand::Operation { task, key } => {
            api.get(&sdlc_path(
                task,
                &format!("/critical-blockers/operations/{}", enc(&key)),
            ))
            .await
        }
    }
}
