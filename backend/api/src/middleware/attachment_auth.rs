//! Mount only on existing attachment upload/list/download, outside generic bearer_auth.
use axum::{
    extract::{OriginalUri, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
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
    match super::central_auth::check_token(token).await {
        super::central_auth::CentralCheck::Validated(central, _) => {
            if app::sdlc::has_pm_grant(&central.scopes) {
                return Err(AppError::Forbidden);
            }
            let machine = app::sdlc_lifecycle::has_delivery_grant(&central.scopes)
                || central.role.as_deref() == Some("service_account")
                || crate::routes::namespace::human_route_machine(&central.user_id);
            if machine {
                if crate::routes::namespace::registered_machine(&central.user_id)
                    || !central.allows_service("task-tracker", request.method().as_str())
                {
                    return Err(AppError::Forbidden);
                }
                let actor = domain::sdlc::Principal {
                    subject: central.user_id,
                    human_session: central.session_id.is_some(),
                    trusted_human: false,
                    scopes: central.scopes,
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
                request.extensions_mut().insert(actor);
                return Ok(next.run(request).await);
            }
        }
        super::central_auth::CentralCheck::Unavailable => {
            return Err(AppError::Unavailable("Central Auth unavailable".into()));
        }
        super::central_auth::CentralCheck::Expired => return Err(AppError::Unauthorized),
        super::central_auth::CentralCheck::FallThrough => (),
    }
    // Preserve existing human/legacy authentication. No machine-to-UserClaims materialization.
    super::auth::bearer_auth(State(ctx), request, next)
        .await
        .map_err(|status| match status {
            StatusCode::UNAUTHORIZED => AppError::Unauthorized,
            StatusCode::FORBIDDEN => AppError::Forbidden,
            StatusCode::SERVICE_UNAVAILABLE => {
                AppError::Unavailable("authentication unavailable".into())
            }
            _ => AppError::internal("attachment authentication failed"),
        })
}
