use axum::{Extension, Json, body::Bytes, extract::State, http::StatusCode};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use shared::{AppError, UserId};
use std::sync::Arc;
use time::Duration;

use crate::dto::{AuthResponse, LoginRequest, RefreshRequest, RegisterRequest};
use app::auth::UserClaims;
use app::commands::{LoginCommand, RegisterCommand};

fn require_legacy_password_auth() -> Result<(), AppError> {
    if std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_some() {
        return Err(AppError::Forbidden);
    }
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    tag = "auth",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered; refresh session is issued in the HttpOnly cookie", body = AuthResponse, headers(("Set-Cookie" = String, description = "Rotated HttpOnly refresh-session cookie"))),
        (status = 400, description = "Bad request"),
    )
)]
pub async fn register(
    State(ctx): State<Arc<app::AppContext>>,
    jar: CookieJar,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, CookieJar, Json<AuthResponse>), AppError> {
    require_legacy_password_auth()?;
    let cmd = RegisterCommand {
        email: body.email,
        username: body.username.clone(),
        name: body.name.unwrap_or(body.username),
        password: body.password,
    };
    let dto = ctx.services.auth.register(cmd).await?;
    let jar = set_refresh_cookie(jar, &ctx.config.auth, &dto.refresh_token);
    Ok((StatusCode::CREATED, jar, Json(map_auth(dto))))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful; refresh session is issued in the HttpOnly cookie", body = AuthResponse, headers(("Set-Cookie" = String, description = "Rotated HttpOnly refresh-session cookie"))),
        (status = 401, description = "Invalid credentials"),
    )
)]
pub async fn login(
    State(ctx): State<Arc<app::AppContext>>,
    jar: CookieJar,
    Json(body): Json<LoginRequest>,
) -> Result<(CookieJar, Json<AuthResponse>), AppError> {
    require_legacy_password_auth()?;
    // MFA (docs/SECURITY.md): when TOTP is enabled the password alone is not
    // enough — respond with totp_required instead of tokens.
    let probe = ctx
        .services
        .auth
        .login(LoginCommand {
            email: body.email.clone(),
            password: body.password.clone(),
        })
        .await?;
    let totp_user_id: shared::UserId = probe.user.id.parse().map_err(|_| AppError::Unauthorized)?;
    if ctx.services.totp.is_enabled(totp_user_id).await? {
        let code = body.totp_code.as_deref().unwrap_or("");
        if code.is_empty() || !ctx.services.totp.login_verify(totp_user_id, code).await? {
            return Ok((
                jar,
                Json(AuthResponse {
                    access_token: String::new(),
                    token_type: "Bearer".to_string(),
                    user_id: probe.user.id.to_string(),
                    email: probe.user.email.clone(),
                    username: probe.user.username.clone(),
                    display_name: probe.user.display_name.clone(),
                    expires_in: 0,
                    totp_required: true,
                }),
            ));
        }
    }
    let cmd = LoginCommand {
        email: body.email,
        password: body.password,
    };
    let dto = ctx.services.auth.login(cmd).await?;
    let jar = set_refresh_cookie(jar, &ctx.config.auth, &dto.refresh_token);
    Ok((jar, Json(map_auth(dto))))
}

pub async fn refresh(
    State(ctx): State<Arc<app::AppContext>>,
    jar: CookieJar,
    body: Bytes,
) -> Result<(CookieJar, Json<AuthResponse>), AppError> {
    require_legacy_password_auth()?;
    let refresh_token = match jar
        .get(&ctx.config.auth.refresh_cookie_name)
        .map(|c| c.value().to_string())
        .filter(|t| !t.is_empty())
    {
        Some(token) => token,
        None => parse_refresh_body(&body)?.ok_or(AppError::Unauthorized)?,
    };
    let dto = ctx.services.auth.refresh(&refresh_token).await?;
    let jar = set_refresh_cookie(jar, &ctx.config.auth, &dto.refresh_token);
    Ok((jar, Json(map_auth(dto))))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    tag = "auth",
    request_body = RefreshRequest,
    params(
        ("refresh_token" = Option<String>, Cookie, description = "HttpOnly refresh-session cookie; browser clients use this primary credential"),
    ),
    responses(
        (status = 200, description = "Tokens refreshed and refresh session rotated", body = AuthResponse, headers(("Set-Cookie" = String, description = "Rotated HttpOnly refresh-session cookie"))),
        (status = 401, description = "Invalid refresh token"),
    )
)]
pub async fn refresh_openapi(
    State(_ctx): State<Arc<app::AppContext>>,
    Json(_body): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    unreachable!("this is a schema-only stub; use refresh handler at runtime")
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    tag = "auth",
    responses(
        (status = 204, description = "Logged out"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []))
)]
pub async fn logout_openapi(
    State(_ctx): State<Arc<app::AppContext>>,
) -> Result<StatusCode, AppError> {
    unreachable!("this is a schema-only stub; use logout handler at runtime")
}

pub async fn logout(
    State(ctx): State<Arc<app::AppContext>>,
    jar: CookieJar,
    Extension(claims): Extension<UserClaims>,
) -> Result<(CookieJar, StatusCode), AppError> {
    let user_id = UserId::from_uuid(
        claims
            .sub
            .parse()
            .map_err(|_| AppError::invalid_input("invalid user id"))?,
    );
    ctx.services.auth.logout(user_id).await?;
    let jar = clear_refresh_cookie(jar, &ctx.config.auth);
    Ok((jar, StatusCode::NO_CONTENT))
}

fn set_refresh_cookie(jar: CookieJar, cfg: &shared::AuthConfig, token: &str) -> CookieJar {
    let mut cookie = Cookie::new(cfg.refresh_cookie_name.clone(), token.to_string());
    cookie.set_http_only(true);
    cookie.set_secure(cfg.refresh_cookie_secure);
    cookie.set_same_site(parse_same_site(&cfg.refresh_cookie_same_site));
    cookie.set_path(cfg.refresh_cookie_path.clone());
    if let Some(domain) = &cfg.refresh_cookie_domain {
        cookie.set_domain(domain.clone());
    }
    jar.add(cookie)
}

fn clear_refresh_cookie(jar: CookieJar, cfg: &shared::AuthConfig) -> CookieJar {
    let mut cookie = Cookie::new(cfg.refresh_cookie_name.clone(), "");
    cookie.set_http_only(true);
    cookie.set_secure(cfg.refresh_cookie_secure);
    cookie.set_same_site(parse_same_site(&cfg.refresh_cookie_same_site));
    cookie.set_path(cfg.refresh_cookie_path.clone());
    if let Some(domain) = &cfg.refresh_cookie_domain {
        cookie.set_domain(domain.clone());
    }
    cookie.set_max_age(Duration::seconds(0));
    jar.add(cookie)
}

fn parse_same_site(value: &str) -> axum_extra::extract::cookie::SameSite {
    match value.to_ascii_lowercase().as_str() {
        "strict" => axum_extra::extract::cookie::SameSite::Strict,
        "none" => axum_extra::extract::cookie::SameSite::None,
        _ => axum_extra::extract::cookie::SameSite::Lax,
    }
}

fn parse_refresh_body(body: &[u8]) -> Result<Option<String>, AppError> {
    if body.is_empty() {
        return Ok(None);
    }
    let request: RefreshRequest =
        serde_json::from_slice(body).map_err(|_| AppError::invalid_input("refresh_token"))?;
    Ok(request.refresh_token.filter(|token| !token.is_empty()))
}

fn map_auth(dto: app::dto::AuthDto) -> AuthResponse {
    // The refresh token travels ONLY in the HttpOnly cookie set by the
    // caller; it must never be serialized into the JSON body.
    AuthResponse {
        access_token: dto.access_token,
        totp_required: false,
        token_type: "Bearer".to_string(),
        user_id: dto.user.id,
        email: dto.user.email,
        username: dto.user.username,
        display_name: dto.user.display_name,
        expires_in: dto.expires_in,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn clear_refresh_cookie_preserves_scope_attributes() {
        let cfg = shared::AuthConfig {
            refresh_cookie_domain: Some("example.com".to_string()),
            refresh_cookie_same_site: "Strict".to_string(),
            refresh_cookie_path: "/api/v1/auth".to_string(),
            ..Default::default()
        };

        let response = clear_refresh_cookie(CookieJar::new(), &cfg).into_response();
        let header = response
            .headers()
            .get_all(axum::http::header::SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .find(|value| value.starts_with("refresh_token="))
            .expect("logout must clear the configured refresh cookie");

        assert!(header.contains("Domain=example.com"), "{header}");
        assert!(header.contains("Path=/api/v1/auth"), "{header}");
        assert!(header.contains("SameSite=Strict"), "{header}");
        assert!(header.contains("Secure"), "{header}");
        assert!(header.contains("HttpOnly"), "{header}");
        assert!(header.contains("Max-Age=0"), "{header}");
    }
}

// --- TOTP MFA management (docs/SECURITY.md, docs/SYSTEM_ADMIN.md) ---

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct TotpCodeRequest {
    pub code: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/totp/setup",
    tag = "auth",
    responses(
        (status = 200, description = "Enrollment started; secret returned exactly once", body = TotpSetupResponse),
        (status = 401, description = "Not authenticated"),
    )
)]
pub async fn totp_setup(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
) -> Result<Json<TotpSetupResponse>, AppError> {
    let user_id: shared::UserId = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let dto = ctx.services.totp.setup(user_id).await?;
    Ok(Json(TotpSetupResponse {
        secret: dto.secret,
        otpauth_uri: dto.otpauth_uri,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/totp/enable",
    tag = "auth",
    request_body = TotpCodeRequest,
    responses(
        (status = 200, description = "MFA enabled; recovery codes returned exactly once", body = TotpEnabledResponse),
        (status = 400, description = "Invalid code"),
    )
)]
pub async fn totp_enable(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    Json(body): Json<TotpCodeRequest>,
) -> Result<Json<TotpEnabledResponse>, AppError> {
    let user_id: shared::UserId = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    let dto = ctx.services.totp.enable(user_id, &body.code).await?;
    Ok(Json(TotpEnabledResponse {
        recovery_codes: dto.recovery_codes,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/totp/disable",
    tag = "auth",
    request_body = TotpCodeRequest,
    responses(
        (status = 204, description = "MFA disabled"),
        (status = 400, description = "Invalid code or not enabled"),
    )
)]
pub async fn totp_disable(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    Json(body): Json<TotpCodeRequest>,
) -> Result<StatusCode, AppError> {
    let user_id: shared::UserId = claims.sub.parse().map_err(|_| AppError::Unauthorized)?;
    ctx.services.totp.disable(user_id, &body.code).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct TotpSetupResponse {
    pub secret: String,
    pub otpauth_uri: String,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct TotpEnabledResponse {
    pub recovery_codes: Vec<String>,
}

// --- Password reset (docs/SYSTEM_ADMIN.md §1.2) ---

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct PasswordResetRequest {
    pub email: String,
}

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct PasswordResetConfirm {
    pub token: String,
    pub new_password: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/password/request",
    tag = "auth",
    request_body = PasswordResetRequest,
    responses(
        (status = 202, description = "Reset email sent when the account exists; indistinguishable for unknown emails"),
        (status = 400, description = "Invalid email"),
    )
)]
pub async fn password_reset_request(
    State(ctx): State<Arc<app::AppContext>>,
    Json(body): Json<PasswordResetRequest>,
) -> Result<StatusCode, AppError> {
    require_legacy_password_auth()?;
    let email = body.email.trim().to_lowercase();
    if !email.contains('@') || email.len() < 5 {
        return Err(AppError::invalid_input("invalid email"));
    }
    ctx.services.auth.request_password_reset(&email).await?;
    Ok(StatusCode::ACCEPTED)
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/password/reset",
    tag = "auth",
    request_body = PasswordResetConfirm,
    responses(
        (status = 204, description = "Password changed; refresh sessions revoked"),
        (status = 400, description = "Weak password"),
        (status = 404, description = "Unknown, expired or already used token"),
    )
)]
pub async fn password_reset_confirm(
    State(ctx): State<Arc<app::AppContext>>,
    Json(body): Json<PasswordResetConfirm>,
) -> Result<StatusCode, AppError> {
    require_legacy_password_auth()?;
    ctx.services
        .auth
        .reset_password(&body.token, &body.new_password)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/oidc/begin",
    tag = "auth",
    responses(
        (status = 302, description = "Redirect to the configured OIDC provider authorization endpoint (state + PKCE)"),
        (status = 404, description = "OIDC is not configured"),
    )
)]
pub async fn oidc_begin(
    State(ctx): State<Arc<app::AppContext>>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    let oidc = ctx
        .services
        .oidc
        .as_ref()
        .ok_or_else(|| AppError::not_found("oidc", "not configured"))?;
    let url = oidc.begin().await?;
    Ok(axum::response::Redirect::temporary(&url))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/oidc/callback",
    tag = "auth",
    params(
        ("state" = String, Query, description = "Single-use authorization state"),
        ("code" = String, Query, description = "Provider authorization code"),
    ),
    responses(
        (status = 200, description = "OIDC login complete; local session tokens issued", body = AuthResponse),
        (status = 401, description = "Invalid/expired state, nonce mismatch or provider error"),
        (status = 404, description = "OIDC is not configured"),
    )
)]
pub async fn oidc_callback(
    State(ctx): State<Arc<app::AppContext>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<axum::Json<AuthResponse>, AppError> {
    let oidc = ctx
        .services
        .oidc
        .as_ref()
        .ok_or_else(|| AppError::not_found("oidc", "not configured"))?;
    let state = params
        .get("state")
        .ok_or_else(|| AppError::invalid_input("missing state"))?;
    let code = params
        .get("code")
        .ok_or_else(|| AppError::invalid_input("missing code"))?;
    let dto = oidc.callback(state, code).await?;
    Ok(axum::Json(map_auth(dto)))
}
