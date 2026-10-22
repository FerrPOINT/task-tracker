use std::{future::IntoFuture, sync::Arc};

use app::AppContext;
use infra::{SmtpEmailSender, build_repositories, run_migrations};
use shared::AppConfig;
use tokio::sync::oneshot;
use tracing::{error, warn};

/// Maximum time to drain in-flight requests after a shutdown signal.
const SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Interval at which the email digest background task runs.
const DIGEST_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3600);

fn digest_date_key(now: shared::Timestamp) -> String {
    now.date_naive().to_string()
}

fn digest_is_due(
    frequency: &str,
    settings: &domain::NotificationUserSettings,
    now: shared::Timestamp,
) -> bool {
    match frequency {
        "hourly" => true,
        "daily" => match settings.last_email_digest_at {
            Some(sent_at) => digest_date_key(sent_at) != digest_date_key(now),
            None => true,
        },
        _ => false,
    }
}

async fn mark_digest_sent(
    ctx: &AppContext,
    frequency: &str,
    user_id: shared::UserId,
    now: shared::Timestamp,
) -> Result<(), shared::AppError> {
    if frequency == "daily" {
        ctx.repos
            .notification_settings
            .mark_email_digest_sent(user_id, now)
            .await?;
    }
    Ok(())
}

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
    let mut ctx = AppContext::with_events(
        config.clone(),
        repos,
        storage.clone(),
        events,
        email.clone(),
    );
    if let Ok(instance_id) = std::env::var("TASKTRACKER_SDLC__INSTANCE_ID") {
        let sdlc_config = domain::sdlc::SdlcConfig {
            single_task_project_ids: domain::sdlc::parse_single_task_projects(
                &std::env::var("TASKTRACKER_SDLC__SINGLE_TASK_PROJECT_IDS").unwrap_or_default(),
            )
            .expect("invalid single-task project allowlist"),
            reservation_scheduler_subject: std::env::var(
                "TASKTRACKER_SDLC__RESERVATION_SCHEDULER_SUBJECT",
            )
            .unwrap_or_default(),
            instance_id,
            orchestrator_subject: std::env::var("TASKTRACKER_SDLC__ORCHESTRATOR_SUBJECT")
                .unwrap_or_default(),
            verifier_subject: std::env::var("TASKTRACKER_SDLC__VERIFIER_SUBJECT")
                .unwrap_or_default(),
        };
        let repository =
            infra::sdlc::PostgresSdlcRepository::connect(&config.database.url, sdlc_config.clone())
                .await
                .expect("failed to initialize stable SDLC storage")
                .with_file_storage(storage.clone());
        ctx.services.attachment = Arc::new(
            app::services::AttachmentServiceImpl::new(
                ctx.repos.attachments.clone(),
                ctx.repos.issues.clone(),
                storage,
                ctx.repos.watchers.clone(),
                ctx.events.clone(),
                ctx.repos.notifications.clone(),
                ctx.repos.notification_settings.clone(),
                ctx.authz.clone(),
            )
            .with_artifact_config(sdlc_config),
        );
        ctx.sdlc = Some(app::sdlc::SdlcService {
            repository: Arc::new(repository),
            fleet_configuration: app::sdlc_configuration::FleetConfigurationReader::new(
                &std::env::var("TASKTRACKER_SDLC__FLEET_URL").unwrap_or_default(),
                &std::env::var("TASKTRACKER_SDLC__FLEET_READ_TOKEN").unwrap_or_default(),
            )
            .ok(),
        });
    }
    if let Some(reader) = infra::task_documents::WikiReader::from_deployment()
        .expect("invalid namespace Wiki reader configuration")
    {
        ctx.task_documents = Arc::new(reader);
    }
    if let Some(reader) = infra::task_repositories::ForgeReader::from_deployment()
        .expect("invalid namespace Forge reader configuration")
    {
        ctx.task_repositories = Arc::new(reader);
    }
    let ctx = Arc::new(ctx);

    // Spawn the email digest background task. It checks unread notifications
    // every hour, sends hourly digests every cycle and daily digests at most
    // once per calendar day. The task is cancelled (aborted) on shutdown.
    let digest_ctx = ctx.clone();
    let digest_handle = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(DIGEST_INTERVAL);
        loop {
            ticker.tick().await;
            if let Err(e) = run_digest_cycle(&digest_ctx, shared::now()).await {
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

    // Signal graceful shutdown immediately, then bound only the drain phase.
    // This avoids applying a timeout to the server's normal healthy lifetime.
    let (shutdown_started_tx, shutdown_started_rx) = tokio::sync::oneshot::channel();
    let server = axum::serve(listener, api::router(ctx.clone()).with_state(ctx))
        .with_graceful_shutdown(async move {
            let _ = shutdown.await;
            let _ = shutdown_started_tx.send(());
        })
        .into_future();
    tokio::pin!(server);

    tokio::select! {
        result = &mut server => {
            if let Err(e) = result {
                error!("server error: {e}");
            }
        }
        _ = shutdown_started_rx => {
            if tokio::time::timeout(SHUTDOWN_TIMEOUT, &mut server).await.is_err() {
                warn!("graceful shutdown exceeded {SHUTDOWN_TIMEOUT:?}; dropping active connections");
            }
        }
    }

    // Cancel the background digest task on shutdown.
    digest_handle.abort();
}

/// Run a single email-digest cycle.
///
/// Collects all unread notifications across all users, groups them by
/// recipient, filters to users whose digest frequency is due, sends one digest
/// email per user, then marks the delivered notifications as read.
async fn run_digest_cycle(
    ctx: &AppContext,
    now: shared::Timestamp,
) -> Result<(), shared::AppError> {
    use std::collections::HashMap;

    // Skip entirely if email is disabled — nothing to send.
    if !ctx.email.is_enabled() {
        return Ok(());
    }

    let notifications = ctx.repos.notifications.list_all_unread().await?;
    if notifications.is_empty() {
        return Ok(());
    }

    let admission_active = ctx.repos.projects.space_admission_enabled().await?;
    let reader = if admission_active {
        Some(app::space_access::notification_reader().await?)
    } else {
        None
    };
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

        if !user.is_active {
            continue;
        }
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
        if !digest_is_due(freq, &settings, now) {
            continue;
        }

        // Fresh recipient admission is independent of the actor who created the
        // historical message. Retain denied notifications; do not mark them read.
        let mut delivery_proof = None;
        let user_notifications = if let Some(reader) = reader.as_ref() {
            let proof = match app::space_access::recipient_read(
                user_id,
                ctx.repos.users.as_ref(),
                ctx.repos.projects.as_ref(),
                reader,
            )
            .await
            {
                Ok(p) => p,
                Err(_) => {
                    warn!("digest: recipient admission unavailable for {user_id}");
                    continue;
                }
            };
            let spaces = proof
                .readable_spaces()
                .map_err(|_| shared::AppError::Forbidden)?;
            let mut visible = Vec::new();
            let mut offset = 0;
            loop {
                if !proof.is_current() {
                    return Err(shared::AppError::Forbidden);
                }
                let (page, _) = ctx
                    .repos
                    .notifications
                    .list_space_visible(user_id, &spaces, false, 50, offset)
                    .await?;
                let count = page.len();
                visible.extend(page);
                if count < 50 {
                    break;
                }
                offset += 50;
                if offset >= 10000 {
                    return Err(shared::AppError::Unavailable(
                        "notification_delivery_limit_exceeded".into(),
                    ));
                }
            }
            if visible.is_empty() {
                continue;
            }
            if !proof.is_current() {
                return Err(shared::AppError::Forbidden);
            }
            delivery_proof = Some(proof);
            visible
        } else {
            user_notifications
        };
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
        let sent = if let Some(proof) = delivery_proof.as_ref() {
            if !proof.is_current() {
                continue;
            }
            match tokio::time::timeout_at(
                proof.monotonic_deadline().into(),
                ctx.email.send(&email_notification),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => {
                    warn!("digest: confirmation expired during delivery for {user_id}");
                    continue;
                }
            }
        } else {
            ctx.email.send(&email_notification).await
        };
        if let Err(e) = sent {
            error!("digest: failed to send email to {}: {e}", user.email);
            continue;
        }
        if let Err(e) = mark_digest_sent(ctx, freq, user_id, now).await {
            error!("digest: failed to mark digest sent for {user_id}: {e}");
            continue;
        }

        // Mark read only the notifications that were part of the delivered
        // digest snapshot. `mark_all_read` would swallow notifications that
        // arrived between `list_all_unread` and this point.
        let delivered: Vec<shared::NotificationId> =
            user_notifications.iter().map(|n| n.id).collect();
        if let Err(e) = ctx.repos.notifications.mark_read_batch(&delivered).await {
            error!("digest: failed to mark notifications read for {user_id}: {e}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(value: &str) -> shared::Timestamp {
        value.parse().unwrap()
    }

    fn settings(
        last_email_digest_at: Option<shared::Timestamp>,
    ) -> domain::NotificationUserSettings {
        domain::NotificationUserSettings {
            user_id: shared::UserId::new(),
            email_frequency: "daily".into(),
            disabled_event_types: Vec::new(),
            notify_own_changes: false,
            last_email_digest_at,
        }
    }

    #[test]
    fn hourly_digest_is_due_every_cycle() {
        let user_settings = settings(None);
        let now = ts("2026-08-31T12:00:00+00:00");

        assert!(digest_is_due("hourly", &user_settings, now));
        assert!(digest_is_due(
            "hourly",
            &settings(Some(now)),
            ts("2026-08-31T18:00:00+00:00")
        ));
    }

    #[test]
    fn daily_digest_is_due_once_per_day() {
        let first = ts("2026-08-31T12:00:00+00:00");
        let later_same_day = ts("2026-08-31T18:00:00+00:00");
        let next_day = ts("2026-09-01T09:00:00+00:00");

        assert!(digest_is_due("daily", &settings(None), first));
        assert!(!digest_is_due(
            "daily",
            &settings(Some(first)),
            later_same_day
        ));
        assert!(digest_is_due("daily", &settings(Some(first)), next_day));
    }

    #[test]
    fn immediate_and_never_are_not_digest_frequencies() {
        let settings = settings(None);
        let now = ts("2026-08-31T12:00:00+00:00");

        assert!(!digest_is_due("immediate", &settings, now));
        assert!(!digest_is_due("never", &settings, now));
    }
}

#[cfg(test)]
mod unified_notification_tests;
