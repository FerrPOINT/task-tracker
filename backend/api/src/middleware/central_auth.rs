//! Bridge between task-tracker auth and the central fleet auth-server
//! (services-base/auth-server, ES256 + JWKS, audience `sdlc`).
//!
//! When `TT_AUTH__CENTRAL_JWKS_URI` is configured the bearer middleware tries
//! central validation first; the verified email claim links a local shadow
//! user (created on demand, unusable password hash). Legacy HS256 access
//! tokens keep working, enabling a zero-downtime cutover.

use sdlc_auth_core::{AuthContext, JwksCache, Validator};
use std::sync::Arc;
use tokio::sync::OnceCell;

pub struct CentralAuth {
    validator: Validator,
    #[allow(dead_code)] // kept for future direct JWKS access (rotation checks)
    jwks: Arc<JwksCache>,
}

static CENTRAL: OnceCell<Option<CentralAuth>> = OnceCell::const_new();

/// Reads `TT_AUTH__CENTRAL_JWKS_URI` / `TT_AUTH__CENTRAL_ISSUER` once.
/// `None` when central auth is not configured (legacy-only mode).
pub async fn central() -> Option<&'static CentralAuth> {
    CENTRAL
        .get_or_init(|| async {
            let uri = std::env::var("TT_AUTH__CENTRAL_JWKS_URI").ok()?;
            let issuer: std::sync::Arc<String> = std::sync::Arc::new(
                std::env::var("TT_AUTH__CENTRAL_ISSUER")
                    .unwrap_or_else(|_| "http://127.0.0.1:7701".into()),
            );
            match JwksCache::connect(&uri).await {
                Ok(jwks) => {
                    let jwks = Arc::new(jwks);
                    let validator = Validator::Jwks {
                        jwks: jwks.clone(),
                        issuer,
                    };
                    jwks.clone().spawn_refresh(std::time::Duration::from_secs(3600));
                    tracing::info!(jwks_uri = %uri, "central auth enabled");
                    Some(CentralAuth { validator, jwks })
                }
                Err(error) => {
                    tracing::warn!(%error, jwks_uri = %uri, "central auth unavailable; falling back to legacy sessions");
                    None
                }
            }
        })
        .await
        .as_ref()
}

/// Attempts central-token validation. `Ok(None)` = not a central token
/// (caller falls back to the legacy HS256 path).
pub async fn try_central(token: &str) -> Result<Option<AuthContext>, shared::AppError> {
    let Some(central) = central().await else {
        return Ok(None);
    };
    match central.validator.validate(token) {
        Ok(ctx) => Ok(Some(ctx)),
        // kid resolution failure = legacy token, not ours
        Err(sdlc_auth_core::AuthError::Jwks(_)) => Ok(None),
        Err(sdlc_auth_core::AuthError::Expired) => Err(shared::AppError::Unauthorized),
        Err(other) => {
            tracing::warn!(error = %other, "central token validation failed");
            Ok(None)
        }
    }
}
