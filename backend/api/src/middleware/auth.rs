use axum::{
    extract::{OriginalUri, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

pub async fn bearer_auth(
    State(ctx): State<Arc<app::AppContext>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let token: String = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| {
            auth.strip_prefix("Bearer ")
                .or_else(|| auth.strip_prefix("bearer "))
                .map(str::to_string)
        })
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let policy_active = ctx
        .repos
        .projects
        .space_admission_enabled()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    // Axum nest strips /api/v1 from the working URI before route middleware.
    // Classify the original registered operation, not the stripped inner path.
    let path = req
        .extensions()
        .get::<OriginalUri>()
        .map(|uri| uri.0.path())
        .unwrap_or_else(|| req.uri().path());
    let operation = request_operation(req.method(), path);
    // Central profile and role are fresh; a signed role hint or PAT principal must
    // not be silently converted into a human administrator during JIT.
    match super::central_auth::BRIDGE.try_verified_token(&token).await {
        Ok(verified) => {
            return verified_request(
                ctx,
                req,
                next,
                verified,
                policy_active,
                operation,
                CentralProfileMode::Provision,
            )
            .await;
        }
        Err(pdlc_auth_core::service_bridge::VerifiedBridgeError::NotConfigured) => {
            if policy_active {
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }
        }
        Err(pdlc_auth_core::service_bridge::VerifiedBridgeError::NotOurs) => {
            if policy_active || std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_some() {
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
        Err(pdlc_auth_core::service_bridge::VerifiedBridgeError::Unavailable) => {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        Err(_) => return Err(StatusCode::UNAUTHORIZED),
    }
    if policy_active || std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_some() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let claims = ctx
        .services
        .auth
        .verify_token(token.as_str())
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Reject tokens belonging to deactivated accounts. Without this check a
    // user disabled by an admin could keep using previously issued tokens.
    let user_id: shared::UserId = claims
        .sub
        .parse()
        .map(shared::UserId::from_uuid)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let user = ctx
        .repos
        .users
        .get_by_id(user_id)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    if !user.is_active {
        return Err(StatusCode::UNAUTHORIZED);
    }

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

/// Resolves a central subject independently of historical local email rows.
async fn find_or_link_central_user(
    ctx: &Arc<app::AppContext>,
    central: &pdlc_auth_core::AuthContext,
    display_name: Option<&str>,
) -> Result<domain::User, shared::AppError> {
    // Older Auth PAT introspection verifies identity/scopes without display
    // metadata. Reuse only an existing exact-subject active profile; never
    // provision, rename or promote a user from missing metadata.
    let Some(display_name) = display_name else {
        let mut profiles = ctx
            .repos
            .users
            .central_profiles(&[central.user_id.clone()])
            .await?;
        return profiles
            .remove(&central.user_id)
            .filter(|user| user.is_active)
            .ok_or(shared::AppError::Unauthorized);
    };
    let email = central.email.as_deref().unwrap_or_default().to_lowercase();
    let email = email.trim();
    if email.is_empty() {
        return Err(shared::AppError::Unauthorized);
    }
    ctx.repos
        .users
        .find_or_create_central_user(&central.user_id, email, display_name)
        .await
}

pub use app::auth::UserClaims;

#[derive(Clone)]
struct ResponseDeadline {
    valid_until: i64,
    deadline: std::time::Instant,
}
/// Only a verified owner-result adapter supplies this in-process extension.
/// Raw response headers never become authority or extend the request window.
pub(crate) fn restrict_response(
    response: &mut Response,
    valid_until: i64,
    deadline: std::time::Instant,
) {
    let previous = response.extensions_mut().remove::<ResponseDeadline>();
    response.extensions_mut().insert(ResponseDeadline {
        valid_until: previous
            .as_ref()
            .map_or(valid_until, |old| old.valid_until.min(valid_until)),
        deadline: previous.map_or(deadline, |old| old.deadline.min(deadline)),
    });
}
fn response_deadline(
    response: &mut Response,
    until: i64,
    deadline: std::time::Instant,
) -> (i64, std::time::Instant) {
    response
        .extensions_mut()
        .remove::<ResponseDeadline>()
        .map_or((until, deadline), |owner| {
            (until.min(owner.valid_until), deadline.min(owner.deadline))
        })
}

// A response can outlive its handler (SSE/downloads). Polling a delayed body may
// not renew or bypass the fixed request proof, even while the source is idle.
pub(crate) fn guard_admission_body(
    body: axum::body::Body,
    deadline: std::time::Instant,
    current: impl Fn() -> bool + Send + Sync + 'static,
) -> axum::body::Body {
    use futures_util::StreamExt;
    let stream = futures_util::stream::try_unfold(
        (body.into_data_stream(), current),
        move |(mut stream, current)| async move {
            let expired = || {
                std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "space_admission_expired",
                )
            };
            if !current() {
                return Err(expired());
            }
            let chunk = tokio::select! {biased; _=tokio::time::sleep_until(deadline.into())=>return Err(expired()),chunk=stream.next()=>chunk};
            if !current() {
                return Err(expired());
            }
            match chunk {
                Some(Ok(bytes)) => Ok(Some((bytes, (stream, current)))),
                Some(Err(error)) => Err(std::io::Error::other(error)),
                None => Ok(None),
            }
        },
    );
    axum::body::Body::from_stream(stream)
}
#[cfg(test)]
mod admission_body_tests {
    use super::*;
    #[tokio::test]
    async fn buffered_bytes_do_not_escape_an_expired_confirmation() {
        let body = guard_admission_body(
            axum::body::Body::from("private task text"),
            std::time::Instant::now() + std::time::Duration::from_secs(10),
            || false,
        );
        assert!(axum::body::to_bytes(body, 1000).await.is_err());
    }
    #[tokio::test]
    async fn idle_stream_closes_at_original_deadline() {
        let body = axum::body::Body::from_stream(futures_util::stream::pending::<
            Result<axum::body::Bytes, std::io::Error>,
        >());
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(20);
        let guarded = guard_admission_body(body, deadline, || true);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            axum::body::to_bytes(guarded, 1000),
        )
        .await
        .expect("idle stream must wake at deadline");
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod response_deadline_tests {
    use super::*;
    #[test]
    fn downstream_result_narrows_both_clocks_and_raw_headers_are_not_authority() {
        let base = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let earlier = base - std::time::Duration::from_secs(40);
        let mut response = Response::new(axum::body::Body::empty());
        response.headers_mut().insert(
            "x-pdlc-admission-valid-until",
            "9999999999".parse().unwrap(),
        );
        assert_eq!(response_deadline(&mut response, 160, base), (160, base));
        restrict_response(&mut response, 120, earlier);
        restrict_response(&mut response, 150, base);
        assert_eq!(response_deadline(&mut response, 160, base), (120, earlier));
        restrict_response(
            &mut response,
            999,
            base + std::time::Duration::from_secs(600),
        );
        assert_eq!(response_deadline(&mut response, 160, base), (160, base));
    }
}

/// Export uses POST only to carry a structured query. It never authorizes a write.
pub(crate) fn request_operation(
    method: &axum::http::Method,
    path: &str,
) -> app::space_access::RequestOperation {
    if matches!(method.as_str(), "GET" | "HEAD" | "OPTIONS")
        || (method == axum::http::Method::POST
            && matches!(path, "/api/v1/export/csv" | "/api/v1/export/json"))
    {
        app::space_access::RequestOperation::Read
    } else {
        app::space_access::RequestOperation::Write
    }
}

#[cfg(test)]
mod request_operation_tests {
    use super::request_operation;
    use app::space_access::RequestOperation::{Read, Write};
    use axum::http::Method;
    #[test]
    fn read_exports_do_not_turn_mutating_post_routes_into_reads() {
        for path in ["/api/v1/export/csv", "/api/v1/export/json"] {
            assert_eq!(request_operation(&Method::POST, path), Read);
            assert_eq!(request_operation(&Method::PUT, path), Write);
            assert_eq!(request_operation(&Method::DELETE, path), Write);
        }
        for path in [
            "/api/v1/projects",
            "/api/v1/issues",
            "/api/v1/export/json/other",
            "/api/v1/export/json-extra",
        ] {
            assert_eq!(request_operation(&Method::POST, path), Write);
            assert_eq!(request_operation(&Method::GET, path), Read);
        }
    }
}

/// Ordinary login may link a profile; strict SDLC requires the existing native
/// identity and must never enroll a user or update metadata while reading ACL.
#[derive(Clone, Copy)]
pub(crate) enum CentralProfileMode {
    Provision,
    Existing,
}

/// Reuse the original verified actor across ordinary and SDLC HTTP adapters.
pub(crate) async fn verified_request(
    ctx: Arc<app::AppContext>,
    mut req: Request,
    next: Next,
    verified: pdlc_auth_core::service_bridge::VerifiedCentralToken,
    policy_active: bool,
    operation: app::space_access::RequestOperation,
    profile_mode: CentralProfileMode,
) -> Result<Response, StatusCode> {
    let central = &verified.context;
    if app::sdlc::has_pm_grant(&central.scopes)
        || app::sdlc_lifecycle::has_delivery_grant(&central.scopes)
        || crate::routes::namespace::human_route_machine(&central.user_id)
        || verified.subject_role.as_deref() == Some("service_account")
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let scope_method = match operation {
        app::space_access::RequestOperation::Read => "GET",
        app::space_access::RequestOperation::Write => req.method().as_str(),
    };
    if !central.allows_service("task-tracker", scope_method) {
        return Err(StatusCode::FORBIDDEN);
    }
    let (user, denied) = match profile_mode {
        CentralProfileMode::Provision => (
            find_or_link_central_user(&ctx, central, verified.display_name.as_deref())
                .await
                .map_err(|_| StatusCode::UNAUTHORIZED)?,
            StatusCode::UNAUTHORIZED,
        ),
        CentralProfileMode::Existing => {
            let mut profiles = ctx
                .repos
                .users
                .central_profiles(&[central.user_id.clone()])
                .await
                .map_err(|error| error.into_response().status())?;
            (
                profiles
                    .remove(&central.user_id)
                    .ok_or(StatusCode::FORBIDDEN)?,
                StatusCode::FORBIDDEN,
            )
        }
    };
    if !user.is_active {
        return Err(denied);
    }
    let space_actor = if policy_active {
        let refs = ctx
            .repos
            .projects
            .space_catalog_refs()
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let proof = crate::routes::project_spaces::client()
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .confirm_many_read(&super::central_auth::BRIDGE, &verified, &refs)
            .await
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        Some(
            app::space_access::SpaceActor::from_read_proof(user.id, &verified, &proof)
                .map_err(|_| StatusCode::FORBIDDEN)?,
        )
    } else {
        None
    };
    let frame = Arc::new(
        app::space_access::CentralRequestContext::verified(
            user.id,
            &verified,
            space_actor,
            operation,
        )
        .map_err(|_| StatusCode::FORBIDDEN)?,
    );
    req.extensions_mut().insert(verified.clone());
    req.extensions_mut().insert(app::auth::UserClaims {
        sub: user.id.as_uuid().to_string(),
        exp: 0,
        typ: Some("access".into()),
        jti: None,
    });
    let response = tokio::time::timeout_at(
        frame.monotonic_deadline().into(),
        app::space_access::with_request_context(frame.clone(), next.run(req)),
    )
    .await
    .map_err(|_| StatusCode::FORBIDDEN)?;
    if !frame.is_current() {
        return Err(StatusCode::FORBIDDEN);
    }
    let mut response = response;
    let (valid_until, deadline) = response_deadline(
        &mut response,
        frame.valid_until(),
        frame.monotonic_deadline(),
    );
    response.headers_mut().insert(
        "x-pdlc-admission-valid-until",
        axum::http::HeaderValue::from_str(&valid_until.to_string())
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
    );
    if frame.spaces().is_some() {
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("private, no-store, no-transform"),
        );
    }
    let (parts, body) = response.into_parts();
    Ok(Response::from_parts(
        parts,
        guard_admission_body(body, deadline, move || frame.is_current()),
    ))
}
