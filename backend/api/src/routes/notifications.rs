use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use shared::AppError;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NotificationResponse {
    pub id: String,
    pub event_type: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub actor_id: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub is_read: bool,
    pub action_url: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NotificationListResponse {
    pub notifications: Vec<NotificationResponse>,
    pub unread_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NotificationSettingsResponse {
    pub email_frequency: String,
    pub disabled_event_types: Vec<String>,
    pub notify_own_changes: bool,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateNotificationSettingsRequest {
    pub email_frequency: String,
    pub disabled_event_types: Vec<String>,
    pub notify_own_changes: bool,
}

#[utoipa::path(
    get,
    path = "/api/v1/notifications",
    responses((status = 200, body = NotificationListResponse))
)]
pub async fn list_notifications(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
) -> Result<Json<NotificationListResponse>, AppError> {
    let user_id = parse_user(&claims)?;
    let result = ctx.services.notification.list_unread(user_id).await?;
    Ok(Json(NotificationListResponse {
        notifications: result
            .notifications
            .into_iter()
            .map(|n| NotificationResponse {
                id: n.id,
                event_type: n.event_type,
                entity_type: n.entity_type,
                entity_id: n.entity_id,
                actor_id: n.actor_id,
                title: n.title,
                body: n.body,
                is_read: n.is_read,
                action_url: n.action_url,
                metadata: n.metadata,
                created_at: n.created_at,
            })
            .collect(),
        unread_count: result.unread_count,
    }))
}

#[utoipa::path(
    patch,
    path = "/api/v1/notifications/{id}/read",
    params(("id" = String, Path, description = "Notification id")),
    responses((status = 204), (status = 400), (status = 404))
)]
pub async fn mark_notification_read(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user(&claims)?;
    ctx.services.notification.mark_read(id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/v1/notifications/read-all",
    responses((status = 204))
)]
pub async fn mark_all_notifications_read(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
) -> Result<StatusCode, AppError> {
    let user_id = parse_user(&claims)?;
    ctx.services.notification.mark_all_read(user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/notification-settings",
    responses((status = 200, body = NotificationSettingsResponse))
)]
pub async fn get_notification_settings(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
) -> Result<Json<NotificationSettingsResponse>, AppError> {
    let user_id = parse_user(&claims)?;
    let settings = ctx.services.notification.get_settings(user_id).await?;
    Ok(Json(NotificationSettingsResponse {
        email_frequency: settings.email_frequency,
        disabled_event_types: settings.disabled_event_types,
        notify_own_changes: settings.notify_own_changes,
    }))
}

#[utoipa::path(
    patch,
    path = "/api/v1/notification-settings",
    request_body = UpdateNotificationSettingsRequest,
    responses((status = 200, body = NotificationSettingsResponse), (status = 400))
)]
pub async fn update_notification_settings(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    Json(req): Json<UpdateNotificationSettingsRequest>,
) -> Result<Json<NotificationSettingsResponse>, AppError> {
    let user_id = parse_user(&claims)?;
    let cmd = app::commands::UpdateNotificationSettingsCommand {
        email_frequency: req.email_frequency.into(),
        disabled_event_types: req
            .disabled_event_types
            .into_iter()
            .map(Into::into)
            .collect(),
        notify_own_changes: req.notify_own_changes,
    };
    let settings = ctx
        .services
        .notification
        .update_settings(user_id, cmd)
        .await?;
    Ok(Json(NotificationSettingsResponse {
        email_frequency: settings.email_frequency,
        disabled_event_types: settings.disabled_event_types,
        notify_own_changes: settings.notify_own_changes,
    }))
}

fn parse_user(claims: &app::auth::UserClaims) -> Result<shared::UserId, AppError> {
    uuid::Uuid::parse_str(&claims.sub)
        .map(shared::UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))
}
