use super::{Api, Result, Value, enc, json_input};
use clap::Subcommand;
use uuid::Uuid;

const SAVE_HELP: &str = r#"Pass a complete JSON object through --from-file FILE or --from-file - (stdin).
Read context and decomposition first. Replace placeholders with current server
values; assignment_version is context.assignment.version. Generate a new UUID
for a new section; preserve existing UUIDs when renaming or reordering.

{
  "expected_revision": 0,
  "idempotency_key": "<original-operation-key>",
  "reason": "<required when changing a saved plan>",
  "fence": {
    "assignment_id": "<context.assignment.assignment_id>",
    "execution_id": "<context.assignment.execution_id>",
    "agent_id": "<context.assignment.agent_id>",
    "assignment_version": 1
  },
  "content": {
    "overview": "<architecture and verified source provenance>",
    "source": {
      "requirements_revision": 1,
      "requirements_hash": "<exact current requirements hash>",
      "analysis_result_id": "<context.previous_result_id>"
    },
    "sections": [{
      "id": "<new or existing section UUID>",
      "title": "<section title>",
      "description": "<expected work and relevant paths>",
      "criteria": "<validation criteria as text>",
      "requirements": "<covered requirements as text>"
    }]
  }
}

Section fields are exactly id, title, description, criteria and requirements.
All five are strings; id must be a UUID. Extra fields and arrays are rejected.
For terminal-only use, a single-quoted heredoc supplies literal JSON to stdin:
  task-tracker decomposition save --task TASK_UUID --from-file - <<'JSON'
  <complete JSON object>
  JSON
The heredoc delimiter must be on its own line without indentation. Do not use
an unescaped apostrophe inside single-quoted printf data. Do not execute JSON
as a script. After an unknown response retain the original key/body and read
decomposition operation, then the exact revision; never silently overwrite CAS.
"#;

#[derive(Subcommand)]
pub enum DecompositionCommand {
    /// Read the current plan and editing restrictions
    Read {
        #[arg(long)]
        task: Uuid,
    },
    /// Save an entire plan with expected_revision and a stable idempotency_key
    #[command(after_long_help = SAVE_HELP)]
    Save {
        #[arg(long)]
        task: Uuid,
        /// Complete JSON command; '-' reads stdin without creating a file
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
