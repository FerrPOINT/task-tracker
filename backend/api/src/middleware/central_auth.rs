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
    Validated(sdlc_auth_core::AuthContext, Option<String>),
    /// Not a central token (or central not configured) — legacy path.
    FallThrough,
    /// Central token, expired.
    Expired,
    Unavailable,
}

pub async fn check_token(token: &str) -> CentralCheck {
    let (outcome, name) = BRIDGE.try_token_with_name(token).await;
    classify(outcome, name)
}

fn classify(outcome: BridgeOutcome, name: Option<String>) -> CentralCheck {
    match outcome {
        BridgeOutcome::Validated(ctx) => CentralCheck::Validated(ctx, name),
        BridgeOutcome::NotOurs | BridgeOutcome::NotConfigured => CentralCheck::FallThrough,
        BridgeOutcome::Expired => CentralCheck::Expired,
        BridgeOutcome::Invalid(reason) => {
            tracing::debug!(reason, "central token rejected");
            CentralCheck::Expired
        }
        BridgeOutcome::Unavailable => CentralCheck::Unavailable,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_absence_preserves_verified_identity_and_failures_stay_closed() {
        let identity = sdlc_auth_core::AuthContext {
            user_id: "exact-owner".into(),
            email: Some("owner@example.test".into()),
            role: None,
            scopes: ["task-tracker:read".into()].into(),
            session_id: None,
            token: "sdlc_pat_test-only".into(),
        };
        let CentralCheck::Validated(ctx, name) = classify(BridgeOutcome::Validated(identity), None)
        else {
            panic!("verified machine/PAT identity cannot depend on display metadata");
        };
        assert!(name.is_none());
        assert_eq!(ctx.user_id, "exact-owner");
        assert!(ctx.allows_service("task-tracker", "GET"));
        assert!(!ctx.allows_service("task-tracker", "PUT"));
        assert!(matches!(
            classify(BridgeOutcome::Invalid("revoked".into()), None),
            CentralCheck::Expired
        ));
        assert!(matches!(
            classify(BridgeOutcome::Unavailable, None),
            CentralCheck::Unavailable
        ));
    }
}
