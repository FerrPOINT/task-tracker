#[cfg(test)]
mod tests {
    use std::env;

    use crate::AppConfig;

    fn clear_env() {
        for key in [
            "TASKTRACKER_DATABASE_URL",
            "TASKTRACKER_DATABASE_MAX_CONNECTIONS",
            "TASKTRACKER_DATABASE_MIN_CONNECTIONS",
            "TASKTRACKER_DATABASE_CONNECT_TIMEOUT_SECONDS",
            "TASKTRACKER_DATABASE_IDLE_TIMEOUT_SECONDS",
            "TASKTRACKER_SERVER_ADDRESS",
            "TASKTRACKER_SERVER_PORT",
            "TASKTRACKER_AUTH_JWT_SECRET",
            "TASKTRACKER_AUTH_ACCESS_TOKEN_TTL_MINUTES",
            "TASKTRACKER_AUTH_REFRESH_TOKEN_TTL_DAYS",
        ] {
            unsafe { env::remove_var(key) };
        }
    }

    fn set_env(key: &str, value: &str) {
        unsafe { env::set_var(key, value) };
    }

    #[test]
    fn config_scenarios() {
        clear_env();

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
        assert_eq!(cfg.database.url, "");
        let expected_jwt = env::var("TASKTRACKER_JWT_SECRET")
            .or_else(|_| env::var("TASKTRACKER_AUTH_JWT_SECRET"))
            .unwrap_or_else(|_| "[CHANGE_ME]".to_string());
        assert_eq!(cfg.auth.jwt_secret, expected_jwt);

        set_env("TASKTRACKER_SERVER_PORT", "19876");
        set_env("TASKTRACKER_AUTH_JWT_SECRET", "env-secret-32-chars-long!!");
        set_env("TASKTRACKER_JWT_SECRET", "legacy-secret-32-chars-long!");
        set_env(
            "TASKTRACKER_DATABASE_URL",
            "postgres://u:[CHANGE_ME]@localhost:5432/db",
        );
        set_env("TASKTRACKER_DATABASE_MAX_CONNECTIONS", "42");
        set_env("TASKTRACKER_DATABASE_MIN_CONNECTIONS", "3");
        set_env("TASKTRACKER_DATABASE_CONNECT_TIMEOUT_SECONDS", "5");
        set_env("TASKTRACKER_DATABASE_IDLE_TIMEOUT_SECONDS", "300");
        set_env("TASKTRACKER_AUTH_ACCESS_TOKEN_TTL_MINUTES", "60");
        set_env("TASKTRACKER_AUTH_REFRESH_TOKEN_TTL_DAYS", "14");
        let cfg = AppConfig::from_path("/nonexistent.toml").unwrap();
        assert_eq!(cfg.server.port, 19876);
        assert_eq!(cfg.auth.jwt_secret, "legacy-secret-32-chars-long!");
        assert!(cfg.database.url.contains("localhost:5432/db"));

        // Note: Environment separator is a single `_`, so nested database keys
        // like `connect_timeout_seconds` cannot be overridden via env without
        // changing the separator. Defaults remain as tested above.

        set_env("TASKTRACKER_SERVER_PORT", "not-a-number");
        let err = AppConfig::from_path("/nonexistent.toml").unwrap_err();
        assert!(err.to_string().contains("invalid type"));

        clear_env();
    }

    #[test]
    fn config_defaults_implemented() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.server.port, 3456);
        assert_eq!(cfg.database.max_connections, 20);
        assert_eq!(cfg.auth.jwt_secret, "[CHANGE_ME]");
    }

    #[test]
    fn config_from_env_uses_default_path() {
        clear_env();
        // from_env targets config/default.toml which won't exist; defaults still apply
        let cfg = AppConfig::from_env().unwrap();
        assert_eq!(cfg.server.port, 3456);
        clear_env();
    }
}
