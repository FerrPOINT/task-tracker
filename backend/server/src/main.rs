use std::sync::Arc;

use server::run;
use shared::AppConfig;

#[tokio::main]
async fn main() {
    pdlc_telemetry::init_tracing_with_options(
        "task-tracker",
        pdlc_telemetry::TracingOptions {
            json: true,
            target: true,
            current_span: true,
            default_filter: "info",
        },
    );

    let config = Arc::new(AppConfig::from_env().expect("failed to load config"));
    let (ready_tx, _ready_rx) = tokio::sync::oneshot::channel();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("failed to observe SIGTERM");
    tokio::spawn(async move {
        #[cfg(unix)]
        let result = tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = terminate.recv() => Ok(()),
        };
        #[cfg(not(unix))]
        let result = tokio::signal::ctrl_c().await;
        if let Err(error) = result {
            tracing::error!(%error, "failed to observe the shutdown signal");
        }
        let _ = shutdown_tx.send(());
    });
    run(config, ready_tx, shutdown_rx).await;
}
