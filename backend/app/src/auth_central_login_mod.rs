//! Login proxy to the central fleet auth-server (services-base/auth-server).
//!
//! When `TT_AUTH__CENTRAL_LOGIN_URL` is set, login first tries the central
//! server with the same credentials; on success its access/refresh tokens are
//! returned verbatim (the bearer middleware validates them via JWKS). On
//! rejection the local password path runs.

#[derive(serde::Deserialize)]
pub(super) struct CentralAuthPair {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default)]
    pub expires_in: Option<u64>,
}

pub(super) struct CentralLoginConfig {
    login_url: String,
    timeout_secs: u64,
}

pub(super) fn central_login_config() -> Option<CentralLoginConfig> {
    let url = std::env::var("TT_AUTH__CENTRAL_LOGIN_URL").ok()?;
    if url.trim().is_empty() {
        return None;
    }
    Some(CentralLoginConfig {
        login_url: url,
        timeout_secs: std::env::var("TT_AUTH__CENTRAL_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5),
    })
}

/// `Ok(None)` — central not configured. `Err(None)` — central rejected the
/// credentials (fall back to local). `Err(Some(err))` — transport error.
pub(super) async fn try_central_login(
    config: &CentralLoginConfig,
    email: &str,
    password: &str,
) -> Result<Option<CentralAuthPair>, Option<String>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .build()
        .map_err(|e| Some(e.to_string()))?;
    let response = client
        .post(&config.login_url)
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, url = %config.login_url, "central login unreachable");
            Some(e.to_string())
        })?;
    if !response.status().is_success() {
        return Err(None);
    }
    let pair = response
        .json::<CentralAuthPair>()
        .await
        .map_err(|e| Some(e.to_string()))?;
    Ok(Some(pair))
}
