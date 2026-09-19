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
    let token: String = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| {
            auth.strip_prefix("Bearer ")
                .or_else(|| auth.strip_prefix("bearer "))
                .map(str::to_string)
        })
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Central fleet auth-server first (ES256 via JWKS); legacy HS256 access
    // tokens remain valid during the migration window.
    match super::central_auth::check_token(&token).await {
        super::central_auth::CentralCheck::Validated(central) => {
            if !central.allows_service("task-tracker", req.method().as_str()) {
                return Err(StatusCode::FORBIDDEN);
            }
            let user = find_or_link_central_user(&ctx, &central)
                .await
                .map_err(|_| StatusCode::UNAUTHORIZED)?;
            let claims = app::auth::UserClaims {
                sub: user.id.as_uuid().to_string(),
                exp: 0, // central token lifetime is enforced by the central validator
                typ: Some("access".to_string()),
                jti: None,
            };
            req.extensions_mut().insert(claims);
            return Ok(next.run(req).await);
        }
        super::central_auth::CentralCheck::Expired => return Err(StatusCode::UNAUTHORIZED),
        super::central_auth::CentralCheck::Unavailable => {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        super::central_auth::CentralCheck::FallThrough => {}
    }

    if std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_some() {
        return Err(StatusCode::UNAUTHORIZED);
    }

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

/// Resolves a central subject independently of historical local email rows.
async fn find_or_link_central_user(
    ctx: &Arc<app::AppContext>,
    central: &sdlc_auth_core::AuthContext,
) -> Result<domain::User, shared::AppError> {
    let email = central.email.as_deref().unwrap_or_default().to_lowercase();
    let email = email.trim();
    if email.is_empty() {
        return Err(shared::AppError::Unauthorized);
    }
    ctx.repos
        .users
        .find_or_create_central_user(
            &central.user_id,
            email,
            email.split('@').next().unwrap_or(email),
        )
        .await
}

pub use app::auth::UserClaims;
