//! Email delivery port — abstracts SMTP so the application layer can
//! trigger email delivery without depending on infrastructure.

use shared::AppError;

/// Value object describing an outgoing email notification.
#[derive(Debug, Clone)]
pub struct EmailNotification {
    pub recipient_address: String,
    pub recipient_name: Option<String>,
    pub subject: String,
    pub body: String,
    pub action_url: Option<String>,
}

/// Port for sending email notifications. Implemented in `infra`.
#[async_trait::async_trait]
pub trait EmailPort: Send + Sync {
    /// Returns true when email delivery is enabled.
    fn is_enabled(&self) -> bool;

    /// Send a single notification email. When disabled, returns Ok(()).
    async fn send(&self, notification: &EmailNotification) -> Result<(), AppError>;
}

/// No-op implementation for tests and disabled configs.
pub struct StubEmailPort;

#[async_trait::async_trait]
impl EmailPort for StubEmailPort {
    fn is_enabled(&self) -> bool {
        false
    }

    async fn send(&self, _notification: &EmailNotification) -> Result<(), AppError> {
        Ok(())
    }
}
