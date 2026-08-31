use axum::{
    extract::State,
    http::{HeaderName, HeaderValue, header::CACHE_CONTROL},
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
};
use std::{convert::Infallible, sync::Arc};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct TrackerEventPayload {
    /// Event discriminator, e.g. `issue_created`, `issue_updated`, `issue_deleted`.
    #[serde(rename = "type")]
    pub r#type: String,
    /// UUID of the affected issue (when applicable).
    pub issue_id: Option<String>,
    /// Project key for cache scoping (when applicable).
    pub project_key: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/events",
    description = "Server-Sent Events stream (`text/event-stream`) of tracker invalidation events. Each message is a `tracker` event whose data is a JSON TrackerEventPayload (type, issue_id, project_key). Clients refetch affected queries. Browser EventSource cannot set headers, so this endpoint accepts an access token in the Authorization header for fetch-based clients or in the `access_token` query parameter.",
    params(
        ("access_token" = Option<String>, Query, description = "Short-lived JWT access token fallback for browser EventSource clients that cannot set Authorization headers."),
    ),
    responses(
        (status = 200, description = "SSE stream (text/event-stream) of `tracker` events", body = TrackerEventPayload, content_type = "text/event-stream"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []), ("events_access_token" = []))
)]
pub async fn events(State(ctx): State<Arc<app::AppContext>>) -> impl IntoResponse {
    let rx = ctx.events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(event) => {
            let json = serde_json::to_string(&event).unwrap_or_default();
            Some(Ok::<Event, Infallible>(
                Event::default().event("tracker").data(json),
            ))
        }
        // Lagged subscribers just refetch; skip the gap notification.
        Err(_) => None,
    });
    (
        [
            (
                CACHE_CONTROL,
                HeaderValue::from_static("no-cache, no-transform"),
            ),
            (
                HeaderName::from_static("x-accel-buffering"),
                HeaderValue::from_static("no"),
            ),
        ],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    )
}
