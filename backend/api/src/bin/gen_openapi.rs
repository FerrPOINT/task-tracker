use std::sync::Arc;

use api::ApiDoc;
use app::context::AppContext;
use domain::Repositories;
use shared::{AppConfig, AuthConfig, DatabaseConfig, EmailConfig, ServerConfig};
use utoipa::OpenApi;

fn main() {
    let config = Arc::new(AppConfig {
        database: DatabaseConfig::default(),
        server: ServerConfig::default(),
        auth: AuthConfig {
            jwt_secret: "openapi-gen".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
            refresh_cookie_name: "refresh_token".to_string(),
            refresh_cookie_secure: false,
            refresh_cookie_same_site: "Lax".to_string(),
            refresh_cookie_domain: None,
            refresh_cookie_path: "/api/v1/auth".to_string(),
        },
        storage: shared::StorageConfig::default(),
        email: EmailConfig::default(),
    });
    let ctx = Arc::new(AppContext::new(
        config,
        Arc::new(Repositories::default()),
        Arc::new(domain::InMemoryStorage::default()),
    ));
    let _ = ctx; // keep alive to avoid dead_code warning
    let openapi = ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&openapi).expect("serialize openapi");
    print!("{}", json);
}
