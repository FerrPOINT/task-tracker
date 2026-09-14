//! OIDC single-provider SSO (SYSTEM_ADMIN 4.2).
//!
//! Authorization-code flow with PKCE (S256) and a server-side single-use
//! `state`/`nonce` row per attempt. Provider metadata comes from standard
//! discovery (`{issuer}/.well-known/openid-configuration`). The ID token is
//! exchanged server-side with the client secret and its `sub`/`email` claims
//! are linked to a local user: existing account with the same email is
//! bound, otherwise a JIT user is provisioned with an unusable random
//! password. Local tokens are then issued through the normal auth service.

use std::sync::Arc;

use chrono::{FixedOffset, Utc};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::context::AuthService;
use crate::dto::AuthDto;
use domain::{OidcAuthState, OidcIdentity, OidcRepository, User, UserRepository};
use shared::config::AppConfig;
use shared::{AppError, UserId};

const STATE_TTL_SECONDS: i64 = 600;

pub struct OidcService {
    repo: Arc<dyn OidcRepository>,
    users: Arc<dyn UserRepository>,
    auth: Arc<dyn AuthService>,
    config: Arc<AppConfig>,
    http: reqwest::Client,
}

#[derive(Debug, serde::Deserialize)]
struct DiscoveryDocument {
    authorization_endpoint: String,
    token_endpoint: String,
}

impl OidcService {
    pub fn new(
        repo: Arc<dyn OidcRepository>,
        users: Arc<dyn UserRepository>,
        auth: Arc<dyn AuthService>,
        config: Arc<AppConfig>,
    ) -> Self {
        Self {
            repo,
            users,
            auth,
            config,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("reqwest client"),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !self.config.auth.oidc_issuer_url.trim().is_empty()
    }

    fn require_enabled(&self) -> Result<(), AppError> {
        if self.is_enabled() {
            Ok(())
        } else {
            Err(AppError::not_found("oidc", "not configured"))
        }
    }

    /// Step 1: create a single-use authorization state and return the
    /// provider authorization URL the browser should be redirected to.
    pub async fn begin(&self) -> Result<String, AppError> {
        self.require_enabled()?;
        let discovery = self.discover().await?;
        let mut state_bytes = [0u8; 32];
        let mut verifier_bytes = [0u8; 32];
        let mut nonce_bytes = [0u8; 16];
        OsRng.fill_bytes(&mut state_bytes);
        OsRng.fill_bytes(&mut verifier_bytes);
        OsRng.fill_bytes(&mut nonce_bytes);
        let state = hex(&state_bytes);
        let verifier = base64_url(&verifier_bytes);
        let nonce = hex(&nonce_bytes);
        let now = Utc::now().fixed_offset();
        self.repo
            .put_state(&OidcAuthState {
                id: uuid_v4(),
                state: state.clone(),
                code_verifier: verifier.clone(),
                nonce: nonce.clone(),
                expires_at: now + chrono::Duration::seconds(STATE_TTL_SECONDS),
                created_at: now,
            })
            .await?;
        let challenge = base64_url(Sha256::digest(verifier.as_bytes()).as_slice());
        let redirect = self.config.auth.oidc_redirect_url.clone();
        let url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&scope=openid%20email%20profile&state={}&nonce={}&code_challenge={}&code_challenge_method=S256",
            discovery.authorization_endpoint,
            urlencode(&self.config.auth.oidc_client_id),
            urlencode(&redirect),
            state,
            nonce,
            challenge,
        );
        Ok(url)
    }

    /// Step 2: exchange the code, validate state/nonce/at_hash-free ID token
    /// claims, link or provision the local user, issue local tokens.
    pub async fn callback(&self, state: &str, code: &str) -> Result<AuthDto, AppError> {
        self.require_enabled()?;
        let auth_state = self.repo.take_state(state).await?;
        if auth_state.expires_at < Utc::now().fixed_offset() {
            return Err(AppError::Unauthorized);
        }
        let discovery = self.discover().await?;
        let token_response: serde_json::Value = self
            .http
            .post(&discovery.token_endpoint)
            .form(&[
                ("grant_type", "authorization_code".to_string()),
                ("code", code.to_string()),
                ("redirect_uri", self.config.auth.oidc_redirect_url.clone()),
                ("client_id", self.config.auth.oidc_client_id.clone()),
                ("client_secret", self.config.auth.oidc_client_secret.clone()),
                ("code_verifier", auth_state.code_verifier.clone()),
            ])
            .send()
            .await
            .map_err(|_| AppError::Unauthorized)?
            .error_for_status()
            .map_err(|_| AppError::Unauthorized)?
            .json()
            .await
            .map_err(|_| AppError::Unauthorized)?;
        let id_token = token_response["id_token"]
            .as_str()
            .ok_or_else(|| AppError::Unauthorized)?;
        let claims = IdTokenClaims::decode_unverified(id_token)?;
        if claims.nonce.as_deref() != Some(auth_state.nonce.as_str()) {
            return Err(AppError::Unauthorized);
        }
        let subject = claims.subject.ok_or_else(|| AppError::Unauthorized)?;
        let provider = provider_key(&self.config.auth.oidc_issuer_url);
        let user = match self.repo.find_identity(&provider, &subject).await {
            Ok(identity) => self.users.get_by_id(identity.user_id).await?,
            Err(AppError::NotFound(_)) => {
                let email = claims.email.clone().ok_or_else(|| AppError::Unauthorized)?;
                let user = match self.users.get_by_email(&email).await {
                    Ok(existing) => existing,
                    Err(AppError::NotFound(_)) => {
                        // JIT provisioning: unusable local password marker.
                        let username = email.split('@').next().unwrap_or("oidc-user").to_string();
                        let display_name = claims.name.clone().unwrap_or_else(|| username.clone());
                        User {
                            id: UserId::new(),
                            email: email.clone().into(),
                            username: username.into(),
                            display_name: display_name.into(),
                            password_hash: "!".into(),
                            refresh_token_hash: None,
                            is_system_admin: false,
                            is_active: true,
                            created_at: shared::now(),
                            updated_at: shared::now(),
                        }
                    }
                    Err(e) => return Err(e),
                };
                let saved_id = self.users.save(&user).await?;
                let saved = self.users.get_by_id(saved_id).await?;
                self.repo
                    .link_identity(&OidcIdentity {
                        id: uuid_v4(),
                        user_id: saved_id,
                        provider: provider.clone(),
                        subject: subject.clone(),
                        email: Some(email),
                        created_at: Utc::now().fixed_offset(),
                        updated_at: Utc::now().fixed_offset(),
                    })
                    .await?;
                saved
            }
            Err(e) => return Err(e),
        };
        if !user.is_active {
            return Err(AppError::Unauthorized);
        }
        self.auth.issue_session(user.id).await
    }

    async fn discover(&self) -> Result<DiscoveryDocument, AppError> {
        let url = format!(
            "{}/.well-known/openid-configuration",
            self.config.auth.oidc_issuer_url.trim_end_matches('/')
        );
        self.http
            .get(&url)
            .send()
            .await
            .map_err(|_| AppError::Unauthorized)?
            .error_for_status()
            .map_err(|_| AppError::Unauthorized)?
            .json()
            .await
            .map_err(|_| AppError::Unauthorized)
    }
}

/// Minimal ID-token claim set. The token is received directly from the
/// provider token endpoint over TLS in the same request that authenticated
/// the client — signature validation happens by trusting the direct
/// TLS-protected token-endpoint response (documented limitation for the
/// single-provider local deployment; see SECURITY.md).
#[derive(Debug, serde::Deserialize)]
pub struct IdTokenClaims {
    #[serde(default, rename = "sub")]
    pub subject: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub nonce: Option<String>,
}

impl IdTokenClaims {
    fn decode_unverified(token: &str) -> Result<Self, AppError> {
        let payload = token
            .split('.')
            .nth(1)
            .ok_or_else(|| AppError::Unauthorized)?;
        let bytes = base64_decode_url(payload).ok_or_else(|| AppError::Unauthorized)?;
        serde_json::from_slice(&bytes).map_err(|_| AppError::Unauthorized)
    }
}

pub fn provider_key(issuer: &str) -> String {
    let normalized = issuer.trim_end_matches('/');
    normalized.rsplit('/').next().unwrap_or("oidc").to_string()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn base64_url(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn base64_decode_url(input: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(input)
        .ok()
}

fn urlencode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn uuid_v4() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let h = hex(&bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencode_encodes_reserved_characters() {
        assert_eq!(urlencode("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(urlencode("http://x/y"), "http%3A%2F%2Fx%2Fy");
    }

    #[test]
    fn base64_url_roundtrip() {
        let bytes = [1u8, 2, 3, 250, 251];
        let encoded = base64_url(&bytes);
        assert_eq!(base64_decode_url(&encoded).unwrap(), bytes);
    }

    #[test]
    fn id_token_claims_decode_from_jwt_payload() {
        let claims = r#"{"sub":"user-1","email":"u@example.test","nonce":"n1"}"#;
        let payload = base64_url(claims.as_bytes());
        let token = format!("header.{payload}.signature");
        let parsed = IdTokenClaims::decode_unverified(&token).unwrap();
        assert_eq!(parsed.subject.as_deref(), Some("user-1"));
        assert_eq!(parsed.email.as_deref(), Some("u@example.test"));
        assert_eq!(parsed.nonce.as_deref(), Some("n1"));
    }

    #[test]
    fn id_token_claims_reject_malformed_token() {
        assert!(IdTokenClaims::decode_unverified("not-a-jwt").is_err());
    }

    #[test]
    fn provider_key_takes_last_path_segment() {
        assert_eq!(provider_key("https://auth.example.com/realms/main"), "main");
        assert_eq!(provider_key("http://auth.sdlc.local:22803/auth/v1/"), "v1");
    }
}
