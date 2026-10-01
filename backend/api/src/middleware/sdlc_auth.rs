use axum::{extract::Request, middleware::Next, response::Response};
use shared::AppError;

/// Separate route layer: SDLC never inherits local-token fallback or project bypass.
pub async fn strict_central_auth(mut request: Request, next: Next) -> Result<Response, AppError> {
    let token = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| {
            h.strip_prefix("Bearer ")
                .or_else(|| h.strip_prefix("bearer "))
        })
        .ok_or(AppError::Unauthorized)?;
    let central = match super::central_auth::check_token(token).await {
        super::central_auth::CentralCheck::Validated(central) => central,
        super::central_auth::CentralCheck::Unavailable => {
            return Err(AppError::Unavailable("Central Auth unavailable".into()));
        }
        _ => return Err(AppError::Unauthorized),
    };
    if !central.allows_service("task-tracker", request.method().as_str()) {
        return Err(AppError::Forbidden);
    }
    request.extensions_mut().insert(domain::sdlc::Principal {
        subject: central.user_id,
        human_session: central.session_id.is_some(),
        scopes: central.scopes,
    });
    Ok(next.run(request).await)
}
