use std::sync::Arc;

use app::AppContext;
use infra::{SmtpEmailSender, build_repositories, run_migrations};
use shared::AppConfig;
use tokio::sync::oneshot;
use tracing::{error, warn};

/// Maximum time to wait for in-flight requests to complete during graceful shutdown.
const SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Interval at which the email digest background task runs.
const DIGEST_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3600);

pub async fn run(
    config: Arc<AppConfig>,
    ready: oneshot::Sender<std::net::SocketAddr>,
    shutdown: oneshot::Receiver<()>,
) {
    run_migrations(config.database.clone())
        .await
        .expect("failed to run migrations");
    let repos = Arc::new(
        build_repositories(config.database.clone())
            .await
            .expect("failed to build repos"),
    );
    let storage: Arc<dyn domain::FileStorage> = Arc::new(infra::FileStorage::new(&config.storage));

    // Construct the SMTP email sender from config and wire it into AppContext
    // as the domain EmailPort implementation.
    let email: Arc<dyn domain::EmailPort> = Arc::new(SmtpEmailSender::new(&config.email));
    let events = app::context::EventBus::default();
    let ctx = Arc::new(AppContext::with_events(
        config.clone(),
        repos,
        storage,
        events,
        email.clone(),
    ));

    // Spawn the email digest background task. It runs every hour, collects
    // unread notifications for users with email_frequency 'hourly' or 'daily',
    // sends a digest email per user, and marks those notifications as read.
    // The task is cancelled (aborted) when the shutdown signal fires.
    let digest_ctx = ctx.clone();
    let digest_handle = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(DIGEST_INTERVAL);
        loop {
            ticker.tick().await;
            if let Err(e) = run_digest_cycle(&digest_ctx).await {
                error!("email digest cycle failed: {e}");
            }
        }
    });

    let address = format!("{}:{}", config.server.address, config.server.port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("failed to bind server");
    let bound_addr = listener.local_addr().expect("local addr");
    let _ = ready.send(bound_addr);
    let server = axum::serve(listener, api::router(ctx.clone()).with_state(ctx))
        .with_graceful_shutdown(async move {
            let _ = shutdown.await;
        });

    // Wrap the server in a 30-second timeout so that even if in-flight
    // requests hang, the process will exit within SHUTDOWN_TIMEOUT of
    // receiving the shutdown signal.
    let _ = tokio::time::timeout(SHUTDOWN_TIMEOUT, server).await;

    // Cancel the background digest task on shutdown.
    digest_handle.abort();
}

/// Run a single email-digest cycle.
///
/// Collects all unread notifications across all users, groups them by
/// recipient, filters to users whose `email_frequency` is `hourly` or
/// `daily`, sends a digest email, then marks the notifications as read.
async fn run_digest_cycle(ctx: &AppContext) -> Result<(), shared::AppError> {
    use std::collections::HashMap;

    // Skip entirely if email is disabled — nothing to send.
    if !ctx.email.is_enabled() {
        return Ok(());
    }

    let notifications = ctx.repos.notifications.list_all_unread().await?;
    if notifications.is_empty() {
        return Ok(());
    }

    // Group unread notifications by recipient.
    let mut by_user: HashMap<shared::UserId, Vec<domain::Notification>> = HashMap::new();
    for n in notifications {
        by_user.entry(n.recipient_id).or_default().push(n);
    }

    for (user_id, user_notifications) in by_user {
        // Look up the user to get their email address and display name.
        let user = match ctx.repos.users.get_by_id(user_id).await {
            Ok(u) => u,
            Err(e) => {
                warn!("digest: skipping user {user_id}: could not load: {e}");
                continue;
            }
        };

        // Check the user's notification settings — only send to users whose
        // email_frequency is 'hourly' or 'daily'.
        let settings = match ctx.repos.notification_settings.get_settings(user_id).await {
            Ok(s) => s,
            Err(e) => {
                warn!("digest: skipping user {user_id}: no settings: {e}");
                continue;
            }
        };

        let freq = settings.email_frequency.as_ref();
        if freq != "hourly" && freq != "daily" {
            continue;
        }

        // Build the digest body.
        let count = user_notifications.len();
        let mut body = format!("You have {count} unread notification(s):\n\n");
        for n in &user_notifications {
            body.push_str(&format!("- {}", n.title));
            if let Some(b) = &n.body {
                body.push_str(&format!(": {b}"));
            }
            body.push('\n');
        }

        let email_notification = domain::EmailNotification {
            recipient_address: user.email.as_ref().to_string(),
            recipient_name: Some(user.display_name.as_ref().to_string()),
            subject: format!("Task Tracker: {count} unread notification(s)"),
            body,
            action_url: None,
        };

        // Send the digest email. On failure, log and skip marking as read
        // so the notifications remain for the next cycle.
        if let Err(e) = ctx.email.send(&email_notification).await {
            error!("digest: failed to send email to {}: {e}", user.email);
            continue;
        }

        // Mark all of this user's notifications as read after a successful send.
        if let Err(e) = ctx.repos.notifications.mark_all_read(user_id).await {
            error!("digest: failed to mark notifications read for {user_id}: {e}");
        }
    }

    Ok(())
}
