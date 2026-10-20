//! Dedicated registered reader surface. It cannot mutate projects, issue jobs or JIT a user.
use axum::{
    Extension, Json,
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use pdlc_auth_core::service_bridge::RegisteredSpaceReader;
use shared::{AppError, project_spaces::*, space_access::*};
use std::sync::Arc;
pub async fn reader_auth(mut req: Request, next: Next) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;
    let verified = crate::middleware::central_auth::BRIDGE
        .try_verified_token(token)
        .await
        .map_err(|e| match e {
            pdlc_auth_core::service_bridge::VerifiedBridgeError::Unavailable
            | pdlc_auth_core::service_bridge::VerifiedBridgeError::NotConfigured => {
                AppError::Unavailable("central_reader_confirmation_unavailable".into())
            }
            _ => AppError::Unauthorized,
        })?;
    let reader = RegisteredSpaceReader::verified_machine(
        verified,
        "task-tracker",
        &std::env::var("TT_SPACES__CONTEXT_READER_SUBJECTS").unwrap_or_default(),
    )
    .map_err(|_| AppError::Forbidden)?;
    req.extensions_mut().insert(reader);
    Ok(next.run(req).await)
}
pub(crate) fn instance() -> Result<uuid::Uuid, AppError> {
    std::env::var("TT_NAMESPACE__INSTANCE_ID")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|id: &uuid::Uuid| !id.is_nil())
        .ok_or_else(|| AppError::Unavailable("project_context_not_configured".into()))
}
fn checked_admission(
    request: &ProjectSpaceContextQuery,
    admission: &SpaceAdmission,
) -> Result<(), AppError> {
    let now = chrono::Utc::now().timestamp();
    if !admission.valid()
        || !admission.identity_active
        || admission.identity != request.actor.identity
        || !admission
            .validity
            .valid_at(now, request.actor.validity.valid_until)
    {
        return Err(AppError::Forbidden);
    }
    if admission.policy_revision != request.policy_revision {
        return Err(AppError::conflict("space_policy_revision_changed"));
    }
    if let ProjectContextSelector::Space { space, .. } = &request.selector {
        if !admission.can_read_space(
            &request.actor.identity,
            space,
            now,
            request.actor.validity.valid_until,
        ) {
            return Err(AppError::Forbidden);
        }
    }
    Ok(())
}
fn visible(
    request: &ProjectSpaceContextQuery,
    admission: &SpaceAdmission,
    contexts: Vec<ProjectSpaceReadContext>,
    page_total: Option<u64>,
) -> Result<ProjectSpaceContextBatch, AppError> {
    checked_admission(request, admission)?;
    let now = chrono::Utc::now().timestamp();
    let mut validity = admission.validity;
    validity.observed_at = validity.observed_at.min(request.actor.validity.observed_at);
    validity.valid_until = validity.valid_until.min(request.actor.validity.valid_until);
    let mut projects = Vec::new();
    for item in contexts {
        let selected = match &request.selector {
            ProjectContextSelector::Projects { projects } => projects.contains(&item.links.project),
            ProjectContextSelector::Space { space, .. } => item.links.spaces.contains(space),
        };
        if !item.valid() || !selected {
            return Err(AppError::Unavailable(
                "invalid_project_context_snapshot".into(),
            ));
        }
        validity.observed_at = validity.observed_at.min(item.links.validity.observed_at);
        validity.valid_until = validity.valid_until.min(item.links.validity.valid_until);
        if admission.can_read_project(
            &request.actor.identity,
            &item.links.project,
            &item.links,
            now,
            request.actor.validity.valid_until,
        ) {
            projects.push(item);
        } else if matches!(request.selector, ProjectContextSelector::Space { .. }) {
            return Err(AppError::Unavailable(
                "project_context_page_visibility_changed".into(),
            ));
        }
    }
    let (total, has_more) = match &request.selector {
        ProjectContextSelector::Projects { .. } => (projects.len() as u64, false),
        ProjectContextSelector::Space { offset, .. } => {
            let total = page_total
                .ok_or_else(|| AppError::Unavailable("project_context_count_missing".into()))?;
            (total, (u64::from(*offset) + projects.len() as u64) < total)
        }
    };
    let result = ProjectSpaceContextBatch {
        schema_version: 1,
        actor: SpaceReadDelegation {
            schema_version: 1,
            identity: request.actor.identity.clone(),
            validity,
        },
        policy_revision: admission.policy_revision,
        total,
        has_more,
        projects,
    };
    if !result.valid_for(request, chrono::Utc::now().timestamp()) {
        return Err(AppError::Unavailable(
            "project_context_expired_or_invalid".into(),
        ));
    }
    Ok(result)
}
#[utoipa::path(operation_id="query_project_contexts_v2",post,path="/api/v2/project-contexts/query",request_body=ProjectSpaceContextQuery,responses((status=200,body=ProjectSpaceContextBatch),(status=403,description="Registered exact-scope reader and original actor admission required"),(status=409,description="Policy revision changed"),(status=400,description="Invalid identifier or command"),(status=401,description="Fresh central token required"),(status=404,description="Requested resource unavailable"),(status=422,description="Invalid strict request schema"),(status=503,description="Fresh owner confirmation unavailable")),security(("bearer"=[])))]
pub async fn query(
    State(ctx): State<Arc<app::AppContext>>,
    Extension(reader): Extension<RegisteredSpaceReader>,
    Json(request): Json<ProjectSpaceContextQuery>,
) -> Result<axum::response::Response, AppError> {
    let issuer = reader.issuer();
    if !reader.is_for("task-tracker")
        || !request
            .actor
            .validity
            .valid_at(chrono::Utc::now().timestamp(), i64::MAX)
        || !request.valid_for(issuer, instance()?)
    {
        return Err(AppError::invalid_input("invalid_project_context_query"));
    }
    let client = super::project_spaces::client().await?;
    let (proof, contexts, total) = match &request.selector {
        ProjectContextSelector::Space {
            space,
            limit,
            offset,
        } => {
            let proof = client
                .confirm_delegated_many_at_revision(
                    &crate::middleware::central_auth::BRIDGE,
                    &reader,
                    &request.actor,
                    &[space.clone()],
                    request.policy_revision,
                )
                .await
                .map_err(|_| {
                    AppError::Unavailable("original_space_admission_unavailable".into())
                })?;
            checked_admission(
                &request,
                proof.admission().map_err(|_| AppError::Forbidden)?,
            )?;
            let page = ctx
                .repos
                .projects
                .project_space_context_page(space, *limit, *offset)
                .await?;
            (proof, page.projects, Some(page.total))
        }
        ProjectContextSelector::Projects { projects } => {
            let contexts = ctx.repos.projects.project_space_contexts(projects).await?;
            if contexts.iter().map(|c| c.links.spaces.len()).sum::<usize>() > 10000 {
                return Err(AppError::invalid_input("project_context_batch_too_large"));
            }
            let mut spaces: Vec<_> = contexts
                .iter()
                .flat_map(|c| c.links.spaces.iter().cloned())
                .collect();
            spaces.sort_by_key(|s| (s.registry_instance_id, s.space_id));
            spaces.dedup();
            let proof = client
                .confirm_delegated_many_at_revision(
                    &crate::middleware::central_auth::BRIDGE,
                    &reader,
                    &request.actor,
                    &spaces,
                    request.policy_revision,
                )
                .await
                .map_err(|_| {
                    AppError::Unavailable("original_space_admission_unavailable".into())
                })?;
            (proof, contexts, None)
        }
    };
    if contexts.iter().map(|c| c.links.spaces.len()).sum::<usize>() > 10000 {
        return Err(AppError::invalid_input("project_context_batch_too_large"));
    }
    let result = visible(
        &request,
        proof.admission().map_err(|_| AppError::Forbidden)?,
        contexts,
        total,
    )?;
    if serde_json::to_vec(&result)
        .map_err(|_| AppError::Internal("project_context_serialization_failed".into()))?
        .len()
        > 1024 * 1024
    {
        return Err(AppError::invalid_input(
            "project_context_response_too_large",
        ));
    }

    if !reader.is_for("task-tracker") {
        return Err(AppError::Forbidden);
    }
    use axum::response::IntoResponse;
    if !proof.is_current() {
        return Err(AppError::Forbidden);
    }
    let deadline = proof.monotonic_deadline();
    let mut response = Json(result).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("private, no-store"),
    );
    let (parts, body) = response.into_parts();
    Ok(axum::response::Response::from_parts(
        parts,
        crate::middleware::auth::guard_admission_body(body, deadline, move || proof.is_current()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (
        ProjectSpaceContextQuery,
        SpaceAdmission,
        Vec<ProjectSpaceReadContext>,
    ) {
        let now = chrono::Utc::now().timestamp();
        let validity = AdmissionValidity::confirmed(now - 20, now + 20, i64::MAX).unwrap();
        let identity = CentralIdentity {
            issuer: "https://auth.example.test".into(),
            subject: "original-user".into(),
        };
        let a = SpaceRef {
            registry_instance_id: uuid::Uuid::new_v4(),
            space_id: uuid::Uuid::new_v4(),
        };
        let b = SpaceRef {
            space_id: uuid::Uuid::new_v4(),
            ..a.clone()
        };
        let tracker = uuid::Uuid::new_v4();
        let contexts: Vec<_> = [vec![a.clone(), b.clone()], vec![b.clone()]]
            .into_iter()
            .map(|spaces| {
                let project = ProjectRef {
                    tracker_instance_id: tracker,
                    project_id: uuid::Uuid::new_v4(),
                };
                ProjectSpaceReadContext {
                    lifecycle: shared::project_spaces::ProjectLifecycle {
                        schema_version: 1,
                        project: project.clone(),
                        revision: 1,
                        archived: false,
                    },
                    key: "SAME".into(),
                    name: "Одинаковое имя".into(),
                    links: ConfirmedProjectSpaces {
                        project,
                        revision: 1,
                        spaces,
                        validity,
                    },
                }
            })
            .collect();
        let query = ProjectSpaceContextQuery {
            schema_version: 1,
            actor: SpaceReadDelegation {
                schema_version: 1,
                identity: identity.clone(),
                validity,
            },
            policy_revision: 7,
            tracker_instance_id: tracker,
            selector: ProjectContextSelector::Projects {
                projects: contexts.iter().map(|c| c.links.project.clone()).collect(),
            },
        };
        let admission = SpaceAdmission {
            schema_version: 1,
            identity,
            identity_active: true,
            policy_revision: 7,
            policy: SpaceAccessPolicy::Selected {
                spaces: vec![a.clone()],
            },
            validity,
            spaces: vec![
                ConfirmedSpace {
                    space: a,
                    revision: 1,
                    state: SpaceState::Archived,
                },
                ConfirmedSpace {
                    space: b,
                    revision: 1,
                    state: SpaceState::Active,
                },
            ],
        };
        (query, admission, contexts)
    }
    #[test]
    fn original_actor_sees_shared_project_but_not_hidden_b_only_project() {
        let (query, admission, contexts) = fixture();
        let first = contexts[0].links.project.clone();
        let result = visible(&query, &admission, contexts, None).unwrap();
        assert_eq!(result.projects.len(), 1);
        assert_eq!(result.projects[0].links.project, first);
        assert_eq!(result.actor.validity, query.actor.validity);
    }
    #[test]
    fn empty_policy_and_inactive_identity_never_become_all() {
        let (query, mut admission, contexts) = fixture();
        admission.policy = SpaceAccessPolicy::Selected { spaces: vec![] };
        assert!(
            visible(&query, &admission, contexts.clone(), None)
                .unwrap()
                .projects
                .is_empty()
        );
        admission.identity_active = false;
        assert!(visible(&query, &admission, contexts, None).is_err());
    }
    #[test]
    fn policy_change_foreign_actor_and_expired_source_fail_closed() {
        let (mut query, mut admission, contexts) = fixture();
        admission.policy_revision += 1;
        assert!(matches!(
            visible(&query, &admission, contexts.clone(), None),
            Err(AppError::Conflict(_))
        ));
        admission.policy_revision -= 1;
        admission.identity.subject = "another-original-user".into();
        assert!(visible(&query, &admission, contexts.clone(), None).is_err());
        admission.identity = query.actor.identity.clone();
        query.actor.validity = AdmissionValidity::confirmed(
            chrono::Utc::now().timestamp() - 70,
            chrono::Utc::now().timestamp() - 10,
            i64::MAX,
        )
        .unwrap();
        assert!(visible(&query, &admission, contexts, None).is_err());
    }
    #[test]
    fn space_selector_requires_that_space_and_keeps_counts_before_pagination() {
        let (mut query, admission, contexts) = fixture();
        let a = contexts[0].links.spaces[0].clone();
        let b = contexts[1].links.spaces[0].clone();
        query.selector = ProjectContextSelector::Space {
            space: a,
            limit: 1,
            offset: 0,
        };
        let result = visible(&query, &admission, vec![contexts[0].clone()], Some(1)).unwrap();
        assert_eq!(result.total, 1);
        assert!(!result.has_more);
        query.selector = ProjectContextSelector::Space {
            space: b,
            limit: 1,
            offset: 0,
        };
        assert!(
            matches!(
                visible(&query, &admission, vec![contexts[0].clone()], Some(1)),
                Err(AppError::Forbidden)
            ),
            "access via A to A+B never opens B's collection"
        );
    }
}
