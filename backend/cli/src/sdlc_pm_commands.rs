use super::{Api, Result, Value, enc, json_input, sdlc_path};
use clap::Subcommand;
use uuid::Uuid;

#[derive(Subcommand)]
pub enum RoutingCommand {
    Policy {
        #[arg(long)]
        project: Uuid,
    },
    Snapshot {
        #[arg(long)]
        task: Uuid,
    },
}

#[derive(Subcommand)]
pub enum ConfirmationCommand {
    /// Read the exact retained owner confirmation receipt directly
    Read {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_parser = clap::value_parser!(i64).range(1..=9_007_199_254_740_991))]
        revision: i64,
    },
}

#[derive(Subcommand)]
pub enum DeliveryResultCommand {
    /// Persist Workflow terminal evidence while this runtime run is still alive
    Propose {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
    /// Read the original proposal receipt after an unknown submission outcome
    Operation {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        key: String,
    },
}

pub async fn delivery_result(api: &Api, command: DeliveryResultCommand) -> Result<Value> {
    match command {
        DeliveryResultCommand::Propose { task, from_file } => {
            api.post(
                &sdlc_path(task, "/delivery-proposals"),
                json_input(&from_file)?,
            )
            .await
        }
        DeliveryResultCommand::Operation { task, key } => {
            api.get(&sdlc_path(
                task,
                &format!("/delivery-proposals/operations/{}", enc(&key)),
            ))
            .await
        }
    }
}

pub async fn routing(api: &Api, command: RoutingCommand) -> Result<Value> {
    match command {
        RoutingCommand::Policy { project } => {
            api.get(&format!("/api/v1/projects/{project}/sdlc/routing-policy"))
                .await
        }
        RoutingCommand::Snapshot { task } => api.get(&sdlc_path(task, "/routing-snapshot")).await,
    }
}

pub async fn confirmation(api: &Api, command: ConfirmationCommand) -> Result<Value> {
    let ConfirmationCommand::Read { task, revision } = command;
    let receipt = api
        .get(&sdlc_path(
            task,
            &format!("/requirements/{revision}/confirmation"),
        ))
        .await?;
    let task_text = task.to_string();
    let canonical_id = receipt.get("id").and_then(Value::as_str).is_some_and(|id| {
        Uuid::parse_str(id).is_ok_and(|value| !value.is_nil() && value.to_string() == id)
    });
    let canonical_hash = receipt["content_hash"].as_str().is_some_and(|hash| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    });
    if !canonical_id
        || !canonical_hash
        || receipt["task_id"].as_str() != Some(task_text.as_str())
        || receipt["revision"].as_i64() != Some(revision)
        || receipt["owner_subject"]
            .as_str()
            .is_none_or(|owner| owner.trim().is_empty())
        || receipt["created_at"]
            .as_str()
            .is_none_or(|time| time.trim().is_empty())
        || receipt["stage"] != "Backlog"
    {
        anyhow::bail!("owner confirmation receipt does not match requested task/revision");
    }
    Ok(receipt)
}

#[derive(Subcommand)]
pub enum DraftCommand {
    /// Create one owner Draft; the JSON idempotency key identifies the creation saga
    Create {
        #[arg(long)]
        project: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
    /// Reconcile unknown creation acceptance before replaying the original command
    Operation {
        #[arg(long)]
        project: Uuid,
        #[arg(long)]
        key: String,
    },
}

#[derive(Subcommand)]
pub enum PmCommand {
    /// Save owner dialogue without manufacturing a clarification answer
    Feedback {
        #[command(subcommand)]
        command: FeedbackCommand,
    },
    /// Reserve or read the existing PM assignment
    Assignment {
        #[command(subcommand)]
        command: AssignmentCommand,
    },
    /// Claim, renew or read the existing PM ownership lease
    Lease {
        #[command(subcommand)]
        command: LeaseCommand,
    },
    /// Submit or reconcile the actual PM terminal completion
    Completion {
        #[command(subcommand)]
        command: CompletionCommand,
    },
}

#[derive(Subcommand)]
pub enum FeedbackCommand {
    Submit {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
    Operation {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        key: String,
    },
}

#[derive(Subcommand)]
pub enum AssignmentCommand {
    Reserve {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
    Read {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        key: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum LeaseCommand {
    Claim {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
    Heartbeat {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
    Read {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        key: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum CompletionCommand {
    Submit {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
    Read {
        #[arg(long)]
        task: Uuid,
    },
    Operation {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        key: String,
    },
}

fn readback_path(task: Uuid, resource: &str, key: Option<String>) -> String {
    let mut path = sdlc_path(task, resource);
    if let Some(key) = key {
        path.push_str(&format!("?idempotency_key={}", enc(&key)));
    }
    path
}

pub async fn draft(api: &Api, command: DraftCommand) -> Result<Value> {
    match command {
        DraftCommand::Create { project, from_file } => {
            api.post(
                &format!("/api/v1/projects/{project}/sdlc/drafts"),
                json_input(&from_file)?,
            )
            .await
        }
        DraftCommand::Operation { project, key } => {
            api.get(&format!(
                "/api/v1/projects/{project}/sdlc/drafts/operations/{}",
                enc(&key)
            ))
            .await
        }
    }
}

pub async fn pm(api: &Api, command: PmCommand) -> Result<Value> {
    match command {
        PmCommand::Feedback { command } => match command {
            FeedbackCommand::Submit { task, from_file } => {
                api.post(&sdlc_path(task, "/pm-feedback"), json_input(&from_file)?)
                    .await
            }
            FeedbackCommand::Operation { task, key } => {
                api.get(&sdlc_path(
                    task,
                    &format!("/pm-feedback/operations/{}", enc(&key)),
                ))
                .await
            }
        },
        PmCommand::Assignment { command } => match command {
            AssignmentCommand::Reserve { task, from_file } => {
                api.post(
                    &sdlc_path(task, "/pm-draft-assignment"),
                    json_input(&from_file)?,
                )
                .await
            }
            AssignmentCommand::Read { task, key } => {
                api.get(&readback_path(task, "/pm-draft-assignment", key))
                    .await
            }
        },
        PmCommand::Lease { command } => match command {
            LeaseCommand::Claim { task, from_file } => {
                api.post(
                    &sdlc_path(task, "/pm-draft-execution-lease"),
                    json_input(&from_file)?,
                )
                .await
            }
            LeaseCommand::Heartbeat { task, from_file } => {
                api.post(
                    &sdlc_path(task, "/pm-draft-execution-lease/heartbeat"),
                    json_input(&from_file)?,
                )
                .await
            }
            LeaseCommand::Read { task, key } => {
                api.get(&readback_path(task, "/pm-draft-execution-lease", key))
                    .await
            }
        },
        PmCommand::Completion { command } => match command {
            CompletionCommand::Submit { task, from_file } => {
                api.post(
                    &sdlc_path(task, "/pm-draft-completion"),
                    json_input(&from_file)?,
                )
                .await
            }
            CompletionCommand::Read { task } => {
                api.get(&sdlc_path(task, "/pm-draft-completion")).await
            }
            CompletionCommand::Operation { task, key } => {
                api.get(&sdlc_path(
                    task,
                    &format!("/pm-draft-completion/operations/{}", enc(&key)),
                ))
                .await
            }
        },
    }
}
