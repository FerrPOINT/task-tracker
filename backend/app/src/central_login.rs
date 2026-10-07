//! Login proxy to the central fleet auth-server via the shared
//! `sdlc_auth_core::service_bridge` (env: TT_AUTH__CENTRAL_*).

use sdlc_auth_core::service_bridge::{BridgeOutcome, CentralTokenPair, ServiceBridge};

/// Env prefix shared with the api-crate middleware bridge instance.
const ENV_PREFIX: &str = "TT_AUTH__CENTRAL";
static BRIDGE: ServiceBridge = ServiceBridge::new(ENV_PREFIX);

/// A configured central bridge must fail closed when login or profile
/// verification fails; the caller only permits legacy login when unconfigured.
pub(super) async fn try_central_login(
    email: &str,
    password: &str,
) -> Option<(CentralTokenPair, sdlc_auth_core::AuthContext, String)> {
    let pair = BRIDGE.try_login(email, password).await.ok().flatten()?;
    match BRIDGE.try_token_with_name(&pair.access_token).await {
        (BridgeOutcome::Validated(ctx), Some(name)) => Some((pair, ctx, name)),
        _ => None,
    }
}
