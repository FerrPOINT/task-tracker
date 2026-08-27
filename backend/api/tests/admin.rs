//! Phase 8: Admin route integration tests.
//!
//! These tests exercise the full HTTP stack: middleware auth → service
//! authorization → business logic → audit logging.  They use the in-memory
//! repositories for audit logs and system settings so that side-effects can be
//! verified, and create both an admin and a regular user to verify the
//! authorization gate.

use std::sync::Arc;

use domain::{
    InMemoryStorage, MemoryAuditLogRepository, MemorySystemSettingRepository, MemoryUserRepository,
    User, UserRepository,
};
use shared::{AppConfig, AuthConfig, DatabaseConfig, ServerConfig, UserId};

use app::context::AppContext;

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

fn make_user(id: &str, email: &str, is_admin: bool) -> User {
    User {
        id: UserId::from_uuid(uuid::Uuid::parse_str(id).unwrap()),
        email: email.into(),
        username: email.split('@').next().unwrap().into(),
        display_name: email.split('@').next().unwrap().into(),
        password_hash: "$argon2id$v=19$m=65536,t=3,p=4$stN/enhZ9yOvgWC9E8Y6BA$IL9I0WONb/I6zoT4rdmdkrPcIFADFxsLCjrO0ySSl0Y"
            .into(),
        refresh_token_hash: None,
        is_system_admin: is_admin,
        is_active: true,
        created_at: shared::now(),
        updated_at: shared::now(),
    }
}

/// Spawn a server with both an admin and a regular user pre-seeded.
/// Returns (url, client, admin_token, regular_token).
async fn spawn_admin_server() -> (String, reqwest::Client, String, String) {
    let admin = make_user(
        "11111111-1111-1111-1111-111111111111",
        "admin@example.com",
        true,
    );
    let regular = make_user(
        "22222222-2222-2222-2222-222222222222",
        "user@example.com",
        false,
    );

    let users = Arc::new(MemoryUserRepository::default());
    users.save(&admin).await.unwrap();
    users.save(&regular).await.unwrap();

    let audit_logs = Arc::new(MemoryAuditLogRepository::default());
    let system_settings = Arc::new(MemorySystemSettingRepository::default());

    let repos = Arc::new(domain::Repositories {
        users: users.clone(),
        audit_logs: audit_logs.clone(),
        system_settings: system_settings.clone(),
        projects: Arc::new(domain::StubProjectRepository),
        issues: Arc::new(domain::StubIssueRepository),
        boards: Arc::new(domain::StubBoardRepository),
        sprints: Arc::new(domain::StubSprintRepository),
        comments: Arc::new(domain::StubCommentRepository),
        worklogs: Arc::new(domain::StubWorklogRepository),
        members: Arc::new(domain::StubProjectMemberRepository),
        statuses: Arc::new(domain::StubStatusRepository),
        transitions: Arc::new(domain::StubWorkflowTransitionRepository),
        issue_types: Arc::new(domain::StubIssueTypeRepository),
        attachments: Arc::new(domain::StubAttachmentRepository),
        labels: Arc::new(domain::StubLabelRepository),
        issue_links: Arc::new(domain::StubIssueLinkRepository),
        notifications: Arc::new(domain::StubNotificationRepository),
        notification_settings: Arc::new(domain::StubUserNotificationSettingsRepository),
        issue_status_history: Arc::new(domain::StubIssueStatusHistoryRepository),
        watchers: Arc::new(domain::StubWatcherRepository),
        votes: Arc::new(domain::StubVoteRepository),
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

    // Login as both users to get tokens.
    let admin_token = login(&url, &client, "admin@example.com").await;
    let regular_token = login(&url, &client, "user@example.com").await;

    (url, client, admin_token, regular_token)
}

async fn login(url: &str, client: &reqwest::Client, email: &str) -> String {
    let res = client
        .post(format!("{}/api/v1/auth/login", url))
        .json(&serde_json::json!({"email": email, "password": "demo"}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "login failed for {email}");
    let body: serde_json::Value = res.json().await.unwrap();
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn admin_endpoints_require_auth() {
    let (url, client, _, _) = spawn_admin_server().await;
    let res = client
        .get(format!("{}/api/v1/admin/users", url))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn admin_users_list_requires_system_admin() {
    let (url, client, admin_token, regular_token) = spawn_admin_server().await;

    // Regular user → 403
    let res = client
        .get(format!("{}/api/v1/admin/users", url))
        .bearer_auth(&regular_token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);

    // Admin → 200
    let res = client
        .get(format!("{}/api/v1/admin/users", url))
        .bearer_auth(&admin_token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let users = body["users"].as_array().unwrap();
    assert_eq!(users.len(), 2);
    // Verify the admin user has is_system_admin=true
    let admin_entry = users
        .iter()
        .find(|u| u["email"] == "admin@example.com")
        .unwrap();
    assert_eq!(admin_entry["is_system_admin"], true);
    assert_eq!(admin_entry["is_active"], true);
}

#[tokio::test]
async fn admin_create_user_success() {
    let (url, client, admin_token, _) = spawn_admin_server().await;
    let res = client
        .post(format!("{}/api/v1/admin/users", url))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({
            "email": "new@example.com",
            "username": "newuser",
            "display_name": "New User",
            "password": "securepass123",
            "is_system_admin": false,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["email"], "new@example.com");
    assert_eq!(body["username"], "newuser");
    assert_eq!(body["is_active"], true);
    assert_eq!(body["is_system_admin"], false);
    // Password must never be in the response.
    assert!(body.get("password").is_none());
    assert!(body.get("password_hash").is_none());
}

#[tokio::test]
async fn admin_create_user_requires_admin() {
    let (url, client, _, regular_token) = spawn_admin_server().await;
    let res = client
        .post(format!("{}/api/v1/admin/users", url))
        .bearer_auth(&regular_token)
        .json(&serde_json::json!({
            "email": "new@example.com",
            "username": "newuser",
            "display_name": "New User",
            "password": "securepass123",
            "is_system_admin": false,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn admin_create_user_duplicate_email() {
    let (url, client, admin_token, _) = spawn_admin_server().await;
    let res = client
        .post(format!("{}/api/v1/admin/users", url))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({
            "email": "admin@example.com",
            "username": "another",
            "display_name": "Another",
            "password": "securepass123",
            "is_system_admin": false,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 409);
}

#[tokio::test]
async fn admin_update_user_status_deactivates() {
    let (url, client, admin_token, _) = spawn_admin_server().await;
    let res = client
        .put(format!(
            "{}/api/v1/admin/users/22222222-2222-2222-2222-222222222222/status",
            url
        ))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({"is_active": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["is_active"], false);
}

#[tokio::test]
async fn admin_update_user_status_requires_admin() {
    let (url, client, _, regular_token) = spawn_admin_server().await;
    let res = client
        .put(format!(
            "{}/api/v1/admin/users/22222222-2222-2222-2222-222222222222/status",
            url
        ))
        .bearer_auth(&regular_token)
        .json(&serde_json::json!({"is_active": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn admin_update_user_status_prevents_last_admin_deactivation() {
    let (url, client, admin_token, _) = spawn_admin_server().await;
    // Deactivate the regular user first (should succeed).
    let _ = client
        .put(format!(
            "{}/api/v1/admin/users/22222222-2222-2222-2222-222222222222/status",
            url
        ))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({"is_active": false}))
        .send()
        .await
        .unwrap();

    // Now try to deactivate the only admin → should fail.
    let res = client
        .put(format!(
            "{}/api/v1/admin/users/11111111-1111-1111-1111-111111111111/status",
            url
        ))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({"is_active": false}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 409);
}

#[tokio::test]
async fn admin_audit_log_list_requires_admin() {
    let (url, client, admin_token, regular_token) = spawn_admin_server().await;

    // Regular user → 403
    let res = client
        .get(format!("{}/api/v1/admin/audit-log", url))
        .bearer_auth(&regular_token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);

    // Admin → 200
    let res = client
        .get(format!("{}/api/v1/admin/audit-log", url))
        .bearer_auth(&admin_token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["entries"].as_array().is_some());
}

#[tokio::test]
async fn admin_system_settings_list_requires_admin() {
    let (url, client, admin_token, regular_token) = spawn_admin_server().await;

    // Regular user → 403
    let res = client
        .get(format!("{}/api/v1/admin/system-settings", url))
        .bearer_auth(&regular_token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);

    // Admin → 200
    let res = client
        .get(format!("{}/api/v1/admin/system-settings", url))
        .bearer_auth(&admin_token)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["settings"].as_array().is_some());
}

#[tokio::test]
async fn admin_system_settings_update_success() {
    let (url, client, admin_token, _) = spawn_admin_server().await;
    let res = client
        .put(format!("{}/api/v1/admin/system-settings", url))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({
            "key": "instance.name",
            "value": "My Tracker",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["key"], "instance.name");
    assert_eq!(body["value"], "My Tracker");
}

#[tokio::test]
async fn admin_system_settings_update_rejects_unsafe_key() {
    let (url, client, admin_token, _) = spawn_admin_server().await;
    let res = client
        .put(format!("{}/api/v1/admin/system-settings", url))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({
            "key": "mail.password",
            "value": "secret",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn admin_system_settings_update_requires_admin() {
    let (url, client, _, regular_token) = spawn_admin_server().await;
    let res = client
        .put(format!("{}/api/v1/admin/system-settings", url))
        .bearer_auth(&regular_token)
        .json(&serde_json::json!({
            "key": "instance.name",
            "value": "x",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

// audit log pagination must page via offset, not repeat the first page
#[tokio::test]
async fn admin_audit_log_pages_with_offset() {
    let (url, client, admin_token, _) = spawn_admin_server().await;

    // Generate audit entries by flipping the regular user's status repeatedly.
    for _ in 0..4 {
        let res = client
            .put(format!(
                "{}/api/v1/admin/users/22222222-2222-2222-2222-222222222222/status",
                url
            ))
            .bearer_auth(&admin_token)
            .json(&serde_json::json!({"is_active": false}))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
    }

    let page1 = client
        .get(format!("{}/api/v1/admin/audit-log?limit=2&offset=0", url))
        .bearer_auth(&admin_token)
        .send()
        .await
        .unwrap();
    assert_eq!(page1.status(), 200);
    let page1: serde_json::Value = page1.json().await.unwrap();
    let page2 = client
        .get(format!("{}/api/v1/admin/audit-log?limit=2&offset=2", url))
        .bearer_auth(&admin_token)
        .send()
        .await
        .unwrap();
    assert_eq!(page2.status(), 200);
    let page2: serde_json::Value = page2.json().await.unwrap();

    let ids = |page: &serde_json::Value| -> Vec<String> {
        page["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap_or_default().to_string())
            .collect()
    };
    let first = ids(&page1);
    let second = ids(&page2);
    assert_eq!(first.len(), 2, "page 1: {first:?}");
    assert_eq!(second.len(), 2, "page 2: {second:?}");
    assert!(
        first.iter().all(|id| !second.contains(id)),
        "offset must move the window, page1={first:?} page2={second:?}"
    );
}
