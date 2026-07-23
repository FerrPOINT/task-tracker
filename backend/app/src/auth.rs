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
            created_at: shared::now(),
            updated_at: shared::now(),
        };

        let id = self.users.save(&user).await?;
        let user = self.users.get_by_id(id).await?;
        let token = create_token(&self.config, user.id)?;

        Ok(AuthDto {
            token,
            user: UserDto::from(user),
        })
    }

    async fn login(&self, cmd: LoginCommand) -> Result<AuthDto, AppError> {
        let user = self.users.get_by_email(&cmd.email).await?;
        if !verify_password(&cmd.password, &user.password_hash)? {
            return Err(AppError::Unauthorized);
        }

        let token = create_token(&self.config, user.id)?;

        Ok(AuthDto {
            token,
            user: UserDto::from(user),
        })
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

fn hash_password(password: &str) -> Result<String, AppError> {
    use argon2::{
        Argon2,
        password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
    };
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::internal(e))?;
    Ok(hash.to_string())
}

fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    use argon2::{
        Argon2,
        password_hash::{PasswordHash, PasswordVerifier},
    };
    let parsed = PasswordHash::new(hash).map_err(|e| AppError::internal(e))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

fn create_token(config: &AuthConfig, user_id: UserId) -> Result<String, AppError> {
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
    .map_err(|e| AppError::internal(e))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserClaims {
    pub sub: String,
    pub exp: usize,
}
