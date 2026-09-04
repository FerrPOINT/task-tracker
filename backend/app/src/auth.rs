use async_trait::async_trait;
use chrono::{Duration, Utc};
use domain::User;
use jsonwebtoken::{EncodingKey, Header};
use shared::{AppError, AuthConfig, UserId};
use std::sync::Arc;

use crate::commands::{LoginCommand, RegisterCommand};
use crate::dto::{AuthDto, UserDto};

/// System setting that gates the public register endpoint. Absent setting
/// means open registration (backwards-compatible default).
const REGISTRATION_SETTING_KEY: &str = "security.allow_registration";
/// Minimum password length enforced on self-service registration and admin
/// user creation.
pub(crate) const MIN_PASSWORD_LEN: usize = 8;

pub(crate) fn ensure_password_policy(password: &str) -> Result<(), AppError> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(AppError::invalid_input(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }
    if password.chars().count() > 128 {
        return Err(AppError::invalid_input(
            "password must not exceed 128 characters",
        ));
    }
    Ok(())
}

pub struct JwtAuthService {
    config: AuthConfig,
    users: Arc<dyn domain::UserRepository>,
    system_settings: Arc<dyn domain::SystemSettingRepository>,
}

#[path = "central_login.rs"]
mod central_login;
use central_login::try_central_login;

impl JwtAuthService {
    /// Finds a local user by the central identity's email; links (creates) a
    /// shadow account on first login. Central users never have a usable local
    /// password ("!" hash — local verify always fails).
    async fn find_or_link_central_user(&self, email: &str) -> Result<User, AppError> {
        let email = email.trim().to_lowercase();
        if let Ok(existing) = self.users.get_by_email(&email).await {
            if !existing.is_active {
                return Err(AppError::Unauthorized);
            }
            return Ok(existing);
        }
        let username = email.split('@').next().unwrap_or("central").to_string();
        let user = User {
            id: UserId::new(),
            email: email.into(),
            username: username.clone().into(),
            display_name: username.into(),
            password_hash: "!".into(),
            refresh_token_hash: None,
            is_system_admin: false,
            is_active: true,
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        let id = self.users.save(&user).await?;
        self.users.get_by_id(id).await
    }
}

impl JwtAuthService {
    pub fn new(
        config: AuthConfig,
        users: Arc<dyn domain::UserRepository>,
        system_settings: Arc<dyn domain::SystemSettingRepository>,
    ) -> Self {
        Self {
            config,
            users,
            system_settings,
        }
    }

    async fn registration_allowed(&self) -> Result<bool, AppError> {
        match self.system_settings.get(REGISTRATION_SETTING_KEY).await {
            Ok(setting) => Ok(!matches!(setting.value, serde_json::Value::Bool(false))),
            // No stored setting → open registration (default).
            Err(AppError::NotFound(_)) => Ok(true),
            Err(e) => Err(e),
        }
    }
}

#[async_trait]
impl crate::context::AuthService for JwtAuthService {
    async fn register(&self, cmd: RegisterCommand) -> Result<AuthDto, AppError> {
        if !self.registration_allowed().await? {
            return Err(AppError::Forbidden);
        }
        ensure_password_policy(&cmd.password)?;
        let existing = self.users.get_by_email(&cmd.email).await;
        if existing.is_ok() {
            return Err(AppError::conflict("email already registered"));
        }

        let password_hash = hash_password(&cmd.password)?;
        let user = User {
            id: UserId::new(),
            email: cmd.email.into(),
            username: cmd.username.into(),
            display_name: cmd.name.into(),
            password_hash: password_hash.into(),
            refresh_token_hash: None,
            is_system_admin: false,
            is_active: true,
            created_at: shared::now(),
            updated_at: shared::now(),
        };

        let id = self.users.save(&user).await?;
        let user = self.users.get_by_id(id).await?;
        self.issue_tokens(user).await
    }

    async fn login(&self, cmd: LoginCommand) -> Result<AuthDto, AppError> {
        // Central fleet auth first; local password login remains the fallback
        // during the migration window (see central_login module).
        if let Some(pair) = try_central_login(&cmd.email, &cmd.password).await {
            // Link the shadow user by verified email so /me and
            // role checks keep working for central identities.
            let user = self.find_or_link_central_user(&cmd.email).await?;
            return Ok(AuthDto {
                access_token: pair.access_token,
                refresh_token: pair.refresh_token.unwrap_or_default(),
                expires_in: pair
                    .expires_in
                    .unwrap_or(self.config.access_token_ttl_minutes * 60),
                user: UserDto::from(user),
            });
        }
        let user = self.users.get_by_email(&cmd.email).await?;
        if !verify_password(&cmd.password, &user.password_hash)? {
            return Err(AppError::Unauthorized);
        }
        // Deactivated accounts must not receive new tokens.
        if !user.is_active {
            return Err(AppError::Unauthorized);
        }

        self.issue_tokens(user).await
    }

    async fn refresh(&self, refresh_token: &str) -> Result<AuthDto, AppError> {
        let key = self.config.jwt_secret.as_bytes();
        let decoded = jsonwebtoken::decode::<UserClaims>(
            refresh_token,
            &jsonwebtoken::DecodingKey::from_secret(key),
            &jsonwebtoken::Validation::default(),
        )
        .map_err(|_| AppError::Unauthorized)?;
        // Only refresh-type tokens may enter the rotation flow.
        if decoded.claims.typ.as_deref() != Some(TOKEN_TYPE_REFRESH) {
            return Err(AppError::Unauthorized);
        }
        let claims = decoded.claims;
        let user_id = claims
            .sub
            .parse::<UserId>()
            .map_err(|_| AppError::invalid_input("invalid user id"))?;
        let user = self.users.get_by_id(user_id).await?;
        if !user.is_active {
            return Err(AppError::Unauthorized);
        }
        let current_hash = hash_refresh_token(refresh_token);
        // Mint the replacement first, then rotate the stored hash with a
        // compare-and-swap: a replayed token no longer matches the stored
        // hash and is rejected atomically (no read-before-write race).
        let access = create_access_token(&self.config, user.id)?;
        let next_refresh = create_refresh_token(&self.config, user.id)?;
        let next_hash = hash_refresh_token(&next_refresh);
        self.users
            .rotate_refresh_token(user_id, &current_hash, &next_hash)
            .await?;
        let expires_in = self.config.access_token_ttl_minutes * 60;
        Ok(AuthDto {
            access_token: access,
            refresh_token: next_refresh,
            user: crate::dto::UserDto::from(user),
            expires_in,
        })
    }

    async fn logout(&self, user_id: UserId) -> Result<(), AppError> {
        // Atomic single UPDATE — cannot race with concurrent refresh rotation
        // (unlike the previous read-modify-write via save()).
        self.users.clear_refresh_token(user_id).await
    }

    async fn me(&self, user_id: UserId) -> Result<crate::dto::UserDto, AppError> {
        let user = self.users.get_by_id(user_id).await?;
        Ok(crate::dto::UserDto::from(user))
    }

    async fn list_active_users(&self) -> Result<Vec<crate::dto::UserDto>, AppError> {
        let users = self.users.list().await?;
        Ok(users
            .into_iter()
            .filter(|user| user.is_active)
            .map(crate::dto::UserDto::from)
            .collect())
    }

    fn verify_token(&self, token: &str) -> Result<UserClaims, AppError> {
        let key = self.config.jwt_secret.as_bytes();
        let token = jsonwebtoken::decode::<UserClaims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(key),
            &jsonwebtoken::Validation::default(),
        )
        .map_err(|_| AppError::Unauthorized)?;
        // Only access tokens may authenticate the protected API. Refresh
        // tokens (7-day TTL) must never be replayed as a Bearer credential.
        if token.claims.typ.as_deref() != Some(TOKEN_TYPE_ACCESS) {
            return Err(AppError::Unauthorized);
        }
        Ok(token.claims)
    }
}

impl JwtAuthService {
    async fn issue_tokens(&self, mut user: User) -> Result<AuthDto, AppError> {
        let access = create_access_token(&self.config, user.id)?;
        let refresh = create_refresh_token(&self.config, user.id)?;
        let token_hash = hash_refresh_token(&refresh);
        user.refresh_token_hash = Some(token_hash.into());
        self.users.save(&user).await?;
        let expires_in = self.config.access_token_ttl_minutes * 60;

        Ok(AuthDto {
            access_token: access,
            refresh_token: refresh,
            expires_in,
            user: UserDto::from(user),
        })
    }
}

fn hash_refresh_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn hash_password(password: &str) -> Result<String, AppError> {
    use argon2::{
        Argon2,
        password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
    };
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(AppError::internal)?;
    Ok(hash.to_string())
}

fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    use argon2::{
        Argon2,
        password_hash::{PasswordHash, PasswordVerifier},
    };
    let parsed = PasswordHash::new(hash).map_err(AppError::internal)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

fn create_access_token(config: &AuthConfig, user_id: UserId) -> Result<String, AppError> {
    let exp = Utc::now() + Duration::minutes(config.access_token_ttl_minutes as i64);
    let claims = UserClaims {
        sub: user_id.to_string(),
        exp: exp.timestamp() as usize,
        // Token type discrimination: the middleware only accepts `access`
        // tokens, so a leaked refresh token cannot be replayed as a Bearer.
        typ: Some(TOKEN_TYPE_ACCESS.to_string()),
        jti: None,
    };
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(AppError::internal)
}

fn create_refresh_token(config: &AuthConfig, user_id: UserId) -> Result<String, AppError> {
    let exp = Utc::now() + Duration::days(config.refresh_token_ttl_days as i64);
    let claims = UserClaims {
        sub: user_id.to_string(),
        exp: exp.timestamp() as usize,
        typ: Some(TOKEN_TYPE_REFRESH.to_string()),
        jti: Some(uuid::Uuid::now_v7().to_string()),
    };
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(AppError::internal)
}

pub const TOKEN_TYPE_ACCESS: &str = "access";
pub const TOKEN_TYPE_REFRESH: &str = "refresh";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserClaims {
    pub sub: String,
    pub exp: usize,
    /// Token type discriminator (`access` | `refresh`). Optional so that
    /// tokens issued before this field existed still parse, but only
    /// `access` tokens may authenticate the protected API.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typ: Option<String>,
    /// Unique token id; present on refresh tokens so that two rotations in
    /// the same second still mint distinct single-use tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn refresh_rotation_is_single_use() {
        use crate::context::AuthService as _;
        let cfg = shared::AuthConfig {
            jwt_secret: "test-secret".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
            refresh_cookie_name: "refresh_token".to_string(),
            refresh_cookie_secure: true,
            refresh_cookie_same_site: "Lax".to_string(),
            refresh_cookie_domain: None,
            refresh_cookie_path: "/api/v1/auth".to_string(),
        };
        let users = std::sync::Arc::new(domain::MemoryUserRepository::default());
        let user = domain::User {
            id: shared::UserId::from_uuid(uuid::Uuid::new_v4()),
            email: "u@e.com".into(),
            username: "u".into(),
            display_name: "U".into(),
            password_hash: "$argon2id$v=19$m=65536,t=3,p=4$stN/enhZ9yOvgWC9E8Y6BA$IL9I0WONb/I6zoT4rdmdkrPcIFADFxsLCjrO0ySSl0Y".into(),
            refresh_token_hash: None,
            is_system_admin: false,
            is_active: true,
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        users.save(&user).await.unwrap();
        let svc = super::JwtAuthService::new(cfg, users.clone(), empty_settings());
        let first = svc.refresh("not-a-token").await;
        assert!(first.is_err(), "invalid signature must be rejected");

        // Direct rotation path: seed a valid refresh token via issue_tokens,
        // then replay it after a successful rotation.
        let dto = svc.issue_tokens(user.clone()).await.unwrap();
        let ok = svc.refresh(&dto.refresh_token).await.unwrap();
        assert_ne!(ok.refresh_token, dto.refresh_token, "must rotate");
        let replay = svc.refresh(&dto.refresh_token).await;
        assert!(replay.is_err(), "replayed token must be rejected");
    }

    use super::*;
    use crate::context::AuthService;
    use domain::{User, UserRepository};
    use shared::{UserId, now};
    use std::sync::Arc as StdArc;

    fn empty_settings() -> StdArc<dyn domain::SystemSettingRepository> {
        StdArc::new(domain::stubs::memory::MemorySystemSettingRepository::default())
    }

    fn test_user() -> User {
        User {
            id: UserId::new(),
            email: "t@e.com".into(),
            username: "t".into(),
            display_name: "T".into(),
            password_hash: "$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$invalid".into(),
            refresh_token_hash: None,
            is_system_admin: false,
            is_active: true,
            created_at: now(),
            updated_at: now(),
        }
    }

    #[test]
    fn create_token_ok() {
        let config = AuthConfig {
            jwt_secret: "test-secret-32-chars-long!!!!!".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
            refresh_cookie_name: "refresh_token".to_string(),
            refresh_cookie_secure: true,
            refresh_cookie_same_site: "Lax".to_string(),
            refresh_cookie_domain: None,
            refresh_cookie_path: "/api/v1/auth".to_string(),
        };
        let token = create_access_token(&config, UserId::new()).unwrap();
        assert!(!token.is_empty());
    }

    #[test]
    fn verify_password_rejects_invalid_hash_format() {
        let result = verify_password("password", "not-a-valid-hash");
        assert!(result.is_err());
    }

    #[test]
    fn verify_password_rejects_wrong_password() {
        let password = "correct horse battery staple";
        let hash = hash_password(password).unwrap();
        let result = verify_password("wrong password", &hash).unwrap();
        assert!(!result);
    }

    #[test]
    fn verify_token_rejects_garbage() {
        let config = AuthConfig {
            jwt_secret: "test-secret-32-chars-long!!!!!".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
            refresh_cookie_name: "refresh_token".to_string(),
            refresh_cookie_secure: true,
            refresh_cookie_same_site: "Lax".to_string(),
            refresh_cookie_domain: None,
            refresh_cookie_path: "/api/v1/auth".to_string(),
        };
        let service = JwtAuthService::new(
            config,
            Arc::new(domain::stubs::memory::MemoryUserRepository::default()),
            empty_settings(),
        );
        assert!(service.verify_token("not.a.token").is_err());
    }

    #[tokio::test]
    async fn register_rejects_duplicate_email() {
        let repo = Arc::new(domain::stubs::memory::MemoryUserRepository::default());
        let user = test_user();
        let id = repo.save(&user).await.unwrap();
        let saved = repo.get_by_id(id).await.unwrap();

        let config = AuthConfig {
            jwt_secret: "test-secret-32-chars-long!!!!!".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
            refresh_cookie_name: "refresh_token".to_string(),
            refresh_cookie_secure: true,
            refresh_cookie_same_site: "Lax".to_string(),
            refresh_cookie_domain: None,
            refresh_cookie_path: "/api/v1/auth".to_string(),
        };
        let service = JwtAuthService::new(config, repo, empty_settings());
        let result = service
            .register(RegisterCommand {
                email: saved.email.to_string(),
                username: "other".to_string(),
                name: "Other".to_string(),
                password: "12345678".to_string(),
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn login_rejects_wrong_password() {
        let repo = Arc::new(domain::stubs::memory::MemoryUserRepository::default());
        let mut user = test_user();
        user.password_hash = hash_password("12345678").unwrap().into();
        repo.save(&user).await.unwrap();

        let config = AuthConfig {
            jwt_secret: "test-secret-32-chars-long!!!!!".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
            refresh_cookie_name: "refresh_token".to_string(),
            refresh_cookie_secure: true,
            refresh_cookie_same_site: "Lax".to_string(),
            refresh_cookie_domain: None,
            refresh_cookie_path: "/api/v1/auth".to_string(),
        };
        let service = JwtAuthService::new(config, repo, empty_settings());
        let result = service
            .login(LoginCommand {
                email: user.email.to_string(),
                password: "wrong".to_string(),
            })
            .await;
        assert!(matches!(result, Err(AppError::Unauthorized)));
    }

    #[tokio::test]
    async fn login_rejects_unknown_email() {
        let repo = Arc::new(domain::stubs::memory::MemoryUserRepository::default());
        let config = AuthConfig {
            jwt_secret: "test-secret-32-chars-long!!!!!".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
            refresh_cookie_name: "refresh_token".to_string(),
            refresh_cookie_secure: true,
            refresh_cookie_same_site: "Lax".to_string(),
            refresh_cookie_domain: None,
            refresh_cookie_path: "/api/v1/auth".to_string(),
        };
        let service = JwtAuthService::new(config, repo, empty_settings());
        let result = service
            .login(LoginCommand {
                email: "missing@example.com".to_string(),
                password: "12345678".to_string(),
            })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn register_rejects_short_password() {
        let repo = Arc::new(domain::stubs::memory::MemoryUserRepository::default());
        let config = test_auth_config();
        let service = JwtAuthService::new(config, repo, empty_settings());
        let result = service
            .register(RegisterCommand {
                email: "new@example.com".to_string(),
                username: "newuser".to_string(),
                name: "Test User".to_string(),
                password: "short".to_string(),
            })
            .await;
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("short password must be rejected"),
        };
        assert!(
            matches!(err, AppError::InvalidInput(ref msg) if msg.contains("password")),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn register_blocked_when_registration_disabled() {
        use domain::SystemSettingRepository;
        let repo = Arc::new(domain::stubs::memory::MemoryUserRepository::default());
        let settings = StdArc::new(domain::stubs::memory::MemorySystemSettingRepository::default());
        let setting = domain::SystemSetting {
            key: "security.allow_registration".into(),
            value: serde_json::json!(false),
            updated_at: shared::now(),
        };
        settings.save(&setting).await.unwrap();

        let config = test_auth_config();
        let service = JwtAuthService::new(config, repo, settings);
        let result = service
            .register(RegisterCommand {
                email: "new@example.com".to_string(),
                username: "newuser".to_string(),
                name: "Test User".to_string(),
                password: "12345678".to_string(),
            })
            .await;
        assert!(matches!(result, Err(AppError::Forbidden)), "{result:?}");
    }

    #[tokio::test]
    async fn register_allowed_by_default_without_setting() {
        let repo = Arc::new(domain::stubs::memory::MemoryUserRepository::default());
        let config = test_auth_config();
        let service = JwtAuthService::new(config, repo, empty_settings());
        let result = service
            .register(RegisterCommand {
                email: "fresh@example.com".to_string(),
                username: "fresh".to_string(),
                name: "Fresh User".to_string(),
                password: "12345678".to_string(),
            })
            .await;
        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test]
    async fn logout_clears_refresh_token_atomically() {
        use domain::UserRepository;
        let repo = Arc::new(domain::stubs::memory::MemoryUserRepository::default());
        let user = test_user();
        let id = repo.save(&user).await.unwrap();
        let dto = {
            let config = test_auth_config();
            let service = JwtAuthService::new(config, repo.clone(), empty_settings());
            service.issue_tokens(user).await.unwrap()
        };
        assert!(
            repo.get_by_id(id)
                .await
                .unwrap()
                .refresh_token_hash
                .is_some()
        );
        {
            let config = test_auth_config();
            let service = JwtAuthService::new(config, repo.clone(), empty_settings());
            service.logout(id).await.unwrap();
        }
        assert!(
            repo.get_by_id(id)
                .await
                .unwrap()
                .refresh_token_hash
                .is_none()
        );
        let _ = dto;
    }

    fn test_auth_config() -> AuthConfig {
        AuthConfig {
            jwt_secret: "test-secret-32-chars-long!!!!!".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
            refresh_cookie_name: "refresh_token".to_string(),
            refresh_cookie_secure: true,
            refresh_cookie_same_site: "Lax".to_string(),
            refresh_cookie_domain: None,
            refresh_cookie_path: "/api/v1/auth".to_string(),
        }
    }
}
