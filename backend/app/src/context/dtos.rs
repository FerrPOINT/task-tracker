#[derive(Debug, Clone, serde::Serialize)]
pub struct AttachmentDto {
    pub id: String,
    pub issue_id: String,
    pub author_id: String,
    pub file_name: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LabelDto {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NotificationDto {
    pub id: String,
    pub event_type: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub actor_id: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub is_read: bool,
    pub action_url: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NotificationListDto {
    pub notifications: Vec<NotificationDto>,
    pub unread_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NotificationSettingsDto {
    pub email_frequency: String,
    pub disabled_event_types: Vec<String>,
    pub notify_own_changes: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IssueLinkDto {
    pub id: String,
    pub source_id: String,
    pub source_key: String,
    pub target_id: String,
    pub target_key: String,
    pub link_type: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VelocitySprintDto {
    pub name: String,
    pub committed: usize,
    pub completed: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BurndownDto {
    pub sprint_name: String,
    pub points: Vec<BurndownPointDto>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BurndownPointDto {
    pub date: String,
    pub remaining: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CumulativeFlowPointDto {
    pub date: String,
    pub todo: usize,
    pub in_progress: usize,
    pub done: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ControlChartPointDto {
    pub issue_key: String,
    pub cycle_time_days: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WatcherDto {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VoteDto {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub voted_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CustomFieldDto {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub field_type: String,
    pub options: Vec<String>,
    pub is_required: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CustomFieldValueDto {
    pub field_id: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ComponentDto {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VersionDto {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: Option<String>,
    pub released: bool,
    pub release_date: Option<String>,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Phase 8: Admin service DTOs
// ---------------------------------------------------------------------------

/// Admin user DTO — includes `is_system_admin` and `is_active` flags that the
/// regular [`crate::dto::UserDto`] intentionally omits.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdminUserDto {
    pub id: String,
    pub email: String,
    pub username: String,
    pub display_name: String,
    pub is_system_admin: bool,
    pub is_active: bool,
}

impl From<domain::User> for AdminUserDto {
    fn from(user: domain::User) -> Self {
        Self {
            id: user.id.to_string(),
            email: user.email.as_ref().to_string(),
            username: user.username.as_ref().to_string(),
            display_name: user.display_name.as_ref().to_string(),
            is_system_admin: user.is_system_admin,
            is_active: user.is_active,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AuditLogDto {
    pub id: String,
    pub actor_id: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

impl From<domain::AuditLog> for AuditLogDto {
    fn from(entry: domain::AuditLog) -> Self {
        Self {
            id: entry.id.to_string(),
            actor_id: entry.actor_id.to_string(),
            action: entry.action.as_ref().to_string(),
            entity_type: entry.entity_type.as_ref().to_string(),
            entity_id: entry.entity_id.map(|id| id.to_string()),
            metadata: entry.metadata,
            created_at: entry.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemSettingDto {
    pub key: String,
    pub value: serde_json::Value,
    pub updated_at: String,
}

impl From<domain::SystemSetting> for SystemSettingDto {
    fn from(setting: domain::SystemSetting) -> Self {
        Self {
            key: setting.key.as_ref().to_string(),
            value: setting.value,
            updated_at: setting.updated_at.to_rfc3339(),
        }
    }
}

/// Command for creating a new user from the admin panel.
/// The password is hashed before storage and never persisted in plaintext.
#[derive(Debug, Clone)]
pub struct AdminCreateUserCommand {
    pub email: String,
    pub username: String,
    pub display_name: String,
    pub password: String,
    pub is_system_admin: bool,
}
