//! Mount only on existing attachment upload/list/download, outside generic bearer_auth.
use axum::{
    extract::{OriginalUri, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use shared::AppError;
use std::sync::Arc;

pub async fn attachment_auth(
    State(ctx): State<Arc<app::AppContext>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .ok_or(AppError::Unauthorized)?;
    match super::central_auth::BRIDGE.try_verified_token(token).await {
        Ok(mut verified) => {
            if verified.context.role.is_none() {
                verified.context.role = verified.subject_role.clone();
            }
            let central = &verified.context;
            let pm = app::sdlc::has_pm_grant(&central.scopes);
            let machine = pm
                || app::sdlc_lifecycle::has_delivery_grant(&central.scopes)
                || central.role.as_deref() == Some("service_account")
                || crate::routes::namespace::human_route_machine(&central.user_id);
            if machine {
                let result: Result<Response, AppError> = async {
                    if crate::routes::namespace::registered_machine(&central.user_id)
                        || !central.allows_service("task-tracker", request.method().as_str())
                    {
                        return Err(AppError::Forbidden);
                    }
                    let actor = domain::sdlc::Principal {
                        subject: central.user_id.clone(),
                        human_session: central.session_id.is_some(),
                        trusted_human: false,
                        scopes: central.scopes.clone(),
                    };
                    let path = request
                        .extensions()
                        .get::<OriginalUri>()
                        .map_or_else(|| request.uri().path(), |uri| uri.0.path());
                    ctx.services.attachment.authorize_artifact_route(
                        &actor,
                        request.method().as_str(),
                        path,
                    )?;
                    let mutation = !matches!(request.method().as_str(), "GET" | "HEAD" | "OPTIONS");
                    let native = app::native_space_access::NativeRequest::verified(
                        &verified, true, mutation,
                    )?;
                    request.extensions_mut().insert(actor);
                    request.extensions_mut().insert(verified.clone());
                    let lifetime = verified.lifetime.clone();
                    let deadline = lifetime.monotonic_deadline();
                    let response = tokio::time::timeout_at(
                        deadline.into(),
                        app::native_space_access::with_request(native, next.run(request)),
                    )
                    .await
                    .map_err(|_| AppError::Forbidden)?;
                    if !lifetime.is_current() {
                        return Err(AppError::Forbidden);
                    }
                    let (parts, body) = response.into_parts();
                    Ok(Response::from_parts(
                        parts,
                        super::auth::guard_admission_body(body, deadline, move || {
                            lifetime.is_current()
                        }),
                    ))
                }
                .await;
                return if pm { Ok(pm_no_store(result)) } else { result };
            }
            let active = ctx.repos.projects.space_admission_enabled().await?;
            let path = request
                .extensions()
                .get::<OriginalUri>()
                .map_or_else(|| request.uri().path(), |uri| uri.0.path());
            let operation = super::auth::request_operation(request.method(), path);
            super::auth::verified_request(ctx, request, next, verified, active, operation)
                .await
                .map_err(map_status)
        }
        Err(pdlc_auth_core::service_bridge::VerifiedBridgeError::Unavailable) => {
            Err(AppError::Unavailable("Central Auth unavailable".into()))
        }
        Err(pdlc_auth_core::service_bridge::VerifiedBridgeError::Forbidden) => {
            Err(AppError::Forbidden)
        }
        Err(
            pdlc_auth_core::service_bridge::VerifiedBridgeError::NotConfigured
            | pdlc_auth_core::service_bridge::VerifiedBridgeError::NotOurs,
        ) => {
            // The legacy adapter itself rejects fallback if either central mode
            // or space admission is active. No accepted central token is read twice.
            super::auth::bearer_auth(State(ctx), request, next)
                .await
                .map_err(map_status)
        }
        Err(_) => Err(AppError::Unauthorized),
    }
}

fn map_status(status: StatusCode) -> AppError {
    match status {
        StatusCode::UNAUTHORIZED => AppError::Unauthorized,
        StatusCode::FORBIDDEN => AppError::Forbidden,
        StatusCode::SERVICE_UNAVAILABLE => {
            AppError::Unavailable("authentication unavailable".into())
        }
        _ => AppError::internal("attachment authentication failed"),
    }
}

fn pm_no_store(outcome: Result<Response, AppError>) -> Response {
    let mut response = outcome.unwrap_or_else(|error| error.into_response());
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}

#[cfg(test)]
#[path = "attachment_pm_review_tests.rs"]
mod pm_review_tests;
