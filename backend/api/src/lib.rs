pub mod routes;

use axum::{Router, routing::get};
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(paths(routes::health))]
pub struct ApiDoc;

pub fn router() -> Router {
    Router::new()
        .route("/api/v1/health", get(routes::health))
        .merge(SwaggerUi::new("/swagger-ui").url("/api/v1/openapi.json", ApiDoc::openapi()))
}

pub fn openapi_json() -> String {
    ApiDoc::openapi().to_json().unwrap()
}

pub async fn serve(_ctx: Arc<app::AppContext>) {
    let app = router();
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3456").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
