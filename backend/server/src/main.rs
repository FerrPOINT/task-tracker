use std::sync::Arc;

use server::run;
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
    let (ready_tx, _ready_rx) = tokio::sync::oneshot::channel();
    let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    run(config, ready_tx, shutdown_rx).await;
}
