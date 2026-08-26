use std::sync::Arc;

use domain::{
    Board, BoardColumn, BoardRepository, InMemoryStorage, IssueRepository,
    MemoryAttachmentRepository, MemoryBoardRepository, MemoryCommentRepository,
    MemoryIssueLinkRepository, MemoryIssueRepository, MemoryIssueStatusHistoryRepository,
    MemoryLabelRepository, MemoryNotificationRepository, MemoryProjectMemberRepository,
    MemoryProjectRepository, MemorySprintRepository, MemoryUserRepository, MemoryWorklogRepository,
    Notification, NotificationRepository, Project, ProjectRepository, SprintRepository,
    StatusCategory, User, UserNotificationSettingsRepository, UserRepository,
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
    })
}

async fn spawn_server() -> (String, reqwest::Client) {
    let (url, client, _) = spawn_server_with_notifications().await;
    (url, client)
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
    let issues = Arc::new(MemoryIssueRepository::default());
    let boards = Arc::new(MemoryBoardRepository::default());
    boards.save(&board).await.unwrap();
    let sprints = Arc::new(MemorySprintRepository::default());

    let notifications = Arc::new(MemoryNotificationRepository::default());
    let repos = Arc::new(domain::Repositories {
        users: users.clone(),
        audit_logs: Arc::new(domain::StubAuditLogRepository),
        system_settings: Arc::new(domain::StubSystemSettingRepository),
        projects: projects.clone(),
        issues: issues.clone(),
        boards: boards.clone(),
        sprints: sprints.clone(),
        comments: Arc::new(MemoryCommentRepository::default()),
        worklogs: Arc::new(MemoryWorklogRepository::default()),
        members: Arc::new(MemoryProjectMemberRepository::default()),
        statuses: Arc::new(domain::StubStatusRepository),
        transitions: Arc::new(domain::StubWorkflowTransitionRepository),
        issue_types: Arc::new(domain::StubIssueTypeRepository),
        attachments: Arc::new(MemoryAttachmentRepository::default()),
        labels: Arc::new(MemoryLabelRepository::default()),
        issue_links: Arc::new(MemoryIssueLinkRepository::default()),
        notifications: notifications.clone(),
        notification_settings: notifications.clone(),
        issue_status_history: Arc::new(domain::MemoryIssueStatusHistoryRepository::default()),
        watchers: Arc::new(domain::MemoryWatcherRepository::default()),
        votes: Arc::new(domain::MemoryVoteRepository::default()),
        components: Arc::new(domain::StubProjectComponentRepository),
        versions: Arc::new(domain::StubProjectVersionRepository),
        custom_fields: Arc::new(domain::StubCustomFieldRepository),
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

#[tokio::test]
async fn health_is_public() {
    let (url, client) = spawn_server().await;
    let res = client
        .get(format!("{}/api/v1/health", url))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.text().await.unwrap(), "ok");
}

#[tokio::test]
async fn projects_requires_auth() {
    let (url, client) = spawn_server().await;
    let res = client
        .get(format!("{}/api/v1/projects", url))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn login_issues_token() {
    let (url, client) = spawn_server().await;
    let res = client
        .post(format!("{}/api/v1/auth/login", url))
        .json(&serde_json::json!({"email":"demo@example.com","password":"demo"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["token_type"], "Bearer");
    assert!(body["access_token"].as_str().unwrap().len() > 10);
    assert!(body["user_id"].as_str().unwrap().len() > 10);
}

#[tokio::test]
async fn register_and_list_projects() {
    let (url, client) = spawn_server().await;
    let res = client
        .post(format!("{}/api/v1/auth/register", url))
        .json(&serde_json::json!({
            "email": "new@example.com",
            "username": "newuser",
            "name": "New User",
            "password": "secret123"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["email"], "new@example.com");
    let token = body["access_token"].as_str().unwrap().to_string();

    let projects = client
        .get(format!("{}/api/v1/projects", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(projects.status(), 200);
    let body: serde_json::Value = projects.json().await.unwrap();
    let list = body["projects"].as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["key"], "TT");
}

#[tokio::test]
async fn dashboard_and_search() {
    let (url, client) = spawn_server().await;
    let login = client
        .post(format!("{}/api/v1/auth/login", url))
        .json(&serde_json::json!({"email":"demo@example.com","password":"demo"}))
        .send()
        .await
        .unwrap();
    let token = login.json::<serde_json::Value>().await.unwrap()["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    // create an issue to search for
    let created = client
        .post(format!("{}/api/v1/issues", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "project_key": "TT",
            "summary": "searchable issue",
            "issue_type": "task",
            "priority": "medium",
            "status_id": "00000000-0000-0000-0000-000000000001",
            "reporter_id": test_user().id.to_string(),
            "assignee_id": test_user().id.to_string()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 201);

    let search = client
        .get(format!("{}/api/v1/search?q=searchable", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(search.status(), 200);
    let body: serde_json::Value = search.json().await.unwrap();
    assert!(!body["issues"].as_array().unwrap().is_empty());

    let dash = client
        .get(format!("{}/api/v1/dashboard", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(dash.status(), 200);
    let body: serde_json::Value = dash.json().await.unwrap();
    assert!(body["assigned_issues"].is_array());
}

#[tokio::test]
async fn backlog_requires_auth_and_returns_issues() {
    let (url, client) = spawn_server().await;
    let login = client
        .post(format!("{}/api/v1/auth/login", url))
        .json(&serde_json::json!({"email":"demo@example.com","password":"demo"}))
        .send()
        .await
        .unwrap();
    let token = login.json::<serde_json::Value>().await.unwrap()["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    let noauth = client
        .get(format!("{}/api/v1/projects/TT/backlog", url))
        .send()
        .await
        .unwrap();
    assert_eq!(noauth.status(), 401);

    let backlog = client
        .get(format!("{}/api/v1/projects/TT/backlog", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(backlog.status(), 200);
    let body: serde_json::Value = backlog.json().await.unwrap();
    assert!(body["backlog_issues"].is_array());
    assert!(body["sprint_issues"].is_array());
}

#[tokio::test]
async fn issue_create_validation_errors() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let bad_project = client
        .post(format!("{}/api/v1/issues", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "project_key": "INVALID_KEY",
            "summary": "x",
            "issue_type": "task",
            "priority": "medium",
            "status_id": "00000000-0000-0000-0000-000000000001",
            "reporter_id": test_user().id.to_string()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_project.status(), 400);

    let bad_reporter = client
        .post(format!("{}/api/v1/issues", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "project_key": "TT",
            "summary": "Bad reporter",
            "issue_type": "task",
            "priority": "medium",
            "status_id": "00000000-0000-0000-0000-000000000001",
            "reporter_id": "not-a-uuid"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_reporter.status(), 400);

    let defaults = client
        .post(format!("{}/api/v1/issues", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "project_key": "TT",
            "summary": "fallback defaults",
            "issue_type": "unknown",
            "priority": "unknown",
            "status_id": "00000000-0000-0000-0000-000000000001",
            "reporter_id": test_user().id.to_string()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(defaults.status(), 201);
    let body: serde_json::Value = defaults.json().await.unwrap();
    assert_eq!(body["issue_type"], "task");
    assert_eq!(body["priority"], "Medium");
}

#[tokio::test]
async fn issue_get_and_update_not_found() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let missing = client
        .get(format!(
            "{}/api/v1/issues/00000000-0000-0000-0000-000000000000",
            url
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);

    let bad_update_id = client
        .patch(format!("{}/api/v1/issues/not-a-uuid", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({"summary": "nope"}))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_update_id.status(), 400);

    let bad_get_id = client
        .get(format!("{}/api/v1/issues/not-a-uuid", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(bad_get_id.status(), 400);

    let missing_update = client
        .patch(format!(
            "{}/api/v1/issues/00000000-0000-0000-0000-000000000000",
            url
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"summary": "nope"}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing_update.status(), 404);
}

#[tokio::test]
async fn board_move_validation() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let bad_key = client
        .get(format!("{}/api/v1/projects/!!/board", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(bad_key.status(), 400);

    let bad_move_issue = client
        .post(format!("{}/api/v1/projects/TT/board/move", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({"issue_id": "not-a-uuid", "status_id": test_status_done().to_string()}))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_move_issue.status(), 400);

    let bad_move_status = client
        .post(format!("{}/api/v1/projects/TT/board/move", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({"issue_id": "00000000-0000-0000-0000-000000000000", "status_id": "not-a-uuid"}))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_move_status.status(), 400);

    let missing_issue = client
        .post(format!("{}/api/v1/projects/TT/board/move", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({"issue_id": "00000000-0000-0000-0000-000000000000", "status_id": test_status_done().to_string()}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing_issue.status(), 404);
}

async fn login_token(url: &str, client: &reqwest::Client) -> String {
    let res = client
        .post(format!("{}/api/v1/auth/login", url))
        .json(&serde_json::json!({"email":"demo@example.com","password":"demo"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

fn test_status_done() -> shared::StatusId {
    shared::StatusId::from_uuid(
        uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap(),
    )
}

#[tokio::test]
async fn board_success_and_move() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let board = client
        .get(format!("{}/api/v1/projects/TT/board", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(board.status(), 200);
    let body: serde_json::Value = board.json().await.unwrap();
    assert!(!body["columns"].as_array().unwrap().is_empty());

    let created = client
        .post(format!("{}/api/v1/issues", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "project_key": "TT",
            "summary": "move me",
            "issue_type": "task",
            "priority": "medium",
            "status_id": "00000000-0000-0000-0000-000000000001",
            "reporter_id": test_user().id.to_string()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 201);
    let issue: serde_json::Value = created.json().await.unwrap();
    let issue_id = issue["id"].as_str().unwrap();

    let moved = client
        .post(format!("{}/api/v1/projects/TT/board/move", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "issue_id": issue_id,
            "status_id": test_status_done().to_string()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(moved.status(), 200);
    let body: serde_json::Value = moved.json().await.unwrap();
    assert!(body["issues"].as_array().is_some());
}

#[tokio::test]
async fn dashboard_returns_assigned_issues() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let created = client
        .post(format!("{}/api/v1/issues", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "project_key": "TT",
            "summary": "assigned to me",
            "issue_type": "task",
            "priority": "medium",
            "status_id": "00000000-0000-0000-0000-000000000001",
            "reporter_id": test_user().id.to_string(),
            "assignee_id": test_user().id.to_string(),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 201);

    let res = client
        .get(format!("{}/api/v1/dashboard", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(!body["assigned_issues"].as_array().unwrap().is_empty());
}
#[tokio::test]
async fn issue_get_not_found() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let res = client
        .get(format!(
            "{}/api/v1/issues/00000000-0000-0000-0000-000000000000",
            url
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn issue_update_not_found() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let res = client
        .patch(format!(
            "{}/api/v1/issues/00000000-0000-0000-0000-000000000000",
            url
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"summary":"x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn comments_crud() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let created = client
        .post(format!("{}/api/v1/issues", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "project_key": "TT",
            "summary": "commentable issue",
            "issue_type": "task",
            "priority": "medium",
            "status_id": "00000000-0000-0000-0000-000000000001",
            "reporter_id": test_user().id.to_string()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 201);
    let issue: serde_json::Value = created.json().await.unwrap();
    let issue_id = issue["id"].as_str().unwrap();

    let list0 = client
        .get(format!("{}/api/v1/issues/{issue_id}/comments", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(list0.status(), 200);
    let body: serde_json::Value = list0.json().await.unwrap();
    assert!(body["comments"].as_array().unwrap().is_empty());

    let create = client
        .post(format!("{}/api/v1/issues/{issue_id}/comments", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({"body": "first comment"}))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);
    let comment: serde_json::Value = create.json().await.unwrap();
    let comment_id = comment["id"].as_str().unwrap();
    assert_eq!(comment["body"], "first comment");

    let update = client
        .patch(format!("{}/api/v1/comments/{comment_id}", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({"body": "updated comment"}))
        .send()
        .await
        .unwrap();
    assert_eq!(update.status(), 200);
    let body: serde_json::Value = update.json().await.unwrap();
    assert_eq!(body["body"], "updated comment");

    let delete = client
        .delete(format!("{}/api/v1/comments/{comment_id}", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), 204);
}

#[tokio::test]
async fn worklogs_crud() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let created = client
        .post(format!("{}/api/v1/issues", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "project_key": "TT",
            "summary": "worklog issue",
            "issue_type": "task",
            "priority": "medium",
            "status_id": "00000000-0000-0000-0000-000000000001",
            "reporter_id": test_user().id.to_string()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 201);
    let issue: serde_json::Value = created.json().await.unwrap();
    let issue_id = issue["id"].as_str().unwrap();

    let list0 = client
        .get(format!("{}/api/v1/issues/{issue_id}/worklogs", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(list0.status(), 200);

    let create = client
        .post(format!("{}/api/v1/issues/{issue_id}/worklogs", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "started_at": "2026-07-21T10:00:00+00:00",
            "duration_seconds": 3600,
            "description": "e2e worklog"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);
    let worklog: serde_json::Value = create.json().await.unwrap();
    let worklog_id = worklog["id"].as_str().unwrap();
    assert_eq!(worklog["duration_seconds"], 3600);

    let update = client
        .patch(format!("{}/api/v1/worklogs/{worklog_id}", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "started_at": "2026-07-21T11:00:00+00:00",
            "duration_seconds": 7200,
            "description": "updated worklog"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(update.status(), 200);
    let body: serde_json::Value = update.json().await.unwrap();
    assert_eq!(body["duration_seconds"], 7200);

    let delete = client
        .delete(format!("{}/api/v1/worklogs/{worklog_id}", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), 204);
}

#[tokio::test]
async fn project_members_crud() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let register = client
        .post(format!("{}/api/v1/auth/register", url))
        .json(&serde_json::json!({
            "email": "member@example.com",
            "username": "member",
            "name": "Member User",
            "password": "secret123"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(register.status(), 201);
    let user: serde_json::Value = register.json().await.unwrap();
    let user_id = user["user_id"].as_str().unwrap();
    let project_id = test_project_id();

    let list0 = client
        .get(format!("{}/api/v1/projects/{project_id}/members", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(list0.status(), 200);

    let add = client
        .post(format!("{}/api/v1/projects/{project_id}/members", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({"user_id": user_id, "role": "member"}))
        .send()
        .await
        .unwrap();
    assert_eq!(add.status(), 201);
    let body: serde_json::Value = add.json().await.unwrap();
    assert_eq!(body["role"], "member");

    let remove = client
        .delete(format!(
            "{}/api/v1/projects/{project_id}/members/{user_id}",
            url
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(remove.status(), 204);
}

#[tokio::test]
async fn issue_transition() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let created = client
        .post(format!("{}/api/v1/issues", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "project_key": "TT",
            "summary": "transition me",
            "issue_type": "task",
            "priority": "medium",
            "status_id": "00000000-0000-0000-0000-000000000001",
            "reporter_id": test_user().id.to_string()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 201);
    let issue: serde_json::Value = created.json().await.unwrap();
    let issue_id = issue["id"].as_str().unwrap();

    let res = client
        .post(format!("{}/api/v1/issues/{issue_id}/transition", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({"target_status_id": test_status_done().to_string()}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "Done");
}

fn test_project_id() -> String {
    "22222222-2222-2222-2222-222222222222".to_string()
}

#[tokio::test]
async fn issue_create_invalid_project_key() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let res = client
        .post(format!("{}/api/v1/issues", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "project_key": "invalid key!",
            "summary": "x",
            "issue_type": "task",
            "priority": "medium",
            "status_id": "00000000-0000-0000-0000-000000000001",
            "reporter_id": test_user().id.to_string()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn users_me_returns_current_user() {
    let (url, client) = spawn_server().await;

    let res = client
        .post(format!("{}/api/v1/auth/register", url))
        .json(&serde_json::json!({
            "email": "me@example.com",
            "username": "meuser",
            "name": "Me User",
            "password": "password123"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let body: serde_json::Value = res.json().await.unwrap();
    let token = body["access_token"].as_str().unwrap();

    let res = client
        .get(format!("{}/api/v1/users/me", url))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["email"], "me@example.com");
    assert_eq!(body["username"], "meuser");
}

// ===== Attachment tests =====

async fn create_issue_via_api(url: &str, client: &reqwest::Client, token: &str) -> String {
    let res = client
        .post(format!("{}/api/v1/issues", url))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "project_key": "TT",
            "issue_type": "task",
            "priority": "medium",
            "status_id": "00000000-0000-0000-0000-000000000001",
            "summary": "attachment test issue",
            "reporter_id": "00000000-0000-0000-0000-000000000001"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let body: serde_json::Value = res.json().await.unwrap();
    body["id"].as_str().unwrap().to_string()
}

fn multipart_file(
    name: &str,
    content_type: &str,
    bytes: &'static [u8],
) -> reqwest::multipart::Form {
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(name.to_string())
        .mime_str(content_type)
        .unwrap();
    reqwest::multipart::Form::new().part("file", part)
}

#[tokio::test]
async fn attachment_upload_download_delete_flow() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let auth = |req: reqwest::RequestBuilder| req.bearer_auth(&token);
    let issue_id = create_issue_via_api(&url, &client, &token).await;

    // upload
    let res = auth(client.post(format!("{}/api/v1/issues/{}/attachments", url, issue_id)))
        .multipart(multipart_file(
            "notes.txt",
            "text/plain",
            b"hello attachment",
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let body: serde_json::Value = res.json().await.unwrap();
    let id = body["id"].as_str().unwrap().to_string();
    assert_eq!(body["file_name"], "notes.txt");
    assert_eq!(body["size_bytes"], 16);

    // list
    let res = auth(client.get(format!("{}/api/v1/issues/{}/attachments", url, issue_id)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let list: serde_json::Value = res.json().await.unwrap();
    assert_eq!(list["attachments"].as_array().unwrap().len(), 1);

    // download
    let res = auth(client.get(format!("{}/api/v1/attachments/{}/download", url, id)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let bytes = res.bytes().await.unwrap();
    assert_eq!(&bytes[..], b"hello attachment");

    // delete
    let res = auth(client.delete(format!("{}/api/v1/attachments/{}", url, id)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);

    // list empty after delete
    let res = auth(client.get(format!("{}/api/v1/issues/{}/attachments", url, issue_id)))
        .send()
        .await
        .unwrap();
    let list: serde_json::Value = res.json().await.unwrap();
    assert_eq!(list["attachments"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn attachment_upload_requires_file_field() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let empty = reqwest::multipart::Form::new();
    let res = client
        .post(format!(
            "{}/api/v1/issues/00000000-0000-0000-0000-000000000101/attachments",
            url
        ))
        .bearer_auth(token)
        .multipart(empty)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn attachment_upload_unknown_issue_404() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let res = client
        .post(format!(
            "{}/api/v1/issues/00000000-0000-0000-0000-00c0ffee0001/attachments",
            url
        ))
        .bearer_auth(token)
        .multipart(multipart_file("x.txt", "text/plain", b"data"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}
// (unknown-issue test uses a random UUID above)

#[tokio::test]
async fn attachment_download_unknown_404() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let res = client
        .get(format!(
            "{}/api/v1/attachments/00000000-0000-0000-0000-00c0ffee0002/download",
            url
        ))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn attachment_upload_empty_file_400() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let issue_id = create_issue_via_api(&url, &client, &token).await;

    let res = client
        .post(format!("{}/api/v1/issues/{}/attachments", url, issue_id))
        .bearer_auth(token)
        .multipart(multipart_file("empty.txt", "text/plain", b""))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn attachments_require_auth() {
    let (url, client) = spawn_server().await;
    let res = client
        .get(format!(
            "{}/api/v1/issues/00000000-0000-0000-0000-000000000101/attachments",
            url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

// ===== Label tests =====

#[tokio::test]
async fn labels_crud_and_issue_attach_flow() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let issue_id = create_issue_via_api(&url, &client, &token).await;

    // create
    let res = client
        .post(format!("{}/api/v1/projects/TT/labels", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "bug", "color": "#ef4444"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let label: serde_json::Value = res.json().await.unwrap();
    let label_id = label["id"].as_str().unwrap().to_string();
    assert_eq!(label["name"], "bug");

    // list by project
    let res = client
        .get(format!("{}/api/v1/projects/TT/labels", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let list: serde_json::Value = res.json().await.unwrap();
    assert!(
        list["labels"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["name"] == "bug")
    );

    // attach to issue
    let res = client
        .post(format!("{}/api/v1/issues/{}/labels", url, issue_id))
        .bearer_auth(&token)
        .json(&serde_json::json!({"label_id": label_id}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);

    // list issue labels
    let res = client
        .get(format!("{}/api/v1/issues/{}/labels", url, issue_id))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let issue_labels: serde_json::Value = res.json().await.unwrap();
    assert_eq!(issue_labels["labels"].as_array().unwrap().len(), 1);

    // update
    let res = client
        .put(format!("{}/api/v1/labels/{}", url, label_id))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "critical-bug", "color": "#dc2626"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let updated: serde_json::Value = res.json().await.unwrap();
    assert_eq!(updated["name"], "critical-bug");

    // detach
    let res = client
        .delete(format!(
            "{}/api/v1/issues/{}/labels/{}",
            url, issue_id, label_id
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);

    // delete label
    let res = client
        .delete(format!("{}/api/v1/labels/{}", url, label_id))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);
}

#[tokio::test]
async fn label_create_empty_name_400() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let res = client
        .post(format!("{}/api/v1/projects/TT/labels", url))
        .bearer_auth(token)
        .json(&serde_json::json!({"name": "  ", "color": "#000000"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn label_create_unknown_project_404() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let res = client
        .post(format!("{}/api/v1/projects/NOPE/labels", url))
        .bearer_auth(token)
        .json(&serde_json::json!({"name": "x", "color": "#000000"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn label_attach_unknown_label_404() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let issue_id = create_issue_via_api(&url, &client, &token).await;

    let res = client
        .post(format!("{}/api/v1/issues/{}/labels", url, issue_id))
        .bearer_auth(token)
        .json(&serde_json::json!({"label_id": "00000000-0000-0000-0000-00c0ffee0099"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn labels_require_auth() {
    let (url, client) = spawn_server().await;
    let res = client
        .get(format!("{}/api/v1/projects/TT/labels", url))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

// ===== Issue link tests =====

async fn create_second_issue(
    url: &str,
    client: &reqwest::Client,
    token: &str,
    summary: &str,
) -> String {
    let res = client
        .post(format!("{}/api/v1/issues", url))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "project_key": "TT",
            "issue_type": "task",
            "priority": "medium",
            "status_id": "00000000-0000-0000-0000-000000000001",
            "summary": summary,
            "reporter_id": "00000000-0000-0000-0000-000000000001"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let body: serde_json::Value = res.json().await.unwrap();
    body["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn issue_links_create_list_delete_flow() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let a = create_issue_via_api(&url, &client, &token).await;
    let b_id = create_second_issue(&url, &client, &token, "link target").await;

    // fetch key of b
    let res = client
        .get(format!("{}/api/v1/issues/{}", url, b_id))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let b: serde_json::Value = res.json().await.unwrap();
    let b_key = b["key"].as_str().unwrap().to_string();

    // create link a -> b (blocks)
    let res = client
        .post(format!("{}/api/v1/issues/{}/links", url, a))
        .bearer_auth(&token)
        .json(&serde_json::json!({"target_key": b_key, "link_type": "blocks"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let link: serde_json::Value = res.json().await.unwrap();
    let link_id = link["id"].as_str().unwrap().to_string();
    assert_eq!(link["link_type"], "blocks");
    assert_eq!(link["target_key"], b_key);

    // list links from both sides
    for iid in [&a, &b_id] {
        let res = client
            .get(format!("{}/api/v1/issues/{}/links", url, iid))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        let list: serde_json::Value = res.json().await.unwrap();
        assert_eq!(list["links"].as_array().unwrap().len(), 1);
    }

    // delete
    let res = client
        .delete(format!("{}/api/v1/issue-links/{}", url, link_id))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);
}

#[tokio::test]
async fn issue_link_self_link_400() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let a = create_issue_via_api(&url, &client, &token).await;

    let res = client
        .get(format!("{}/api/v1/issues/{}", url, a))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let issue: serde_json::Value = res.json().await.unwrap();
    let key = issue["key"].as_str().unwrap();

    let res = client
        .post(format!("{}/api/v1/issues/{}/links", url, a))
        .bearer_auth(token)
        .json(&serde_json::json!({"target_key": key, "link_type": "relates"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn issue_link_unknown_type_400() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let a = create_issue_via_api(&url, &client, &token).await;

    let res = client
        .post(format!("{}/api/v1/issues/{}/links", url, a))
        .bearer_auth(token)
        .json(&serde_json::json!({"target_key": "TT-999", "link_type": "banana"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn issue_link_unknown_target_404() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let a = create_issue_via_api(&url, &client, &token).await;

    let res = client
        .post(format!("{}/api/v1/issues/{}/links", url, a))
        .bearer_auth(token)
        .json(&serde_json::json!({"target_key": "TT-424242", "link_type": "relates"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

// ===== Member edge-case tests =====

#[tokio::test]
async fn member_readd_is_idempotent_upsert() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let project_id = test_project_id();

    // register a second user
    let register = client
        .post(format!("{}/api/v1/auth/register", url))
        .json(&serde_json::json!({
            "email": "readd@example.com",
            "username": "readd",
            "name": "Re Add",
            "password": "secret123"
        }))
        .send()
        .await
        .unwrap();
    let user: serde_json::Value = register.json().await.unwrap();
    let user_id = user["user_id"].as_str().unwrap();

    // add twice
    for expected_role in ["member", "admin"] {
        let res = client
            .post(format!("{}/api/v1/projects/{project_id}/members", url))
            .bearer_auth(&token)
            .json(&serde_json::json!({"user_id": user_id, "role": expected_role}))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 201);
        let body: serde_json::Value = res.json().await.unwrap();
        assert_eq!(body["role"], expected_role);
    }

    // list shows exactly one membership with the latest role
    let res = client
        .get(format!("{}/api/v1/projects/{project_id}/members", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let list: serde_json::Value = res.json().await.unwrap();
    let members = list["members"].as_array().unwrap();
    let hits = members
        .iter()
        .filter(|m| m["user_id"].as_str().unwrap() == user_id)
        .collect::<Vec<_>>();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["role"], "admin");
}

#[tokio::test]
async fn member_add_unknown_project_404() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let res = client
        .post(format!("{}/api/v1/projects/00000000-0000-0000-0000-00c0ffee7777/members", url))
        .bearer_auth(token)
        .json(&serde_json::json!({"user_id": "00000000-0000-0000-0000-000000000001", "role": "member"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn member_remove_returns_204() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let project_id = test_project_id();

    let register = client
        .post(format!("{}/api/v1/auth/register", url))
        .json(&serde_json::json!({
            "email": "remove-me@example.com",
            "username": "removeme",
            "name": "Remove Me",
            "password": "secret123"
        }))
        .send()
        .await
        .unwrap();
    let user: serde_json::Value = register.json().await.unwrap();
    let user_id = user["user_id"].as_str().unwrap();

    let add = client
        .post(format!("{}/api/v1/projects/{project_id}/members", url))
        .bearer_auth(&token)
        .json(&serde_json::json!({"user_id": user_id, "role": "member"}))
        .send()
        .await
        .unwrap();
    assert_eq!(add.status(), 201);

    let remove = client
        .delete(format!(
            "{}/api/v1/projects/{project_id}/members/{user_id}",
            url
        ))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(remove.status(), 204);
}

// ===== Sprint workflow tests =====

#[tokio::test]
async fn sprint_lifecycle_create_start_close() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    // create
    let create = client
        .post(format!("{url}/api/v1/projects/TT/sprints"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "Sprint 1", "goal": "Ship it"}))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);
    let sprint: serde_json::Value = create.json().await.unwrap();
    let sprint_id = sprint["id"].as_str().unwrap().to_string();
    assert_eq!(sprint["state"], "future");

    // start
    let start = client
        .post(format!(
            "{url}/api/v1/projects/TT/sprints/{sprint_id}/start"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(start.status(), 200);
    let started: serde_json::Value = start.json().await.unwrap();
    assert_eq!(started["state"], "active");

    // close
    let close = client
        .post(format!(
            "{url}/api/v1/projects/TT/sprints/{sprint_id}/close"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(close.status(), 200);
    let closed: serde_json::Value = close.json().await.unwrap();
    assert_eq!(closed["state"], "closed");

    // list contains it
    let list = client
        .get(format!("{url}/api/v1/projects/TT/sprints"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), 200);
    let body: serde_json::Value = list.json().await.unwrap();
    assert!(
        body["sprints"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"].as_str() == Some(sprint_id.as_str()))
    );
}

#[tokio::test]
async fn sprint_move_and_remove_issue() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let sprint = client
        .post(format!("{url}/api/v1/projects/TT/sprints"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "Sprint M"}))
        .send()
        .await
        .unwrap();
    assert_eq!(sprint.status(), 201);
    let sprint_json: serde_json::Value = sprint.json().await.unwrap();
    let sprint_id = sprint_json["id"].as_str().unwrap().to_string();

    let issue_id = create_issue_via_api(&url, &client, &token).await;

    // move in
    let mv = client
        .post(format!(
            "{url}/api/v1/projects/TT/sprints/{sprint_id}/issues"
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"issue_id": issue_id}))
        .send()
        .await
        .unwrap();
    assert_eq!(mv.status(), 200);

    // remove
    let rm = client
        .post(format!(
            "{url}/api/v1/projects/TT/sprints/{sprint_id}/remove-issue"
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"issue_id": issue_id}))
        .send()
        .await
        .unwrap();
    assert!(
        rm.status() == 200 || rm.status() == 204,
        "unexpected {}",
        rm.status()
    );
}

#[tokio::test]
async fn sprint_update_and_get() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let created = client
        .post(format!("{url}/api/v1/projects/TT/sprints"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "Before"}))
        .send()
        .await
        .unwrap();
    let created_json: serde_json::Value = created.json().await.unwrap();
    let sid = created_json["id"].as_str().unwrap().to_string();

    let upd = client
        .patch(format!("{url}/api/v1/projects/TT/sprints/{sid}"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "After", "goal": "updated"}))
        .send()
        .await
        .unwrap();
    assert_eq!(upd.status(), 200);
    let upd_json: serde_json::Value = upd.json().await.unwrap();
    assert_eq!(upd_json["name"], "After");

    let got = client
        .get(format!("{url}/api/v1/projects/TT/sprints/{sid}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(got.status(), 200);
    let got_json: serde_json::Value = got.json().await.unwrap();
    assert_eq!(got_json["goal"], "updated");
}

#[tokio::test]
async fn sprint_unknown_404() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let res = client
        .get(format!(
            "{url}/api/v1/projects/TT/sprints/00000000-0000-0000-0000-00c0ffee7777"
        ))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

// ===== Workflow (statuses/transitions/issue-types) tests =====

#[tokio::test]
async fn workflow_lists_reachable() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    for path in [
        "/api/v1/statuses",
        "/api/v1/transitions",
        "/api/v1/issue-types",
    ] {
        let res = client
            .get(format!("{url}{path}"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200, "{path}");
        let body: serde_json::Value = res.json().await.unwrap();
        assert!(body.as_array().is_some(), "{path} did not return an array");
    }
}

#[tokio::test]
async fn workflow_lists_require_auth() {
    let (url, client) = spawn_server().await;
    for path in [
        "/api/v1/statuses",
        "/api/v1/transitions",
        "/api/v1/issue-types",
    ] {
        let res = client.get(format!("{url}{path}")).send().await.unwrap();
        assert_eq!(res.status(), 401, "{path}");
    }
}

// ===== SSE events tests =====

#[tokio::test]
async fn sse_stream_receives_issue_events() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    // subscribe (SSE), then create an issue and expect the event
    let stream = client
        .get(format!("{url}/api/v1/events"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(stream.status(), 200);
    assert!(
        stream
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );

    // spawn a reader that collects events into a channel
    use futures_util::StreamExt;
    let mut byte_stream = stream.bytes_stream();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(8);
    tokio::spawn(async move {
        let mut buf = String::new();
        while let Some(chunk) = byte_stream.next().await {
            match chunk {
                Ok(bytes) => {
                    buf.push_str(&String::from_utf8_lossy(&bytes));
                    // split complete SSE frames
                    while let Some(pos) = buf.find("\n\n") {
                        let frame: String = buf.drain(..pos + 2).collect();
                        let _ = tx.send(frame).await;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // create an issue -> expect issue_created event
    let created = client
        .post(format!("{url}/api/v1/issues"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "project_key": "TT",
            "issue_type": "task",
            "priority": "medium",
            "status_id": "00000000-0000-0000-0000-000000000001",
            "summary": "sse test issue",
            "reporter_id": "00000000-0000-0000-0000-000000000001"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 201);

    let mut got_created = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await {
            Ok(Some(frame)) => {
                if frame.contains("event: tracker") && frame.contains("issue_created") {
                    got_created = true;
                    break;
                }
            }
            _ => continue,
        }
    }
    assert!(got_created, "did not receive issue_created SSE event");
}

#[tokio::test]
async fn sse_requires_auth() {
    let (url, client) = spawn_server().await;
    let res = client
        .get(format!("{url}/api/v1/events"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn sse_accepts_query_token() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let res = client
        .get(format!("{url}/api/v1/events?access_token={token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert!(
        res.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );
}

#[tokio::test]
async fn sse_query_token_rejected_for_other_paths() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    // access_token query must NOT authorize non-SSE endpoints
    let res = client
        .get(format!("{url}/api/v1/projects?access_token={token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

fn make_notification(recipient: UserId, offset_secs: i64, title: &str) -> Notification {
    Notification {
        id: shared::NotificationId::new(),
        recipient_id: recipient,
        event_type: "issue_assigned".into(),
        entity_type: "issue".into(),
        entity_id: Some(shared::IssueId::new().as_uuid()),
        actor_id: Some(UserId::new()),
        title: title.into(),
        body: Some("body".into()),
        is_read: false,
        read_at: None,
        action_url: Some("/issues/TT-1".into()),
        metadata: serde_json::Value::Null,
        created_at: shared::now() + chrono::Duration::seconds(offset_secs),
    }
}

#[tokio::test]
async fn notifications_list_returns_newest_ten_with_unread_count() {
    let (url, client, repo) = spawn_server_with_notifications().await;
    let token = login_token(&url, &client).await;
    let user_id = test_user().id;

    for i in 0..12 {
        repo.save(&make_notification(user_id, i, &format!("N{i}")))
            .await
            .unwrap();
    }

    let res = client
        .get(format!("{url}/api/v1/notifications"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["unread_count"], 12);
    let list = body["notifications"].as_array().unwrap();
    assert_eq!(list.len(), 10);
    assert_eq!(list[0]["title"], "N11");
    assert_eq!(list[9]["title"], "N2");
}

#[tokio::test]
async fn notifications_read_marks_only_own_unread_and_isolates_ownership() {
    let (url, client, repo) = spawn_server_with_notifications().await;
    let token = login_token(&url, &client).await;
    let user_id = test_user().id;
    let other = UserId::new();
    let own = make_notification(user_id, 0, "own");
    let foreign = make_notification(other, 0, "foreign");
    repo.save(&own).await.unwrap();
    repo.save(&foreign).await.unwrap();

    // malformed UUID → 400
    let bad = client
        .patch(format!("{url}/api/v1/notifications/not-a-uuid/read"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 400);

    // foreign notification → 404
    let foreign_res = client
        .patch(format!("{url}/api/v1/notifications/{}/read", foreign.id))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(foreign_res.status(), 404);

    // own notification → 204
    let own_res = client
        .patch(format!("{url}/api/v1/notifications/{}/read", own.id))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(own_res.status(), 204);

    // after read, list is empty for own user
    let list = client
        .get(format!("{url}/api/v1/notifications"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = list.json().await.unwrap();
    assert_eq!(body["unread_count"], 0);
}

#[tokio::test]
async fn notifications_read_all_marks_all_own_read() {
    let (url, client, repo) = spawn_server_with_notifications().await;
    let token = login_token(&url, &client).await;
    let user_id = test_user().id;
    let other = UserId::new();
    repo.save(&make_notification(user_id, 0, "a"))
        .await
        .unwrap();
    repo.save(&make_notification(user_id, 1, "b"))
        .await
        .unwrap();
    repo.save(&make_notification(other, 0, "c")).await.unwrap();

    let res = client
        .post(format!("{url}/api/v1/notifications/read-all"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);

    let list = client
        .get(format!("{url}/api/v1/notifications"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = list.json().await.unwrap();
    assert_eq!(body["unread_count"], 0);
    // other user's notifications untouched
    assert_eq!(repo.list_unread(other).await.unwrap().len(), 1);
}

#[tokio::test]
async fn notification_settings_get_returns_defaults_without_persisting() {
    let (url, client, repo) = spawn_server_with_notifications().await;
    let token = login_token(&url, &client).await;
    let user_id = test_user().id;

    let res = client
        .get(format!("{url}/api/v1/notification-settings"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["email_frequency"], "immediate");
    assert_eq!(body["disabled_event_types"].as_array().unwrap().len(), 0);
    assert_eq!(body["notify_own_changes"], false);
    // not persisted
    assert!(repo.get_settings(user_id).await.is_err());
}

#[tokio::test]
async fn notification_settings_patch_persists_and_round_trips() {
    let (url, client, _repo) = spawn_server_with_notifications().await;
    let token = login_token(&url, &client).await;

    let res = client
        .patch(format!("{url}/api/v1/notification-settings"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "email_frequency": "daily",
            "disabled_event_types": ["issue_commented"],
            "notify_own_changes": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["email_frequency"], "daily");
    assert_eq!(body["notify_own_changes"], true);

    // GET after PATCH returns persisted values
    let get_res = client
        .get(format!("{url}/api/v1/notification-settings"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let get_body: serde_json::Value = get_res.json().await.unwrap();
    assert_eq!(get_body["email_frequency"], "daily");
    assert_eq!(get_body["notify_own_changes"], true);
}

#[tokio::test]
async fn notification_settings_patch_invalid_frequency_returns_400() {
    let (url, client, _repo) = spawn_server_with_notifications().await;
    let token = login_token(&url, &client).await;

    let res = client
        .patch(format!("{url}/api/v1/notification-settings"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "email_frequency": "weekly",
            "disabled_event_types": [],
            "notify_own_changes": false
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}
// ─── Report integration tests ──────────────────────────────────────

async fn spawn_server_with_reports() -> (
    String,
    reqwest::Client,
    Arc<MemoryIssueRepository>,
    Arc<MemorySprintRepository>,
    Arc<MemoryIssueStatusHistoryRepository>,
) {
    let user = test_user();
    let project_id = shared::ProjectId::from_uuid(
        uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
    );
    let project = Project {
        id: project_id,
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
    let done =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap());

    let statuses = vec![
        domain::Status {
            id: todo,
            name: "Todo".into(),
            category: StatusCategory::Todo,
            position: 0,
            is_default: true,
            is_closed: false,
        },
        domain::Status {
            id: in_progress,
            name: "In Progress".into(),
            category: StatusCategory::InProgress,
            position: 1,
            is_default: false,
            is_closed: false,
        },
        domain::Status {
            id: done,
            name: "Done".into(),
            category: StatusCategory::Done,
            position: 2,
            is_default: false,
            is_closed: true,
        },
    ];
    let status_repo = Arc::new(domain::MemoryStatusRepository::new(statuses));

    let users = Arc::new(MemoryUserRepository::default());
    users.save(&user).await.unwrap();
    let projects = Arc::new(MemoryProjectRepository::default());
    projects.save(&project).await.unwrap();
    let issues = Arc::new(MemoryIssueRepository::default());
    let boards = Arc::new(MemoryBoardRepository::default());
    let sprints = Arc::new(MemorySprintRepository::default());
    let history = Arc::new(MemoryIssueStatusHistoryRepository::default());

    let repos = Arc::new(domain::Repositories {
        users: users.clone(),
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
        notifications: Arc::new(MemoryNotificationRepository::default()),
        notification_settings: Arc::new(MemoryNotificationRepository::default()),
        issue_status_history: history.clone(),
        watchers: Arc::new(domain::MemoryWatcherRepository::default()),
        votes: Arc::new(domain::MemoryVoteRepository::default()),
        components: Arc::new(domain::StubProjectComponentRepository),
        versions: Arc::new(domain::StubProjectVersionRepository),
        custom_fields: Arc::new(domain::StubCustomFieldRepository),
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
    (url, client, issues, sprints, history)
}

fn make_test_issue(
    id: &str,
    project_id: shared::ProjectId,
    num: u32,
    status_id: StatusId,
    sprint_id: Option<shared::SprintId>,
    created_at: shared::Timestamp,
) -> domain::Issue {
    domain::Issue {
        id: shared::IssueId::from_uuid(uuid::Uuid::parse_str(id).unwrap()),
        project_id,
        key: shared::IssueKey::new(ProjectKey::new("TT"), num),
        issue_type: shared::IssueType::Task,
        status_id,
        summary: "test issue".into(),
        description: None,
        assignee_id: None,
        reporter_id: test_user().id,
        priority: shared::Priority::Medium,
        labels: vec![],
        sprint_id,
        position: 0.0,
        due_date: None,
        original_estimate_seconds: None,
        remaining_estimate_seconds: None,
        time_spent_seconds: 0,
        component_id: None,
        affected_version_id: None,
        fix_version_id: None,
        created_at,
        updated_at: created_at,
        deleted_at: None,
        events: vec![],
    }
}

#[tokio::test]
async fn reports_velocity_requires_auth() {
    let (url, client, _issues, _sprints, _history) = spawn_server_with_reports().await;
    let res = client
        .get(format!(
            "{url}/api/v1/reports/velocity?project_id=22222222-2222-2222-2222-222222222222"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn reports_velocity_returns_data() {
    let (url, client, issues, sprints, _history) = spawn_server_with_reports().await;
    let token = login_token(&url, &client).await;
    let project_id = shared::ProjectId::from_uuid(
        uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
    );
    let done =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap());
    let todo =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());

    let sprint = domain::Sprint {
        id: shared::SprintId::new(),
        project_id,
        name: "Sprint 1".into(),
        goal: None,
        state: domain::SprintState::Closed,
        start_date: Some(shared::now() - chrono::Duration::days(10)),
        end_date: Some(shared::now()),
        velocity: None,
    };
    sprints.save(&sprint).await.unwrap();

    for i in 1..=3u32 {
        let st = if i <= 2 { done } else { todo };
        let issue = make_test_issue(
            &format!("aaaa0000-0000-0000-0000-00000000000{i}"),
            project_id,
            i,
            st,
            Some(sprint.id),
            shared::now() - chrono::Duration::days(5),
        );
        issues.save(&issue).await.unwrap();
    }

    let res = client
        .get(format!(
            "{url}/api/v1/reports/velocity?project_id=22222222-2222-2222-2222-222222222222&count=6"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sprints_arr = body["sprints"].as_array().unwrap();
    assert_eq!(sprints_arr.len(), 1);
    assert_eq!(sprints_arr[0]["name"], "Sprint 1");
    assert_eq!(sprints_arr[0]["committed"], 3);
    assert_eq!(sprints_arr[0]["completed"], 2);
}

#[tokio::test]
async fn reports_burndown_returns_data() {
    let (url, client, issues, sprints, _history) = spawn_server_with_reports().await;
    let token = login_token(&url, &client).await;
    let project_id = shared::ProjectId::from_uuid(
        uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
    );
    let todo =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());

    let sprint = domain::Sprint {
        id: shared::SprintId::new(),
        project_id,
        name: "Active Sprint".into(),
        goal: None,
        state: domain::SprintState::Active,
        start_date: Some(shared::now() - chrono::Duration::days(2)),
        end_date: Some(shared::now() + chrono::Duration::days(2)),
        velocity: None,
    };
    sprints.save(&sprint).await.unwrap();

    for i in 1..=5u32 {
        let issue = make_test_issue(
            &format!("bbbb0000-0000-0000-0000-00000000000{i}"),
            project_id,
            i,
            todo,
            Some(sprint.id),
            shared::now() - chrono::Duration::days(2),
        );
        issues.save(&issue).await.unwrap();
    }

    let res = client
        .get(format!(
            "{url}/api/v1/reports/burndown?sprint_id={}",
            sprint.id
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["sprint_name"], "Active Sprint");
    assert!(!body["points"].as_array().unwrap().is_empty());
    assert_eq!(body["points"][0]["remaining"], 5);
}

#[tokio::test]
async fn reports_cumulative_flow_returns_data() {
    let (url, client, issues, _sprints, history) = spawn_server_with_reports().await;
    let token = login_token(&url, &client).await;
    let project_id = shared::ProjectId::from_uuid(
        uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
    );
    let todo =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let done =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap());

    let created = shared::now() - chrono::Duration::days(2);
    let issue = make_test_issue(
        "cccc0000-0000-0000-0000-000000000001",
        project_id,
        1,
        done,
        None,
        created,
    );
    issues.save(&issue).await.unwrap();

    history.save_with_project(
        &domain::IssueStatusHistory {
            id: shared::IssueStatusHistoryId::new(),
            issue_id: issue.id,
            from_status_id: None,
            to_status_id: todo,
            changed_by_id: test_user().id,
            changed_at: created,
        },
        project_id,
    );
    history.save_with_project(
        &domain::IssueStatusHistory {
            id: shared::IssueStatusHistoryId::new(),
            issue_id: issue.id,
            from_status_id: Some(todo),
            to_status_id: done,
            changed_by_id: test_user().id,
            changed_at: shared::now() - chrono::Duration::days(1),
        },
        project_id,
    );

    let res = client
        .get(format!(
            "{url}/api/v1/reports/cumulative-flow?project_id=22222222-2222-2222-2222-222222222222"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let points = body["points"].as_array().unwrap();
    assert!(!points.is_empty());
    let last = points.last().unwrap();
    assert_eq!(last["done"], 1);
}

#[tokio::test]
async fn reports_control_chart_returns_data() {
    let (url, client, issues, _sprints, history) = spawn_server_with_reports().await;
    let token = login_token(&url, &client).await;
    let project_id = shared::ProjectId::from_uuid(
        uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
    );
    let todo =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let done =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap());

    let created = shared::now() - chrono::Duration::days(5);
    let done_time = shared::now() - chrono::Duration::days(1);
    let issue = make_test_issue(
        "dddd0000-0000-0000-0000-000000000001",
        project_id,
        1,
        done,
        None,
        created,
    );
    issues.save(&issue).await.unwrap();

    history.save_with_project(
        &domain::IssueStatusHistory {
            id: shared::IssueStatusHistoryId::new(),
            issue_id: issue.id,
            from_status_id: None,
            to_status_id: todo,
            changed_by_id: test_user().id,
            changed_at: created,
        },
        project_id,
    );
    history.save_with_project(
        &domain::IssueStatusHistory {
            id: shared::IssueStatusHistoryId::new(),
            issue_id: issue.id,
            from_status_id: Some(todo),
            to_status_id: done,
            changed_by_id: test_user().id,
            changed_at: done_time,
        },
        project_id,
    );

    let res = client
        .get(format!(
            "{url}/api/v1/reports/control-chart?project_id=22222222-2222-2222-2222-222222222222"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let points = body["points"].as_array().unwrap();
    assert_eq!(points.len(), 1);
    assert!(points[0]["issue_key"].as_str().unwrap().starts_with("TT-"));
    let cycle = points[0]["cycle_time_days"].as_f64().unwrap();
    assert!((cycle - 4.0).abs() < 0.2);
}

#[tokio::test]
async fn reports_velocity_invalid_project_id_returns_400() {
    let (url, client, _issues, _sprints, _history) = spawn_server_with_reports().await;
    let token = login_token(&url, &client).await;

    let res = client
        .get(format!(
            "{url}/api/v1/reports/velocity?project_id=not-a-uuid"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

// ─── Soft-delete / restore / purge / trash integration tests ─────────

#[tokio::test]
async fn soft_delete_issue_returns_204_and_hides_from_list() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let issue_id = create_issue_via_api(&url, &client, &token).await;

    // delete (soft)
    let del = client
        .delete(format!("{url}/api/v1/issues/{issue_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);

    // GET returns 404 because get_by_id filters deleted
    let get = client
        .get(format!("{url}/api/v1/issues/{issue_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), 404);
}

#[tokio::test]
async fn restore_deleted_issue_returns_200_and_makes_it_visible() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let issue_id = create_issue_via_api(&url, &client, &token).await;

    // soft-delete first
    let del = client
        .delete(format!("{url}/api/v1/issues/{issue_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);

    // restore
    let restore = client
        .post(format!("{url}/api/v1/issues/{issue_id}/restore"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(restore.status(), 200);
    let body: serde_json::Value = restore.json().await.unwrap();
    assert_eq!(body["id"], issue_id);

    // GET works again
    let get = client
        .get(format!("{url}/api/v1/issues/{issue_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), 200);
}

#[tokio::test]
async fn purge_deleted_issue_returns_204_and_removes_permanently() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let issue_id = create_issue_via_api(&url, &client, &token).await;

    // soft-delete first
    let del = client
        .delete(format!("{url}/api/v1/issues/{issue_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);

    // purge (hard delete)
    let purge = client
        .delete(format!("{url}/api/v1/issues/{issue_id}/trash"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(purge.status(), 204);

    // restore after purge → 404 (issue gone)
    let restore = client
        .post(format!("{url}/api/v1/issues/{issue_id}/restore"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(restore.status(), 404);
}

#[tokio::test]
async fn list_trash_shows_soft_deleted_issues() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let issue_id = create_issue_via_api(&url, &client, &token).await;

    // trash is empty before delete
    let trash0 = client
        .get(format!("{url}/api/v1/projects/TT/trash"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(trash0.status(), 200);
    let body: serde_json::Value = trash0.json().await.unwrap();
    assert!(body["issues"].as_array().unwrap().is_empty());

    // soft-delete
    let del = client
        .delete(format!("{url}/api/v1/issues/{issue_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);

    // trash now contains the deleted issue
    let trash1 = client
        .get(format!("{url}/api/v1/projects/TT/trash"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(trash1.status(), 200);
    let body: serde_json::Value = trash1.json().await.unwrap();
    let list = body["issues"].as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], issue_id);
}

// ─── Auth refresh / logout / me / users list integration tests ───────

#[tokio::test]
async fn auth_refresh_returns_new_access_token() {
    let (url, client) = spawn_server().await;
    let login = client
        .post(format!("{url}/api/v1/auth/login"))
        .json(&serde_json::json!({"email":"demo@example.com","password":"demo"}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = login.json().await.unwrap();
    let access_token = body["access_token"].as_str().unwrap().to_string();
    let refresh_token = body["refresh_token"].as_str().unwrap().to_string();

    // The refresh endpoint is behind the auth middleware, so we need the bearer token too
    let refresh_res = client
        .post(format!("{url}/api/v1/auth/refresh"))
        .bearer_auth(&access_token)
        .json(&serde_json::json!({"refresh_token": refresh_token}))
        .send()
        .await
        .unwrap();
    assert_eq!(refresh_res.status(), 200);
    let body: serde_json::Value = refresh_res.json().await.unwrap();
    let new_access = body["access_token"].as_str().unwrap().to_string();
    assert!(!new_access.is_empty());
    assert_eq!(body["token_type"], "Bearer");
    assert!(body["expires_in"].as_u64().is_some());
}

#[tokio::test]
async fn auth_logout_clears_refresh_and_invalidates_token() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    // logout
    let logout = client
        .post(format!("{url}/api/v1/auth/logout"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(logout.status(), 204);

    // after logout, refresh with old token should fail (refresh_token_hash cleared)
    // We need to get the refresh_token from login first — re-login to get it
    let login = client
        .post(format!("{url}/api/v1/auth/login"))
        .json(&serde_json::json!({"email":"demo@example.com","password":"demo"}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = login.json().await.unwrap();
    let refresh_token = body["refresh_token"].as_str().unwrap().to_string();

    // logout again
    let token2 = body["access_token"].as_str().unwrap().to_string();
    let logout2 = client
        .post(format!("{url}/api/v1/auth/logout"))
        .bearer_auth(&token2)
        .send()
        .await
        .unwrap();
    assert_eq!(logout2.status(), 204);

    // refresh should now fail
    let refresh_res = client
        .post(format!("{url}/api/v1/auth/refresh"))
        .json(&serde_json::json!({"refresh_token": refresh_token}))
        .send()
        .await
        .unwrap();
    assert_eq!(refresh_res.status(), 401);
}

#[tokio::test]
async fn auth_me_returns_current_user_info() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let res = client
        .get(format!("{url}/api/v1/auth/me"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["email"], "demo@example.com");
    assert_eq!(body["username"], "demo");
    assert_eq!(body["display_name"], "Demo User");
}

#[tokio::test]
async fn users_list_returns_all_users() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    // register a second user so list has > 1
    let _reg = client
        .post(format!("{url}/api/v1/auth/register"))
        .json(&serde_json::json!({
            "email": "second@example.com",
            "username": "second",
            "name": "Second",
            "password": "secret123"
        }))
        .send()
        .await
        .unwrap();

    let res = client
        .get(format!("{url}/api/v1/users"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let users = body["users"].as_array().unwrap();
    assert!(users.len() >= 2);
    assert!(users.iter().any(|u| u["email"] == "demo@example.com"));
    assert!(users.iter().any(|u| u["email"] == "second@example.com"));
}

// ─── Watchers / votes integration tests ──────────────────────────────

#[tokio::test]
async fn watch_unwatch_and_list_watchers_flow() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let issue_id = create_issue_via_api(&url, &client, &token).await;

    // watch
    let watch = client
        .post(format!("{url}/api/v1/issues/{issue_id}/watch"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(watch.status(), 204);

    // list watchers — should contain our user
    let list = client
        .get(format!("{url}/api/v1/issues/{issue_id}/watchers"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), 200);
    let body: serde_json::Value = list.json().await.unwrap();
    let watchers = body["watchers"].as_array().unwrap();
    assert_eq!(watchers.len(), 1);
    assert_eq!(watchers[0]["username"], "demo");

    // unwatch
    let unwatch = client
        .delete(format!("{url}/api/v1/issues/{issue_id}/watch"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(unwatch.status(), 204);

    // list watchers — should be empty now
    let list2 = client
        .get(format!("{url}/api/v1/issues/{issue_id}/watchers"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body2: serde_json::Value = list2.json().await.unwrap();
    assert_eq!(body2["watchers"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn watch_requires_auth() {
    let (url, client) = spawn_server().await;
    let res = client
        .post(format!(
            "{url}/api/v1/issues/00000000-0000-0000-0000-000000000001/watch"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn vote_unvote_and_list_votes_flow() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;
    let issue_id = create_issue_via_api(&url, &client, &token).await;

    // vote
    let vote = client
        .post(format!("{url}/api/v1/issues/{issue_id}/vote"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(vote.status(), 201);
    let body: serde_json::Value = vote.json().await.unwrap();
    assert!(!body["voted_at"].as_str().unwrap().is_empty());

    // list votes
    let list = client
        .get(format!("{url}/api/v1/issues/{issue_id}/votes"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), 200);
    let body: serde_json::Value = list.json().await.unwrap();
    let votes = body["votes"].as_array().unwrap();
    assert_eq!(votes.len(), 1);
    assert_eq!(body["count"], 1);

    // unvote
    let unvote = client
        .delete(format!("{url}/api/v1/issues/{issue_id}/vote"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(unvote.status(), 204);

    // list votes — empty
    let list2 = client
        .get(format!("{url}/api/v1/issues/{issue_id}/votes"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body2: serde_json::Value = list2.json().await.unwrap();
    assert_eq!(body2["votes"].as_array().unwrap().len(), 0);
    assert_eq!(body2["count"], 0);
}

#[tokio::test]
async fn vote_requires_auth() {
    let (url, client) = spawn_server().await;
    let res = client
        .post(format!(
            "{url}/api/v1/issues/00000000-0000-0000-0000-000000000001/vote"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn list_watchers_and_votes_require_auth() {
    let (url, client) = spawn_server().await;
    let res = client
        .get(format!(
            "{url}/api/v1/issues/00000000-0000-0000-0000-000000000001/watchers"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);

    let res = client
        .get(format!(
            "{url}/api/v1/issues/00000000-0000-0000-0000-000000000001/votes"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

// ─── Server variant with memory repos for components/versions/custom-fields ──

async fn spawn_server_with_memory_repos() -> (String, reqwest::Client) {
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
    let issues = Arc::new(MemoryIssueRepository::default());
    let boards = Arc::new(MemoryBoardRepository::default());
    boards.save(&board).await.unwrap();
    let sprints = Arc::new(MemorySprintRepository::default());

    let notifications = Arc::new(MemoryNotificationRepository::default());
    let repos = Arc::new(domain::Repositories {
        users: users.clone(),
        audit_logs: Arc::new(domain::StubAuditLogRepository),
        system_settings: Arc::new(domain::StubSystemSettingRepository),
        projects: projects.clone(),
        issues: issues.clone(),
        boards: boards.clone(),
        sprints: sprints.clone(),
        comments: Arc::new(MemoryCommentRepository::default()),
        worklogs: Arc::new(MemoryWorklogRepository::default()),
        members: Arc::new(MemoryProjectMemberRepository::default()),
        statuses: Arc::new(domain::StubStatusRepository),
        transitions: Arc::new(domain::StubWorkflowTransitionRepository),
        issue_types: Arc::new(domain::StubIssueTypeRepository),
        attachments: Arc::new(MemoryAttachmentRepository::default()),
        labels: Arc::new(MemoryLabelRepository::default()),
        issue_links: Arc::new(MemoryIssueLinkRepository::default()),
        notifications: notifications.clone(),
        notification_settings: notifications.clone(),
        issue_status_history: Arc::new(domain::MemoryIssueStatusHistoryRepository::default()),
        watchers: Arc::new(domain::MemoryWatcherRepository::default()),
        votes: Arc::new(domain::MemoryVoteRepository::default()),
        components: Arc::new(domain::MemoryProjectComponentRepository::default()),
        versions: Arc::new(domain::MemoryProjectVersionRepository::default()),
        custom_fields: Arc::new(domain::MemoryCustomFieldRepository::default()),
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
    (url, client)
}

// ─── Components integration tests ────────────────────────────────────

#[tokio::test]
async fn components_crud_flow() {
    let (url, client) = spawn_server_with_memory_repos().await;
    let token = login_token(&url, &client).await;

    // list — empty initially
    let list0 = client
        .get(format!("{url}/api/v1/projects/TT/components"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(list0.status(), 200);
    let body: serde_json::Value = list0.json().await.unwrap();
    assert!(body["components"].as_array().unwrap().is_empty());

    // create
    let create = client
        .post(format!("{url}/api/v1/projects/TT/components"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "Backend", "description": "Backend layer"}))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);
    let comp: serde_json::Value = create.json().await.unwrap();
    let comp_id = comp["id"].as_str().unwrap().to_string();
    assert_eq!(comp["name"], "Backend");
    assert_eq!(comp["description"], "Backend layer");

    // list — contains our component
    let list1 = client
        .get(format!("{url}/api/v1/projects/TT/components"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = list1.json().await.unwrap();
    let components = body["components"].as_array().unwrap();
    assert_eq!(components.len(), 1);
    assert_eq!(components[0]["name"], "Backend");

    // update
    let update = client
        .put(format!("{url}/api/v1/projects/TT/components/{comp_id}"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "API", "description": "API layer"}))
        .send()
        .await
        .unwrap();
    assert_eq!(update.status(), 200);
    let body: serde_json::Value = update.json().await.unwrap();
    assert_eq!(body["name"], "API");
    assert_eq!(body["description"], "API layer");

    // delete
    let delete = client
        .delete(format!("{url}/api/v1/projects/TT/components/{comp_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), 204);

    // list — empty again
    let list2 = client
        .get(format!("{url}/api/v1/projects/TT/components"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = list2.json().await.unwrap();
    assert!(body["components"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn component_create_empty_name_returns_400() {
    let (url, client) = spawn_server_with_memory_repos().await;
    let token = login_token(&url, &client).await;

    let res = client
        .post(format!("{url}/api/v1/projects/TT/components"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "  ", "description": null}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn component_create_unknown_project_returns_404() {
    let (url, client) = spawn_server_with_memory_repos().await;
    let token = login_token(&url, &client).await;

    let res = client
        .post(format!("{url}/api/v1/projects/NOPE/components"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "x", "description": null}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn components_require_auth() {
    let (url, client) = spawn_server_with_memory_repos().await;
    let res = client
        .get(format!("{url}/api/v1/projects/TT/components"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

// ─── Versions integration tests ──────────────────────────────────────

#[tokio::test]
async fn versions_crud_flow() {
    let (url, client) = spawn_server_with_memory_repos().await;
    let token = login_token(&url, &client).await;

    // list — empty initially
    let list0 = client
        .get(format!("{url}/api/v1/projects/TT/versions"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(list0.status(), 200);
    let body: serde_json::Value = list0.json().await.unwrap();
    assert!(body["versions"].as_array().unwrap().is_empty());

    // create
    let create = client
        .post(format!("{url}/api/v1/projects/TT/versions"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "name": "v1.0",
            "description": "First release",
            "released": false,
            "release_date": null
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);
    let ver: serde_json::Value = create.json().await.unwrap();
    let ver_id = ver["id"].as_str().unwrap().to_string();
    assert_eq!(ver["name"], "v1.0");
    assert_eq!(ver["released"], false);

    // list — contains our version
    let list1 = client
        .get(format!("{url}/api/v1/projects/TT/versions"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = list1.json().await.unwrap();
    let versions = body["versions"].as_array().unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0]["name"], "v1.0");

    // update
    let update = client
        .put(format!("{url}/api/v1/projects/TT/versions/{ver_id}"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "name": "v1.1",
            "description": "Patched release",
            "released": true,
            "release_date": "2026-08-26T00:00:00+00:00"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(update.status(), 200);
    let body: serde_json::Value = update.json().await.unwrap();
    assert_eq!(body["name"], "v1.1");
    assert_eq!(body["released"], true);

    // delete
    let delete = client
        .delete(format!("{url}/api/v1/projects/TT/versions/{ver_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), 204);

    // list — empty again
    let list2 = client
        .get(format!("{url}/api/v1/projects/TT/versions"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = list2.json().await.unwrap();
    assert!(body["versions"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn version_create_empty_name_returns_400() {
    let (url, client) = spawn_server_with_memory_repos().await;
    let token = login_token(&url, &client).await;

    let res = client
        .post(format!("{url}/api/v1/projects/TT/versions"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "", "description": null, "released": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn version_create_unknown_project_returns_404() {
    let (url, client) = spawn_server_with_memory_repos().await;
    let token = login_token(&url, &client).await;

    let res = client
        .post(format!("{url}/api/v1/projects/NOPE/versions"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "v1", "description": null, "released": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn versions_require_auth() {
    let (url, client) = spawn_server_with_memory_repos().await;
    let res = client
        .get(format!("{url}/api/v1/projects/TT/versions"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

// ─── Custom fields integration tests ─────────────────────────────────

#[tokio::test]
async fn custom_fields_crud_flow() {
    let (url, client) = spawn_server_with_memory_repos().await;
    let token = login_token(&url, &client).await;

    // list — empty initially
    let list0 = client
        .get(format!("{url}/api/v1/projects/TT/custom-fields"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(list0.status(), 200);
    let body: serde_json::Value = list0.json().await.unwrap();
    assert!(body["fields"].as_array().unwrap().is_empty());

    // create
    let create = client
        .post(format!("{url}/api/v1/projects/TT/custom-fields"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "name": "Priority Score",
            "field_type": "number",
            "options": [],
            "is_required": false
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);
    let field: serde_json::Value = create.json().await.unwrap();
    let field_id = field["id"].as_str().unwrap().to_string();
    assert_eq!(field["name"], "Priority Score");
    assert_eq!(field["field_type"], "number");
    assert_eq!(field["is_required"], false);

    // list — contains our field
    let list1 = client
        .get(format!("{url}/api/v1/projects/TT/custom-fields"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = list1.json().await.unwrap();
    let fields = body["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0]["name"], "Priority Score");

    // update
    let update = client
        .put(format!("{url}/api/v1/custom-fields/{field_id}"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "name": "Score",
            "field_type": "number",
            "options": [],
            "is_required": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(update.status(), 200);
    let body: serde_json::Value = update.json().await.unwrap();
    assert_eq!(body["name"], "Score");
    assert_eq!(body["is_required"], true);

    // delete
    let delete = client
        .delete(format!("{url}/api/v1/custom-fields/{field_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), 204);

    // list — empty again
    let list2 = client
        .get(format!("{url}/api/v1/projects/TT/custom-fields"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = list2.json().await.unwrap();
    assert!(body["fields"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn custom_field_create_empty_name_returns_400() {
    let (url, client) = spawn_server_with_memory_repos().await;
    let token = login_token(&url, &client).await;

    let res = client
        .post(format!("{url}/api/v1/projects/TT/custom-fields"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "  ", "field_type": "text", "options": [], "is_required": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn custom_field_create_unknown_project_returns_404() {
    let (url, client) = spawn_server_with_memory_repos().await;
    let token = login_token(&url, &client).await;

    let res = client
        .post(format!("{url}/api/v1/projects/NOPE/custom-fields"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "x", "field_type": "text", "options": [], "is_required": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn custom_field_set_and_list_issue_values() {
    let (url, client) = spawn_server_with_memory_repos().await;
    let token = login_token(&url, &client).await;
    let issue_id = create_issue_via_api(&url, &client, &token).await;

    // create a custom field first
    let create = client
        .post(format!("{url}/api/v1/projects/TT/custom-fields"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "Sprint Points", "field_type": "number", "options": [], "is_required": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);
    let field: serde_json::Value = create.json().await.unwrap();
    let field_id = field["id"].as_str().unwrap().to_string();

    // set value on issue
    let set = client
        .put(format!(
            "{url}/api/v1/issues/{issue_id}/custom-fields/{field_id}/value"
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({"value": 8}))
        .send()
        .await
        .unwrap();
    assert_eq!(set.status(), 204);

    // list issue custom field values
    let list = client
        .get(format!("{url}/api/v1/issues/{issue_id}/custom-fields"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), 200);
    let body: serde_json::Value = list.json().await.unwrap();
    let values = body["values"].as_array().unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0]["field_id"], field_id);
    assert_eq!(values[0]["value"], 8);
}

#[tokio::test]
async fn custom_fields_require_auth() {
    let (url, client) = spawn_server_with_memory_repos().await;
    let res = client
        .get(format!("{url}/api/v1/projects/TT/custom-fields"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

// ─── Project update / delete integration tests ───────────────────────

#[tokio::test]
async fn project_update_changes_name_and_description() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    let update = client
        .patch(format!("{url}/api/v1/projects/TT"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "Updated Tracker", "description": "New desc"}))
        .send()
        .await
        .unwrap();
    assert_eq!(update.status(), 200);
    let body: serde_json::Value = update.json().await.unwrap();
    assert_eq!(body["name"], "Updated Tracker");
    assert_eq!(body["description"], "New desc");
    assert_eq!(body["key"], "TT");
}

#[tokio::test]
async fn project_delete_returns_204_and_hides_project() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    // create a separate project to delete (so we don't break other tests on this server)
    let create = client
        .post(format!("{url}/api/v1/projects"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"key": "DEL", "name": "To Delete", "description": null}))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);

    // delete it
    let delete = client
        .delete(format!("{url}/api/v1/projects/DEL"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), 204);

    // GET on deleted project returns 404
    let get = client
        .get(format!("{url}/api/v1/projects/DEL"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), 404);
}

#[tokio::test]
async fn project_update_forbidden_for_non_owner() {
    let (url, client) = spawn_server().await;
    let token = login_token(&url, &client).await;

    // register a second user
    let reg = client
        .post(format!("{url}/api/v1/auth/register"))
        .json(&serde_json::json!({
            "email": "other@example.com",
            "username": "other",
            "name": "Other",
            "password": "secret123"
        }))
        .send()
        .await
        .unwrap();
    let reg_body: serde_json::Value = reg.json().await.unwrap();
    let other_token = reg_body["access_token"].as_str().unwrap().to_string();

    // second user creates their own project
    let create = client
        .post(format!("{url}/api/v1/projects"))
        .bearer_auth(&other_token)
        .json(&serde_json::json!({"key": "OWN", "name": "Owned", "description": null}))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);

    // first user (demo) tries to update other's project → 403
    let update = client
        .patch(format!("{url}/api/v1/projects/OWN"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"name": "Hacked"}))
        .send()
        .await
        .unwrap();
    assert_eq!(update.status(), 403);
}

#[tokio::test]
async fn project_delete_forbidden_for_non_owner() {
    let (url, client) = spawn_server().await;

    // register a second user
    let reg = client
        .post(format!("{url}/api/v1/auth/register"))
        .json(&serde_json::json!({
            "email": "other2@example.com",
            "username": "other2",
            "name": "Other2",
            "password": "secret123"
        }))
        .send()
        .await
        .unwrap();
    let reg_body: serde_json::Value = reg.json().await.unwrap();
    let other_token = reg_body["access_token"].as_str().unwrap().to_string();

    // demo user's token
    let demo_token = login_token(&url, &client).await;

    // second user creates their own project
    let create = client
        .post(format!("{url}/api/v1/projects"))
        .bearer_auth(&other_token)
        .json(&serde_json::json!({"key": "OWN2", "name": "Owned2", "description": null}))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), 201);

    // demo user tries to delete other's project → 403
    let delete = client
        .delete(format!("{url}/api/v1/projects/OWN2"))
        .bearer_auth(&demo_token)
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), 403);
}
