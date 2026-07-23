use std::sync::Arc;

use domain::Repositories;
use shared::{AppConfig, AppError, DatabaseConfig};

pub async fn build_repositories(_config: DatabaseConfig) -> Result<Repositories, AppError> {
    Ok(Repositories::default())
}

pub async fn build_context(config: AppConfig) -> Result<app::AppContext, AppError> {
    let repos = Arc::new(build_repositories(config.database.clone()).await?);
    Ok(app::AppContext::new(Arc::new(config), repos))
}
