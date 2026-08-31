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
    #[serde(default)]
    pub email: EmailConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from_address: String,
    pub from_name: String,
    pub starttls: bool,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: String::new(),
            port: 587,
            username: None,
            password: None,
            from_address: String::new(),
            from_name: "Task Tracker".to_string(),
            starttls: true,
        }
    }
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
pub struct MetricsConfig {
    pub public: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self { public: true }
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
    /// Auth endpoints rate limit: burst size (requests per period per IP).
    #[serde(default = "default_auth_rate_burst")]
    pub auth_rate_burst: u32,
    /// Auth endpoints rate limit period in seconds.
    #[serde(default = "default_auth_rate_period_secs")]
    pub auth_rate_period_secs: u64,
    /// General API rate limit: burst size.
    #[serde(default = "default_general_rate_burst")]
    pub general_rate_burst: u32,
    /// General API sustained rate: tokens replenished per second (GCRA).
    #[serde(default = "default_general_rate_per_second")]
    pub general_rate_per_second: u64,
}

fn default_auth_rate_burst() -> u32 {
    5
}
fn default_auth_rate_period_secs() -> u64 {
    15
}
fn default_general_rate_burst() -> u32 {
    60
}
fn default_general_rate_per_second() -> u64 {
    60
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
            .set_default(
                "server.cors_allowed_origins",
                vec![
                    "http://localhost:19877",
                    "http://localhost:4173",
                    "http://localhost:5173",
                    "http://127.0.0.1:19877",
                    "http://127.0.0.1:4173",
                    "http://127.0.0.1:5173",
                ],
            )?
            .set_default("server.auth_rate_burst", 5u32)?
            .set_default("server.auth_rate_period_secs", 15u64)?
            .set_default("server.general_rate_burst", 60u32)?
            .set_default("server.general_rate_per_second", 60u64)?
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
            .set_default("email.enabled", false)?
            .set_default("email.host", "")?
            .set_default("email.port", 587u16)?
            .set_default("email.username", Option::<String>::None)?
            .set_default("email.password", Option::<String>::None)?
            .set_default("email.from_address", "")?
            .set_default("email.from_name", "Task Tracker")?
            .set_default("email.starttls", true)?
            .set_default("metrics.public", true)?
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

        if cfg.email.enabled {
            if cfg.email.host.trim().is_empty() {
                return Err(ConfigError::Message(
                    "email.host must be set when email is enabled".to_string(),
                ));
            }
            if !is_valid_mail_address(&cfg.email.from_address) {
                return Err(ConfigError::Message(
                    "email.from_address must be a valid mail address".to_string(),
                ));
            }
            let has_user = cfg.email.username.is_some();
            let has_pass = cfg.email.password.is_some();
            if has_user != has_pass {
                return Err(ConfigError::Message(
                    "email.username and email.password must be set together or both omitted"
                        .to_string(),
                ));
            }
        }

        // Rate limits: governor panics on a zero period; validate early so a
        // bad config surfaces as a configuration error, not a startup crash.
        if cfg.server.auth_rate_period_secs == 0 || cfg.server.general_rate_per_second == 0 {
            return Err(ConfigError::Message(
                "server rate-limit periods must be greater than zero".to_string(),
            ));
        }
        if cfg.server.general_rate_per_second > 1_000_000_000 {
            return Err(ConfigError::Message(
                "server.general_rate_per_second must not exceed 1000000000".to_string(),
            ));
        }
        if cfg.server.auth_rate_burst == 0 || cfg.server.general_rate_burst == 0 {
            return Err(ConfigError::Message(
                "server rate-limit bursts must be at least 1".to_string(),
            ));
        }

        Ok(cfg)
    }
}

/// Minimal RFC-ish local@domain sanity check for the from-address.
fn is_valid_mail_address(addr: &str) -> bool {
    let addr = addr.trim();
    if addr.is_empty() || !addr.contains('@') {
        return false;
    }
    let (local, domain) = addr.split_once('@').unwrap();
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
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
            cors_allowed_origins: vec![
                "http://localhost:19877".to_string(),
                "http://localhost:4173".to_string(),
                "http://localhost:5173".to_string(),
                "http://127.0.0.1:19877".to_string(),
                "http://127.0.0.1:4173".to_string(),
                "http://127.0.0.1:5173".to_string(),
            ],
            auth_rate_burst: default_auth_rate_burst(),
            auth_rate_period_secs: default_auth_rate_period_secs(),
            general_rate_burst: default_general_rate_burst(),
            general_rate_per_second: default_general_rate_per_second(),
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
