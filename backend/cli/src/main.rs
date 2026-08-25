use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use reqwest::Client;
use serde_json::{Value, json};
use std::process::ExitCode;

// ─── Top-level CLI ───────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "task-tracker")]
#[command(about = "Task Tracker CLI — manage projects, issues, boards, sprints and more")]
#[command(version)]
struct Cli {
    /// API base URL
    #[arg(
        long,
        env = "TASKTRACKER_API_URL",
        default_value = "http://localhost:19876"
    )]
    api_url: String,

    /// Bearer auth token (or set TASKTRACKER_TOKEN env)
    #[arg(long, env = "TASKTRACKER_TOKEN")]
    token: Option<String>,

    /// Output format: json | table | compact
    #[arg(long, env = "TASKTRACKER_OUTPUT", default_value = "json")]
    output: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authentication
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
    /// Project management
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },
    /// Issue management
    Issue {
        #[command(subcommand)]
        command: IssueCommands,
    },
    /// Board / Kanban
    Board {
        #[command(subcommand)]
        command: BoardCommands,
    },
    /// Sprint management
    Sprint {
        #[command(subcommand)]
        command: SprintCommands,
    },
    /// Comments
    Comment {
        #[command(subcommand)]
        command: CommentCommands,
    },
    /// Labels
    Label {
        #[command(subcommand)]
        command: LabelCommands,
    },
    /// Search (basic + JQL)
    Search {
        #[command(subcommand)]
        command: SearchCommands,
    },
    /// Notifications
    Notification {
        #[command(subcommand)]
        command: NotificationCommands,
    },
    /// Reports
    Report {
        #[command(subcommand)]
        command: ReportCommands,
    },
    /// Admin (system admin only)
    Admin {
        #[command(subcommand)]
        command: AdminCommands,
    },
    /// Members
    Member {
        #[command(subcommand)]
        command: MemberCommands,
    },
}

// ─── Auth ────────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum AuthCommands {
    /// Register a new user
    Register {
        #[arg(long)]
        email: String,
        #[arg(long)]
        username: String,
        #[arg(long)]
        display_name: String,
        #[arg(long)]
        password: String,
    },
    /// Login and get access token
    Login {
        #[arg(long)]
        email: String,
        #[arg(long)]
        password: String,
    },
    /// Logout (invalidates session)
    Logout,
    /// Show current user info
    Whoami,
}

// ─── Project ─────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum ProjectCommands {
    /// List all projects
    List,
    /// Create a new project
    Create {
        #[arg(long)]
        key: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: Option<String>,
    },
    /// Get project details
    Get { key: String },
    /// Update project
    Update {
        key: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete project
    Delete { key: String },
}

// ─── Issue ───────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum IssueCommands {
    /// Create a new issue
    Create {
        #[arg(long)]
        project_key: String,
        #[arg(long)]
        summary: String,
        #[arg(long, default_value = "task")]
        issue_type: String,
        #[arg(long, default_value = "medium")]
        priority: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        assignee_id: Option<String>,
        #[arg(long)]
        status_id: Option<String>,
    },
    /// Get issue details
    Get { key: String },
    /// Update issue
    Update {
        key: String,
        #[arg(long)]
        summary: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        priority: Option<String>,
        #[arg(long)]
        status_id: Option<String>,
        #[arg(long)]
        assignee_id: Option<String>,
    },
    /// Delete issue (soft-delete)
    Delete { key: String },
    /// Transition issue status
    Transition {
        key: String,
        #[arg(long)]
        to: String,
    },
    /// List issues for a project
    List {
        #[arg(long)]
        project_key: String,
    },
}

// ─── Board ───────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum BoardCommands {
    /// Get board (kanban columns + issues)
    Get {
        #[arg(long)]
        project_key: String,
    },
    /// Get backlog
    Backlog {
        #[arg(long)]
        project_key: String,
    },
    /// Move issue to a different column
    Move {
        #[arg(long)]
        project_key: String,
        #[arg(long)]
        issue_id: String,
        #[arg(long)]
        status_id: String,
    },
}

// ─── Sprint ─────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum SprintCommands {
    /// List sprints for a project
    List {
        #[arg(long)]
        project_key: String,
    },
    /// Create a sprint
    Create {
        #[arg(long)]
        project_key: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        goal: Option<String>,
    },
    /// Get sprint details
    Get { id: String },
    /// Start a sprint
    Start { id: String },
    /// Close a sprint
    Close { id: String },
    /// Move issue to sprint
    AddIssue {
        #[arg(long)]
        sprint_id: String,
        #[arg(long)]
        issue_id: String,
    },
    /// Remove issue from sprint
    RemoveIssue {
        #[arg(long)]
        sprint_id: String,
        #[arg(long)]
        issue_id: String,
    },
}

// ─── Comment ─────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum CommentCommands {
    /// List comments for an issue
    List { issue_id: String },
    /// Add a comment
    Add {
        #[arg(long)]
        issue_id: String,
        #[arg(long)]
        body: String,
    },
    /// Update a comment
    Update {
        #[arg(long)]
        comment_id: String,
        #[arg(long)]
        body: String,
    },
    /// Delete a comment
    Delete { comment_id: String },
}

// ─── Label ───────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum LabelCommands {
    /// List labels for a project
    List { project_key: String },
    /// Create a label
    Create {
        #[arg(long)]
        project_key: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        color: Option<String>,
    },
    /// Delete a label
    Delete { label_id: String },
    /// Attach a label to an issue
    Attach {
        #[arg(long)]
        issue_id: String,
        #[arg(long)]
        label_id: String,
    },
    /// Detach a label from an issue
    Detach {
        #[arg(long)]
        issue_id: String,
        #[arg(long)]
        label_id: String,
    },
}

// ─── Search ──────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum SearchCommands {
    /// Global search across all projects
    Global {
        #[arg(long)]
        q: String,
        #[arg(long)]
        project_key: Option<String>,
        #[arg(long)]
        priority: Option<String>,
        #[arg(long)]
        assignee_id: Option<String>,
    },
    /// Execute a JQL query
    Jql {
        /// JQL expression, e.g. 'project = "TT" AND status = "Open"'
        query: String,
    },
}

// ─── Notification ───────────────────────────────────────────────────

#[derive(Subcommand)]
enum NotificationCommands {
    /// List unread notifications
    List,
    /// Mark a notification as read
    Read { id: String },
    /// Mark all notifications as read
    ReadAll,
    /// Get notification settings
    Settings,
    /// Update notification settings
    UpdateSettings {
        #[arg(long)]
        email_frequency: Option<String>,
        #[arg(long)]
        notify_own_changes: Option<bool>,
    },
}

// ─── Report ─────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum ReportCommands {
    /// Velocity report (sprints)
    Velocity {
        #[arg(long)]
        project_key: String,
        #[arg(long, default_value = "5")]
        count: u32,
    },
    /// Burndown chart for a sprint
    Burndown {
        #[arg(long)]
        sprint_id: String,
    },
    /// Cumulative flow diagram
    CumulativeFlow {
        #[arg(long)]
        project_key: String,
    },
    /// Control chart (cycle time)
    ControlChart {
        #[arg(long)]
        project_key: String,
    },
}

// ─── Admin ──────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum AdminCommands {
    /// List all users
    ListUsers,
    /// Create a user (admin only)
    CreateUser {
        #[arg(long)]
        email: String,
        #[arg(long)]
        username: String,
        #[arg(long)]
        display_name: String,
        #[arg(long)]
        password: String,
        #[arg(long, default_value = "false")]
        is_admin: bool,
    },
    /// Activate/deactivate a user
    ToggleUser {
        user_id: String,
        #[arg(long)]
        active: bool,
    },
    /// List audit log entries
    AuditLog {
        #[arg(long, default_value = "20")]
        limit: u32,
    },
    /// List system settings
    Settings,
    /// Update a system setting
    SetSetting {
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
    },
}

// ─── Member ─────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum MemberCommands {
    /// List project members
    List { project_key: String },
    /// Add a member to a project
    Add {
        #[arg(long)]
        project_key: String,
        #[arg(long)]
        user_id: String,
        #[arg(long, default_value = "member")]
        role: String,
    },
    /// Remove a member from a project
    Remove {
        #[arg(long)]
        project_key: String,
        #[arg(long)]
        user_id: String,
    },
}

// ─── API client ─────────────────────────────────────────────────────

struct Api {
    client: Client,
    base: String,
    token: Option<String>,
}

impl Api {
    fn new(base: String, token: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base: base.trim_end_matches('/').to_string(),
            token,
        }
    }

    fn auth_header(&self) -> Result<String> {
        match &self.token {
            Some(t) => Ok(format!("Bearer {}", t)),
            None => bail!("not authenticated: pass --token or TASKTRACKER_TOKEN"),
        }
    }

    async fn get(&self, path: &str) -> Result<Value> {
        self.request("GET", path, Value::Null).await
    }

    async fn post(&self, path: &str, payload: Value) -> Result<Value> {
        self.request("POST", path, payload).await
    }

    async fn patch(&self, path: &str, payload: Value) -> Result<Value> {
        self.request("PATCH", path, payload).await
    }

    async fn put(&self, path: &str, payload: Value) -> Result<Value> {
        self.request("PUT", path, payload).await
    }

    async fn delete(&self, path: &str) -> Result<Value> {
        self.request("DELETE", path, Value::Null).await
    }

    async fn request(&self, method: &str, path: &str, payload: Value) -> Result<Value> {
        let url = format!("{}{}", self.base, path);
        let mut req = match method {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url).json(&payload),
            "PATCH" => self.client.patch(&url).json(&payload),
            "PUT" => self.client.put(&url).json(&payload),
            "DELETE" => self.client.delete(&url),
            _ => bail!("unsupported method: {}", method),
        };
        if !path.ends_with("/auth/register") && !path.ends_with("/auth/login") {
            req = req.bearer_auth(self.auth_header()?);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let body: Value = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::String(text.clone()))
        };
        if !status.is_success() {
            bail!("API error {}: {}", status, body);
        }
        Ok(body)
    }
}

// ─── Output helpers ──────────────────────────────────────────────────

fn print_output(output: &str, value: &Value) {
    match output {
        "compact" => print_compact(value),
        "table" => print_table(value),
        _ => println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        ),
    }
}

fn print_compact(value: &Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                if let Some(obj) = item.as_object() {
                    let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let key = obj.get("key").and_then(|v| v.as_str()).unwrap_or("");
                    let name = obj
                        .get("name")
                        .and_then(|v| v.as_str())
                        .or_else(|| obj.get("summary").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    let status = obj.get("status").and_then(|v| v.as_str()).unwrap_or("");
                    println!("{id} | {key} | {name} | {status}");
                }
            }
        }
        Value::Object(obj) => {
            let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let key = obj.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .or_else(|| obj.get("summary").and_then(|v| v.as_str()))
                .unwrap_or("");
            println!("{id} | {key} | {name}");
        }
        _ => println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        ),
    }
}

fn print_table(value: &Value) {
    match value {
        Value::Array(items) => {
            // Collect keys from first item for headers
            if let Some(first) = items.first().and_then(|v| v.as_object()) {
                let keys: Vec<&str> = first.keys().map(|k| k.as_str()).collect();
                println!("{}", keys.join("\t"));
                for item in items {
                    if let Some(obj) = item.as_object() {
                        let row: Vec<String> = keys
                            .iter()
                            .map(|k| {
                                obj.get(*k)
                                    .map(|v| match v {
                                        Value::String(s) => s.clone(),
                                        Value::Number(n) => n.to_string(),
                                        Value::Bool(b) => b.to_string(),
                                        _ => v.to_string(),
                                    })
                                    .unwrap_or_default()
                            })
                            .collect();
                        println!("{}", row.join("\t"));
                    }
                }
            }
        }
        _ => println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        ),
    }
}

// ─── Main ───────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(e) = run(cli).await {
        eprintln!("error: {:#}", e);
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn enc(s: &str) -> String {
    urlencoding::encode(s).to_string()
}

async fn run(cli: Cli) -> Result<()> {
    let api = Api::new(cli.api_url.clone(), cli.token.clone());
    let out = cli.output.as_str();

    match cli.command {
        // ── Auth ──
        Commands::Auth { command } => match command {
            AuthCommands::Register {
                email,
                username,
                display_name,
                password,
            } => {
                let body = api.post("/api/v1/auth/register", json!({
                    "email": email, "username": username, "display_name": display_name, "password": password
                })).await?;
                print_output(out, &body);
            }
            AuthCommands::Login { email, password } => {
                let body = api
                    .post(
                        "/api/v1/auth/login",
                        json!({
                            "email": email, "password": password
                        }),
                    )
                    .await?;
                print_output(out, &body);
            }
            AuthCommands::Logout => {
                api.post("/api/v1/auth/logout", json!({})).await?;
                println!("logged out");
            }
            AuthCommands::Whoami => {
                let body = api.get("/api/v1/users/me").await?;
                print_output(out, &body);
            }
        },

        // ── Project ──
        Commands::Project { command } => match command {
            ProjectCommands::List => {
                let body = api.get("/api/v1/projects").await?;
                print_output(out, &body);
            }
            ProjectCommands::Create {
                key,
                name,
                description,
            } => {
                let body = api
                    .post(
                        "/api/v1/projects",
                        json!({
                            "key": key, "name": name, "description": description
                        }),
                    )
                    .await?;
                print_output(out, &body);
            }
            ProjectCommands::Get { key } => {
                let body = api.get(&format!("/api/v1/projects/{}", enc(&key))).await?;
                print_output(out, &body);
            }
            ProjectCommands::Update {
                key,
                name,
                description,
            } => {
                let mut payload = json!({});
                if let Some(n) = name {
                    payload["name"] = Value::String(n);
                }
                if let Some(d) = description {
                    payload["description"] = Value::String(d);
                }
                let body = api
                    .patch(&format!("/api/v1/projects/{}", enc(&key)), payload)
                    .await?;
                print_output(out, &body);
            }
            ProjectCommands::Delete { key } => {
                api.delete(&format!("/api/v1/projects/{}", enc(&key)))
                    .await?;
                println!("project {} deleted", key);
            }
        },

        // ── Issue ──
        Commands::Issue { command } => match command {
            IssueCommands::Create {
                project_key,
                summary,
                issue_type,
                priority,
                description,
                assignee_id,
                status_id,
            } => {
                let project = api
                    .get(&format!("/api/v1/projects/{}", enc(&project_key)))
                    .await?;
                let project_id = project["id"].as_str().context("project missing id")?;
                let mut payload = json!({
                    "project_id": project_id,
                    "summary": summary,
                    "issue_type": issue_type,
                    "priority": priority,
                    "reporter_id": project_id, // will be overridden by auth
                });
                if let Some(d) = description {
                    payload["description"] = Value::String(d);
                }
                if let Some(a) = assignee_id {
                    payload["assignee_id"] = Value::String(a);
                }
                if let Some(s) = status_id {
                    payload["status_id"] = Value::String(s);
                }
                let body = api.post("/api/v1/issues", payload).await?;
                print_output(out, &body);
            }
            IssueCommands::Get { key } => {
                let body = api.get(&format!("/api/v1/issues/{}", enc(&key))).await?;
                print_output(out, &body);
            }
            IssueCommands::Update {
                key,
                summary,
                description,
                priority,
                status_id,
                assignee_id,
            } => {
                let mut payload = json!({});
                if let Some(s) = summary {
                    payload["summary"] = Value::String(s);
                }
                if let Some(d) = description {
                    payload["description"] = Value::String(d);
                }
                if let Some(p) = priority {
                    payload["priority"] = Value::String(p);
                }
                if let Some(s) = status_id {
                    payload["status_id"] = Value::String(s);
                }
                if let Some(a) = assignee_id {
                    payload["assignee_id"] = Value::String(a);
                }
                let body = api
                    .patch(&format!("/api/v1/issues/{}", enc(&key)), payload)
                    .await?;
                print_output(out, &body);
            }
            IssueCommands::Delete { key } => {
                api.delete(&format!("/api/v1/issues/{}", enc(&key))).await?;
                println!("issue {} deleted", key);
            }
            IssueCommands::Transition { key, to } => {
                let body = api
                    .post(
                        &format!("/api/v1/issues/{}/transition", enc(&key)),
                        json!({
                            "target_status_id": to
                        }),
                    )
                    .await?;
                print_output(out, &body);
            }
            IssueCommands::List { project_key } => {
                let body = api
                    .get(&format!("/api/v1/issues?project_key={}", enc(&project_key)))
                    .await?;
                print_output(out, &body);
            }
        },

        // ── Board ──
        Commands::Board { command } => match command {
            BoardCommands::Get { project_key } => {
                let body = api
                    .get(&format!("/api/v1/projects/{}/board", enc(&project_key)))
                    .await?;
                print_output(out, &body);
            }
            BoardCommands::Backlog { project_key } => {
                let body = api
                    .get(&format!("/api/v1/projects/{}/backlog", enc(&project_key)))
                    .await?;
                print_output(out, &body);
            }
            BoardCommands::Move {
                project_key,
                issue_id,
                status_id,
            } => {
                let body = api
                    .post(
                        &format!("/api/v1/projects/{}/board/move", enc(&project_key)),
                        json!({
                            "issue_id": issue_id, "status_id": status_id
                        }),
                    )
                    .await?;
                print_output(out, &body);
            }
        },

        // ── Sprint ──
        Commands::Sprint { command } => match command {
            SprintCommands::List { project_key } => {
                let project = api
                    .get(&format!("/api/v1/projects/{}", enc(&project_key)))
                    .await?;
                let pid = project["id"].as_str().context("project missing id")?;
                let body = api
                    .get(&format!("/api/v1/sprints?project_id={}", enc(pid)))
                    .await?;
                print_output(out, &body);
            }
            SprintCommands::Create {
                project_key,
                name,
                goal,
            } => {
                let project = api
                    .get(&format!("/api/v1/projects/{}", enc(&project_key)))
                    .await?;
                let pid = project["id"].as_str().context("project missing id")?;
                let mut payload = json!({ "name": name });
                if let Some(g) = goal {
                    payload["goal"] = Value::String(g);
                }
                let body = api
                    .post(&format!("/api/v1/sprints?project_id={}", enc(pid)), payload)
                    .await?;
                print_output(out, &body);
            }
            SprintCommands::Get { id } => {
                let body = api.get(&format!("/api/v1/sprints/{}", enc(&id))).await?;
                print_output(out, &body);
            }
            SprintCommands::Start { id } => {
                let body = api
                    .post(&format!("/api/v1/sprints/{}/start", enc(&id)), json!({}))
                    .await?;
                print_output(out, &body);
            }
            SprintCommands::Close { id } => {
                let body = api
                    .post(&format!("/api/v1/sprints/{}/close", enc(&id)), json!({}))
                    .await?;
                print_output(out, &body);
            }
            SprintCommands::AddIssue {
                sprint_id,
                issue_id,
            } => {
                let body = api
                    .post(
                        &format!("/api/v1/sprints/{}/issues", enc(&sprint_id)),
                        json!({
                            "issue_id": issue_id
                        }),
                    )
                    .await?;
                print_output(out, &body);
            }
            SprintCommands::RemoveIssue {
                sprint_id,
                issue_id,
            } => {
                let body = api
                    .delete(&format!(
                        "/api/v1/sprints/{}/issues/{}",
                        enc(&sprint_id),
                        enc(&issue_id)
                    ))
                    .await?;
                print_output(out, &body);
            }
        },

        // ── Comment ──
        Commands::Comment { command } => match command {
            CommentCommands::List { issue_id } => {
                let body = api
                    .get(&format!("/api/v1/issues/{}/comments", enc(&issue_id)))
                    .await?;
                print_output(out, &body);
            }
            CommentCommands::Add { issue_id, body } => {
                let resp = api
                    .post(
                        &format!("/api/v1/issues/{}/comments", enc(&issue_id)),
                        json!({
                            "body": body
                        }),
                    )
                    .await?;
                print_output(out, &resp);
            }
            CommentCommands::Update { comment_id, body } => {
                let resp = api
                    .patch(
                        &format!("/api/v1/comments/{}", enc(&comment_id)),
                        json!({
                            "body": body
                        }),
                    )
                    .await?;
                print_output(out, &resp);
            }
            CommentCommands::Delete { comment_id } => {
                api.delete(&format!("/api/v1/comments/{}", enc(&comment_id)))
                    .await?;
                println!("comment {} deleted", comment_id);
            }
        },

        // ── Label ──
        Commands::Label { command } => match command {
            LabelCommands::List { project_key } => {
                let body = api
                    .get(&format!("/api/v1/projects/{}/labels", enc(&project_key)))
                    .await?;
                print_output(out, &body);
            }
            LabelCommands::Create {
                project_key,
                name,
                color,
            } => {
                let mut payload = json!({ "name": name });
                if let Some(c) = color {
                    payload["color"] = Value::String(c);
                }
                let body = api
                    .post(
                        &format!("/api/v1/projects/{}/labels", enc(&project_key)),
                        payload,
                    )
                    .await?;
                print_output(out, &body);
            }
            LabelCommands::Delete { label_id } => {
                api.delete(&format!("/api/v1/labels/{}", enc(&label_id)))
                    .await?;
                println!("label {} deleted", label_id);
            }
            LabelCommands::Attach { issue_id, label_id } => {
                let body = api
                    .post(
                        &format!("/api/v1/issues/{}/labels", enc(&issue_id)),
                        json!({
                            "label_id": label_id
                        }),
                    )
                    .await?;
                print_output(out, &body);
            }
            LabelCommands::Detach { issue_id, label_id } => {
                api.delete(&format!(
                    "/api/v1/issues/{}/labels/{}",
                    enc(&issue_id),
                    enc(&label_id)
                ))
                .await?;
                println!("label detached");
            }
        },

        // ── Search ──
        Commands::Search { command } => match command {
            SearchCommands::Global {
                q,
                project_key,
                priority,
                assignee_id,
            } => {
                let mut params = vec![format!("q={}", enc(&q))];
                if let Some(pk) = project_key {
                    params.push(format!("project_key={}", enc(&pk)));
                }
                if let Some(p) = priority {
                    params.push(format!("priority={}", enc(&p)));
                }
                if let Some(a) = assignee_id {
                    params.push(format!("assignee_id={}", enc(&a)));
                }
                let body = api
                    .get(&format!("/api/v1/search?{}", params.join("&")))
                    .await?;
                print_output(out, &body);
            }
            SearchCommands::Jql { query } => {
                let body = api.post("/api/v1/search", json!({ "jql": query })).await?;
                print_output(out, &body);
            }
        },

        // ── Notifications ──
        Commands::Notification { command } => match command {
            NotificationCommands::List => {
                let body = api.get("/api/v1/notifications").await?;
                print_output(out, &body);
            }
            NotificationCommands::Read { id } => {
                api.post(
                    &format!("/api/v1/notifications/{}/read", enc(&id)),
                    json!({}),
                )
                .await?;
                println!("notification {} marked as read", id);
            }
            NotificationCommands::ReadAll => {
                api.post("/api/v1/notifications/read-all", json!({}))
                    .await?;
                println!("all notifications marked as read");
            }
            NotificationCommands::Settings => {
                let body = api.get("/api/v1/notifications/settings").await?;
                print_output(out, &body);
            }
            NotificationCommands::UpdateSettings {
                email_frequency,
                notify_own_changes,
            } => {
                let mut payload = json!({});
                if let Some(f) = email_frequency {
                    payload["email_frequency"] = Value::String(f);
                }
                if let Some(b) = notify_own_changes {
                    payload["notify_own_changes"] = Value::Bool(b);
                }
                let body = api.patch("/api/v1/notifications/settings", payload).await?;
                print_output(out, &body);
            }
        },

        // ── Reports ──
        Commands::Report { command } => match command {
            ReportCommands::Velocity { project_key, count } => {
                let project = api
                    .get(&format!("/api/v1/projects/{}", enc(&project_key)))
                    .await?;
                let pid = project["id"].as_str().context("project missing id")?;
                let body = api
                    .get(&format!(
                        "/api/v1/reports/velocity?project_id={}&count={}",
                        enc(pid),
                        count
                    ))
                    .await?;
                print_output(out, &body);
            }
            ReportCommands::Burndown { sprint_id } => {
                let body = api
                    .get(&format!(
                        "/api/v1/reports/burndown?sprint_id={}",
                        enc(&sprint_id)
                    ))
                    .await?;
                print_output(out, &body);
            }
            ReportCommands::CumulativeFlow { project_key } => {
                let project = api
                    .get(&format!("/api/v1/projects/{}", enc(&project_key)))
                    .await?;
                let pid = project["id"].as_str().context("project missing id")?;
                let body = api
                    .get(&format!(
                        "/api/v1/reports/cumulative-flow?project_id={}",
                        enc(pid)
                    ))
                    .await?;
                print_output(out, &body);
            }
            ReportCommands::ControlChart { project_key } => {
                let project = api
                    .get(&format!("/api/v1/projects/{}", enc(&project_key)))
                    .await?;
                let pid = project["id"].as_str().context("project missing id")?;
                let body = api
                    .get(&format!(
                        "/api/v1/reports/control-chart?project_id={}",
                        enc(pid)
                    ))
                    .await?;
                print_output(out, &body);
            }
        },

        // ── Admin ──
        Commands::Admin { command } => match command {
            AdminCommands::ListUsers => {
                let body = api.get("/api/v1/admin/users").await?;
                print_output(out, &body);
            }
            AdminCommands::CreateUser {
                email,
                username,
                display_name,
                password,
                is_admin,
            } => {
                let body = api
                    .post(
                        "/api/v1/admin/users",
                        json!({
                            "email": email, "username": username, "display_name": display_name,
                            "password": password, "is_system_admin": is_admin
                        }),
                    )
                    .await?;
                print_output(out, &body);
            }
            AdminCommands::ToggleUser { user_id, active } => {
                let body = api
                    .put(
                        &format!("/api/v1/admin/users/{}/status", enc(&user_id)),
                        json!({
                            "is_active": active
                        }),
                    )
                    .await?;
                print_output(out, &body);
            }
            AdminCommands::AuditLog { limit } => {
                let body = api
                    .get(&format!("/api/v1/admin/audit-log?limit={}", limit))
                    .await?;
                print_output(out, &body);
            }
            AdminCommands::Settings => {
                let body = api.get("/api/v1/admin/settings").await?;
                print_output(out, &body);
            }
            AdminCommands::SetSetting { key, value } => {
                let body = api
                    .put(
                        "/api/v1/admin/settings",
                        json!({
                            "key": key, "value": value
                        }),
                    )
                    .await?;
                print_output(out, &body);
            }
        },

        // ── Members ──
        Commands::Member { command } => match command {
            MemberCommands::List { project_key } => {
                let body = api
                    .get(&format!("/api/v1/projects/{}/members", enc(&project_key)))
                    .await?;
                print_output(out, &body);
            }
            MemberCommands::Add {
                project_key,
                user_id,
                role,
            } => {
                let body = api
                    .post(
                        &format!("/api/v1/projects/{}/members", enc(&project_key)),
                        json!({
                            "user_id": user_id, "role": role
                        }),
                    )
                    .await?;
                print_output(out, &body);
            }
            MemberCommands::Remove {
                project_key,
                user_id,
            } => {
                api.delete(&format!(
                    "/api/v1/projects/{}/members/{}",
                    enc(&project_key),
                    enc(&user_id)
                ))
                .await?;
                println!("member removed");
            }
        },
    }
    Ok(())
}
