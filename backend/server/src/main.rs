use std::sync::Arc;

use server::run;
use shared::AppConfig;

#[tokio::main]
async fn main() {
    sdlc_telemetry::init_tracing_with_options(
        "task-tracker",
        sdlc_telemetry::TracingOptions {
            json: true,
            target: true,
            current_span: true,
            default_filter: "info",
        },
    );

    let config = Arc::new(AppConfig::from_env().expect("failed to load config"));
    let (ready_tx, _ready_rx) = tokio::sync::oneshot::channel();
    let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    run(config, ready_tx, shutdown_rx).await;
}
