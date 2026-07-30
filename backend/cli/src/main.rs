use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use reqwest::Client;
use serde_json::Value;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "task-tracker")]
#[command(about = "Task Tracker CLI")]
struct Cli {
    #[arg(
        long,
        env = "TASKTRACKER_API_URL",
        default_value = "http://localhost:19876"
    )]
    api_url: String,
    #[arg(long, env = "TASKTRACKER_TOKEN")]
    token: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },
    Issue {
        #[command(subcommand)]
        command: IssueCommands,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    Login {
        #[arg(long)]
        email: String,
        #[arg(long)]
        password: String,
    },
    Logout,
    Whoami,
}

#[derive(Subcommand)]
enum ProjectCommands {
    List,
    Create {
        #[arg(long)]
        key: String,
        #[arg(long)]
        name: String,
    },
    Get {
        key: String,
    },
    Update {
        key: String,
        #[arg(long)]
        name: Option<String>,
    },
    Delete {
        key: String,
    },
}

#[derive(Subcommand)]
enum IssueCommands {
    Create {
        #[arg(long)]
        project_key: String,
        #[arg(long)]
        summary: String,
        #[arg(long)]
        #[clap(default_value = "task")]
        issue_type: String,
        #[arg(long)]
        status_id: Option<String>,
    },
    Get {
        key: String,
    },
    Update {
        key: String,
        #[arg(long)]
        summary: Option<String>,
        #[arg(long)]
        status_id: Option<String>,
    },
    Delete {
        key: String,
    },
    Transition {
        key: String,
        #[arg(long)]
        to: String,
    },
}

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
        let url = format!("{}{}", self.base, path);
        let req = self.client.get(&url).bearer_auth(self.auth_header()?);
        let resp = req.send().await?;
        let status = resp.status();
        let body = resp.json::<Value>().await?;
        if !status.is_success() {
            bail!("API error {}: {}", status, body);
        }
        Ok(body)
    }

    async fn post(&self, path: &str, payload: Value) -> Result<Value> {
        let url = format!("{}{}", self.base, path);
        let req = self
            .client
            .post(&url)
            .json(&payload)
            .bearer_auth(self.auth_header()?);
        let resp = req.send().await?;
        let status = resp.status();
        let body = resp.json::<Value>().await?;
        if !status.is_success() {
            bail!("API error {}: {}", status, body);
        }
        Ok(body)
    }

    async fn patch(&self, path: &str, payload: Value) -> Result<Value> {
        let url = format!("{}{}", self.base, path);
        let req = self
            .client
            .patch(&url)
            .json(&payload)
            .bearer_auth(self.auth_header()?);
        let resp = req.send().await?;
        let status = resp.status();
        let body = resp.json::<Value>().await?;
        if !status.is_success() {
            bail!("API error {}: {}", status, body);
        }
        Ok(body)
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.base, path);
        let req = self.client.delete(&url).bearer_auth(self.auth_header()?);
        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.json::<Value>().await.unwrap_or(Value::Null);
            bail!("API error {}: {}", status, body);
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Err(e) = run(cli).await {
        eprintln!("error: {:#}", e);
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

async fn run(cli: Cli) -> Result<()> {
    let api = Api::new(cli.api_url, cli.token);

    match cli.command {
        Commands::Auth { command } => match command {
            AuthCommands::Login { email, password } => {
                let body = api
                    .post(
                        "/api/v1/auth/login",
                        serde_json::json!({ "email": email, "password": password }),
                    )
                    .await?;
                println!("{}", serde_json::to_string_pretty(&body)?);
            }
            AuthCommands::Logout => {
                api.post("/api/v1/auth/logout", serde_json::json!({}))
                    .await?;
                println!("logged out");
            }
            AuthCommands::Whoami => {
                let body = api.get("/api/v1/auth/whoami").await?;
                println!("{}", serde_json::to_string_pretty(&body)?);
            }
        },
        Commands::Project { command } => match command {
            ProjectCommands::List => {
                let body = api.get("/api/v1/projects").await?;
                println!("{}", serde_json::to_string_pretty(&body)?);
            }
            ProjectCommands::Create { key, name } => {
                let body = api
                    .post(
                        "/api/v1/projects",
                        serde_json::json!({ "key": key, "name": name, "description": null }),
                    )
                    .await?;
                println!("{}", serde_json::to_string_pretty(&body)?);
            }
            ProjectCommands::Get { key } => {
                let body = api
                    .get(&format!("/api/v1/projects/{}", urlencode(&key)))
                    .await?;
                println!("{}", serde_json::to_string_pretty(&body)?);
            }
            ProjectCommands::Update { key, name } => {
                let mut payload = serde_json::json!({});
                if let Some(name) = name {
                    payload["name"] = Value::String(name);
                }
                let body = api
                    .patch(&format!("/api/v1/projects/{}", urlencode(&key)), payload)
                    .await?;
                println!("{}", serde_json::to_string_pretty(&body)?);
            }
            ProjectCommands::Delete { key } => {
                api.delete(&format!("/api/v1/projects/{}", urlencode(&key)))
                    .await?;
                println!("project {} deleted", key);
            }
        },
        Commands::Issue { command } => match command {
            IssueCommands::Create {
                project_key,
                summary,
                issue_type,
                status_id,
            } => {
                let project = api
                    .get(&format!("/api/v1/projects/{}", urlencode(&project_key)))
                    .await?;
                let project_id = project["id"].as_str().context("project missing id")?;
                let mut payload = serde_json::json!({
                    "project_id": project_id,
                    "summary": summary,
                    "issue_type": issue_type,
                });
                if let Some(status_id) = status_id {
                    payload["status_id"] = Value::String(status_id);
                }
                let body = api.post("/api/v1/issues", payload).await?;
                println!("{}", serde_json::to_string_pretty(&body)?);
            }
            IssueCommands::Get { key } => {
                let body = api
                    .get(&format!("/api/v1/issues/{}", urlencode(&key)))
                    .await?;
                println!("{}", serde_json::to_string_pretty(&body)?);
            }
            IssueCommands::Update {
                key,
                summary,
                status_id,
            } => {
                let mut payload = serde_json::json!({});
                if let Some(summary) = summary {
                    payload["summary"] = Value::String(summary);
                }
                if let Some(status_id) = status_id {
                    payload["status_id"] = Value::String(status_id);
                }
                let body = api
                    .patch(&format!("/api/v1/issues/{}", urlencode(&key)), payload)
                    .await?;
                println!("{}", serde_json::to_string_pretty(&body)?);
            }
            IssueCommands::Delete { key } => {
                api.delete(&format!("/api/v1/issues/{}", urlencode(&key)))
                    .await?;
                println!("issue {} deleted", key);
            }
            IssueCommands::Transition { key, to } => {
                let body = api
                    .post(
                        &format!("/api/v1/issues/{}/transition", urlencode(&key)),
                        serde_json::json!({ "target_status_id": to }),
                    )
                    .await?;
                println!("{}", serde_json::to_string_pretty(&body)?);
            }
        },
    }
    Ok(())
}

fn urlencode(s: &str) -> String {
    s.replace(' ', "%20")
}
