//! Task-tracker wiring of the shared central-auth bridge.
//!
//! All JWKS/login mechanics live in `sdlc_auth_core::service_bridge`;
//! this file only maps bridge outcomes to task-tracker types.

use sdlc_auth_core::service_bridge::{BridgeOutcome, ServiceBridge};

/// Env prefix: TT_AUTH__CENTRAL_{JWKS_URI,ISSUER,LOGIN_URL,TIMEOUT_SECS}.
pub static BRIDGE: ServiceBridge = ServiceBridge::new("TT_AUTH__CENTRAL");

/// Central-first bearer validation result, flattened for the middleware.
pub enum CentralCheck {
    /// Validated centrally — shadow user must be linked by the caller.
    Validated(sdlc_auth_core::AuthContext),
    /// Not a central token (or central not configured) — legacy path.
    FallThrough,
    /// Central token, expired.
    Expired,
}

pub async fn check_token(token: &str) -> CentralCheck {
    match BRIDGE.try_token(token).await {
        BridgeOutcome::Validated(ctx) => CentralCheck::Validated(ctx),
        BridgeOutcome::NotOurs | BridgeOutcome::NotConfigured => CentralCheck::FallThrough,
        BridgeOutcome::Expired => CentralCheck::Expired,
        BridgeOutcome::Invalid(reason) => {
            tracing::debug!(reason, "bearer is not a valid central token; legacy path");
            CentralCheck::FallThrough
        }
    }
}

/// Central login proxy; `None` = not configured / rejected / unreachable
/// (transport errors are logged, local login stays the fallback).
pub async fn try_login(
    email: &str,
    password: &str,
) -> Option<sdlc_auth_core::service_bridge::CentralTokenPair> {
    match BRIDGE.try_login(email, password).await {
        Ok(pair) => pair,
        Err(transport) => {
            tracing::warn!(%transport, "central login failed; local fallback");
            None
        }
    }
}
