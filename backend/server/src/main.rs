use std::sync::Arc;

use app::AppContext;
use infra::{build_repositories, run_migrations};
use shared::AppConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let config = Arc::new(AppConfig::from_env().expect("failed to load config"));
    run_migrations(config.database.clone())
        .await
        .expect("failed to run migrations");
    let repos = Arc::new(
        build_repositories(config.database.clone())
            .await
            .expect("failed to build repos"),
    );
    let ctx = Arc::new(AppContext::new(config, repos));

    api::serve(ctx).await;
}
