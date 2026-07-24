#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses((status = 200, body = String))
)]
pub async fn health() -> &'static str {
    "ok"
}
