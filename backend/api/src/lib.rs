use app::AppContext;
use axum::{Json, Router, extract::State, routing::get};
use shared::Health;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

async fn health(State(_ctx): State<Arc<AppContext>>) -> Json<Health> {
    Json(Health {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

pub fn router(ctx: Arc<AppContext>) -> Router {
    Router::new()
        .route("/health", get(health))
        .with_state(ctx)
        .layer(CorsLayer::permissive())
}

pub async fn serve(ctx: Arc<AppContext>) -> anyhow::Result<()> {
    let addr = format!("{}:{}", ctx.config.server_address, ctx.config.server_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Server listening on http://{}", addr);
    axum::serve(listener, router(ctx)).await?;
    Ok(())
}
