use std::sync::Arc;

use domain::{
    Board, BoardColumn, BoardRepository, InMemoryStorage, MemoryAttachmentRepository,
    MemoryBoardRepository, MemoryCommentRepository, MemoryIssueLinkRepository,
    MemoryIssueRepository, MemoryLabelRepository, MemoryProjectMemberRepository,
    MemoryProjectRepository, MemorySprintRepository, MemoryUserRepository, MemoryWorklogRepository,
    Project, ProjectRepository, StatusCategory, User, UserRepository,
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
    })
}

async fn spawn_server() -> (String, reqwest::Client) {
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

    let repos = Arc::new(domain::Repositories {
        users: users.clone(),
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
    assert_eq!(created.status(), 200);
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
    assert_eq!(created.status(), 200);
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
    assert_eq!(created.status(), 200);
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
    assert_eq!(res.status(), 200);
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
    assert_eq!(res.status(), 200);
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
    assert_eq!(created.status(), 200);

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
