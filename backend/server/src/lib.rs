use std::sync::Arc;

use app::AppContext;
use infra::{build_repositories, run_migrations};
use shared::AppConfig;
use tokio::sync::oneshot;

pub async fn run(
    config: Arc<AppConfig>,
    ready: oneshot::Sender<std::net::SocketAddr>,
    shutdown: oneshot::Receiver<()>,
) {
    run_migrations(config.database.clone())
        .await
        .expect("failed to run migrations");
    let repos = Arc::new(
        build_repositories(config.database.clone())
            .await
            .expect("failed to build repos"),
    );
    let ctx = Arc::new(AppContext::new(config.clone(), repos));

    let address = format!("{}:{}", config.server.address, config.server.port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("failed to bind server");
    let bound_addr = listener.local_addr().expect("local addr");
    let _ = ready.send(bound_addr);
    let server = axum::serve(listener, api::router(ctx.clone()).with_state(ctx));
    let _ = server
        .with_graceful_shutdown(async move {
            let _ = shutdown.await;
        })
        .await;
}
