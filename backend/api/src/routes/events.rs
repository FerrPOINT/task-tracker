use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream::Stream;
use std::{convert::Infallible, sync::Arc};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

#[utoipa::path(
    get,
    path = "/api/v1/events",
    description = "Server-Sent Events stream of tracker invalidation events (issue_created, issue_moved, ...). Clients refetch affected queries.",
    responses(
        (status = 200, description = "SSE stream (text/event-stream)"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []))
)]
pub async fn events(
    State(ctx): State<Arc<app::AppContext>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = ctx.events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(event) => {
            let json = serde_json::to_string(&event).unwrap_or_default();
            Some(Ok(Event::default().event("tracker").data(json)))
        }
        // Lagged subscribers just refetch; skip the gap notification.
        Err(_) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
