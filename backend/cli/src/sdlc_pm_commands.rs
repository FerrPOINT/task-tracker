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
    /// Read an accepted typed result by its authoritative result UUID
    Read {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        result: Uuid,
    },
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
        DeliveryResultCommand::Read { task, result } => {
            api.get(&sdlc_path(task, &format!("/lifecycle/results/{result}")))
                .await
        }
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
    let context = if receipt["stage"] == "Draft" {
        Some(api.get(&sdlc_path(task, "/context")).await?)
    } else {
        None
    };
    validate_confirmation(&receipt, task, revision, context.as_ref())?;
    Ok(receipt)
}

fn validate_confirmation(
    receipt: &Value,
    task: Uuid,
    revision: i64,
    configured_context: Option<&Value>,
) -> Result<()> {
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
        || receipt["stage"]
            != if configured_context.is_some() {
                "Draft"
            } else {
                "Backlog"
            }
    {
        anyhow::bail!("owner confirmation receipt does not match requested task/revision");
    }
    if let Some(context) = configured_context {
        if context["interaction_protocol"] != "configured-cli-v1"
            || context["task_id"].as_str() != Some(task_text.as_str())
            || context["root_task_id"].as_str() != Some(task_text.as_str())
            || context["owner_subject"] != receipt["owner_subject"]
            || context["requirement_revision"].as_i64() != Some(revision)
            || context["content_hash"] != receipt["content_hash"]
            || !((context["stage"] == "Draft" && context["status"] == "InProgress")
                || (context["stage"] == "Backlog" && context["status"] == "Ready"))
        {
            anyhow::bail!("configured owner confirmation differs from current task context");
        }
    }
    Ok(())
}

#[cfg(test)]
mod confirmation_tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> (Uuid, Value, Value) {
        let task = Uuid::from_u128(11);
        let receipt = json!({"id":Uuid::from_u128(12),"task_id":task,"revision":3,
            "owner_subject":Uuid::from_u128(13),"content_hash":"a".repeat(64),
            "created_at":"2026-10-26T10:00:00Z","stage":"Draft"});
        let context = json!({"interaction_protocol":"configured-cli-v1", "task_id":task,
            "root_task_id":task,"owner_subject":receipt["owner_subject"],
            "requirement_revision":3,"content_hash":receipt["content_hash"],
            "stage":"Draft","status":"InProgress"});
        (task, receipt, context)
    }

    #[test]
    fn legacy_backlog_confirmation_retains_direct_readback_validation() {
        let (task, mut receipt, _) = fixture();
        receipt["stage"] = json!("Backlog");
        validate_confirmation(&receipt, task, 3, None).unwrap();
        for stage in ["Draft", "Analysis", "Completed", ""] {
            receipt["stage"] = json!(stage);
            assert!(validate_confirmation(&receipt, task, 3, None).is_err());
        }
    }

    #[test]
    fn configured_draft_receipt_survives_terminal_completion_without_rewriting_stage() {
        let (task, receipt, mut context) = fixture();
        let original = receipt.clone();
        validate_confirmation(&receipt, task, 3, Some(&context)).unwrap();
        context["stage"] = json!("Backlog");
        context["status"] = json!("Ready");
        for _ in 0..2 {
            validate_confirmation(&receipt, task, 3, Some(&context)).unwrap();
        }
        assert_eq!(receipt, original);
        assert_eq!(receipt["stage"], "Draft");
        assert!(validate_confirmation(&receipt, task, 2, Some(&context)).is_err());
        assert!(validate_confirmation(&receipt, task, 4, Some(&context)).is_err());
    }

    #[test]
    fn configured_confirmation_requires_exact_protocol_current_revision_and_owner_tuple() {
        let (task, receipt, context) = fixture();
        for (field, value) in [
            ("interaction_protocol", json!(null)),
            ("interaction_protocol", json!("legacy")),
            ("interaction_protocol", json!("configured-cli-v2")),
            ("task_id", json!(Uuid::from_u128(21))),
            ("root_task_id", json!(Uuid::from_u128(22))),
            ("owner_subject", json!(Uuid::from_u128(23))),
            ("requirement_revision", json!(4)),
            ("requirement_revision", json!("3")),
            ("content_hash", json!("b".repeat(64))),
            ("stage", json!("Analysis")),
            ("status", json!("Ready")),
        ] {
            let mut changed = context.clone();
            changed[field] = value;
            assert!(
                validate_confirmation(&receipt, task, 3, Some(&changed)).is_err(),
                "{field}"
            );
        }
        for field in [
            "interaction_protocol",
            "task_id",
            "root_task_id",
            "owner_subject",
            "requirement_revision",
            "content_hash",
            "stage",
            "status",
        ] {
            let mut changed = context.clone();
            changed.as_object_mut().unwrap().remove(field);
            assert!(
                validate_confirmation(&receipt, task, 3, Some(&changed)).is_err(),
                "{field}"
            );
        }
        let mut false_publication = receipt.clone();
        false_publication["stage"] = json!("Backlog");
        assert!(validate_confirmation(&false_publication, task, 3, Some(&context)).is_err());
    }

    #[test]
    fn malformed_or_foreign_receipts_fail_in_both_protocols() {
        let (task, receipt, context) = fixture();
        for configured in [false, true] {
            let mut base = receipt.clone();
            if !configured {
                base["stage"] = json!("Backlog");
            }
            let proof = configured.then_some(&context);
            for (field, value) in [
                ("id", json!(Uuid::nil())),
                ("id", json!("not-a-uuid")),
                ("task_id", json!(Uuid::from_u128(31))),
                ("revision", json!(2)),
                ("content_hash", json!("x".repeat(64))),
                ("owner_subject", json!(" ")),
                ("created_at", json!("")),
            ] {
                let mut changed = base.clone();
                changed[field] = value;
                assert!(
                    validate_confirmation(&changed, task, 3, proof).is_err(),
                    "{field}"
                );
            }
        }
    }
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
