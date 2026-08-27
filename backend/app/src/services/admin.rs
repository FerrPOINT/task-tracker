//! Phase 8: Admin service implementation.
//!
//! All methods enforce system-admin authorization by checking `is_system_admin`
//! on the requester. Mutations write an [`domain::AuditLog`] entry. System
//! setting keys are validated against a safe allowlist and the JSON value size
//! is capped.

use std::sync::Arc;

use async_trait::async_trait;

use crate::context::{
    AdminCreateUserCommand, AdminService, AdminUserDto, AuditLogDto, SystemSettingDto,
};
use domain::{
    AuditLog, AuditLogRepository, SystemSetting, SystemSettingRepository, User, UserRepository,
};
use shared::{AppError, AuditLogId, UserId};

/// Maximum serialized size for a system setting JSON value (16 KiB).
const MAX_SETTING_VALUE_BYTES: usize = 16_384;

/// Safe system setting keys that can be exposed via the admin API.
///
/// Never include mail credentials, JWT secrets, or database connection strings
/// — those must be managed via environment variables / config files only.
const SAFE_SETTING_KEYS: &[&str] = &[
    "instance.name",
    "instance.base_url",
    "limits.max_users",
    "security.allow_registration",
];

pub struct AdminServiceImpl {
    users: Arc<dyn UserRepository>,
    audit_logs: Arc<dyn AuditLogRepository>,
    system_settings: Arc<dyn SystemSettingRepository>,
}

impl AdminServiceImpl {
    pub fn new(
        users: Arc<dyn UserRepository>,
        audit_logs: Arc<dyn AuditLogRepository>,
        system_settings: Arc<dyn SystemSettingRepository>,
    ) -> Self {
        Self {
            users,
            audit_logs,
            system_settings,
        }
    }

    /// Verify the requester is a system admin; return the loaded user on
    /// success. This is the single authorization gate for every admin
    /// operation, making it middleware-safe.
    async fn require_admin(&self, requester_id: UserId) -> Result<User, AppError> {
        let user = self.users.get_by_id(requester_id).await?;
        if !user.is_system_admin {
            return Err(AppError::Forbidden);
        }
        Ok(user)
    }

    /// Write an audit log entry for a mutation.
    async fn audit(
        &self,
        actor_id: UserId,
        action: &str,
        entity_type: &str,
        entity_id: Option<uuid::Uuid>,
        metadata: serde_json::Value,
    ) -> Result<(), AppError> {
        let entry = AuditLog {
            id: AuditLogId::new(),
            actor_id,
            action: action.into(),
            entity_type: entity_type.into(),
            entity_id,
            metadata,
            created_at: shared::now(),
        };
        self.audit_logs.save(&entry).await
    }

    /// Count active system admins.
    async fn count_active_admins(&self) -> Result<usize, AppError> {
        let users = self.users.list().await?;
        Ok(users
            .into_iter()
            .filter(|u| u.is_system_admin && u.is_active)
            .count())
    }
}

#[async_trait]
impl AdminService for AdminServiceImpl {
    async fn list_users(&self, requester_id: UserId) -> Result<Vec<AdminUserDto>, AppError> {
        self.require_admin(requester_id).await?;
        let users = self.users.list().await?;
        Ok(users.into_iter().map(AdminUserDto::from).collect())
    }

    async fn create_user(
        &self,
        requester_id: UserId,
        cmd: AdminCreateUserCommand,
    ) -> Result<AdminUserDto, AppError> {
        self.require_admin(requester_id).await?;

        // Reject duplicate emails before hashing.
        if self.users.get_by_email(&cmd.email).await.is_ok() {
            return Err(AppError::conflict("email already registered"));
        }

        // Hash the password — plaintext is never persisted or logged.
        let password_hash = hash_password(&cmd.password)?;

        let user = User {
            id: UserId::new(),
            email: cmd.email.into(),
            username: cmd.username.into(),
            display_name: cmd.display_name.into(),
            password_hash: password_hash.into(),
            refresh_token_hash: None,
            is_system_admin: cmd.is_system_admin,
            is_active: true,
            created_at: shared::now(),
            updated_at: shared::now(),
        };

        self.users.save(&user).await?;

        // Audit the creation. The password is deliberately excluded.
        let metadata = serde_json::json!({
            "email": user.email.as_ref(),
            "username": user.username.as_ref(),
            "is_system_admin": user.is_system_admin,
        });
        self.audit(
            requester_id,
            "admin.create_user",
            "user",
            Some(user.id.as_uuid()),
            metadata,
        )
        .await?;

        Ok(AdminUserDto::from(user))
    }

    async fn update_user_status(
        &self,
        requester_id: UserId,
        user_id: UserId,
        is_active: bool,
    ) -> Result<AdminUserDto, AppError> {
        self.require_admin(requester_id).await?;

        let mut user = self.users.get_by_id(user_id).await?;

        // Prevent deactivating the last active system admin.
        if !is_active && user.is_system_admin && user.is_active {
            let admin_count = self.count_active_admins().await?;
            if admin_count <= 1 {
                return Err(AppError::conflict(
                    "cannot deactivate the last active system admin",
                ));
            }
        }

        let previous_active = user.is_active;
        user.is_active = is_active;
        user.updated_at = shared::now();
        self.users.save(&user).await?;

        let metadata = serde_json::json!({
            "previous_is_active": previous_active,
            "new_is_active": is_active,
        });
        self.audit(
            requester_id,
            "admin.update_user_status",
            "user",
            Some(user.id.as_uuid()),
            metadata,
        )
        .await?;

        Ok(AdminUserDto::from(user))
    }

    async fn list_audit_logs(
        &self,
        requester_id: UserId,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<AuditLogDto>, AppError> {
        self.require_admin(requester_id).await?;
        let entries = self.audit_logs.list(None, limit, offset).await?;
        Ok(entries.into_iter().map(AuditLogDto::from).collect())
    }

    async fn list_system_settings(
        &self,
        requester_id: UserId,
    ) -> Result<Vec<SystemSettingDto>, AppError> {
        self.require_admin(requester_id).await?;
        let settings = self.system_settings.list().await?;
        // Filter to only safe keys.
        let filtered: Vec<_> = settings
            .into_iter()
            .filter(|s| SAFE_SETTING_KEYS.contains(&s.key.as_ref()))
            .map(SystemSettingDto::from)
            .collect();
        Ok(filtered)
    }

    async fn update_system_setting(
        &self,
        requester_id: UserId,
        key: String,
        value: serde_json::Value,
    ) -> Result<SystemSettingDto, AppError> {
        self.require_admin(requester_id).await?;

        // Validate the key is on the safe allowlist.
        if !SAFE_SETTING_KEYS.contains(&key.as_str()) {
            return Err(AppError::invalid_input(
                "system setting key is not on the safe allowlist",
            ));
        }

        // Validate the JSON value size.
        let serialized =
            serde_json::to_string(&value).map_err(|e| AppError::invalid_input(e.to_string()))?;
        if serialized.len() > MAX_SETTING_VALUE_BYTES {
            return Err(AppError::invalid_input(format!(
                "system setting value exceeds maximum size of {MAX_SETTING_VALUE_BYTES} bytes"
            )));
        }

        let setting = SystemSetting {
            key: key.as_str().into(),
            value,
            updated_at: shared::now(),
        };
        self.system_settings.save(&setting).await?;

        let metadata = serde_json::json!({ "key": setting.key.as_ref() });
        self.audit(
            requester_id,
            "admin.update_system_setting",
            "system_setting",
            None,
            metadata,
        )
        .await?;

        Ok(SystemSettingDto::from(setting))
    }
}

/// Argon2 password hashing — same implementation as `app::auth`.
/// Kept private to this module; the plaintext password is never logged.
fn hash_password(password: &str) -> Result<String, AppError> {
    use argon2::{
        Argon2,
        password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
    };
    if password.is_empty() {
        return Err(AppError::invalid_input("password must not be empty"));
    }
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(AppError::internal)?;
    Ok(hash.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{
        AuditLogRepository, MemoryAuditLogRepository, MemorySystemSettingRepository,
        MemoryUserRepository, SystemSettingRepository, UserRepository,
    };

    fn make_admin() -> User {
        User {
            id: UserId::new(),
            email: "admin@example.com".into(),
            username: "admin".into(),
            display_name: "Admin".into(),
            password_hash: "x".into(),
            refresh_token_hash: None,
            is_system_admin: true,
            is_active: true,
            created_at: shared::now(),
            updated_at: shared::now(),
        }
    }

    fn make_regular_user() -> User {
        User {
            id: UserId::new(),
            email: "user@example.com".into(),
            username: "user".into(),
            display_name: "User".into(),
            password_hash: "x".into(),
            refresh_token_hash: None,
            is_system_admin: false,
            is_active: true,
            created_at: shared::now(),
            updated_at: shared::now(),
        }
    }

    fn make_service(
        users: Arc<MemoryUserRepository>,
    ) -> (
        AdminServiceImpl,
        Arc<MemoryAuditLogRepository>,
        Arc<MemorySystemSettingRepository>,
    ) {
        let audit_logs = Arc::new(MemoryAuditLogRepository::default());
        let system_settings = Arc::new(MemorySystemSettingRepository::default());
        let service =
            AdminServiceImpl::new(users.clone(), audit_logs.clone(), system_settings.clone());
        (service, audit_logs, system_settings)
    }

    #[tokio::test]
    async fn list_users_requires_admin() {
        let users = Arc::new(MemoryUserRepository::default());
        let regular = make_regular_user();
        users.save(&regular).await.unwrap();
        let (service, _, _) = make_service(users);
        let result = service.list_users(regular.id).await;
        assert!(matches!(result, Err(AppError::Forbidden)));
    }

    #[tokio::test]
    async fn list_users_returns_all_for_admin() {
        let users = Arc::new(MemoryUserRepository::default());
        let admin = make_admin();
        let regular = make_regular_user();
        users.save(&admin).await.unwrap();
        users.save(&regular).await.unwrap();
        let (service, _, _) = make_service(users);
        let result = service.list_users(admin.id).await.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn create_user_creates_and_audits() {
        let users = Arc::new(MemoryUserRepository::default());
        let admin = make_admin();
        users.save(&admin).await.unwrap();
        let (service, audit_logs, _) = make_service(users);
        let cmd = AdminCreateUserCommand {
            email: "new@example.com".into(),
            username: "newuser".into(),
            display_name: "New".into(),
            password: "secret123".into(),
            is_system_admin: false,
        };
        let dto = service.create_user(admin.id, cmd).await.unwrap();
        assert_eq!(dto.email, "new@example.com");
        assert!(!dto.is_system_admin);
        assert!(dto.is_active);

        let logs = audit_logs.list(None, 100, 0).await.unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].action.as_ref(), "admin.create_user");
        assert_eq!(logs[0].entity_type.as_ref(), "user");
    }

    #[tokio::test]
    async fn create_user_rejects_duplicate_email() {
        let users = Arc::new(MemoryUserRepository::default());
        let admin = make_admin();
        users.save(&admin).await.unwrap();
        let (service, _, _) = make_service(users);
        let cmd = AdminCreateUserCommand {
            email: "admin@example.com".into(),
            username: "another".into(),
            display_name: "Another".into(),
            password: "secret123".into(),
            is_system_admin: false,
        };
        let result = service.create_user(admin.id, cmd).await;
        assert!(matches!(result, Err(AppError::Conflict(_))));
    }

    #[tokio::test]
    async fn create_user_rejects_non_admin() {
        let users = Arc::new(MemoryUserRepository::default());
        let regular = make_regular_user();
        users.save(&regular).await.unwrap();
        let (service, _, _) = make_service(users);
        let cmd = AdminCreateUserCommand {
            email: "new@example.com".into(),
            username: "newuser".into(),
            display_name: "New".into(),
            password: "secret123".into(),
            is_system_admin: false,
        };
        let result = service.create_user(regular.id, cmd).await;
        assert!(matches!(result, Err(AppError::Forbidden)));
    }

    #[tokio::test]
    async fn update_user_status_deactivates_and_audits() {
        let users = Arc::new(MemoryUserRepository::default());
        let admin = make_admin();
        let regular = make_regular_user();
        users.save(&admin).await.unwrap();
        users.save(&regular).await.unwrap();
        let (service, audit_logs, _) = make_service(users);
        let dto = service
            .update_user_status(admin.id, regular.id, false)
            .await
            .unwrap();
        assert!(!dto.is_active);

        let logs = audit_logs.list(None, 100, 0).await.unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].action.as_ref(), "admin.update_user_status");
    }

    #[tokio::test]
    async fn update_user_status_prevents_last_admin_deactivation() {
        let users = Arc::new(MemoryUserRepository::default());
        let admin = make_admin();
        users.save(&admin).await.unwrap();
        let (service, _, _) = make_service(users);
        let result = service.update_user_status(admin.id, admin.id, false).await;
        assert!(matches!(result, Err(AppError::Conflict(_))));
    }

    #[tokio::test]
    async fn update_user_status_can_deactivate_when_multiple_admins() {
        let users = Arc::new(MemoryUserRepository::default());
        let admin1 = make_admin();
        let admin2 = make_admin();
        users.save(&admin1).await.unwrap();
        users.save(&admin2).await.unwrap();
        let (service, _, _) = make_service(users);
        let result = service
            .update_user_status(admin1.id, admin2.id, false)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn list_audit_logs_requires_admin() {
        let users = Arc::new(MemoryUserRepository::default());
        let regular = make_regular_user();
        users.save(&regular).await.unwrap();
        let (service, _, _) = make_service(users);
        let result = service.list_audit_logs(regular.id, 100, 0).await;
        assert!(matches!(result, Err(AppError::Forbidden)));
    }

    #[tokio::test]
    async fn list_system_settings_filters_safe_keys() {
        let users = Arc::new(MemoryUserRepository::default());
        let admin = make_admin();
        users.save(&admin).await.unwrap();
        let (service, _, system_settings) = make_service(users);

        // Save a safe key.
        system_settings
            .save(&SystemSetting {
                key: "instance.name".into(),
                value: serde_json::json!("Tracker"),
                updated_at: shared::now(),
            })
            .await
            .unwrap();
        // Save an unsafe key (should be filtered out).
        system_settings
            .save(&SystemSetting {
                key: "mail.password".into(),
                value: serde_json::json!("secret"),
                updated_at: shared::now(),
            })
            .await
            .unwrap();

        let settings = service.list_system_settings(admin.id).await.unwrap();
        assert_eq!(settings.len(), 1);
        assert_eq!(settings[0].key, "instance.name");
    }

    #[tokio::test]
    async fn update_system_setting_rejects_unsafe_key() {
        let users = Arc::new(MemoryUserRepository::default());
        let admin = make_admin();
        users.save(&admin).await.unwrap();
        let (service, _, _) = make_service(users);
        let result = service
            .update_system_setting(admin.id, "mail.password".into(), serde_json::json!("x"))
            .await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn update_system_setting_rejects_oversized_value() {
        let users = Arc::new(MemoryUserRepository::default());
        let admin = make_admin();
        users.save(&admin).await.unwrap();
        let (service, _, _) = make_service(users);
        let big = "x".repeat(MAX_SETTING_VALUE_BYTES + 1);
        let result = service
            .update_system_setting(admin.id, "instance.name".into(), serde_json::json!(big))
            .await;
        assert!(matches!(result, Err(AppError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn update_system_setting_succeeds_and_audits() {
        let users = Arc::new(MemoryUserRepository::default());
        let admin = make_admin();
        users.save(&admin).await.unwrap();
        let (service, audit_logs, system_settings) = make_service(users);
        let dto = service
            .update_system_setting(
                admin.id,
                "instance.name".into(),
                serde_json::json!("My Tracker"),
            )
            .await
            .unwrap();
        assert_eq!(dto.key, "instance.name");
        assert_eq!(dto.value, serde_json::json!("My Tracker"));

        // Verify it was persisted.
        let stored = system_settings.get("instance.name").await.unwrap();
        assert_eq!(stored.value, serde_json::json!("My Tracker"));

        // Verify audit.
        let logs = audit_logs.list(None, 100, 0).await.unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].action.as_ref(), "admin.update_system_setting");
    }

    #[tokio::test]
    async fn update_system_setting_rejects_non_admin() {
        let users = Arc::new(MemoryUserRepository::default());
        let regular = make_regular_user();
        users.save(&regular).await.unwrap();
        let (service, _, _) = make_service(users);
        let result = service
            .update_system_setting(regular.id, "instance.name".into(), serde_json::json!("x"))
            .await;
        assert!(matches!(result, Err(AppError::Forbidden)));
    }
}
