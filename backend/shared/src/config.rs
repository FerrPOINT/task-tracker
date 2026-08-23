use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::{env, path::Path};

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub auth: AuthConfig,
    #[serde(default)]
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub dir: String,
    pub max_upload_bytes: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            dir: "/var/lib/tasktracker/uploads".to_string(),
            max_upload_bytes: 25 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub address: String,
    pub port: u16,
    pub cors_allowed_origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub access_token_ttl_minutes: u64,
    pub refresh_token_ttl_days: u64,
    pub refresh_cookie_name: String,
    pub refresh_cookie_secure: bool,
    pub refresh_cookie_same_site: String,
    pub refresh_cookie_domain: Option<String>,
    pub refresh_cookie_path: String,
}

impl AppConfig {
    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.server.address, self.server.port)
    }

    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_path("config/default.toml")
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let defaults = Config::builder()
            .set_default("database.url", "")?
            .set_default("database.max_connections", 20u64)?
            .set_default("database.min_connections", 5u64)?
            .set_default("database.connect_timeout_seconds", 10u64)?
            .set_default("database.idle_timeout_seconds", 600u64)?
            .set_default("server.address", "0.0.0.0")?
            .set_default("server.port", 3456u16)?
            .set_default("server.cors_allowed_origins", vec!["*"])?
            .set_default("auth.jwt_secret", "[CHANGE_ME]")?
            .set_default("auth.access_token_ttl_minutes", 15u64)?
            .set_default("auth.refresh_token_ttl_days", 7u64)?
            .set_default("auth.refresh_cookie_name", "refresh_token")?
            .set_default("auth.refresh_cookie_secure", true)?
            .set_default("auth.refresh_cookie_same_site", "Lax")?
            .set_default("auth.refresh_cookie_domain", Option::<String>::None)?
            .set_default("auth.refresh_cookie_path", "/api/v1/auth")?
            .set_default("storage.dir", "/var/lib/tasktracker/uploads")?
            .set_default("storage.max_upload_bytes", 26214400u64)?
            .build()?;

        let mut cfg: AppConfig = Config::builder()
            .add_source(defaults)
            .add_source(File::from(path.as_ref()).required(false))
            .add_source(
                Environment::with_prefix("TASKTRACKER")
                    .separator("__")
                    .prefix_separator("_")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()?;

        // Backwards-compatible alias: TASKTRACKER_JWT_SECRET maps to auth.jwt_secret
        if let Ok(secret) = env::var("TASKTRACKER_JWT_SECRET") {
            cfg.auth.jwt_secret = secret;
        }

        if cfg.auth.jwt_secret == "[CHANGE_ME]" {
            return Err(ConfigError::Message(
                "auth.jwt_secret must be changed from default [CHANGE_ME]".to_string(),
            ));
        }

        Ok(cfg)
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            max_connections: 20,
            min_connections: 5,
            connect_timeout_seconds: 10,
            idle_timeout_seconds: 600,
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            address: "0.0.0.0".to_string(),
            port: 3456,
            cors_allowed_origins: vec!["*".to_string()],
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "[CHANGE_ME]".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
            refresh_cookie_name: "refresh_token".to_string(),
            refresh_cookie_secure: true,
            refresh_cookie_same_site: "Lax".to_string(),
            refresh_cookie_domain: None,
            refresh_cookie_path: "/api/v1/auth".to_string(),
        }
    }
}
