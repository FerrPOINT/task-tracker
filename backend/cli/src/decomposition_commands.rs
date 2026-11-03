use super::{Api, Result, Value, enc, json_input};
use clap::Subcommand;
use uuid::Uuid;

#[derive(Subcommand)]
pub enum DecompositionCommand {
    /// Read the current plan and editing restrictions
    Read {
        #[arg(long)]
        task: Uuid,
    },
    /// Save an entire plan with expected_revision and a stable idempotency_key
    Save {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        from_file: String,
    },
    /// List immutable revisions, newest first
    History {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        before: Option<i64>,
        #[arg(long, value_parser=clap::value_parser!(u16).range(1..=100))]
        limit: Option<u16>,
    },
    /// Read an exact revision, including removed sections
    Revision {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        revision: i64,
    },
    /// Reconcile an uncertain save using its original key; never invent a new key
    Operation {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        key: String,
    },
}
pub async fn execute(api: &Api, command: DecompositionCommand) -> Result<Value> {
    match command {
        DecompositionCommand::Read { task } => api.get(&path(task, "")).await,
        DecompositionCommand::Save { task, from_file } => {
            api.post(&path(task, ""), json_input(&from_file)?).await
        }
        DecompositionCommand::History {
            task,
            before,
            limit,
        } => {
            api.get(&super::query(
                &path(task, "/revisions"),
                vec![
                    ("before", before.map(|v| v.to_string())),
                    ("limit", limit.map(|v| v.to_string())),
                ],
            ))
            .await
        }
        DecompositionCommand::Revision { task, revision } => {
            api.get(&path(task, &format!("/revisions/{revision}")))
                .await
        }
        DecompositionCommand::Operation { task, key } => {
            // URL clients normalize dot segments before the server can validate
            // the key. Never mistake the parent plan for an operation receipt.
            if matches!(key.as_str(), "." | "..") {
                anyhow::bail!("invalid decomposition operation key");
            }
            api.get(&path(task, &format!("/operations/{}", enc(&key))))
                .await
        }
    }
}
fn path(task: Uuid, suffix: &str) -> String {
    format!("/api/v1/issues/{task}/decomposition{suffix}")
}
