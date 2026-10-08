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
    let central = match super::central_auth::check_token(token).await {
        super::central_auth::CentralCheck::Validated(central, _verified_name) => central,
        super::central_auth::CentralCheck::Unavailable => {
            return Err(AppError::Unavailable("Central Auth unavailable".into()));
        }
        _ => return Err(AppError::Unauthorized),
    };
    if !central.allows_service("task-tracker", request.method().as_str()) {
        return Err(AppError::Forbidden);
    }
    if crate::routes::namespace::registered_machine(&central.user_id) {
        return Err(AppError::Forbidden);
    }
    let configured_machine = [
        "TASKTRACKER_SDLC__ORCHESTRATOR_SUBJECT",
        "TASKTRACKER_SDLC__VERIFIER_SUBJECT",
        "TASKTRACKER_SDLC__RESERVATION_SCHEDULER_SUBJECT",
    ]
    .iter()
    .any(|key| {
        std::env::var(key).is_ok_and(|subject| !subject.is_empty() && subject == central.user_id)
    });
    let trusted_human = is_trusted_human(&central, configured_machine);
    let actor = domain::sdlc::Principal {
        subject: central.user_id,
        human_session: central.session_id.is_some(),
        trusted_human,
        scopes: central.scopes,
    };
    let path = request
        .extensions()
        .get::<OriginalUri>()
        .map_or_else(|| request.uri().path(), |uri| uri.0.path());
    if (configured_machine || central.role.as_deref() == Some("service_account"))
        && matches!(
            path,
            "/api/v1/sdlc/project-directory" | "/api/v1/sdlc/project-access"
        )
    {
        return Err(AppError::Forbidden);
    }
    pm_request_policy(&actor, request.method().as_str(), path)?;
    request.extensions_mut().insert(actor);
    Ok(next.run(request).await)
}

fn is_trusted_human(central: &sdlc_auth_core::AuthContext, configured_machine: bool) -> bool {
    !configured_machine
        && central.role.as_deref() != Some("service_account")
        && (central.session_id.is_some()
            // Auth's verified personal-token response identifies a user and
            // scopes, but intentionally does not carry a role or browser SID.
            || central.token.starts_with("sdlc_pat_")
            || matches!(central.role.as_deref(), Some("admin" | "member" | "user")))
        && !app::sdlc::has_pm_grant(&central.scopes)
}

#[cfg(test)]
mod human_identity_tests {
    use super::*;
    #[test]
    fn verified_personal_identity_preserves_machine_and_owner_session_boundaries() {
        let mut central = sdlc_auth_core::AuthContext {
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
    }
}

fn pm_request_policy(
    actor: &domain::sdlc::Principal,
    method: &str,
    path: &str,
) -> Result<(), AppError> {
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
        _ => false,
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
            "clarifications/00000000-0000-0000-0000-000000000000/cancel",
            "requirements/01",
            "requirements/0",
            "requirements/9007199254740992",
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
