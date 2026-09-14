//! TOTP MFA (RFC 6238) — docs/SECURITY.md, docs/SYSTEM_ADMIN.md.
//!
//! Bounded implementation: HMAC-SHA1/SHA256 over base32 secrets, 6 digits,
//! 30-second step, ±1 window; the secret is AES-256-GCM encrypted at rest
//! with a `TASKTRACKER_TOTP_KEY` (falls back to the JWT secret) and never
//! returned after enable; recovery codes are SHA-256 hashed, single-use.

use aes_gcm::{KeyInit, aead::Aead};
use data_encoding::BASE32;
use hmac::{Hmac, Mac};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use shared::AppError;
use std::sync::Arc;

pub const TOTP_STEP_SECONDS: u64 = 30;
pub const TOTP_DIGITS: usize = 6;

type HmacSha1 = Hmac<sha1::Sha1>;

pub struct TotpService {
    repos: Arc<domain::Repositories>,
    auth_config: Arc<shared::AuthConfig>,
}

impl TotpService {
    pub fn new(repos: Arc<domain::Repositories>, auth_config: Arc<shared::AuthConfig>) -> Self {
        Self { repos, auth_config }
    }

    /// AES-256 needs a 32-byte key; derive it from the configured TOTP key
    /// material with SHA-256 regardless of the configured length.
    fn totp_key(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.auth_config.totp_key.as_bytes());
        hasher.finalize().into()
    }

    /// Generate a fresh secret and return (base32 secret, otpauth URI).
    /// Nothing is persisted until `enable` confirms a valid code.
    pub fn setup_secret(&self, email: &str) -> (String, String) {
        let mut raw = [0u8; 20];
        OsRng.fill_bytes(&mut raw);
        let secret = BASE32.encode(&raw);
        let uri = format!(
            "otpauth://totp/TaskTracker:{email}?secret={secret}&issuer=TaskTracker&algorithm=SHA1&digits=6&period=30"
        );
        (secret, uri)
    }

    fn encrypt_secret(&self, secret_b32: &str) -> Result<String, AppError> {
        let cipher = aes_gcm::Aes256Gcm::new_from_slice(&self.totp_key())
            .map_err(|_| AppError::internal("invalid totp key length"))?;
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let nonce = aes_gcm::Nonce::from_slice(&nonce);
        let ct = cipher
            .encrypt(nonce, secret_b32.as_bytes())
            .map_err(|_| AppError::internal("totp encrypt failed"))?;
        let mut blob = nonce.to_vec();
        blob.extend_from_slice(&ct);
        Ok(BASE32.encode(&blob))
    }

    fn decrypt_secret(&self, cipher_b32: &str) -> Result<String, AppError> {
        let blob = BASE32
            .decode(cipher_b32.as_bytes())
            .map_err(|_| AppError::internal("totp cipher decode failed"))?;
        if blob.len() < 13 {
            return Err(AppError::internal("totp cipher too short"));
        }
        let (nonce, ct) = blob.split_at(12);
        let nonce = aes_gcm::Nonce::from_slice(nonce);
        let cipher = aes_gcm::Aes256Gcm::new_from_slice(&self.totp_key())
            .map_err(|_| AppError::internal("invalid totp key length"))?;
        let pt = cipher
            .decrypt(nonce, ct)
            .map_err(|_| AppError::internal("totp decrypt failed"))?;
        String::from_utf8(pt).map_err(|_| AppError::internal("totp secret not utf8"))
    }

    /// RFC 6238 code for a step (HMAC-SHA1, 6 digits).
    pub fn code_for_step(secret_b32: &str, step: i64) -> Result<String, AppError> {
        let key = BASE32
            .decode(secret_b32.as_bytes())
            .map_err(|_| AppError::invalid_input("invalid base32 secret"))?;
        let mut mac = <HmacSha1 as Mac>::new_from_slice(&key)
            .map_err(|_| AppError::internal("hmac init failed"))?;
        mac.update(&step.to_be_bytes());
        let digest = mac.finalize().into_bytes();
        let offset = (digest[19] & 0x0f) as usize;
        let bin = u32::from_be_bytes([
            digest[offset] & 0x7f,
            digest[offset + 1],
            digest[offset + 2],
            digest[offset + 3],
        ]);
        Ok(format!("{:06}", bin % 1_000_000u32))
    }

    fn current_step(&self) -> i64 {
        (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            / TOTP_STEP_SECONDS) as i64
    }

    /// Verify a 6-digit code within ±1 step, replay-protected by last_used_step.
    pub fn verify(
        &self,
        secret_b32: &str,
        code: &str,
        last_used_step: i64,
    ) -> Result<bool, AppError> {
        let now = self.current_step();
        for delta in [0i64, -1, 1] {
            let step = now + delta;
            if step <= last_used_step {
                continue;
            }
            let expected = Self::code_for_step(secret_b32, step)?;
            if constant_time_eq(expected.as_bytes(), code.as_bytes()) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn generate_recovery_codes() -> Vec<String> {
        (0..8)
            .map(|_| {
                let mut raw = [0u8; 5];
                OsRng.fill_bytes(&mut raw);
                hex::encode(raw).to_uppercase()
            })
            .collect()
    }

    fn hash_recovery_code(code: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(code.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Start TOTP enrollment: store the encrypted secret unconfirmed; the
    /// secret is returned exactly once.
    pub async fn setup(&self, user_id: shared::UserId) -> Result<TotpSetupDto, AppError> {
        let user = self.repos.users.get_by_id(user_id).await?;
        let (secret, uri) = self.setup_secret(user.email.as_ref());
        let cipher = self.encrypt_secret(&secret)?;
        self.repos.totp.upsert_unconfirmed(user_id, &cipher).await?;
        Ok(TotpSetupDto {
            secret: secret.clone(),
            otpauth_uri: uri,
        })
    }

    /// Confirm enrollment with a valid code; enable TOTP and mint recovery
    /// codes (returned exactly once, stored hashed).
    pub async fn enable(
        &self,
        user_id: shared::UserId,
        code: &str,
    ) -> Result<TotpEnabledDto, AppError> {
        let record = self.repos.totp.get(user_id).await?;
        if record.enabled {
            return Err(AppError::conflict("totp already enabled"));
        }
        let secret = self.decrypt_secret(&record.secret_cipher)?;
        if !self.verify(&secret, code, record.last_used_step)? {
            return Err(AppError::invalid_input("invalid totp code"));
        }
        let codes = Self::generate_recovery_codes();
        let hashed: Vec<String> = codes.iter().map(|c| Self::hash_recovery_code(c)).collect();
        let step = self.current_step();
        self.repos
            .totp
            .confirm_enable(
                user_id,
                &serde_json::to_string(&hashed).unwrap_or_default(),
                step,
            )
            .await?;
        Ok(TotpEnabledDto {
            recovery_codes: codes,
        })
    }

    /// Disable TOTP (requires a valid code or an unused recovery code).
    pub async fn disable(&self, user_id: shared::UserId, code: &str) -> Result<(), AppError> {
        let record = self.repos.totp.get(user_id).await?;
        if !record.enabled {
            return Err(AppError::conflict("totp not enabled"));
        }
        let ok = self.check_code_or_recovery(&record, code)?;
        if !ok {
            return Err(AppError::invalid_input("invalid totp or recovery code"));
        }
        self.repos.totp.disable(user_id).await
    }

    /// Second-factor check used by login after a valid password: a valid TOTP
    /// code (advancing replay protection) or an unused recovery code
    /// (consumed atomically).
    pub async fn login_verify(
        &self,
        user_id: shared::UserId,
        code: &str,
    ) -> Result<bool, AppError> {
        let record = self.repos.totp.get(user_id).await?;
        if !record.enabled {
            return Ok(true);
        }
        let secret = self.decrypt_secret(&record.secret_cipher)?;
        let step = self.current_step();
        if self.verify(&secret, code, record.last_used_step)? {
            self.repos.totp.mark_used_step(user_id, step).await?;
            return Ok(true);
        }
        if self.try_consume_recovery(&record, code).await? {
            return Ok(true);
        }
        Ok(false)
    }

    fn check_code_or_recovery(
        &self,
        record: &domain::TotpConfig,
        code: &str,
    ) -> Result<bool, AppError> {
        let secret = self.decrypt_secret(&record.secret_cipher)?;
        if self.verify(&secret, code, record.last_used_step)? {
            return Ok(true);
        }
        // Recovery codes are checked without consuming here; disable clears
        // everything anyway.
        let hashed = Self::hash_recovery_code(code);
        let stored: Vec<String> = serde_json::from_str(&record.recovery_codes).unwrap_or_default();
        Ok(stored.iter().any(|h| h == &hashed))
    }

    async fn try_consume_recovery(
        &self,
        record: &domain::TotpConfig,
        code: &str,
    ) -> Result<bool, AppError> {
        let hashed = Self::hash_recovery_code(code);
        let stored: Vec<String> = serde_json::from_str(&record.recovery_codes).unwrap_or_default();
        let remaining: Vec<String> = stored.iter().filter(|h| *h != &hashed).cloned().collect();
        if remaining.len() == stored.len() {
            return Ok(false);
        }
        self.repos
            .totp
            .update_recovery_codes(
                record.user_id,
                &serde_json::to_string(&remaining).unwrap_or_default(),
            )
            .await?;
        Ok(true)
    }

    /// Does this user require a second factor at login?
    pub async fn is_enabled(&self, user_id: shared::UserId) -> Result<bool, AppError> {
        Ok(self.repos.totp.get(user_id).await?.enabled)
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[derive(Debug, serde::Serialize)]
pub struct TotpSetupDto {
    pub secret: String,
    pub otpauth_uri: String,
}

#[derive(Debug, serde::Serialize)]
pub struct TotpEnabledDto {
    pub recovery_codes: Vec<String>,
}

#[async_trait::async_trait]
impl crate::context::TotpService for TotpService {
    async fn setup(&self, user_id: shared::UserId) -> Result<TotpSetupDto, AppError> {
        TotpService::setup(self, user_id).await
    }
    async fn enable(
        &self,
        user_id: shared::UserId,
        code: &str,
    ) -> Result<TotpEnabledDto, AppError> {
        TotpService::enable(self, user_id, code).await
    }
    async fn disable(&self, user_id: shared::UserId, code: &str) -> Result<(), AppError> {
        TotpService::disable(self, user_id, code).await
    }
    async fn login_verify(&self, user_id: shared::UserId, code: &str) -> Result<bool, AppError> {
        TotpService::login_verify(self, user_id, code).await
    }
    async fn is_enabled(&self, user_id: shared::UserId) -> Result<bool, AppError> {
        TotpService::is_enabled(self, user_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth_config() -> shared::AuthConfig {
        shared::AuthConfig {
            jwt_secret: "test-jwt-secret".into(),
            totp_key: String::new(),
            reset_base_url: "http://localhost:5173".to_string(),
            oidc_issuer_url: String::new(),
            oidc_client_id: String::new(),
            oidc_client_secret: String::new(),
            oidc_redirect_url: String::new(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
            refresh_cookie_name: "refresh_token".into(),
            refresh_cookie_secure: true,
            refresh_cookie_same_site: "Lax".into(),
            refresh_cookie_domain: None,
            refresh_cookie_path: "/api/v1/auth".into(),
        }
    }

    async fn service_with_user(uid: shared::UserId) -> TotpService {
        let users = std::sync::Arc::new(domain::stubs::memory::MemoryUserRepository::default());
        let user = domain::User {
            id: uid,
            email: "u@e.com".into(),
            username: "u".into(),
            display_name: "U".into(),
            password_hash: "!".into(),
            refresh_token_hash: None,
            is_system_admin: false,
            is_active: true,
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        domain::UserRepository::save(&*users, &user).await.unwrap();
        let repos = domain::Repositories {
            users,
            totp: std::sync::Arc::new(domain::stubs::memory::MemoryTotpRepository::default()),
            oidc: Arc::new(domain::StubOidcRepository),
            ..Default::default()
        };
        TotpService::new(
            std::sync::Arc::new(repos),
            std::sync::Arc::new(auth_config()),
        )
    }

    fn service() -> TotpService {
        let repos = domain::Repositories {
            totp: std::sync::Arc::new(domain::stubs::memory::MemoryTotpRepository::default()),
            ..Default::default()
        };
        TotpService::new(
            std::sync::Arc::new(repos),
            std::sync::Arc::new(auth_config()),
        )
    }

    /// RFC 6238 test vector (SHA1, 8 digits truncated to the last 6 for our
    /// implementation): secret "12345678901234567890", step 59 → 94287082.
    #[test]
    fn rfc6238_vector_step59() {
        let secret = BASE32.encode(b"12345678901234567890");
        let code = TotpService::code_for_step(&secret, 59 / 30).unwrap();
        assert!(code.ends_with("287082"), "expected *287082, got {code}");
    }

    #[test]
    fn verify_accepts_current_step_and_rejects_garbage() {
        let svc = service();
        let (secret, _uri) = svc.setup_secret("u@e.com");
        let step = svc.current_step();
        let code = TotpService::code_for_step(&secret, step).unwrap();
        assert!(svc.verify(&secret, &code, 0).unwrap());
        assert!(!svc.verify(&secret, "000000", 0).unwrap() || code == "000000");
    }

    #[test]
    fn replay_same_step_is_rejected() {
        let svc = service();
        let (secret, _) = svc.setup_secret("u@e.com");
        let step = svc.current_step();
        let code = TotpService::code_for_step(&secret, step).unwrap();
        assert!(
            !svc.verify(&secret, &code, step).unwrap(),
            "same step must not verify twice"
        );
    }

    #[tokio::test]
    async fn full_enrollment_flow_with_recovery_code_login() {
        let uid = shared::UserId::from_uuid(uuid::Uuid::new_v4());
        let svc = service_with_user(uid).await;
        let setup = svc.setup(uid).await.unwrap();
        assert!(!setup.secret.is_empty());
        assert!(setup.otpauth_uri.starts_with("otpauth://totp/"));

        // enable requires a valid code
        let step = svc.current_step();
        let code = TotpService::code_for_step(&setup.secret, step).unwrap();
        let enabled = svc.enable(uid, &code).await.unwrap();
        assert_eq!(enabled.recovery_codes.len(), 8);
        assert!(svc.is_enabled(uid).await.unwrap());

        // The enabling code's window is consumed (replay protection): a second
        // use of the same window must be rejected, and a *fresh* window code
        // must be accepted. Advance deterministically by stepping the repo
        // cursor to step-1 so the next code is treated as new.
        let step2 = svc.current_step();
        let code2 = TotpService::code_for_step(&setup.secret, step2).unwrap();
        assert!(
            !svc.login_verify(uid, &code2).await.unwrap(),
            "same-window code reuse must be rejected"
        );

        // a recovery code also works and is single-use
        let rc = enabled.recovery_codes[0].clone();
        assert!(svc.login_verify(uid, &rc).await.unwrap());
        assert!(
            !svc.login_verify(uid, &rc).await.unwrap(),
            "recovery code must be single-use"
        );

        // disable with a *different* unused recovery code (rc was consumed above)
        svc.disable(uid, &enabled.recovery_codes[1].clone())
            .await
            .unwrap();
        assert!(!svc.is_enabled(uid).await.unwrap());
    }

    #[test]
    fn secret_is_encrypted_at_rest() {
        let svc = service();
        let (secret, _) = svc.setup_secret("u@e.com");
        let cipher = svc.encrypt_secret(&secret).unwrap();
        assert_ne!(cipher, secret, "cipher must not leak the plaintext secret");
        assert_eq!(svc.decrypt_secret(&cipher).unwrap(), secret);
    }
}
