use super::{Api, Result, Value, enc, json_input, sdlc_path};
use clap::Subcommand;
use uuid::Uuid;

#[derive(Subcommand)]
pub enum DiscussionCommand {
    /// Read the ticket and participants' comments; follow next_after with the same snapshot
    Read {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_parser = clap::value_parser!(u16).range(1..=500))]
        limit: Option<u16>,
        #[arg(long, requires = "snapshot")]
        after: Option<Uuid>,
        #[arg(long)]
        snapshot: Option<Uuid>,
    },
    /// Add an ordinary Tracker comment under the current assignment
    Publish {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        from_file: String,
    },
    /// Reconcile the original comment operation; never choose a fresh key after an uncertain POST
    Operation {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        key: String,
    },
}

pub async fn execute(api: &Api, command: DiscussionCommand) -> Result<Value> {
    match command {
        DiscussionCommand::Read {
            task,
            limit,
            after,
            snapshot,
        } => {
            let mut query = Vec::new();
            if let Some(value) = limit {
                query.push(format!("limit={value}"));
            }
            if let Some(value) = after {
                query.push(format!("after={value}"));
            }
            if let Some(value) = snapshot {
                query.push(format!("snapshot={value}"));
            }
            let suffix = if query.is_empty() {
                String::new()
            } else {
                format!("?{}", query.join("&"))
            };
            api.get(&format!("{}{suffix}", sdlc_path(task, "/discussion")))
                .await
        }
        DiscussionCommand::Publish { task, from_file } => {
            api.post(&sdlc_path(task, "/discussion"), json_input(&from_file)?)
                .await
        }
        DiscussionCommand::Operation { task, key } => {
            api.get(&sdlc_path(
                task,
                &format!("/discussion/operations/{}", enc(&key)),
            ))
            .await
        }
    }
}
