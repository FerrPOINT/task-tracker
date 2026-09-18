#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, body = String))
)]
pub async fn catalog_health() -> &'static str {
    "ok"
}

#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses((status = 200, body = String))
)]
pub async fn health() -> &'static str {
    catalog_health().await
}
