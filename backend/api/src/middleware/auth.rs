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
            req.uri().query()?.split('&').find_map(|pair| {
                pair.strip_prefix("access_token=")
                    .and_then(|token| urlencoding::decode(token).ok())
                    .map(|token| token.into_owned())
            })
        })
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Central fleet auth-server first (ES256 via JWKS); legacy HS256 access
    // tokens remain valid during the migration window.
    if let Some(central) = super::central_auth::try_central(&token)
        .await
        .ok()
        .flatten()
    {
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

/// Resolves the local user for a central identity by its verified email,
/// creating a shadow account on first login (password_hash "!" — local
/// password verify always fails, the central server owns credentials).
async fn find_or_link_central_user(
    ctx: &Arc<app::AppContext>,
    central: &sdlc_auth_core::AuthContext,
) -> Result<domain::User, shared::AppError> {
    let email = central.email.as_deref().unwrap_or_default().to_lowercase();
    let email = email.trim();
    if email.is_empty() {
        return Err(shared::AppError::Unauthorized);
    }
    if let Ok(existing) = ctx.repos.users.get_by_email(email).await {
        if !existing.is_active {
            return Err(shared::AppError::Unauthorized);
        }
        return Ok(existing);
    }
    let username = email.split('@').next().unwrap_or("central");
    let user = domain::User {
        id: shared::UserId::new(),
        email: email.to_string().into(),
        username: username.to_string().into(),
        display_name: username.to_string().into(),
        password_hash: "!".to_string().into(),
        refresh_token_hash: None,
        is_system_admin: false,
        is_active: true,
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    let id = ctx.repos.users.save(&user).await?;
    ctx.repos.users.get_by_id(id).await
}

pub use app::auth::UserClaims;
