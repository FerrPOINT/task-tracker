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
    // Bearer header is the primary auth; `?access_token=` query is accepted only
    // for the SSE endpoint where EventSource cannot set headers.
    let token: String = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| {
            auth.strip_prefix("Bearer ")
                .or_else(|| auth.strip_prefix("bearer "))
                .map(str::to_string)
        })
        .or_else(|| {
            // EventSource cannot set headers; the SSE endpoint also accepts
            // an `access_token` query parameter.
            if !req.uri().path().ends_with("/events") {
                return None;
            }
            req.uri()
                .query()?
                .split('&')
                .find_map(|pair| pair.strip_prefix("access_token=").map(str::to_string))
        })
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let claims = ctx
        .services
        .auth
        .verify_token(token.as_str())
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Reject tokens belonging to deactivated accounts. Without this check a
    // user disabled by an admin could keep using previously issued tokens.
    let user_id: shared::UserId = claims
        .sub
        .parse()
        .map(shared::UserId::from_uuid)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let user = ctx
        .repos
        .users
        .get_by_id(user_id)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    if !user.is_active {
        return Err(StatusCode::UNAUTHORIZED);
    }

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

pub use app::auth::UserClaims;
