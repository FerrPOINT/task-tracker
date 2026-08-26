use async_trait::async_trait;
use chrono::{Duration, Utc};
use domain::User;
use jsonwebtoken::{EncodingKey, Header};
use shared::{AppError, AuthConfig, UserId};
use std::sync::Arc;

use crate::commands::{LoginCommand, RegisterCommand};
use crate::dto::{AuthDto, UserDto};

pub struct JwtAuthService {
    config: AuthConfig,
    users: Arc<dyn domain::UserRepository>,
}

impl JwtAuthService {
    pub fn new(config: AuthConfig, users: Arc<dyn domain::UserRepository>) -> Self {
        Self { config, users }
    }
}

#[async_trait]
impl crate::context::AuthService for JwtAuthService {
    async fn register(&self, cmd: RegisterCommand) -> Result<AuthDto, AppError> {
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
        let claims = self.verify_token(refresh_token)?;
        let user_id = claims
            .sub
            .parse::<UserId>()
            .map_err(|_| AppError::invalid_input("invalid user id"))?;
        let user = self.users.get_by_id(user_id).await?;
        let token_hash = hash_refresh_token(refresh_token);
        if user.refresh_token_hash.as_deref() != Some(&token_hash) {
            return Err(AppError::Unauthorized);
        }
        self.issue_tokens(user).await
    }

    async fn logout(&self, user_id: UserId) -> Result<(), AppError> {
        let mut user = self.users.get_by_id(user_id).await?;
        user.refresh_token_hash = None;
        self.users.save(&user).await.map(|_| ())
    }

    async fn me(&self, user_id: UserId) -> Result<crate::dto::UserDto, AppError> {
        let user = self.users.get_by_id(user_id).await?;
        Ok(crate::dto::UserDto::from(user))
    }

    async fn list_users(&self) -> Result<Vec<crate::dto::UserDto>, AppError> {
        let users = self.users.list().await?;
        Ok(users.into_iter().map(crate::dto::UserDto::from).collect())
    }

    fn verify_token(&self, token: &str) -> Result<UserClaims, AppError> {
        let key = self.config.jwt_secret.as_bytes();
        let token = jsonwebtoken::decode::<UserClaims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(key),
            &jsonwebtoken::Validation::default(),
        )
        .map_err(|_| AppError::Unauthorized)?;
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
    };
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(AppError::internal)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserClaims {
    pub sub: String,
    pub exp: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::AuthService;
    use domain::{User, UserRepository};
    use shared::{UserId, now};

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
        let service = JwtAuthService::new(config, repo);
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
        let service = JwtAuthService::new(config, repo);
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
        let service = JwtAuthService::new(config, repo);
        let result = service
            .login(LoginCommand {
                email: "missing@example.com".to_string(),
                password: "12345678".to_string(),
            })
            .await;
        assert!(result.is_err());
    }
}
