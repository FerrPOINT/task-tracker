#[cfg(test)]
mod tests {
    use std::env;

    use crate::AppConfig;

    #[test]
    fn defaults_load_from_path() {
        unsafe {
            env::remove_var("TASKTRACKER_SERVER_PORT");
            env::remove_var("TASKTRACKER_AUTH_JWT_SECRET");
        }
        let cfg = AppConfig::from_path("/nonexistent.toml").unwrap();
        assert_eq!(cfg.server.address, "0.0.0.0");
        assert_eq!(cfg.server.port, 3456);
        assert_eq!(cfg.server_addr(), "0.0.0.0:3456");
        assert_eq!(cfg.database.max_connections, 20);
        assert_eq!(cfg.auth.access_token_ttl_minutes, 15);
    }
}
