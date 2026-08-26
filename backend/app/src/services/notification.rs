use async_trait::async_trait;
use std::sync::Arc;

use shared::{AppError, UserId};

pub struct NotificationServiceImpl {
    notifications: Arc<dyn domain::NotificationRepository>,
    settings: Arc<dyn domain::UserNotificationSettingsRepository>,
}

impl NotificationServiceImpl {
    pub fn new(
        notifications: Arc<dyn domain::NotificationRepository>,
        settings: Arc<dyn domain::UserNotificationSettingsRepository>,
    ) -> Self {
        Self {
            notifications,
            settings,
        }
    }

    fn settings_dto(
        settings: domain::NotificationUserSettings,
    ) -> crate::context::NotificationSettingsDto {
        crate::context::NotificationSettingsDto {
            email_frequency: settings.email_frequency.to_string(),
            disabled_event_types: settings
                .disabled_event_types
                .into_iter()
                .map(|value| value.to_string())
                .collect(),
            notify_own_changes: settings.notify_own_changes,
        }
    }
}

#[async_trait]
impl crate::context::NotificationService for NotificationServiceImpl {
    async fn list_unread(
        &self,
        user_id: UserId,
    ) -> Result<crate::context::NotificationListDto, AppError> {
        let notifications = self.notifications.list_unread(user_id).await?;
        let unread_count = notifications.len();
        let mut notifications: Vec<_> = notifications
            .into_iter()
            .map(|notification| crate::context::NotificationDto {
                id: notification.id.to_string(),
                event_type: notification.event_type.to_string(),
                entity_type: notification.entity_type.to_string(),
                entity_id: notification.entity_id.map(|id| id.to_string()),
                actor_id: notification.actor_id.map(|id| id.to_string()),
                title: notification.title.to_string(),
                body: notification.body.map(|body| body.to_string()),
                is_read: notification.is_read,
                action_url: notification.action_url.map(|url| url.to_string()),
                metadata: notification.metadata,
                created_at: notification.created_at.to_rfc3339(),
            })
            .collect();
        notifications.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        notifications.truncate(10);
        Ok(crate::context::NotificationListDto {
            notifications,
            unread_count,
        })
    }

    async fn mark_read(&self, id: String, user_id: UserId) -> Result<(), AppError> {
        let id = id
            .parse::<shared::NotificationId>()
            .map_err(|_| AppError::invalid_input("invalid notification id"))?;
        self.notifications.mark_read(id, user_id).await
    }

    async fn mark_all_read(&self, user_id: UserId) -> Result<(), AppError> {
        self.notifications.mark_all_read(user_id).await
    }

    async fn get_settings(
        &self,
        user_id: UserId,
    ) -> Result<crate::context::NotificationSettingsDto, AppError> {
        match self.settings.get_settings(user_id).await {
            Ok(settings) => Ok(Self::settings_dto(settings)),
            Err(AppError::NotFound(_)) => Ok(crate::context::NotificationSettingsDto {
                email_frequency: "immediate".to_string(),
                disabled_event_types: Vec::new(),
                notify_own_changes: false,
            }),
            Err(error) => Err(error),
        }
    }

    async fn update_settings(
        &self,
        user_id: UserId,
        cmd: crate::commands::UpdateNotificationSettingsCommand,
    ) -> Result<crate::context::NotificationSettingsDto, AppError> {
        if !matches!(
            cmd.email_frequency.as_ref(),
            "immediate" | "hourly" | "daily" | "never"
        ) {
            return Err(AppError::invalid_input("invalid email_frequency"));
        }
        if cmd
            .disabled_event_types
            .iter()
            .any(|event_type| event_type.is_empty() || event_type.len() > 100)
        {
            return Err(AppError::invalid_input("invalid disabled_event_types"));
        }
        let settings = domain::NotificationUserSettings {
            user_id,
            email_frequency: cmd.email_frequency,
            disabled_event_types: cmd.disabled_event_types,
            notify_own_changes: cmd.notify_own_changes,
        };
        self.settings.save_settings(&settings).await?;
        Ok(Self::settings_dto(settings))
    }
}
