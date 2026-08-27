//! Phase 8: Admin REST API routes.
//!
//! All endpoints are behind the standard bearer-auth middleware. The system
//! admin authorization check is performed in the service layer — not in the
//! middleware — so that the gate is co-located with the business logic and
//! cannot be accidentally bypassed by a routing misconfiguration.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use shared::{AppError, UserId};

// ---------------------------------------------------------------------------
// Request / response schemas
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AdminUserResponse {
    pub id: String,
    pub email: String,
    pub username: String,
    pub display_name: String,
    pub is_system_admin: bool,
    pub is_active: bool,
}

impl From<app::context::AdminUserDto> for AdminUserResponse {
    fn from(dto: app::context::AdminUserDto) -> Self {
        Self {
            id: dto.id,
            email: dto.email,
            username: dto.username,
            display_name: dto.display_name,
            is_system_admin: dto.is_system_admin,
            is_active: dto.is_active,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AdminUserListResponse {
    pub users: Vec<AdminUserResponse>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct AdminCreateUserRequest {
    pub email: String,
    pub username: String,
    pub display_name: String,
    /// The password is hashed before storage and never returned or logged.
    pub password: String,
    #[serde(default)]
    pub is_system_admin: bool,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateUserStatusRequest {
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AuditLogResponse {
    pub id: String,
    pub actor_id: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

impl From<app::context::AuditLogDto> for AuditLogResponse {
    fn from(dto: app::context::AuditLogDto) -> Self {
        Self {
            id: dto.id,
            actor_id: dto.actor_id,
            action: dto.action,
            entity_type: dto.entity_type,
            entity_id: dto.entity_id,
            metadata: dto.metadata,
            created_at: dto.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AuditLogListResponse {
    pub entries: Vec<AuditLogResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SystemSettingResponse {
    pub key: String,
    pub value: serde_json::Value,
    pub updated_at: String,
}

impl From<app::context::SystemSettingDto> for SystemSettingResponse {
    fn from(dto: app::context::SystemSettingDto) -> Self {
        Self {
            key: dto.key,
            value: dto.value,
            updated_at: dto.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SystemSettingListResponse {
    pub settings: Vec<SystemSettingResponse>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateSystemSettingRequest {
    pub key: String,
    pub value: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_user_id(claims: &app::auth::UserClaims) -> Result<UserId, AppError> {
    uuid::Uuid::parse_str(&claims.sub)
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/api/v1/admin/users",
    responses(
        (status = 200, body = AdminUserListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden — system admin required"),
    ),
    tag = "admin",
    operation_id = "admin_list_users",
)]
pub async fn list_users(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
) -> Result<Json<AdminUserListResponse>, AppError> {
    let requester_id = parse_user_id(&claims)?;
    let users = ctx.services.admin.list_users(requester_id).await?;
    Ok(Json(AdminUserListResponse {
        users: users.into_iter().map(AdminUserResponse::from).collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/users",
    request_body = AdminCreateUserRequest,
    responses(
        (status = 201, body = AdminUserResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden — system admin required"),
        (status = 409, description = "Conflict — email already registered"),
    ),
    tag = "admin",
)]
pub async fn create_user(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    Json(req): Json<AdminCreateUserRequest>,
) -> Result<(StatusCode, Json<AdminUserResponse>), AppError> {
    let requester_id = parse_user_id(&claims)?;
    let cmd = app::context::AdminCreateUserCommand {
        email: req.email,
        username: req.username,
        display_name: req.display_name,
        password: req.password,
        is_system_admin: req.is_system_admin,
    };
    let user = ctx.services.admin.create_user(requester_id, cmd).await?;
    Ok((StatusCode::CREATED, Json(AdminUserResponse::from(user))))
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/users/{id}/status",
    params(("id" = String, Path, description = "User id")),
    request_body = UpdateUserStatusRequest,
    responses(
        (status = 200, body = AdminUserResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden — system admin required"),
        (status = 409, description = "Conflict — cannot deactivate last system admin"),
    ),
    tag = "admin",
)]
pub async fn update_user_status(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    Path(id): Path<String>,
    Json(req): Json<UpdateUserStatusRequest>,
) -> Result<Json<AdminUserResponse>, AppError> {
    let requester_id = parse_user_id(&claims)?;
    let user_id = uuid::Uuid::parse_str(&id)
        .map(UserId::from_uuid)
        .map_err(|_| AppError::invalid_input("invalid user id"))?;
    let user = ctx
        .services
        .admin
        .update_user_status(requester_id, user_id, req.is_active)
        .await?;
    Ok(Json(AdminUserResponse::from(user)))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/audit-log",
    params(("limit" = Option<u64>, Query, description = "Maximum entries (default 100)"), ("offset" = Option<u64>, Query, description = "Pagination offset (default 0)")),
    responses(
        (status = 200, body = AuditLogListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden — system admin required"),
    ),
    tag = "admin",
)]
pub async fn list_audit_logs(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    axum::extract::Query(params): axum::extract::Query<AuditLogQueryParams>,
) -> Result<Json<AuditLogListResponse>, AppError> {
    let requester_id = parse_user_id(&claims)?;
    let limit = params.limit.unwrap_or(100).min(1000);
    let offset = params.offset.unwrap_or(0);
    let entries = ctx
        .services
        .admin
        .list_audit_logs(requester_id, limit, offset)
        .await?;
    Ok(Json(AuditLogListResponse {
        entries: entries.into_iter().map(AuditLogResponse::from).collect(),
    }))
}

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
pub struct AuditLogQueryParams {
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/system-settings",
    responses(
        (status = 200, body = SystemSettingListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden — system admin required"),
    ),
    tag = "admin",
)]
pub async fn list_system_settings(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
) -> Result<Json<SystemSettingListResponse>, AppError> {
    let requester_id = parse_user_id(&claims)?;
    let settings = ctx
        .services
        .admin
        .list_system_settings(requester_id)
        .await?;
    Ok(Json(SystemSettingListResponse {
        settings: settings
            .into_iter()
            .map(SystemSettingResponse::from)
            .collect(),
    }))
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/system-settings",
    request_body = UpdateSystemSettingRequest,
    responses(
        (status = 200, body = SystemSettingResponse),
        (status = 400, description = "Invalid key or value too large"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden — system admin required"),
    ),
    tag = "admin",
)]
pub async fn update_system_setting(
    State(ctx): State<Arc<app::AppContext>>,
    claims: axum::Extension<app::auth::UserClaims>,
    Json(req): Json<UpdateSystemSettingRequest>,
) -> Result<Json<SystemSettingResponse>, AppError> {
    let requester_id = parse_user_id(&claims)?;
    let setting = ctx
        .services
        .admin
        .update_system_setting(requester_id, req.key, req.value)
        .await?;
    Ok(Json(SystemSettingResponse::from(setting)))
}
