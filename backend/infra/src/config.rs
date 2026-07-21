use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub database_url: String,
    pub server_address: String,
    pub server_port: u16,
    pub jwt_secret: String,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let cfg = config::Config::builder()
            .add_source(config::Environment::with_prefix("TASKTRACKER").separator("_"))
            .build()?;

        Ok(Self {
            database_url: cfg.get_string("database.url")?,
            server_address: cfg
                .get_string("server.address")
                .unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: cfg
                .get_string("server.port")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3456),
            jwt_secret: cfg.get_string("jwt.secret")?,
        })
    }
}
