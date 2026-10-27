use super::{Api, enc, support};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use reqwest::{Method, multipart};
use serde_json::{Value, json};
use std::path::PathBuf;
use uuid::Uuid;

#[path = "sdlc_critical_commands.rs"]
mod critical_commands;
#[path = "sdlc_pm_commands.rs"]
mod pm_commands;

#[derive(Debug, Args)]
pub struct Page {
    #[arg(long)]
    pub limit: Option<u64>,
    #[arg(long)]
    pub offset: Option<u64>,
}
impl Page {
    pub fn pairs(&self) -> Vec<(&'static str, Option<String>)> {
        vec![
            ("limit", self.limit.map(|v| v.to_string())),
            ("offset", self.offset.map(|v| v.to_string())),
        ]
    }
}
#[derive(Debug, Args)]
pub struct Filters {
    #[command(flatten)]
    pub page: Page,
    #[arg(long)]
    pub q: Option<String>,
    #[arg(long)]
    pub priority: Option<String>,
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long)]
    pub assignee_id: Option<String>,
    #[arg(long)]
    pub sort_by: Option<String>,
    #[arg(long)]
    pub sort_order: Option<String>,
}
impl Filters {
    pub fn pairs(&self) -> Vec<(&'static str, Option<String>)> {
        let mut pairs = self.page.pairs();
        pairs.extend([
            ("q", self.q.clone()),
            ("priority", self.priority.clone()),
            ("status", self.status.clone()),
            ("assignee_id", self.assignee_id.clone()),
            ("sort_by", self.sort_by.clone()),
            ("sort_order", self.sort_order.clone()),
        ]);
        pairs
    }
}
pub fn query(path: &str, pairs: Vec<(&str, Option<String>)>) -> String {
    let mut path = path.to_string();
    for (key, value) in pairs {
        if let Some(value) = value {
            path.push(if path.contains('?') { '&' } else { '?' });
            path.push_str(key);
            path.push('=');
            path.push_str(&enc(&value));
        }
    }
    path
}

#[derive(Subcommand)]
pub enum WorkCommand {
    /// Справочник статусов
    Statuses,
    /// Справочник типов задач
    IssueTypes,
    /// Переходы из текущего статуса задачи (окончательная проверка на сервере)
    Transitions {
        #[arg(long)]
        issue: String,
    },
    /// Корзина проекта
    Trash {
        #[arg(long)]
        project_key: String,
        #[command(flatten)]
        page: Page,
    },
    Link {
        #[command(subcommand)]
        command: LinkCommand,
    },
    Attachment {
        #[command(subcommand)]
        command: AttachmentCommand,
    },
    Field {
        #[command(subcommand)]
        command: FieldCommand,
    },
    Worklog {
        #[command(subcommand)]
        command: WorklogCommand,
    },
}
#[derive(Subcommand)]
pub enum LinkCommand {
    List {
        #[arg(long)]
        issue: String,
    },
    Add {
        #[arg(long)]
        issue: String,
        #[arg(long)]
        target_key: String,
        #[arg(long)]
        link_type: String,
    },
    Delete {
        id: String,
    },
}
#[derive(Subcommand)]
pub enum AttachmentCommand {
    List {
        #[arg(long)]
        issue: String,
    },
    Upload {
        #[arg(long)]
        issue: String,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        content_type: Option<String>,
    },
    Download {
        id: String,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        overwrite: bool,
    },
    Delete {
        id: String,
    },
}
#[derive(Subcommand)]
pub enum FieldCommand {
    List {
        #[arg(long)]
        project_key: String,
    },
    Values {
        #[arg(long)]
        issue: String,
    },
    Set {
        #[arg(long)]
        issue: String,
        #[arg(long)]
        field: String,
        #[arg(
            long,
            required_unless_present = "from_file",
            conflicts_with = "from_file"
        )]
        value: Option<String>,
        #[arg(long)]
        from_file: Option<String>,
    },
}
#[derive(Subcommand)]
pub enum WorklogCommand {
    List {
        #[arg(long)]
        issue: String,
        #[command(flatten)]
        page: Page,
    },
    Create {
        #[arg(long)]
        issue: String,
        #[arg(long)]
        started_at: String,
        #[arg(long)]
        duration_seconds: i64,
        #[arg(long, conflicts_with = "from_file")]
        description: Option<String>,
        #[arg(long)]
        from_file: Option<String>,
    },
    Update {
        id: String,
        #[arg(long)]
        started_at: Option<String>,
        #[arg(long)]
        duration_seconds: Option<i64>,
        #[arg(long, conflicts_with = "from_file")]
        description: Option<String>,
        #[arg(long)]
        from_file: Option<String>,
    },
    Delete {
        id: String,
    },
}

#[derive(Subcommand)]
pub enum SdlcCommand {
    /// Propose or reconcile a critical business question without advancing the task
    CriticalBlocker {
        #[command(subcommand)]
        command: critical_commands::CriticalBlockerCommand,
    },
    /// Read the current authoritative single-task assignment and lifecycle
    Lifecycle {
        #[arg(long)]
        task: Uuid,
    },
    /// Propose a typed role result; only Fleet accepts it after runtime terminal
    Result {
        #[command(subcommand)]
        command: pm_commands::DeliveryResultCommand,
    },
    /// Read existing project defaults or immutable task routing
    Routing {
        #[command(subcommand)]
        command: pm_commands::RoutingCommand,
    },
    /// Read the actual owner confirmation receipt without owner write authority
    Confirmation {
        #[command(subcommand)]
        command: pm_commands::ConfirmationCommand,
    },
    /// Create or reconcile the first-message owner Draft
    Draft {
        #[command(subcommand)]
        command: pm_commands::DraftCommand,
    },
    /// Use existing Tracker PM reservation, lease and completion contracts
    Pm {
        #[command(subcommand)]
        command: pm_commands::PmCommand,
    },
    /// Read server-derived SDLC task context by canonical issue UUID
    Context {
        #[arg(long)]
        task: Uuid,
    },
    /// Read immutable PM draft input by canonical issue UUID
    Input {
        #[arg(long)]
        task: Uuid,
    },
    /// List clarification questions by canonical issue UUID
    Questions {
        #[arg(long)]
        task: Uuid,
    },
    /// Read current requirements revision by canonical issue UUID
    Requirements {
        #[arg(long)]
        task: Uuid,
    },
    /// Read frozen Analysis intent by canonical issue UUID
    Intent {
        #[arg(long)]
        task: Uuid,
    },
    /// Use Tracker-authoritative Analysis producer/readback endpoints
    Analysis {
        #[command(subcommand)]
        command: SdlcAnalysisCommand,
    },
    /// Publish PM clarification questions
    Question {
        #[command(subcommand)]
        command: SdlcQuestionCommand,
    },
    /// Publish PM requirements revisions
    Revision {
        #[command(subcommand)]
        command: SdlcRevisionCommand,
    },
}

#[derive(Subcommand)]
pub enum SdlcAnalysisCommand {
    /// Read Fleet configuration preflight for the prepared Analysis assignment
    Preflight {
        #[arg(long)]
        task: Uuid,
    },
    /// Claim, heartbeat or read the prepared Analysis reservation
    Reservation {
        #[command(subcommand)]
        command: SdlcAnalysisReservationCommand,
    },
    /// Submit or read the Tracker-authoritative Analysis terminal result
    Result {
        #[command(subcommand)]
        command: SdlcAnalysisResultCommand,
    },
}

#[derive(Subcommand)]
pub enum SdlcAnalysisReservationCommand {
    /// Read current Analysis reservation state
    Read {
        #[arg(long)]
        task: Uuid,
    },
    /// Reserve Analysis capacity with a strict JSON payload from a file or stdin
    Reserve {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
    /// Renew Analysis reservation with a strict JSON payload from a file or stdin
    Heartbeat {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
    /// Read a retained Analysis reservation operation by idempotency key
    Operation {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        key: String,
    },
}

#[derive(Subcommand)]
pub enum SdlcAnalysisResultCommand {
    /// Submit an Analysis terminal result JSON payload from a file or stdin
    Submit {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
    /// Read a retained Analysis result operation by idempotency key
    Operation {
        #[arg(long)]
        task: Uuid,
        #[arg(long)]
        key: String,
    },
}

#[derive(Subcommand)]
pub enum SdlcQuestionCommand {
    /// Publish a clarification question JSON payload from a file or stdin
    #[command(
        after_long_help = r#"Read `task-tracker sdlc context --task TASK_UUID` first. Copy the current
assignment_id, execution_id, agent_id and assignment version into fence.
The current unconfirmed requirements revision must already exist: publish a
working revision with `sdlc revision publish` before asking a formal question.
Continue free-form brainstorming until enough substantive product facts are known.
Never publish a placeholder merely to obtain a revision for a question.
Use its actual requirement_revision, not the illustrative value below.
Generate stable request/question/option UUIDs once for this logical question.
Use expected_question_version=null for a new question; updates use its current
version and request_id. Repeat an uncertain operation with the same key/payload.
The field is text, not question. Options are objects, not strings. Mode is
single, multiple or text; text requires options=[] and no recommendation.
For configured-cli-v1 omit checkpoint_id. Legacy mode still requires the real
native checkpoint_id; never invent a checkpoint or copy the sample identities.
PM cannot answer or confirm on behalf of the human owner.
Pipe the JSON into `task-tracker sdlc question publish --task TASK_UUID --from-file -`.

JSON example (configured-cli-v1):
{
  "fence": {
    "assignment_id": "22222222-2222-4222-8222-222222222222",
    "execution_id": "33333333-3333-4333-8333-333333333333",
    "agent_id": "44444444-4444-4444-8444-444444444444",
    "assignment_version": 1
  },
  "request_id": "55555555-5555-4555-8555-555555555555",
  "question_id": "66666666-6666-4666-8666-666666666666",
  "expected_question_version": null,
  "requirement_revision": 1,
  "requirement_reference": null,
  "text": "Which audience should the first release serve?",
  "rationale": "The audience determines the initial scope.",
  "required": true,
  "mode": "single",
  "options": [
    {
      "id": "77777777-7777-4777-8777-777777777777",
      "label": "Internal users",
      "consequences": "Prioritize existing employee workflows.",
      "is_custom": false
    },
    {
      "id": "88888888-8888-4888-8888-888888888888",
      "label": "External users",
      "consequences": "Include public onboarding in the initial scope.",
      "is_custom": false
    }
  ],
  "recommended_option_id": "77777777-7777-4777-8777-777777777777",
  "idempotency_key": "example-question-1"
}"#
    )]
    Publish {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
}

#[derive(Subcommand)]
pub enum SdlcRevisionCommand {
    /// Publish a requirements revision JSON payload from a file or stdin
    #[command(
        after_long_help = r#"Read `task-tracker sdlc context --task TASK_UUID` first and copy the current
assignment fence. All identities and document content below are illustrative.
For the first working revision use expected_requirement_revision=null; for an
update use the current revision from context. Keep the same key/payload on an
uncertain outcome. The server returns the new revision and content_hash.
A working revision may precede formal questions, but must contain real product
requirements grounded in known facts, not a PM-process document.
Continue free-form brainstorming until enough substantive product facts are known.
Never publish a placeholder merely to obtain a revision for a question.
The product below is a syntax example, not a decision for your task.
Required open questions must be resolved before publishing a later revision.
Document checklist/prerequisites
are business content, not fabricated infrastructure verification receipts.
This command publishes requirements; it does not confirm them. Confirmation is
owner-only: PM must not call revision confirm or fabricate human consent.
Pipe the JSON into `task-tracker sdlc revision publish --task TASK_UUID --from-file -`.

JSON example (configured-cli-v1):
{
  "fence": {
    "assignment_id": "22222222-2222-4222-8222-222222222222",
    "execution_id": "33333333-3333-4333-8333-333333333333",
    "agent_id": "44444444-4444-4444-8444-444444444444",
    "assignment_version": 1
  },
  "expected_requirement_revision": null,
  "document": {
    "goal": "Keep a personal browser task list without losing tasks after reload.",
    "scope": [
      "Create a task with user-entered text.",
      "Mark an existing task as completed.",
      "Persist task text and completion state across browser reloads."
    ],
    "exclusions": [],
    "scenarios": [
      "The user enters a task and sees it in the list.",
      "The user completes one task while other tasks remain unchanged.",
      "The user reloads the page and continues using the saved list."
    ],
    "acceptance_criteria": [
      "Creating a task adds exactly one incomplete item with the entered text.",
      "Completing a task marks that item completed without removing other items.",
      "Reloading restores the same task texts and completion states."
    ],
    "constraints": [],
    "dependencies": [],
    "assumptions": [],
    "checklist": [
      "Task text remains visible after completion.",
      "Reload preserves both incomplete and completed tasks."
    ],
    "prerequisites": ["The browser permits storage for this site."]
  },
  "idempotency_key": "example-revision-1"
}"#
    )]
    Publish {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
    /// Confirm an exact revision using the existing owner-only endpoint
    Confirm {
        #[arg(long)]
        task: Uuid,
        #[arg(long, value_parser = clap::value_parser!(i64).range(1..=9_007_199_254_740_991))]
        revision: i64,
        #[arg(long, value_name = "PATH|-")]
        from_file: String,
    },
}

pub async fn issue_id(api: &Api, identifier: &str) -> Result<String> {
    let value = api
        .get(&format!("/api/v1/issues/{}", enc(identifier)))
        .await?;
    Ok(value
        .get("id")
        .and_then(Value::as_str)
        .context("В ответе задачи нет id")?
        .to_owned())
}

// Attachment machine grants do not authorize generic issue GET. Keys retain normal lookup.
async fn attachment_issue_id(api: &Api, identifier: &str) -> Result<String> {
    if let Some(id) = canonical_attachment_issue_id(identifier) {
        return Ok(id);
    }
    issue_id(api, identifier).await
}

fn canonical_attachment_issue_id(identifier: &str) -> Option<String> {
    let id: uuid::Uuid = identifier.parse().ok()?;
    (!id.is_nil() && id.to_string() == identifier).then(|| id.to_string())
}

fn sdlc_path(task: Uuid, suffix: &str) -> String {
    format!("/api/v1/issues/{task}/sdlc{suffix}")
}

fn json_input(path: &str) -> Result<Value> {
    let text = support::read_text(path)?;
    serde_json::from_str(&text).context("SDLC payload должен быть JSON")
}

pub async fn execute_sdlc(api: &Api, command: SdlcCommand) -> Result<Value> {
    match command {
        SdlcCommand::CriticalBlocker { command } => {
            critical_commands::critical_blocker(api, command).await
        }
        SdlcCommand::Lifecycle { task } => api.get(&sdlc_path(task, "/lifecycle")).await,
        SdlcCommand::Result { command } => pm_commands::delivery_result(api, command).await,
        SdlcCommand::Routing { command } => pm_commands::routing(api, command).await,
        SdlcCommand::Confirmation { command } => pm_commands::confirmation(api, command).await,
        SdlcCommand::Draft { command } => pm_commands::draft(api, command).await,
        SdlcCommand::Pm { command } => pm_commands::pm(api, command).await,
        SdlcCommand::Context { task } => api.get(&sdlc_path(task, "/context")).await,
        SdlcCommand::Input { task } => api.get(&sdlc_path(task, "/pm-draft-input")).await,
        SdlcCommand::Questions { task } => api.get(&sdlc_path(task, "/clarifications")).await,
        SdlcCommand::Requirements { task } => api.get(&sdlc_path(task, "/requirements")).await,
        SdlcCommand::Intent { task } => api.get(&sdlc_path(task, "/analysis-intent")).await,
        SdlcCommand::Analysis { command } => match command {
            SdlcAnalysisCommand::Preflight { task } => {
                api.get(&sdlc_path(
                    task,
                    "/analysis-reservation/configuration-preflight",
                ))
                .await
            }
            SdlcAnalysisCommand::Reservation { command } => match command {
                SdlcAnalysisReservationCommand::Read { task } => {
                    api.get(&sdlc_path(task, "/analysis-reservation")).await
                }
                SdlcAnalysisReservationCommand::Reserve { task, from_file } => {
                    api.post(
                        &sdlc_path(task, "/analysis-reservation"),
                        json_input(&from_file)?,
                    )
                    .await
                }
                SdlcAnalysisReservationCommand::Heartbeat { task, from_file } => {
                    api.post(
                        &sdlc_path(task, "/analysis-reservation/heartbeat"),
                        json_input(&from_file)?,
                    )
                    .await
                }
                SdlcAnalysisReservationCommand::Operation { task, key } => {
                    api.get(&sdlc_path(
                        task,
                        &format!("/analysis-reservation/operations/{}", enc(&key)),
                    ))
                    .await
                }
            },
            SdlcAnalysisCommand::Result { command } => match command {
                SdlcAnalysisResultCommand::Submit { task, from_file } => {
                    api.post(
                        &sdlc_path(task, "/analysis-result"),
                        json_input(&from_file)?,
                    )
                    .await
                }
                SdlcAnalysisResultCommand::Operation { task, key } => {
                    api.get(&sdlc_path(
                        task,
                        &format!("/analysis-result/operations/{}", enc(&key)),
                    ))
                    .await
                }
            },
        },
        SdlcCommand::Question { command } => match command {
            SdlcQuestionCommand::Publish { task, from_file } => {
                api.post(&sdlc_path(task, "/clarifications"), json_input(&from_file)?)
                    .await
            }
        },
        SdlcCommand::Revision { command } => match command {
            SdlcRevisionCommand::Confirm {
                task,
                revision,
                from_file,
            } => {
                api.post(
                    &sdlc_path(task, &format!("/requirements/{revision}/confirm")),
                    json_input(&from_file)?,
                )
                .await
            }
            SdlcRevisionCommand::Publish { task, from_file } => {
                api.post(&sdlc_path(task, "/requirements"), json_input(&from_file)?)
                    .await
            }
        },
    }
}

fn request(api: &Api, method: Method, path: &str) -> Result<reqwest::RequestBuilder> {
    api.client
        .request(
            method,
            if api.base_has_version {
                path.strip_prefix("/api/v1").unwrap_or(path)
            } else {
                path
            },
        )
        .map_err(Into::into)
}
pub async fn execute(api: &Api, command: WorkCommand) -> Result<Value> {
    match command {
        WorkCommand::Statuses => api.get("/api/v1/statuses").await,
        WorkCommand::IssueTypes => api.get("/api/v1/issue-types").await,
        WorkCommand::Transitions { issue } => {
            let issue = api.get(&format!("/api/v1/issues/{}", enc(&issue))).await?;
            let transitions = api.get("/api/v1/transitions").await?;
            let status = issue.get("status_id").context("В ответе задачи нет status_id")?;
            let items = transitions.as_array().context("Некорректный список переходов")?.iter().filter(|t| t.get("from_status_id") == Some(status)).cloned().collect::<Vec<_>>();
            Ok(json!({"issue_id":issue["id"],"status_id":status,"transitions":items}))
        }
        WorkCommand::Trash { project_key, page } => api.get(&query(&format!("/api/v1/projects/{}/trash",enc(&project_key)),page.pairs())).await,
        WorkCommand::Link { command } => match command {
            LinkCommand::List { issue } => api.get(&format!("/api/v1/issues/{}/links",enc(&issue_id(api,&issue).await?))).await,
            LinkCommand::Add { issue, target_key, link_type } => api.post(&format!("/api/v1/issues/{}/links",enc(&issue_id(api,&issue).await?)),json!({"target_key":target_key,"link_type":link_type})).await,
            LinkCommand::Delete { id } => api.delete(&format!("/api/v1/issue-links/{}",enc(&id))).await,
        },
        WorkCommand::Attachment { command } => match command {
            AttachmentCommand::List { issue } => api.get(&format!("/api/v1/issues/{}/attachments",enc(&attachment_issue_id(api,&issue).await?))).await,
            AttachmentCommand::Delete { id } => api.delete(&format!("/api/v1/attachments/{}",enc(&id))).await,
            AttachmentCommand::Download { id, out, overwrite } => {
                support::check_destination(&out,overwrite)?;
                let data = support::bytes_response(request(api,Method::GET,&format!("/api/v1/attachments/{}/download",enc(&id)))?,&api.secrets).await?;
                support::save_download(&out,&data,overwrite)?;
                Ok(json!({"status":"ok","path":out,"size_bytes":data.len()}))
            }
            AttachmentCommand::Upload { issue, file, content_type } => {
                let id = attachment_issue_id(api,&issue).await?;
                let content_type=content_type.unwrap_or_else(||mime_guess::from_path(&file).first_or_octet_stream().to_string());
                let part = multipart::Part::bytes(std::fs::read(&file)?).file_name(file.file_name().context("Нет имени файла")?.to_string_lossy().into_owned()).mime_str(&content_type)?;
                support::json_response(request(api,Method::POST,&format!("/api/v1/issues/{}/attachments",enc(&id)))?.multipart(multipart::Form::new().part("file",part)),&api.secrets).await
            }
        },
        WorkCommand::Field { command } => match command {
            FieldCommand::List { project_key } => api.get(&format!("/api/v1/projects/{}/custom-fields",enc(&project_key))).await,
            FieldCommand::Values { issue } => api.get(&format!("/api/v1/issues/{}/custom-fields",enc(&issue_id(api,&issue).await?))).await,
            FieldCommand::Set { issue, field, value, from_file } => {
                let text = support::text_input(value,from_file)?.context("Значение обязательно")?;
                let value: Value = serde_json::from_str(&text).context("Значение поля должно быть JSON")?;
                api.put(&format!("/api/v1/issues/{}/custom-fields/{}/value",enc(&issue_id(api,&issue).await?),enc(&field)),json!({"value":value})).await
            }
        },
        WorkCommand::Worklog { command } => match command {
            WorklogCommand::List { issue, page } => api.get(&query(&format!("/api/v1/issues/{}/worklogs",enc(&issue_id(api,&issue).await?)),page.pairs())).await,
            WorklogCommand::Create { issue, started_at, duration_seconds, description, from_file } => api.post(&format!("/api/v1/issues/{}/worklogs",enc(&issue_id(api,&issue).await?)),json!({"started_at":started_at,"duration_seconds":duration_seconds,"description":support::text_input(description,from_file)?})).await,
            WorklogCommand::Update { id, started_at, duration_seconds, description, from_file } => {
                let mut body=json!({});
                if let Some(v)=started_at {body["started_at"]=json!(v);}
                if let Some(v)=duration_seconds {body["duration_seconds"]=json!(v);}
                if let Some(v)=support::text_input(description,from_file)? {body["description"]=json!(v);}
                api.patch(&format!("/api/v1/worklogs/{}",enc(&id)),body).await
            }
            WorklogCommand::Delete { id } => api.delete(&format!("/api/v1/worklogs/{}",enc(&id))).await,
        }
    }
}

#[cfg(test)]
mod attachment_tests {
    use super::*;
    #[test]
    fn only_canonical_non_nil_uuid_bypasses_generic_issue_lookup() {
        let id = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
        assert_eq!(canonical_attachment_issue_id(id), Some(id.into()));
        for identifier in [
            "SDLC-7",
            "not-a-uuid",
            "00000000-0000-0000-0000-000000000000",
            "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE",
            "aaaaaaaabbbb4ccc8dddeeeeeeeeeeee",
        ] {
            assert_eq!(canonical_attachment_issue_id(identifier), None);
        }
    }
}
