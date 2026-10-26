use axum::{
    extract::{OriginalUri, Request},
    middleware::Next,
    response::Response,
};
use shared::AppError;

/// Separate route layer: SDLC never inherits local-token fallback or project bypass.
pub async fn strict_central_auth(mut request: Request, next: Next) -> Result<Response, AppError> {
    let token = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| {
            h.strip_prefix("Bearer ")
                .or_else(|| h.strip_prefix("bearer "))
        })
        .ok_or(AppError::Unauthorized)?;
    let verified = match request
        .extensions()
        .get::<pdlc_auth_core::service_bridge::VerifiedCentralToken>()
    {
        Some(proof) if proof.lifetime.is_current() => proof.clone(),
        Some(_) => return Err(AppError::Unauthorized),
        None => super::central_auth::BRIDGE
            .try_verified_token(token)
            .await
            .map_err(|error| match error {
                pdlc_auth_core::service_bridge::VerifiedBridgeError::Unavailable
                | pdlc_auth_core::service_bridge::VerifiedBridgeError::NotConfigured => {
                    AppError::Unavailable("Central Auth unavailable".into())
                }
                pdlc_auth_core::service_bridge::VerifiedBridgeError::Forbidden => {
                    AppError::Forbidden
                }
                _ => AppError::Unauthorized,
            })?,
    };
    let central = verified.context.clone();
    if !central.allows_service("task-tracker", request.method().as_str()) {
        return Err(AppError::Forbidden);
    }
    if crate::routes::namespace::registered_machine(&central.user_id) {
        return Err(AppError::Forbidden);
    }
    let configured_machine = registered_sdlc_subject(&central.user_id)
        || verified.subject_role.as_deref() == Some("service_account");
    let configured_cli_grant = central
        .scopes
        .iter()
        .any(|scope| scope.starts_with(app::sdlc_configured_cli::AGENT_SCOPE_PREFIX));
    if (configured_machine
        || configured_cli_grant
        || app::sdlc::has_pm_grant(&central.scopes)
        || app::sdlc_lifecycle::has_delivery_grant(&central.scopes))
        && verified.subject_role.as_deref() != Some("service_account")
    {
        return Err(AppError::Forbidden);
    }
    let trusted_human = is_trusted_human(&central, configured_machine);
    let human_session = is_human_session(&central, configured_machine);
    let actor = domain::sdlc::Principal {
        subject: central.user_id,
        human_session,
        trusted_human,
        scopes: central.scopes,
    };
    let path = request
        .extensions()
        .get::<OriginalUri>()
        .map_or_else(|| request.uri().path(), |uri| uri.0.path());
    // V1 directory reads retain explicit native owner/member ACL. Generic
    // machines remain native principals; assignment-scoped grants are rejected
    // by their exact request policy below and never become user claims.
    if trusted_human && requires_fleet_application(request.method().as_str(), path) {
        verified
            .require_applications(
                &["task-tracker", "fleet-control"],
                request.method().as_str(),
            )
            .map_err(|_| AppError::Forbidden)?;
    }
    pm_request_policy(&actor, request.method().as_str(), path)?;
    app::sdlc_lifecycle::delivery_request_policy(&actor, request.method().as_str(), path)?;
    request.extensions_mut().insert(actor);
    Ok(next.run(request).await)
}

/// Only mutations that assign or advance Fleet execution need the second app.
/// Ordinary Tracker history, questions and requirements review remain Tracker work.
fn requires_fleet_application(method: &str, path: &str) -> bool {
    if matches!(method, "GET" | "HEAD" | "OPTIONS") {
        return false;
    }
    let Some((_, action)) = path.split_once("/sdlc/") else {
        return false;
    };
    matches!(
        action,
        "assignment" | "pm-draft-assignment" | "pm-draft-execution-lease"
    ) || action.starts_with("pm-draft-execution-lease/")
}

#[cfg(test)]
mod chained_application_routes {
    use super::requires_fleet_application;
    #[test]
    fn dispatch_and_resume_need_fleet_but_neighboring_tracker_work_does_not() {
        for id in [
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
        ] {
            for action in [
                "assignment",
                "pm-draft-assignment",
                "pm-draft-execution-lease",
                "pm-draft-execution-lease/resume",
                "pm-draft-execution-lease/retry-bootstrap",
                "pm-draft-execution-lease/recover-unsubmitted",
            ] {
                let path = format!("/api/v1/issues/{id}/sdlc/{action}");
                assert!(requires_fleet_application("POST", &path));
                assert!(!requires_fleet_application("GET", &path));
            }
            for action in [
                "context",
                "binding",
                "clarifications",
                "requirements/2/confirm",
                "metadata",
                "assignment-note",
            ] {
                assert!(!requires_fleet_application(
                    "POST",
                    &format!("/api/v1/issues/{id}/sdlc/{action}")
                ));
            }
        }
    }
}

fn is_human_session(central: &pdlc_auth_core::AuthContext, configured_machine: bool) -> bool {
    central.session_id.is_some()
        && !configured_machine
        && central.role.as_deref() != Some("service_account")
}

fn is_trusted_human(central: &pdlc_auth_core::AuthContext, configured_machine: bool) -> bool {
    !configured_machine
        && central.role.as_deref() != Some("service_account")
        && (central.session_id.is_some()
            // Auth's verified personal-token response identifies a user and
            // scopes, but intentionally does not carry a role or browser SID.
            || pdlc_auth_core::is_personal_token(&central.token)
            || matches!(central.role.as_deref(), Some("admin" | "member" | "user")))
        && !app::sdlc::has_pm_grant(&central.scopes)
        && !app::sdlc_lifecycle::has_delivery_grant(&central.scopes)
}

#[cfg(test)]
mod human_identity_tests {
    use super::*;
    #[test]
    fn verified_personal_identity_preserves_machine_and_owner_session_boundaries() {
        let mut central = pdlc_auth_core::AuthContext {
            user_id: "verified-user".into(),
            role: None,
            scopes: ["task-tracker:read".into()].into(),
            session_id: None,
            email: Some("human@example.test".into()),
            token: "sdlc_pat_verified".into(),
        };
        assert!(is_trusted_human(&central, false));
        assert!(central.session_id.is_none());
        assert!(!is_trusted_human(&central, true));
        central.role = Some("service_account".into());
        assert!(!is_trusted_human(&central, false));
        central.role = None;
        central
            .scopes
            .insert(format!("{}task-grant", app::sdlc::PM_GRANT_PREFIX));
        assert!(!is_trusted_human(&central, false));
        central.scopes.clear();
        central.token = "untyped-token".into();
        assert!(!is_trusted_human(&central, false));
        central.session_id = Some("verified-session".into());
        assert!(is_trusted_human(&central, false));
        assert!(is_human_session(&central, false));
        assert!(!is_human_session(&central, true));
        central.role = Some("service_account".into());
        assert!(!is_human_session(&central, false));
        central.role = None;
        central.session_id = None;
        assert!(!is_human_session(&central, false));
    }
}

#[cfg(test)]
#[path = "sdlc_critical_pm_read_tests.rs"]
mod critical_pm_read_tests;

fn pm_request_policy(
    actor: &domain::sdlc::Principal,
    method: &str,
    path: &str,
) -> Result<(), AppError> {
    if app::sdlc_configured_cli::configured_agent(actor)?.is_some() {
        let tail = path
            .strip_prefix("/api/v1/issues/")
            .ok_or(AppError::Forbidden)?;
        let (task, action) = tail.split_once("/sdlc/").ok_or(AppError::Forbidden)?;
        app::sdlc::canonical_pm_uuid(task).ok_or(AppError::Forbidden)?;
        let parts: Vec<_> = action.split('/').collect();
        let allowed = match (method, parts.as_slice()) {
            (
                "GET",
                [
                    "context"
                    | "pm-draft-input"
                    | "clarifications"
                    | "requirements"
                    | "events"
                    | "pm-draft-assignment"
                    | "pm-draft-completion",
                ],
            )
            | ("GET", ["requirements", "revisions"])
            | ("POST", ["clarifications" | "requirements" | "pm-draft-completion"]) => true,
            ("GET", ["requirements", revision]) | ("GET", ["requirements", revision, "diff"]) => {
                app::sdlc::canonical_pm_version(revision)
            }
            ("POST", ["clarifications", question, "cancel"]) => {
                app::sdlc::canonical_pm_uuid(question).is_some()
            }
            ("GET", ["pm-draft-completion", "operations", key]) => {
                app::sdlc::validate_key(key).is_ok()
            }
            _ => app::sdlc_confirmation::is_confirmation_read(method, &parts),
        };
        return if allowed {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        };
    }
    let Some(task) = app::sdlc::pm_grant_task(actor)? else {
        return Ok(());
    };
    let prefix = format!("/api/v1/issues/{task}/sdlc/");
    let resource = path.strip_prefix(&prefix).ok_or(AppError::Forbidden)?;
    let parts: Vec<_> = resource.split('/').collect();
    let allowed = match (method, parts.as_slice()) {
        (
            "GET",
            [
                "context"
                | "pm-draft-input"
                | "clarifications"
                | "requirements"
                | "events"
                | "pm-draft-execution-lease",
            ],
        )
        | ("GET", ["requirements", "revisions"])
        | ("POST", ["clarifications" | "requirements" | "pm-draft-execution-lease"])
        | ("POST", ["pm-draft-execution-lease", "heartbeat"]) => true,
        ("GET", ["requirements", revision]) | ("GET", ["requirements", revision, "diff"]) => {
            app::sdlc::canonical_pm_version(revision)
        }
        ("POST", ["clarifications", question, "cancel"]) => {
            app::sdlc::canonical_pm_uuid(question).is_some()
        }
        _ => {
            app::sdlc_confirmation::is_confirmation_read(method, parts.as_slice())
                || (actor.scopes.contains("task-tracker:read")
                    && app::sdlc_critical_blockers::is_pm_critical_blocker_read(
                        method,
                        parts.as_slice(),
                    ))
                || (actor.scopes.contains("task-tracker:read")
                    && app::sdlc_lifecycle::is_pm_feedback_message_read(method, parts.as_slice()))
        }
    };
    if allowed {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::sdlc::PmAssignment;
    use uuid::Uuid;

    fn actor() -> (domain::sdlc::Principal, Uuid, String) {
        let task = Uuid::new_v4();
        let assignment = PmAssignment {
            assignment_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            agent_id: Uuid::new_v4(),
            version: 1,
            machine_subject: "pm".into(),
        };
        let scope = assignment.scope(task);
        (
            domain::sdlc::Principal {
                subject: "pm".into(),
                human_session: false,
                trusted_human: false,
                scopes: [scope.clone()].into(),
            },
            task,
            scope,
        )
    }

    #[test]
    fn pm_policy_allows_only_exact_task_methods_and_resources() {
        let (actor, task, _) = actor();
        let path = format!("/api/v1/issues/{task}/sdlc/");
        for resource in [
            "context",
            "pm-draft-input",
            "clarifications",
            "requirements",
            "requirements/revisions",
            "requirements/1",
            "requirements/1/diff",
            "requirements/1/confirmation",
            "events",
            "pm-draft-execution-lease",
        ] {
            assert!(pm_request_policy(&actor, "GET", &format!("{path}{resource}")).is_ok());
        }
        for resource in [
            "clarifications",
            "requirements",
            "pm-draft-execution-lease",
            "pm-draft-execution-lease/heartbeat",
        ] {
            assert!(pm_request_policy(&actor, "POST", &format!("{path}{resource}")).is_ok());
        }
        assert!(
            pm_request_policy(
                &actor,
                "POST",
                &format!("{path}clarifications/{}/cancel", Uuid::new_v4())
            )
            .is_ok()
        );
        for resource in [
            "binding",
            "assignment",
            "pm-draft-assignment",
            "evidence",
            "requirements/1/confirm",
            "pm-draft-execution-lease/resume",
            "pm-draft-execution-lease/resume/operations/original",
            "clarifications/00000000-0000-0000-0000-000000000000/cancel",
            "requirements/01",
            "requirements/0",
            "requirements/9007199254740992",
            "requirements/01/confirmation",
            "requirements/0/confirmation",
            "requirements/9007199254740992/confirmation",
            "context/",
        ] {
            for method in ["GET", "POST"] {
                assert!(pm_request_policy(&actor, method, &format!("{path}{resource}")).is_err());
            }
        }
        for method in ["HEAD", "PATCH", "DELETE", "PUT"] {
            assert!(pm_request_policy(&actor, method, &format!("{path}context")).is_err());
        }
        for path in [
            "/api/v1/sdlc/project-directory".into(),
            "/api/v1/sdlc/project-access".into(),
            format!("/api/v1/issues/{}/sdlc/context", Uuid::new_v4()),
            format!("/api/v1/issues/{task}/sdlc/context/../events"),
            format!(
                "/api/v1/issues/{}/sdlc/context",
                task.to_string().to_uppercase()
            ),
        ] {
            assert!(pm_request_policy(&actor, "GET", &path).is_err());
        }
    }

    #[test]
    fn pm_feedback_message_policy_is_exact_task_canonical_uuid_get_only() {
        let (mut actor, task, _) = actor();
        let message = Uuid::new_v4();
        let base = format!("/api/v1/issues/{task}/sdlc/pm-feedback");
        let path = format!("{base}/messages/{message}");
        assert!(pm_request_policy(&actor, "GET", &path).is_err());
        actor.scopes.insert("task-tracker:write".into());
        assert!(pm_request_policy(&actor, "GET", &path).is_err());
        actor.scopes.insert("task-tracker:read".into());
        assert!(pm_request_policy(&actor, "GET", &path).is_ok());
        for method in ["HEAD", "POST", "PUT", "PATCH", "DELETE"] {
            assert!(pm_request_policy(&actor, method, &path).is_err());
        }
        for path in [
            format!("{base}/messages"),
            format!("{base}/messages/{message}/"),
            format!("{base}/messages/{message}/extra"),
            format!("{base}/messages/{}", message.to_string().to_uppercase()),
            format!("{base}/messages/{}", Uuid::nil()),
            format!("{base}/messages/not-a-uuid"),
            format!("{base}/operations/owner-key"),
            format!(
                "/api/v1/issues/{}/sdlc/pm-feedback/messages/{message}",
                Uuid::new_v4()
            ),
        ] {
            assert!(pm_request_policy(&actor, "GET", &path).is_err(), "{path}");
        }
    }

    #[test]
    fn malformed_ambiguous_and_human_pm_grants_fail_closed() {
        let (mut actor, task, scope) = actor();
        for grant in [
            "task-tracker:sdlc:pm:".into(),
            scope
                .to_uppercase()
                .replacen("TASK-TRACKER:SDLC:PM:", app::sdlc::PM_GRANT_PREFIX, 1),
            format!("{scope}:extra"),
            scope.trim_end_matches('1').to_string() + "01",
            scope.replace(&task.to_string(), "00000000-0000-0000-0000-000000000000"),
        ] {
            actor.scopes = [grant].into();
            assert!(app::sdlc::has_pm_grant(&actor.scopes));
            assert!(app::sdlc::pm_grant_task(&actor).is_err());
        }
        actor.scopes = [scope.clone(), format!("{scope}0")].into();
        assert!(app::sdlc::pm_grant_task(&actor).is_err());
        actor.scopes = [scope].into();
        actor.human_session = true;
        assert!(app::sdlc::pm_grant_task(&actor).is_err());
    }

    #[test]
    fn generic_principals_keep_existing_request_policy() {
        let (mut actor, _, _) = actor();
        actor.scopes = ["task-tracker:read".into(), "task-tracker:write".into()].into();
        for human in [true, false] {
            actor.human_session = human;
            assert!(pm_request_policy(&actor, "GET", "/api/v1/sdlc/project-directory").is_ok());
        }
    }
}

fn registered_sdlc_subject(subject: &str) -> bool {
    [
        "TASKTRACKER_SDLC__ORCHESTRATOR_SUBJECT",
        "TASKTRACKER_SDLC__VERIFIER_SUBJECT",
        "TASKTRACKER_SDLC__RESERVATION_SCHEDULER_SUBJECT",
    ]
    .iter()
    .any(|key| {
        std::env::var(key).is_ok_and(|configured| !configured.is_empty() && configured == subject)
    })
}

/// The serving router mounts this outside strict SDLC classification. Typed native
/// capabilities keep their existing protocol; human operations share the ordinary
/// request frame, original clocks and transaction-local write guard.
pub async fn space_guard(
    axum::extract::State(ctx): axum::extract::State<std::sync::Arc<app::AppContext>>,
    request: Request,
    next: Next,
) -> Response {
    use axum::response::IntoResponse;
    let mut response = match space_guard_inner(ctx, request, next).await {
        Ok(response) => response,
        Err(status) => match status {
            axum::http::StatusCode::UNAUTHORIZED => AppError::Unauthorized.into_response(),
            axum::http::StatusCode::FORBIDDEN => AppError::Forbidden.into_response(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE => {
                AppError::Unavailable("authorization unavailable".into()).into_response()
            }
            axum::http::StatusCode::INTERNAL_SERVER_ERROR => {
                AppError::internal("authorization unavailable").into_response()
            }
            _ => status.into_response(),
        },
    };
    response
        .headers_mut()
        .entry(axum::http::header::CACHE_CONTROL)
        .or_insert(axum::http::HeaderValue::from_static("no-store"));
    response
}
async fn space_guard_inner(
    ctx: std::sync::Arc<app::AppContext>,
    mut request: Request,
    next: Next,
) -> Result<Response, axum::http::StatusCode> {
    use axum::http::StatusCode;
    let token = request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| {
            h.strip_prefix("Bearer ")
                .or_else(|| h.strip_prefix("bearer "))
        })
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let verified = super::central_auth::BRIDGE
        .try_verified_token(token)
        .await
        .map_err(|error| match error {
            pdlc_auth_core::service_bridge::VerifiedBridgeError::Unavailable
            | pdlc_auth_core::service_bridge::VerifiedBridgeError::NotConfigured => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            pdlc_auth_core::service_bridge::VerifiedBridgeError::Forbidden => StatusCode::FORBIDDEN,
            _ => StatusCode::UNAUTHORIZED,
        })?;
    let native = registered_sdlc_subject(&verified.context.user_id)
        || verified.subject_role.as_deref() == Some("service_account")
        || app::sdlc::has_pm_grant(&verified.context.scopes)
        || app::sdlc_lifecycle::has_delivery_grant(&verified.context.scopes);
    if native {
        let mutation = !matches!(request.method().as_str(), "GET" | "HEAD" | "OPTIONS");
        let scope = app::native_space_access::NativeRequest::verified(&verified, native, mutation)
            .map_err(|_| StatusCode::FORBIDDEN)?;
        let response_scope = scope.clone();
        let deadline = scope.deadline();
        request.extensions_mut().insert(verified.clone());
        let mut response = tokio::time::timeout_at(
            deadline.into(),
            app::native_space_access::with_request(scope, next.run(request)),
        )
        .await
        .map_err(|_| StatusCode::FORBIDDEN)?;
        if !verified.lifetime.is_current() {
            return Err(StatusCode::UNAUTHORIZED);
        }
        let validity = response_scope
            .validity()
            .map_err(|_| StatusCode::FORBIDDEN)?;
        super::auth::restrict_response(
            &mut response,
            validity.valid_until,
            response_scope.deadline(),
        );
        return Ok(response);
    }
    let policy = ctx
        .repos
        .projects
        .space_admission_enabled()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let path = request
        .extensions()
        .get::<OriginalUri>()
        .map_or_else(|| request.uri().path(), |uri| uri.0.path());
    let operation = super::auth::request_operation(request.method(), path);
    super::auth::verified_request(
        ctx,
        request,
        next,
        verified,
        policy,
        operation,
        super::auth::CentralProfileMode::Existing,
    )
    .await
}
