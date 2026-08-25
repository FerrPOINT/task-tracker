//! SMTP email delivery boundary — transport/config only (Phase 6).
//!
//! Public surface:
//! - [`EmailNotification`] — value object describing a notification to send.
//! - [`EmailSender`] — trait abstracting delivery so tests can swap
//!   the SMTP transport for a no-op/fake.
//! - [`SmtpEmailSender`] — concrete Lettre async tokio/rustls implementation.
//! - [`render_notification`] — pure HTML/plain-text rendering used by the
//!   sender and exercised directly in tests.
//!
//! When `EmailConfig::enabled` is false, [`SmtpEmailSender::send_notification`]
//! is a no-op success. When enabled but the transport cannot connect, the
//! error is mapped to [`AppError::Internal`] with a generic message that does
//! NOT leak host, credentials, or email addresses.

use shared::{AppError, EmailConfig};

#[cfg(test)]
#[path = "email_tests.rs"]
mod tests;

/// Dynamic content for a single outgoing notification.
///
/// Lives in infra (not domain) because domain scope is prohibited for this
/// phase and the email boundary is an infrastructure concern.
#[derive(Debug, Clone)]
pub struct EmailNotification {
    pub recipient_address: String,
    pub recipient_name: Option<String>,
    pub subject: String,
    pub body: String,
    pub action_url: Option<String>,
}

/// Rendered notification ready for transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedEmail {
    pub html: String,
    pub plain_text: String,
}

/// Escape dynamic content for safe inclusion in HTML text nodes/attributes.
///
/// Escapes `& < > " '` — the five characters recommended by OWASP.
fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Pure rendering function — no I/O, used by the sender and by tests.
pub fn render_notification(notification: &EmailNotification) -> RenderedEmail {
    let subject = escape_html(&notification.subject);
    let recipient_name = notification
        .recipient_name
        .as_deref()
        .map(escape_html)
        .unwrap_or_default();
    let body = escape_html(&notification.body);
    let action_url_escaped = notification.action_url.as_deref().map(escape_html);

    // Plain-text fallback: no escaping, but strip nothing — the raw text is
    // what the user sees in plain-text clients.
    let mut plain_text = format!("{}\n\n{}", notification.subject, notification.body);
    if let Some(url) = &notification.action_url {
        plain_text.push_str(&format!("\n\nOpen: {url}"));
    }

    let action_html = match &action_url_escaped {
        Some(url) => format!(r#"<p><a href="{url}">Open in Task Tracker</a></p>"#),
        None => String::new(),
    };

    let name_line = if recipient_name.is_empty() {
        String::new()
    } else {
        format!("<p>Hi, {recipient_name},</p>")
    };

    let html = format!(
        "<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head><meta charset=\"utf-8\"><title>{subject}</title></head>\n\
<body>\n\
{name_line}\n\
<p>{body}</p>\n\
{action_html}\n\
<hr><p style=\"color:#888;font-size:12px\">This is an automated notification from Task Tracker.</p>\n\
</body>\n</html>"
    );

    RenderedEmail { html, plain_text }
}

/// Delivery boundary — abstracts SMTP so callers (and tests) can swap impls.
#[async_trait::async_trait]
pub trait EmailSender: Send + Sync {
    async fn send_notification(&self, notification: &EmailNotification) -> Result<(), AppError>;
}

/// Concrete SMTP sender using Lettre async tokio/rustls.
///
/// Construct with [`SmtpEmailSender::new`] from an [`EmailConfig`]. When
/// `enabled` is false, `send_notification` returns `Ok(())` without touching
/// the network.
pub struct SmtpEmailSender {
    config: EmailConfig,
}

impl SmtpEmailSender {
    pub fn new(config: &EmailConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    /// Returns true when email delivery is enabled and a real transport
    /// will be used.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

#[async_trait::async_trait]
impl EmailSender for SmtpEmailSender {
    async fn send_notification(&self, notification: &EmailNotification) -> Result<(), AppError> {
        if !self.config.enabled {
            // Default-disabled: no-op success, no network, no credentials needed.
            return Ok(());
        }

        // Build the transport. We construct it inside the async path so that
        // connection failures map to a generic AppError::Internal without
        // leaking host/credential details.
        self.send_via_lettre(notification).await
    }
}

/// Adapter implementing the domain [`EmailPort`] trait by delegating to
/// [`SmtpEmailSender`] (which implements the infra [`EmailSender`] trait).
///
/// This is the glue that lets the application layer depend on
/// `domain::EmailPort` without knowing about SMTP/Letre details.
#[async_trait::async_trait]
impl domain::EmailPort for SmtpEmailSender {
    fn is_enabled(&self) -> bool {
        SmtpEmailSender::is_enabled(self)
    }

    async fn send(&self, notification: &domain::EmailNotification) -> Result<(), AppError> {
        // Translate the domain value object into the infra value object.
        let infra_notification = EmailNotification {
            recipient_address: notification.recipient_address.clone(),
            recipient_name: notification.recipient_name.clone(),
            subject: notification.subject.clone(),
            body: notification.body.clone(),
            action_url: notification.action_url.clone(),
        };
        EmailSender::send_notification(self, &infra_notification).await
    }
}

impl SmtpEmailSender {
    async fn send_via_lettre(&self, notification: &EmailNotification) -> Result<(), AppError> {
        use lettre::message::header::ContentType;
        use lettre::message::{Mailbox, MultiPart, SinglePart};
        use lettre::transport::smtp::authentication::Credentials;
        use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

        let rendered = render_notification(notification);

        let from_mailbox: Mailbox =
            format!("{} <{}>", self.config.from_name, self.config.from_address)
                .parse()
                .map_err(|_| AppError::internal("email sender address is invalid"))?;

        let to_addr = notification.recipient_address.trim();
        let to_mailbox: Mailbox = match &notification.recipient_name {
            Some(name) if !name.trim().is_empty() => format!("{name} <{to_addr}>")
                .parse()
                .map_err(|_| AppError::internal("email recipient address is invalid"))?,
            _ => to_addr
                .parse()
                .map_err(|_| AppError::internal("email recipient address is invalid"))?,
        };

        let message = Message::builder()
            .from(from_mailbox)
            .to(to_mailbox)
            .subject(&notification.subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(rendered.plain_text.clone()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(rendered.html.clone()),
                    ),
            )
            .map_err(|_| AppError::internal("failed to build email message"))?;

        // Build the SMTP transport. rustls is used to avoid linking OpenSSL.
        let mut transport_builder =
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.host)
                .map_err(|_| AppError::internal("failed to build email message"))?
                .port(self.config.port);

        if let (Some(user), Some(pass)) = (&self.config.username, &self.config.password) {
            transport_builder =
                transport_builder.credentials(Credentials::new(user.clone(), pass.clone()));
        }

        let transport = transport_builder.build();

        transport
            .send(message)
            .await
            .map_err(|_| AppError::internal("failed to send email notification via SMTP"))?;

        Ok(())
    }
}
