//! Login proxy to the central fleet auth-server via the shared
//! `sdlc_auth_core::service_bridge` (env: TT_AUTH__CENTRAL_*).

use sdlc_auth_core::service_bridge::CentralTokenPair;

/// Env prefix shared with the api-crate middleware bridge instance.
const ENV_PREFIX: &str = "TT_AUTH__CENTRAL";

/// `None` = not configured / rejected centrally / unreachable — local
/// password login remains the fallback (transport errors are logged).
pub(super) async fn try_central_login(email: &str, password: &str) -> Option<CentralTokenPair> {
    sdlc_auth_core::service_bridge::ServiceBridge::new(ENV_PREFIX)
        .try_login(email, password)
        .await
        .ok()
        .flatten()
}
