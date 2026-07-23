use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

pub async fn bearer_auth(
    State(ctx): State<Arc<app::AppContext>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth
        .strip_prefix("Bearer ")
        .or_else(|| auth.strip_prefix("bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let claims = ctx
        .services
        .auth
        .verify_token(token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

pub use app::auth::UserClaims;
