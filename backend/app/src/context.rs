use infra::{AppConfig, connect};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppContext {
    pub config: AppConfig,
    pub db: Arc<DatabaseConnection>,
}

impl AppContext {
    pub async fn new(config: AppConfig) -> anyhow::Result<Self> {
        let db = connect(&config.database_url).await?;
        Ok(Self {
            config,
            db: Arc::new(db),
        })
    }
}
