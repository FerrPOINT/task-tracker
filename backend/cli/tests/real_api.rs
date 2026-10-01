use std::sync::Arc;

use domain::{
    Board, BoardColumn, BoardRepository, InMemoryStorage, MemoryAttachmentRepository,
    MemoryBoardRepository, MemoryCommentRepository, MemoryIssueLinkRepository,
    MemoryIssueRepository, MemoryIssueStatusHistoryRepository, MemoryLabelRepository,
    MemoryNotificationRepository, MemoryProjectMemberRepository, MemoryProjectRepository,
    MemorySprintRepository, MemoryUserRepository, MemoryWorklogRepository, Project,
    ProjectRepository, StatusCategory, User, UserRepository,
};
use shared::{AppConfig, AuthConfig, DatabaseConfig, ProjectKey, ServerConfig, StatusId, UserId};

use app::context::AppContext;

fn test_user() -> User {
    User {
        id: UserId::from_uuid(uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()),
        email: "demo@example.com".into(),
        username: "demo".into(),
        display_name: "Demo User".into(),
        password_hash: "$argon2id$v=19$m=65536,t=3,p=4$stN/enhZ9yOvgWC9E8Y6BA$IL9I0WONb/I6zoT4rdmdkrPcIFADFxsLCjrO0ySSl0Y".into(),
        refresh_token_hash: None,
        is_system_admin: false,
        is_active: true,
        created_at: shared::now(),
        updated_at: shared::now(),
    }
}

fn test_config() -> Arc<AppConfig> {
    Arc::new(AppConfig {
        database: DatabaseConfig::default(),
        server: ServerConfig::default(),
        auth: AuthConfig {
            jwt_secret: "test-secret".to_string(),
            totp_key: String::new(),
            reset_base_url: "http://localhost:5173".to_string(),
            oidc_issuer_url: String::new(),
            oidc_client_id: String::new(),
            oidc_client_secret: String::new(),
            oidc_redirect_url: String::new(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
            refresh_cookie_name: "refresh_token".to_string(),
            refresh_cookie_secure: true,
            refresh_cookie_same_site: "Lax".to_string(),
            refresh_cookie_domain: None,
            refresh_cookie_path: "/api/v1/auth".to_string(),
        },
        storage: shared::StorageConfig::default(),
        email: shared::EmailConfig::default(),
        metrics: shared::MetricsConfig::default(),
    })
}

async fn spawn_server_with_notifications()
-> (String, reqwest::Client, Arc<MemoryNotificationRepository>) {
    let user = test_user();
    let mut project = Project {
        id: shared::ProjectId::from_uuid(
            uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
        ),
        key: ProjectKey::new("TT"),
        name: "Task Tracker".into(),
        description: None,
        owner_id: user.id,
        default_board_id: shared::BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };

    let todo =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let in_progress =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap());
    let review =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000004").unwrap());
    let done =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap());
    project.default_board_id = shared::BoardId::new();
    let board = Board {
        id: project.default_board_id,
        project_id: project.id,
        name: "TT Kanban".into(),
        columns: vec![
            BoardColumn {
                id: todo,
                name: "Todo".into(),
                category: StatusCategory::Todo,
                wip_limit: None,
                position: 0,
            },
            BoardColumn {
                id: in_progress,
                name: "In Progress".into(),
                category: StatusCategory::InProgress,
                wip_limit: Some(5),
                position: 1,
            },
            BoardColumn {
                id: review,
                name: "Review".into(),
                category: StatusCategory::InProgress,
                wip_limit: None,
                position: 2,
            },
            BoardColumn {
                id: done,
                name: "Done".into(),
                category: StatusCategory::Done,
                wip_limit: None,
                position: 3,
            },
        ],
    };

    let users = Arc::new(MemoryUserRepository::default());
    users.save(&user).await.unwrap();
    let projects = Arc::new(MemoryProjectRepository::default());
    projects.save(&project).await.unwrap();
    let shared_history = Arc::new(MemoryIssueStatusHistoryRepository::default());
    let (hist_store, hist_projects) = shared_history.store();
    let custom_fields = Arc::new(domain::MemoryCustomFieldRepository::default());
    let issues = Arc::new(MemoryIssueRepository::with_shared_stores(
        hist_store,
        hist_projects,
        custom_fields.value_store(),
    ));
    let boards = Arc::new(MemoryBoardRepository::default());
    boards.save(&board).await.unwrap();
    let sprints = Arc::new(MemorySprintRepository::default());

    let notifications = Arc::new(MemoryNotificationRepository::default());
    let status_repo = Arc::new(domain::MemoryStatusRepository::new(vec![
        domain::Status {
            id: todo,
            name: "To Do".into(),
            category: domain::StatusCategory::Todo,
            position: 0,
            is_default: true,
            is_closed: false,
        },
        domain::Status {
            id: in_progress,
            name: "In Progress".into(),
            category: domain::StatusCategory::InProgress,
            position: 1,
            is_default: false,
            is_closed: false,
        },
        domain::Status {
            id: review,
            name: "Review".into(),
            category: domain::StatusCategory::InProgress,
            position: 2,
            is_default: false,
            is_closed: false,
        },
        domain::Status {
            id: done,
            name: "Done".into(),
            category: domain::StatusCategory::Done,
            position: 3,
            is_default: false,
            is_closed: true,
        },
    ]));
    let repos = Arc::new(domain::Repositories {
        users: users.clone(),
        totp: std::sync::Arc::new(domain::stubs::memory::MemoryTotpRepository::default()),
        oidc: Arc::new(domain::StubOidcRepository),
        password_resets: Arc::new(domain::stubs::memory::MemoryPasswordResetRepository::default()),
        audit_logs: Arc::new(domain::StubAuditLogRepository),
        system_settings: Arc::new(domain::StubSystemSettingRepository),
        projects: projects.clone(),
        issues: issues.clone(),
        boards: boards.clone(),
        sprints: sprints.clone(),
        comments: Arc::new(MemoryCommentRepository::default()),
        worklogs: Arc::new(MemoryWorklogRepository::default()),
        members: Arc::new(MemoryProjectMemberRepository::default()),
        statuses: status_repo,
        transitions: Arc::new(domain::StubWorkflowTransitionRepository),
        issue_types: Arc::new(domain::StubIssueTypeRepository),
        attachments: Arc::new(MemoryAttachmentRepository::default()),
        labels: Arc::new(MemoryLabelRepository::default()),
        issue_links: Arc::new(MemoryIssueLinkRepository::default()),
        notifications: notifications.clone(),
        notification_settings: notifications.clone(),
        issue_status_history: shared_history,
        watchers: Arc::new(domain::MemoryWatcherRepository::default()),
        votes: Arc::new(domain::MemoryVoteRepository::default()),
        components: Arc::new(domain::stubs::memory::MemoryProjectComponentRepository::default()),
        versions: Arc::new(domain::stubs::memory::MemoryProjectVersionRepository::default()),
        custom_fields,
    });

    let ctx = Arc::new(AppContext::new(
        test_config(),
        repos,
        Arc::new(InMemoryStorage::default()),
    ));
    let router = api::router(ctx.clone()).with_state(ctx);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{addr}");
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let client = reqwest::Client::new();
    (url, client, notifications)
}

fn run(url: &str, token: &str, args: &[&str], input: Option<&str>) -> serde_json::Value {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(env!("CARGO_BIN_EXE_task-tracker"))
        .env_remove("SDLC_API_TOKEN")
        .env_remove("WIKI_TOKEN")
        .env_remove("TASK_TRACKER_TOKEN")
        .args([
            "--api-url",
            &format!("{url}/api/v1"),
            "--token",
            token,
            "--output",
            "json",
        ])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_lifecycle_against_real_application_api() {
    let (url, client, _) = spawn_server_with_notifications().await;
    let auth: serde_json::Value = client
        .post(format!("{url}/api/v1/auth/login"))
        .json(&serde_json::json!({"email":"demo@example.com","password":"demo"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = auth["access_token"].as_str().unwrap();
    let issue = run(
        &url,
        token,
        &[
            "issue",
            "create",
            "--project-key",
            "TT",
            "--summary",
            "CLI fixture",
            "--from-file",
            "-",
        ],
        Some("Описание из stdin"),
    );
    let key = issue["key"].as_str().unwrap();
    let id = issue["id"].as_str().unwrap();
    run(&url, token, &["statuses"], None);
    run(&url, token, &["issue-types"], None);
    run(&url, token, &["transitions", "--issue", key], None);
    run(
        &url,
        token,
        &["comment", "add", "--issue-id", key, "--from-file", "-"],
        Some("Комментарий из stdin"),
    );
    run(
        &url,
        token,
        &["comment", "list", key, "--limit", "10"],
        None,
    );
    let worklog = run(
        &url,
        token,
        &[
            "worklog",
            "create",
            "--issue",
            key,
            "--started-at",
            "2026-10-01T10:00:00Z",
            "--duration-seconds",
            "60",
        ],
        None,
    );
    let worklog_id = worklog["id"].as_str().unwrap();
    run(
        &url,
        token,
        &["worklog", "update", worklog_id, "--duration-seconds", "120"],
        None,
    );
    run(
        &url,
        token,
        &["worklog", "list", "--issue", key, "--limit", "10"],
        None,
    );
    run(&url, token, &["worklog", "delete", worklog_id], None);
    assert_eq!(run(&url, token, &["issue", "get", key], None)["id"], id);
    assert_eq!(run(&url, token, &["issue", "get", id], None)["key"], key);
    run(
        &url,
        token,
        &[
            "issue",
            "list",
            "--project-key",
            "TT",
            "--limit",
            "1",
            "--offset",
            "0",
        ],
        None,
    );
    run(
        &url,
        token,
        &["issue", "update", key, "--summary", "Changed", "--unassign"],
        None,
    );
    let target = run(
        &url,
        token,
        &[
            "issue",
            "create",
            "--project-key",
            "TT",
            "--summary",
            "Target",
        ],
        None,
    );
    run(
        &url,
        token,
        &[
            "link",
            "add",
            "--issue",
            key,
            "--target-key",
            target["key"].as_str().unwrap(),
            "--link-type",
            "relates",
        ],
        None,
    );
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("sample.txt");
    std::fs::write(&file, b"fixture file").unwrap();
    let attachment = run(
        &url,
        token,
        &[
            "attachment",
            "upload",
            "--issue",
            key,
            "--file",
            file.to_str().unwrap(),
        ],
        None,
    );
    run(&url, token, &["attachment", "list", "--issue", key], None);
    let out = dir.path().join("download.txt");
    run(
        &url,
        token,
        &[
            "attachment",
            "download",
            attachment["id"].as_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(std::fs::read(out).unwrap(), b"fixture file");
    run(
        &url,
        token,
        &[
            "issue",
            "transition",
            key,
            "--to",
            "00000000-0000-0000-0000-000000000003",
        ],
        None,
    );
    run(&url, token, &["issue", "delete", key], None);
    run(
        &url,
        token,
        &["trash", "--project-key", "TT", "--limit", "10"],
        None,
    );
    run(&url, token, &["issue", "restore", key], None);
    assert_eq!(
        run(&url, token, &["issue", "get", key], None)["summary"],
        "Changed"
    );
}
