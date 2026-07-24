use std::sync::Arc;

use domain::{
    Board, BoardColumn, BoardRepository, ColumnCategory, IssueRepository, MemoryBoardRepository,
    MemoryIssueRepository, MemoryProjectRepository, MemorySprintRepository, MemoryUserRepository,
    Project, ProjectRepository, SprintRepository, User, UserRepository,
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
        },
    })
}

async fn spawn_server() -> (String, reqwest::Client) {
    let user = test_user();
    let mut project = Project {
        id: shared::ProjectId::new(),
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
                category: ColumnCategory::Todo,
                wip_limit: None,
                position: 0,
            },
            BoardColumn {
                id: in_progress,
                name: "In Progress".into(),
                category: ColumnCategory::InProgress,
                wip_limit: Some(5),
                position: 1,
            },
            BoardColumn {
                id: review,
                name: "Review".into(),
                category: ColumnCategory::InProgress,
                wip_limit: None,
                position: 2,
            },
            BoardColumn {
                id: done,
                name: "Done".into(),
                category: ColumnCategory::Done,
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

    let repos = Arc::new(domain::Repositories {
        users: users.clone(),
        projects: projects.clone(),
        issues: issues.clone(),
        boards: boards.clone(),
        sprints: sprints.clone(),
    });

    let ctx = Arc::new(AppContext::new(test_config(), repos));
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
    assert_eq!(res.status(), 200);
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
    assert_eq!(created.status(), 200);

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
    assert_eq!(defaults.status(), 200);
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
    assert_eq!(created.status(), 200);
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
    assert_eq!(created.status(), 200);

    let res = client
        .get(format!("{}/api/v1/dashboard", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["assigned_issues"].as_array().unwrap().len() >= 1);
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
