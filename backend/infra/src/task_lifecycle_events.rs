//! PostgreSQL broadcasts invalidations after commit to every serving replica.
//! The event table is durable; reconnecting UI refetches rather than trusting a gap.
use sea_orm::sqlx::postgres::PgListener;
use std::time::Duration;

pub async fn run(database_url: String, events: app::context::EventBus) {
    loop {
        let connected = async {
            let mut listener = PgListener::connect(&database_url).await?;
            listener.listen("pdlc_task_lifecycle").await?;
            Ok::<_, sea_orm::sqlx::Error>(listener)
        }
        .await;
        match connected {
            Ok(mut listener) => loop {
                match listener.recv().await {
                    Ok(notification) => {
                        match serde_json::from_str::<shared::TrackerEvent>(notification.payload()) {
                            Ok(
                                event @ (shared::TrackerEvent::IssueUpdated { .. }
                                | shared::TrackerEvent::TaskLifecycleActivated),
                            ) => events.publish(event),
                            _ => tracing::warn!("ignored invalid task lifecycle notification"),
                        }
                    }
                    Err(_) => {
                        tracing::warn!("task lifecycle notification connection interrupted");
                        break;
                    }
                }
            },
            Err(_) => tracing::warn!("task lifecycle notification connection unavailable"),
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
