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
    let context = if api.configured_cli {
        Some(api.get(&sdlc_path(task, "/context")).await?)
    } else {
        None
    };
    validate_confirmation(
        &receipt,
        task,
        revision,
        api.configured_cli,
        context.as_ref(),
    )?;
    Ok(receipt)
}

fn validate_confirmation(
    receipt: &Value,
    task: Uuid,
    revision: i64,
    configured_cli: bool,
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
        || receipt["stage"] != if configured_cli { "Draft" } else { "Backlog" }
    {
        anyhow::bail!("owner confirmation receipt does not match requested task/revision");
    }
    if configured_cli {
        let context = configured_context.ok_or_else(|| {
            anyhow::anyhow!("configured owner confirmation requires current task context")
        })?;
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
        validate_confirmation(&receipt, task, 3, false, None).unwrap();
        for stage in ["Draft", "Analysis", "Completed", ""] {
            receipt["stage"] = json!(stage);
            assert!(validate_confirmation(&receipt, task, 3, false, None).is_err());
        }
    }

    #[test]
    fn configured_draft_receipt_survives_terminal_completion_without_rewriting_stage() {
        let (task, receipt, mut context) = fixture();
        let original = receipt.clone();
        validate_confirmation(&receipt, task, 3, true, Some(&context)).unwrap();
        context["stage"] = json!("Backlog");
        context["status"] = json!("Ready");
        for _ in 0..2 {
            validate_confirmation(&receipt, task, 3, true, Some(&context)).unwrap();
        }
        assert_eq!(receipt, original);
        assert_eq!(receipt["stage"], "Draft");
        assert!(validate_confirmation(&receipt, task, 2, true, Some(&context)).is_err());
        assert!(validate_confirmation(&receipt, task, 4, true, Some(&context)).is_err());
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
                validate_confirmation(&receipt, task, 3, true, Some(&changed)).is_err(),
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
                validate_confirmation(&receipt, task, 3, true, Some(&changed)).is_err(),
                "{field}"
            );
        }
        let mut false_publication = receipt.clone();
        false_publication["stage"] = json!("Backlog");
        assert!(validate_confirmation(&false_publication, task, 3, true, Some(&context)).is_err());
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
                    validate_confirmation(&changed, task, 3, configured, proof).is_err(),
                    "{field}"
                );
            }
        }
    }

    #[test]
    fn configured_mode_never_falls_back_based_on_receipt_stage_or_missing_context() {
        let (task, draft, context) = fixture();
        let mut backlog = draft.clone();
        backlog["stage"] = json!("Backlog");
        for proof in [None, Some(&context)] {
            assert!(validate_confirmation(&backlog, task, 3, true, proof).is_err());
            assert!(validate_confirmation(&draft, task, 3, false, proof).is_err());
        }
        assert!(validate_confirmation(&draft, task, 3, true, None).is_err());
        validate_confirmation(&draft, task, 3, true, Some(&context)).unwrap();
        validate_confirmation(&backlog, task, 3, false, None).unwrap();
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
pub enum CompletionCommand {
    /// Submit the actual PM completion JSON payload from a file or stdin
    #[command(
        after_long_help = r#"Configured-cli-v1 only: first read `task-tracker sdlc context --task TASK_UUID`.
Use the current assigned PM service identity, not human/scheduler credentials.
Read the latest real requirements and their exact saved human-owner confirmation
with `task-tracker sdlc confirmation read --task TASK_UUID --revision REVISION`.
Draft publication or a clarification answer is NOT that confirmation.
Finish the same PM Workflow only after that consent. Before this submit, require
its accepted complete=true result with the real terminal_receipt. This command
does not create the Workflow receipt or confirm requirements for the owner.

Copy fence from fresh Tracker context.assignment: assignment_id, execution_id,
agent_id and version (as assignment_version). Match those to terminal_receipt
assignment_ref, tracker_execution_id, concrete_agent_ref and assignment_version.
Tracker execution_id is NOT Workflow execution_ref; never change either to match.
Take requirement_revision from terminal_receipt.requirements_revision (singular
in this request), content_hash from requirements_sha256, namespace_id from
namespace_id, workflow_execution_ref from execution_ref, workflow_operation_key
from operation_key, and workflow_result_version from result_version.
All identities, versions, keys and hash below are illustrative, not authority.
Send ONLY these fields; do not paste workflow_receipt/terminal_receipt objects
or add native checkpoint/lease fields. Tracker reads the terminal receipt
server-to-server and atomically publishes Draft -> Backlog / Ready. Stop there;
do not queue Analysis or dispatch Analyst. Historical completion remains readable;
new submissions use only this configured command, without native checkpoints or leases.
Pipe JSON into `task-tracker sdlc pm completion submit --task TASK_UUID --from-file -`.

After success, unknown acceptance or an exact replay, read the original operation:
`task-tracker sdlc pm completion operation --task TASK_UUID --key COMPLETION_KEY`,
then `task-tracker sdlc pm completion read --task TASK_UUID` and the task context.
Retain the exact body and COMPLETION_KEY; no blind resend or replacement key after
an unknown outcome. Match the saved result's task/root, revision/hash and
Backlog / Ready state. An unchanged exact replay must not create another completion.

JSON example (configured-cli-v1):
{
  "interaction_protocol": "configured-cli-v1",
  "fence": {
    "assignment_id": "22222222-2222-4222-8222-222222222222",
    "execution_id": "33333333-3333-4333-8333-333333333333",
    "agent_id": "44444444-4444-4444-8444-444444444444",
    "assignment_version": 1
  },
  "requirement_revision": 2,
  "content_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "namespace_id": 2,
  "workflow_execution_ref": "99999999-9999-4999-8999-999999999999",
  "workflow_operation_key": "example-final-report",
  "workflow_result_version": 4,
  "idempotency_key": "example-completion-1"
}"#
    )]
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
