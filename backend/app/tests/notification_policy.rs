//! Narrow production helper boundary: distinguish absent preferences from failed storage.
pub mod context {
    pub use app::context::*;
}
pub mod dto {
    pub use app::dto::*;
}
#[allow(dead_code)]
#[path = "../src/services/helpers.rs"]
mod production_helpers;
use domain::{NotificationRepository, UserNotificationSettingsRepository};
use shared::{AppError, UserId};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tracing::instrument::WithSubscriber;

#[derive(Default)]
struct Capture(Mutex<Vec<String>>);
impl tracing::Subscriber for Capture {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        struct Fields(String);
        impl tracing::field::Visit for Fields {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.0.push_str(&format!("{}={value:?}", field.name()));
            }
        }
        let mut fields = Fields(String::new());
        event.record(&mut fields);
        self.0.lock().unwrap().push(fields.0);
    }
}
struct Settings(u8);
#[async_trait::async_trait]
impl UserNotificationSettingsRepository for Settings {
    async fn get_settings(&self, id: UserId) -> Result<domain::NotificationUserSettings, AppError> {
        match self.0 {
            0 => Err(AppError::not_found("settings", id)),
            1 => Err(AppError::Internal(
                "PRIVATE_DATABASE_AND_MESSAGE_CONTENT".into(),
            )),
            mode => Ok(domain::NotificationUserSettings {
                user_id: id,
                email_frequency: "immediate".into(),
                disabled_event_types: if mode == 2 {
                    vec!["issue_commented".into()]
                } else {
                    vec![]
                },
                notify_own_changes: mode == 3,
                last_email_digest_at: None,
            }),
        }
    }
    async fn save_settings(&self, _: &domain::NotificationUserSettings) -> Result<(), AppError> {
        Ok(())
    }
    async fn mark_email_digest_sent(
        &self,
        _: UserId,
        _: shared::Timestamp,
    ) -> Result<(), AppError> {
        Ok(())
    }
}
#[derive(Default)]
struct Notifications {
    saved: AtomicUsize,
    fail: bool,
}
#[async_trait::async_trait]
impl NotificationRepository for Notifications {
    async fn save(&self, n: &domain::Notification) -> Result<shared::NotificationId, AppError> {
        self.saved.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(AppError::Internal("PRIVATE_SAVE_CONTENT".into()))
        } else {
            Ok(n.id)
        }
    }
    async fn mark_read_batch(&self, _: &[shared::NotificationId]) -> Result<(), AppError> {
        Ok(())
    }
    async fn list_unread(&self, _: UserId) -> Result<Vec<domain::Notification>, AppError> {
        Ok(vec![])
    }
    async fn list_all_unread(&self) -> Result<Vec<domain::Notification>, AppError> {
        Ok(vec![])
    }
    async fn mark_read(&self, _: shared::NotificationId, _: UserId) -> Result<(), AppError> {
        Ok(())
    }
    async fn mark_all_read(&self, _: UserId) -> Result<(), AppError> {
        Ok(())
    }
}
fn notification(actor_is_recipient: bool) -> domain::Notification {
    let recipient = UserId::new();
    domain::Notification {
        id: shared::NotificationId::new(),
        recipient_id: recipient,
        event_type: "issue_commented".into(),
        entity_type: "issue".into(),
        entity_id: None,
        actor_id: Some(if actor_is_recipient {
            recipient
        } else {
            UserId::new()
        }),
        title: "PRIVATE_TITLE".into(),
        body: Some("PRIVATE_BODY".into()),
        is_read: false,
        read_at: None,
        action_url: None,
        metadata: serde_json::json!({}),
        created_at: shared::now(),
    }
}
#[tokio::test]
async fn missing_preferences_keep_defaults_but_storage_errors_are_visible_without_private_payload()
{
    for (mode, own, save_failure, want_saved, want_warning) in [
        (0, false, false, 1, false),
        (0, true, false, 0, false),
        (1, false, false, 0, true),
        (2, false, false, 0, false),
        (3, true, false, 1, false),
        (0, false, true, 1, true),
    ] {
        let concrete = Arc::new(Notifications {
            fail: save_failure,
            ..Default::default()
        });
        let store: Arc<dyn NotificationRepository> = concrete.clone();
        let settings: Arc<dyn UserNotificationSettingsRepository> = Arc::new(Settings(mode));
        let capture = Arc::new(Capture::default());
        let events = app::context::EventBus::default();
        let mut receiver = events.subscribe();
        production_helpers::create_notification_if_allowed(
            &store,
            &settings,
            &events,
            notification(own),
        )
        .with_subscriber(capture.clone())
        .await;
        assert_eq!(concrete.saved.load(Ordering::SeqCst), want_saved);
        assert_eq!(
            receiver.try_recv().is_ok(),
            want_saved == 1 && !save_failure
        );
        let warnings = capture.0.lock().unwrap();
        assert_eq!(
            !warnings.is_empty(),
            want_warning,
            "storage failure must produce an operational signal"
        );
        assert!(
            warnings.iter().all(|value| !value.contains("PRIVATE")),
            "private message/database details must not be logged"
        );
    }
}
