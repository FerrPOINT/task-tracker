use axum::{
    Extension,
    extract::State,
    http::{HeaderName, HeaderValue, header::CACHE_CONTROL},
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
};
use std::{convert::Infallible, sync::Arc};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use shared::{AppError, ProjectKey, TrackerEvent, UserId};

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct TrackerEventPayload {
    /// Event discriminator, e.g. `issue_created`, `issue_updated`, `issue_deleted`.
    #[serde(rename = "type")]
    pub r#type: String,
    /// UUID of the affected issue (when applicable).
    pub issue_id: Option<String>,
    /// Project key for cache scoping (when applicable).
    pub project_key: Option<String>,
    /// Notification recipient UUID (when applicable).
    pub recipient_id: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/events",
    description = "Server-Sent Events stream (`text/event-stream`) of tracker invalidation events. Each message is a `tracker` event whose data is a JSON TrackerEventPayload (type, issue_id, project_key, recipient_id). Clients refetch affected queries. Browser EventSource cannot set headers, so this endpoint accepts an access token in the Authorization header for fetch-based clients or in the `access_token` query parameter.",
    params(
        ("access_token" = Option<String>, Query, description = "Short-lived JWT access token fallback for browser EventSource clients that cannot set Authorization headers."),
    ),
    responses(
        (status = 200, description = "SSE stream (text/event-stream) of `tracker` events", body = TrackerEventPayload, content_type = "text/event-stream"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer" = []), ("events_access_token" = []))
)]
pub async fn events(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(claims): Extension<app::auth::UserClaims>,
) -> Result<impl IntoResponse, AppError> {
    let requester = parse_user_id(&claims)?;
    let rx = ctx.events.subscribe();
    let stream = BroadcastStream::new(rx)
        .then(move |msg| {
            let ctx = ctx.clone();
            async move {
                let event = match msg {
                    Ok(event) => event,
                    // Lagged subscribers just refetch; skip the gap notification.
                    Err(_) => return None,
                };
                if !event_visible_to(&ctx, &event, requester).await {
                    return None;
                }
                let json = serde_json::to_string(&event).unwrap_or_default();
                Some(Ok::<Event, Infallible>(
                    Event::default().event("tracker").data(json),
                ))
            }
        })
        .filter_map(|event| event);
    Ok((
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
    ))
}

fn parse_user_id(claims: &app::auth::UserClaims) -> Result<UserId, AppError> {
    claims
        .sub
        .parse()
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))
}

async fn event_visible_to(ctx: &app::AppContext, event: &TrackerEvent, requester: UserId) -> bool {
    match event {
        TrackerEvent::NotificationCreated { recipient_id } => {
            recipient_id == &requester.to_string()
        }
        _ => project_event_visible_to(ctx, event.project_key(), requester).await,
    }
}

async fn project_event_visible_to(
    ctx: &app::AppContext,
    project_key: &str,
    requester: UserId,
) -> bool {
    let key = ProjectKey::new(project_key.to_string());
    if !key.is_valid() {
        return false;
    }
    let Ok(project) = ctx.repos.projects.get_by_key(&key).await else {
        return false;
    };
    ctx.authz
        .require_project_access(project.id, requester)
        .await
        .is_ok()
}
