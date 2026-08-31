use std::env;
use std::sync::Mutex;

use crate::AppConfig;

// Tests that mutate process-wide env vars must not run in parallel.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn clear_env() {
    for key in [
        "TASKTRACKER_DATABASE__URL",
        "TASKTRACKER_DATABASE__MAX_CONNECTIONS",
        "TASKTRACKER_DATABASE__MIN_CONNECTIONS",
        "TASKTRACKER_DATABASE__CONNECT_TIMEOUT_SECONDS",
        "TASKTRACKER_DATABASE__IDLE_TIMEOUT_SECONDS",
        "TASKTRACKER_SERVER__ADDRESS",
        "TASKTRACKER_SERVER__PORT",
        "TASKTRACKER_AUTH__JWT_SECRET",
        "TASKTRACKER_JWT_SECRET",
        "TASKTRACKER_AUTH__ACCESS_TOKEN_TTL_MINUTES",
        "TASKTRACKER_AUTH__REFRESH_TOKEN_TTL_DAYS",
        "TASKTRACKER_AUTH__REFRESH_TOKEN_COOKIE_NAME",
        "TASKTRACKER_AUTH__REFRESH_COOKIE_SECURE",
        "TASKTRACKER_AUTH__REFRESH_COOKIE_SAME_SITE",
        "TASKTRACKER_AUTH__REFRESH_COOKIE_DOMAIN",
        "TASKTRACKER_AUTH__REFRESH_COOKIE_PATH",
        "TASKTRACKER_EMAIL__ENABLED",
        "TASKTRACKER_EMAIL__HOST",
        "TASKTRACKER_EMAIL__PORT",
        "TASKTRACKER_EMAIL__USERNAME",
        "TASKTRACKER_EMAIL__PASSWORD",
        "TASKTRACKER_EMAIL__FROM_ADDRESS",
        "TASKTRACKER_EMAIL__FROM_NAME",
        "TASKTRACKER_EMAIL__STARTTLS",
        "TASKTRACKER_METRICS__PUBLIC",
        "TASKTRACKER_SERVER__AUTH_RATE_BURST",
        "TASKTRACKER_SERVER__AUTH_RATE_PERIOD_SECS",
        "TASKTRACKER_SERVER__GENERAL_RATE_BURST",
        "TASKTRACKER_SERVER__GENERAL_RATE_PERIOD_SECS",
        "TASKTRACKER_SERVER__GENERAL_RATE_PER_SECOND",
    ] {
        unsafe { env::remove_var(key) };
    }
}

fn set_env(key: &str, value: &str) {
    unsafe { env::set_var(key, value) };
}

#[test]
fn config_scenarios() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_env();
    set_env("TASKTRACKER_JWT_SECRET", "test-secret-32-chars-long!!!!!");

    // Defaults
    let cfg = AppConfig::from_path("/nonexistent.toml").unwrap();
    assert_eq!(cfg.server.address, "0.0.0.0");
    assert_eq!(cfg.server.port, 3456);
    assert_eq!(cfg.server_addr(), "0.0.0.0:3456");
    assert_eq!(cfg.database.max_connections, 20);
    assert_eq!(cfg.database.min_connections, 5);
    assert_eq!(cfg.database.connect_timeout_seconds, 10);
    assert_eq!(cfg.database.idle_timeout_seconds, 600);
    assert_eq!(cfg.auth.access_token_ttl_minutes, 15);
    assert_eq!(cfg.auth.refresh_token_ttl_days, 7);
    assert_eq!(cfg.auth.refresh_cookie_name, "refresh_token");
    assert!(cfg.auth.refresh_cookie_secure);
    assert_eq!(cfg.auth.refresh_cookie_same_site, "Lax");
    assert_eq!(cfg.auth.refresh_cookie_path, "/api/v1/auth");
    assert!(cfg.metrics.public);
    assert_eq!(cfg.database.url, "");
    assert_eq!(
        cfg.server.cors_allowed_origins,
        vec![
            "http://localhost:19877",
            "http://localhost:4173",
            "http://localhost:5173",
            "http://127.0.0.1:19877",
            "http://127.0.0.1:4173",
            "http://127.0.0.1:5173",
        ]
    );
    assert_eq!(cfg.auth.jwt_secret, "test-secret-32-chars-long!!!!!");
    set_env(
        "TASKTRACKER_DATABASE__URL",
        "postgres://u:***@localhost:5432/db",
    );
    set_env("TASKTRACKER_DATABASE__MAX_CONNECTIONS", "42");
    set_env("TASKTRACKER_DATABASE__MIN_CONNECTIONS", "3");
    set_env("TASKTRACKER_DATABASE__CONNECT_TIMEOUT_SECONDS", "5");
    set_env("TASKTRACKER_DATABASE__IDLE_TIMEOUT_SECONDS", "300");
    set_env("TASKTRACKER_SERVER__PORT", "19876");
    set_env("TASKTRACKER_AUTH__ACCESS_TOKEN_TTL_MINUTES", "60");
    set_env("TASKTRACKER_AUTH__REFRESH_TOKEN_TTL_DAYS", "14");
    set_env(
        "TASKTRACKER_AUTH__JWT_SECRET",
        "test-secret-32-chars-long!!!!!",
    );
    let cfg = AppConfig::from_path("/nonexistent.toml").unwrap();
    assert_eq!(cfg.server.port, 19876);
    assert_eq!(cfg.auth.jwt_secret, "test-secret-32-chars-long!!!!!");
    assert!(cfg.database.url.contains("localhost:5432/db"));

    // Environment separator is `__`, so nested keys become
    // `TASKTRACKER_DATABASE__CONNECT_TIMEOUT_SECONDS`.
    assert_eq!(cfg.database.connect_timeout_seconds, 5);
    assert_eq!(cfg.database.idle_timeout_seconds, 300);
    assert_eq!(cfg.database.max_connections, 42);
    assert_eq!(cfg.database.min_connections, 3);

    set_env("TASKTRACKER_METRICS__PUBLIC", "false");
    let cfg = AppConfig::from_path("/nonexistent.toml").unwrap();
    assert!(!cfg.metrics.public);

    set_env("TASKTRACKER_SERVER__PORT", "not-a-number");
    let err = AppConfig::from_path("/nonexistent.toml").unwrap_err();
    assert!(err.to_string().contains("invalid type"));

    clear_env();
}

#[test]
fn config_defaults_implemented() {
    let _guard = ENV_LOCK.lock().unwrap();
    let cfg = AppConfig::default();
    assert_eq!(cfg.server.port, 3456);
    assert_eq!(cfg.database.max_connections, 20);
    assert_eq!(cfg.auth.jwt_secret, "[CHANGE_ME]");
    assert!(cfg.metrics.public);
}

#[test]
fn email_defaults_are_disabled_and_safe() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_env();
    set_env("TASKTRACKER_JWT_SECRET", "test-secret-32-chars-long!!!!!");

    let cfg = AppConfig::from_path("/nonexistent.toml").unwrap();

    assert!(!cfg.email.enabled);
    assert_eq!(cfg.email.host, "");
    assert_eq!(cfg.email.port, 587);
    assert_eq!(cfg.email.username, None);
    assert_eq!(cfg.email.password, None);
    assert_eq!(cfg.email.from_address, "");
    assert_eq!(cfg.email.from_name, "Task Tracker");
    assert!(cfg.email.starttls);

    clear_env();
}

#[test]
fn enabled_email_requires_a_complete_valid_configuration() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_env();
    set_env("TASKTRACKER_JWT_SECRET", "test-secret-32-chars-long!!!!!");
    set_env("TASKTRACKER_EMAIL__ENABLED", "true");
    set_env("TASKTRACKER_EMAIL__FROM_ADDRESS", "noreply@example.test");

    let err = AppConfig::from_path("/nonexistent.toml").unwrap_err();
    assert!(err.to_string().contains("email.host"));

    set_env("TASKTRACKER_EMAIL__HOST", "smtp.example.test");
    set_env("TASKTRACKER_EMAIL__FROM_ADDRESS", "not an email");
    let err = AppConfig::from_path("/nonexistent.toml").unwrap_err();
    assert!(err.to_string().contains("email.from_address"));

    set_env("TASKTRACKER_EMAIL__FROM_ADDRESS", "noreply@example.test");
    set_env("TASKTRACKER_EMAIL__USERNAME", "mailer");
    let err = AppConfig::from_path("/nonexistent.toml").unwrap_err();
    assert!(
        err.to_string()
            .contains("email.username and email.password")
    );

    set_env("TASKTRACKER_EMAIL__PASSWORD", "test-password");
    let cfg = AppConfig::from_path("/nonexistent.toml").unwrap();
    assert!(cfg.email.enabled);
    assert_eq!(cfg.email.host, "smtp.example.test");
    assert_eq!(cfg.email.username.as_deref(), Some("mailer"));
    assert_eq!(cfg.email.password.as_deref(), Some("test-password"));

    clear_env();
}

#[test]
fn config_from_env_uses_default_path() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_env();
    set_env("TASKTRACKER_JWT_SECRET", "test-secret-32-chars-long!!!!!");
    // from_env targets config/default.toml which won't exist; defaults still apply
    let cfg = AppConfig::from_env().unwrap();
    assert_eq!(cfg.server.port, 3456);
    clear_env();
}

#[test]
fn rate_limit_defaults_and_env_override() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_env();
    set_env("TASKTRACKER_JWT_SECRET", "test-secret-32-chars-long!!!!!");

    let cfg = AppConfig::from_path("/nonexistent.toml").unwrap();
    assert_eq!(cfg.server.auth_rate_burst, 5);
    assert_eq!(cfg.server.auth_rate_period_secs, 15);
    assert_eq!(cfg.server.general_rate_burst, 60);
    assert_eq!(cfg.server.general_rate_per_second, 60);

    set_env("TASKTRACKER_SERVER__AUTH_RATE_BURST", "100");
    set_env("TASKTRACKER_SERVER__AUTH_RATE_PERIOD_SECS", "1");
    set_env("TASKTRACKER_SERVER__GENERAL_RATE_BURST", "10000");
    set_env("TASKTRACKER_SERVER__GENERAL_RATE_PER_SECOND", "100");
    set_env("TASKTRACKER_AUTH__JWT_SECRET", "test-secret-123");

    let cfg = AppConfig::from_path("/nonexistent.toml").unwrap();
    assert_eq!(cfg.server.auth_rate_burst, 100);
    assert_eq!(cfg.server.auth_rate_period_secs, 1);
    assert_eq!(cfg.server.general_rate_burst, 10000);
    assert_eq!(cfg.server.general_rate_per_second, 100);
}

#[test]
fn rate_limit_zero_values_rejected() {
    let _guard = ENV_LOCK.lock().unwrap();

    clear_env();
    unsafe { env::set_var("TASKTRACKER_SERVER__AUTH_RATE_BURST", "0") };
    let err = AppConfig::from_path("/nonexistent.toml");
    assert!(err.is_err(), "zero auth burst must be a config error");

    clear_env();
    unsafe { env::set_var("TASKTRACKER_SERVER__GENERAL_RATE_PER_SECOND", "0") };
    let err = AppConfig::from_path("/nonexistent.toml");
    assert!(err.is_err(), "zero general rate must be a config error");
}

#[test]
fn general_rate_above_nanosecond_precision_is_rejected() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_env();
    set_env("TASKTRACKER_JWT_SECRET", "test-secret-32-chars-long!!!!!");
    set_env("TASKTRACKER_SERVER__GENERAL_RATE_PER_SECOND", "1000000001");

    let err = AppConfig::from_path("/nonexistent.toml").unwrap_err();
    assert!(err.to_string().contains("must not exceed 1000000000"));
    clear_env();
}
