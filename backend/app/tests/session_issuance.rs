//! Public local-login boundary: a concurrent identity update must survive token issuance.
use app::{auth::JwtAuthService, commands::LoginCommand, context::AuthService};
use domain::{MemoryUserRepository, User, UserRepository};
use shared::{AppError, UserId};
use std::sync::{Arc, Mutex};

struct Users {
    inner: MemoryUserRepository,
    replacement_after_read: Mutex<Option<User>>,
}
#[async_trait::async_trait]
impl UserRepository for Users {
    async fn get_by_ids(&self, ids: &[UserId]) -> Result<Vec<User>, AppError> {
        self.inner.get_by_ids(ids).await
    }
    async fn get_by_id(&self, id: UserId) -> Result<User, AppError> {
        self.inner.get_by_id(id).await
    }
    async fn get_by_email(&self, email: &str) -> Result<User, AppError> {
        let snapshot = self.inner.get_by_email(email).await?;
        let replacement = self.replacement_after_read.lock().unwrap().take();
        if let Some(current) = replacement {
            self.inner.save(&current).await?;
        }
        Ok(snapshot)
    }
    async fn get_by_refresh_token(&self, hash: &str) -> Result<User, AppError> {
        self.inner.get_by_refresh_token(hash).await
    }
    async fn set_refresh_token_if_password_matches(
        &self,
        id: UserId,
        password_hash: &str,
        new_hash: &str,
    ) -> Result<User, AppError> {
        self.inner
            .set_refresh_token_if_password_matches(id, password_hash, new_hash)
            .await
    }
    async fn rotate_refresh_token(&self, id: UserId, old: &str, new: &str) -> Result<(), AppError> {
        self.inner.rotate_refresh_token(id, old, new).await
    }
    async fn clear_refresh_token(&self, id: UserId) -> Result<(), AppError> {
        self.inner.clear_refresh_token(id).await
    }
    async fn save(&self, user: &User) -> Result<UserId, AppError> {
        self.inner.save(user).await
    }
    async fn list(&self) -> Result<Vec<User>, AppError> {
        self.inner.list().await
    }
}

#[tokio::test]
async fn local_login_preserves_concurrent_identity_changes() {
    assert!(std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_none());
    let password = "Local-session-fixture-42!";
    let original_hash = pdlc_auth_core::password::hash_password(password).unwrap();
    let replacement_hash =
        pdlc_auth_core::password::hash_password("Replacement-fixture-43!").unwrap();
    for change in ["disabled", "password", "profile", "unchanged"] {
        let now = shared::now();
        let snapshot = User {
            id: UserId::new(),
            email: "session@example.test".into(),
            username: "session".into(),
            display_name: "Original profile".into(),
            password_hash: original_hash.clone().into(),
            refresh_token_hash: Some("previous-refresh".into()),
            is_system_admin: true,
            is_active: true,
            created_at: now,
            updated_at: now,
        };
        let mut current = snapshot.clone();
        if change != "unchanged" {
            current.display_name = "Current profile".into();
            current.is_system_admin = false;
        }
        match change {
            "disabled" => current.is_active = false,
            "password" => current.password_hash = replacement_hash.clone().into(),
            _ => {}
        }
        let users = Arc::new(Users {
            inner: MemoryUserRepository::default(),
            replacement_after_read: Mutex::new((change != "unchanged").then_some(current.clone())),
        });
        users.save(&snapshot).await.unwrap();
        let mut config = shared::AppConfig::default().auth;
        config.jwt_secret = "local-session-fixture-signing-secret".into();
        let service = JwtAuthService::new(
            config,
            users.clone(),
            Arc::new(domain::stubs::memory::MemorySystemSettingRepository::default()),
            Arc::new(domain::stubs::memory::MemoryPasswordResetRepository::default()),
            Arc::new(domain::StubEmailPort),
        );
        // The repository commits the competing identity update after the password
        // snapshot has been read, without sleeps or a different authentication path.
        let issued = service
            .login(LoginCommand {
                email: "session@example.test".into(),
                password: password.into(),
            })
            .await;
        let stored = users.get_by_id(current.id).await.unwrap();
        assert_eq!(stored.is_active, current.is_active, "{change}");
        assert_eq!(stored.password_hash, current.password_hash, "{change}");
        assert_eq!(stored.is_system_admin, current.is_system_admin, "{change}");
        assert_eq!(stored.display_name, current.display_name, "{change}");
        if matches!(change, "disabled" | "password") {
            assert!(matches!(issued, Err(AppError::Unauthorized)), "{change}");
            assert_eq!(stored.refresh_token_hash, current.refresh_token_hash);
        } else {
            let issued = issued.unwrap();
            assert_eq!(issued.user.is_system_admin, current.is_system_admin);
            assert_eq!(issued.user.display_name, current.display_name.as_ref());
            assert_ne!(stored.refresh_token_hash, current.refresh_token_hash);
        }
    }
}

#[tokio::test]
async fn password_verified_before_mfa_cannot_be_rebound_to_a_reset_credential() {
    let users = Arc::new(MemoryUserRepository::default());
    let now = shared::now();
    let user = User {
        id: UserId::new(),
        email: "mfa-session@example.test".into(),
        username: "mfa-session".into(),
        display_name: "MFA session".into(),
        password_hash: pdlc_auth_core::password::hash_password("Verified-fixture-42!")
            .unwrap()
            .into(),
        refresh_token_hash: Some("existing-session".into()),
        is_system_admin: false,
        is_active: true,
        created_at: now,
        updated_at: now,
    };
    users.save(&user).await.unwrap();
    let mut config = shared::AppConfig::default().auth;
    config.jwt_secret = "mfa-session-fixture-signing-secret".into();
    let service = JwtAuthService::new(
        config,
        users.clone(),
        Arc::new(domain::stubs::memory::MemorySystemSettingRepository::default()),
        Arc::new(domain::stubs::memory::MemoryPasswordResetRepository::default()),
        Arc::new(domain::StubEmailPort),
    );
    let proof = service
        .authenticate_password(LoginCommand {
            email: user.email.to_string(),
            password: "Verified-fixture-42!".into(),
        })
        .await
        .unwrap();
    let mut reset = user.clone();
    reset.password_hash = pdlc_auth_core::password::hash_password("Reset-fixture-43!")
        .unwrap()
        .into();
    users.save(&reset).await.unwrap();
    assert!(matches!(
        service.complete_password_login(proof).await,
        Err(AppError::Unauthorized)
    ));
    let stored = users.get_by_id(user.id).await.unwrap();
    assert_eq!(stored.password_hash, reset.password_hash);
    assert_eq!(stored.refresh_token_hash, user.refresh_token_hash);
    // The external-identity path remains separate and uses the current identity.
    assert!(service.issue_session(user.id).await.is_ok());
}
