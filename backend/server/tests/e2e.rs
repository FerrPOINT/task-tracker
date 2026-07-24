use std::sync::Arc;

use serial_test::serial;
use server::run;
use shared::{AppConfig, AuthConfig, DatabaseConfig, ServerConfig};

fn test_config() -> Arc<AppConfig> {
    let url = std::env::var("TASKTRACKER_DATABASE_URL")
        .expect("set TASKTRACKER_DATABASE_URL, e.g. postgres://tasktracker:[CHANGE_ME]@127.0.0.1:3458/tasktracker_test");
    Arc::new(AppConfig {
        database: DatabaseConfig {
            url,
            max_connections: 5,
            min_connections: 1,
            connect_timeout_seconds: 10,
            idle_timeout_seconds: 600,
        },
        server: ServerConfig {
            address: "127.0.0.1".to_string(),
            port: 0,
        },
        auth: AuthConfig {
            jwt_secret: "test-secret".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
        },
    })
}

#[tokio::test]
#[serial]
#[ignore = "requires docker test stack"]
async fn server_starts_runs_migrations_and_serves_health() {
    let config = test_config();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(run(config, ready_tx, shutdown_rx));

    let addr = tokio::time::timeout(std::time::Duration::from_secs(30), ready_rx)
        .await
        .expect("server did not become ready")
        .expect("ready channel closed");
    let url = format!("http://{}/api/v1/health", addr);

    let client = reqwest::Client::new();
    let res = client
        .get(&url)
        .send()
        .await
        .expect("health request failed");
    assert_eq!(res.status(), 200);
    assert_eq!(res.text().await.unwrap(), "ok");

    let _ = shutdown_tx.send(());
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
    assert!(result.is_ok(), "server did not shut down in time");
}

#[tokio::test]
#[serial]
#[ignore = "requires docker test stack"]
async fn full_smoke_with_real_repositories() {
    let config = test_config();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(run(config, ready_tx, shutdown_rx));

    let addr = tokio::time::timeout(std::time::Duration::from_secs(30), ready_rx)
        .await
        .expect("server did not become ready")
        .expect("ready channel closed");
    let url = format!("http://{}", addr);
    let client = reqwest::Client::new();

    let login = client
        .post(format!("{}/api/v1/auth/login", url))
        .json(&serde_json::json!({"email":"demo@example.com","password":"demo"}))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), 200);
    let token = login.json::<serde_json::Value>().await.unwrap()["access_token"]
        .as_str()
        .unwrap()
        .to_string();

    let projects = client
        .get(format!("{}/api/v1/projects", url))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(projects.status(), 200);
    let body: serde_json::Value = projects.json().await.unwrap();
    let projects_arr = body["projects"].as_array().unwrap();
    assert!(!projects_arr.is_empty());
    let project_key = projects_arr[0]["key"].as_str().unwrap();

    let board = client
        .get(format!("{}/api/v1/projects/{}/board", url, project_key))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(board.status(), 200);
    let body: serde_json::Value = board.json().await.unwrap();
    assert!(!body["columns"].as_array().unwrap().is_empty());

    let _ = shutdown_tx.send(());
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
    assert!(result.is_ok(), "server did not shut down in time");
}
