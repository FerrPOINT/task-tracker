//! Request-owned central proof. Visibility never grants product edit/admin roles.
use domain::project_spaces::SpaceWritePermit;
use pdlc_auth_core::service_bridge::VerifiedCentralToken;
use shared::{AppError, UserId, space_access::*, space_lifecycle::SpaceOwnerAuthority};
use std::sync::Arc;
pub static CENTRAL_BRIDGE: pdlc_auth_core::service_bridge::ServiceBridge =
    pdlc_auth_core::service_bridge::ServiceBridge::new("TT_AUTH__CENTRAL");
static CLIENT: tokio::sync::OnceCell<pdlc_auth_core::service_bridge::SpaceAdmissionClient> =
    tokio::sync::OnceCell::const_new();
pub async fn admission_client()
-> Result<&'static pdlc_auth_core::service_bridge::SpaceAdmissionClient, AppError> {
    CLIENT
        .get_or_try_init(|| async {
            let url = std::env::var("TT_SPACES__ADMISSION_URL")
                .map_err(|_| AppError::Unavailable("space_admission_not_configured".into()))?;
            let token = std::env::var("TT_SPACES__ADMISSION_TOKEN")
                .map_err(|_| AppError::Unavailable("space_admission_not_configured".into()))?;
            pdlc_auth_core::service_bridge::SpaceAdmissionClient::new(&url, token)
                .map_err(|_| AppError::Unavailable("space_admission_configuration_invalid".into()))
        })
        .await
}
static CATALOG_CLIENT: tokio::sync::OnceCell<pdlc_auth_core::service_bridge::SpaceCatalogClient> =
    tokio::sync::OnceCell::const_new();
pub async fn catalog_client()
-> Result<&'static pdlc_auth_core::service_bridge::SpaceCatalogClient, AppError> {
    CATALOG_CLIENT
        .get_or_try_init(|| async {
            let url = std::env::var("TT_SPACES__CATALOG_URL")
                .map_err(|_| AppError::Unavailable("space_catalog_not_configured".into()))?;
            let token = std::env::var("TT_SPACES__ADMISSION_TOKEN")
                .map_err(|_| AppError::Unavailable("space_catalog_not_configured".into()))?;
            pdlc_auth_core::service_bridge::SpaceCatalogClient::new(&url, token, "task-tracker")
                .map_err(|_| AppError::Unavailable("space_catalog_configuration_invalid".into()))
        })
        .await
}
/// Created once for one delivery cycle. This registration grants metadata read
/// processing only, never a synthetic user session or an execution capability.
pub async fn notification_reader()
-> Result<pdlc_auth_core::service_bridge::RegisteredSpaceReader, AppError> {
    let token = std::env::var("TT_SPACES__ADMISSION_TOKEN")
        .map_err(|_| AppError::Unavailable("space_admission_not_configured".into()))?;
    let registration = std::env::var("TT_SPACES__ADMISSION_READER_SUBJECTS")
        .map_err(|_| AppError::Unavailable("notification_reader_not_registered".into()))?;
    let actor = CENTRAL_BRIDGE
        .try_verified_token(&token)
        .await
        .map_err(|_| AppError::Unavailable("notification_reader_not_confirmed".into()))?;
    pdlc_auth_core::service_bridge::RegisteredSpaceReader::verified_machine(
        actor,
        "admin-panel",
        &registration,
    )
    .map_err(|_| AppError::Forbidden)
}

#[derive(Debug, Clone)]
pub struct SpaceActor {
    user: UserId,
    identity: CentralIdentity,
    window: SpaceOwnerAuthority,
    admission: SpaceAdmission,
    may_write: bool,
}
impl SpaceActor {
    /// API verifies the immutable central-subject/local-profile mapping first.
    pub fn verified(
        user: UserId,
        actor: &VerifiedCentralToken,
        mut admission: SpaceAdmission,
    ) -> Result<Self, AppError> {
        let identity = CentralIdentity {
            issuer: actor.issuer.clone(),
            subject: actor.context.user_id.clone(),
        };
        let expiry = actor.lifetime.local_valid_until().min(
            actor
                .lifetime
                .source_observed_at
                .checked_add(AdmissionValidity::MAX_AGE_SECONDS)
                .ok_or(AppError::Forbidden)?,
        );
        admission.validity = admission
            .validity
            .attenuate(expiry)
            .ok_or(AppError::Forbidden)?;
        if !actor.lifetime.is_current()
            || !admission.valid()
            || !admission.identity_active
            || admission.identity != identity
        {
            return Err(AppError::Forbidden);
        }
        let window = SpaceOwnerAuthority::confirmed(identity.clone(), admission.validity)
            .ok_or(AppError::Forbidden)?
            .restrict_deadline(actor.lifetime.monotonic_deadline());
        Ok(Self {
            user,
            identity,
            window,
            admission,
            may_write: actor.context.allows_service("task-tracker", "PATCH"),
        })
    }
    pub fn user(&self) -> UserId {
        self.user
    }
    pub fn identity(&self) -> &CentralIdentity {
        &self.identity
    }
    pub fn is_current(&self) -> bool {
        self.window.is_current()
            && self
                .admission
                .validity
                .valid_at(chrono::Utc::now().timestamp(), self.window.valid_until())
    }
    pub fn valid_until(&self) -> i64 {
        self.window.valid_until()
    }
    /// A later verification can only shorten the original request's monotonic window.
    pub fn restrict_deadline(mut self, deadline: std::time::Instant) -> Self {
        self.window = self.window.restrict_deadline(deadline);
        self
    }
    pub fn require_read(&self, links: &ConfirmedProjectSpaces) -> Result<(), AppError> {
        if self.is_current()
            && self.admission.can_read_project(
                &self.identity,
                &links.project,
                links,
                chrono::Utc::now().timestamp(),
                self.window.valid_until(),
            )
        {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
    /// Canonical link metadata does not renew the original caller's admission.
    pub fn confirm_links(
        &self,
        mut links: ConfirmedProjectSpaces,
    ) -> Result<ConfirmedProjectSpaces, AppError> {
        self.require_read(&links)?;
        links.validity = AdmissionValidity {
            observed_at: links
                .validity
                .observed_at
                .min(self.admission.validity.observed_at),
            valid_until: links.validity.valid_until.min(self.valid_until()),
        };
        if !links.valid() || !self.is_current() {
            return Err(AppError::Forbidden);
        }
        Ok(links)
    }
    pub fn read_validity(&self) -> Result<AdmissionValidity, AppError> {
        if !self.is_current() {
            return Err(AppError::Forbidden);
        }
        self.admission
            .validity
            .attenuate(self.valid_until())
            .ok_or(AppError::Forbidden)
    }
    pub fn monotonic_deadline(&self) -> std::time::Instant {
        self.window.monotonic_deadline()
    }
    pub fn policy_revision(&self) -> i64 {
        self.admission.policy_revision
    }
    pub fn active_spaces(&self) -> Result<Vec<SpaceRef>, AppError> {
        if !self.is_current() {
            return Err(AppError::Forbidden);
        }
        Ok(self
            .admission
            .spaces
            .iter()
            .filter(|s| {
                self.admission.has_active_space(
                    &self.identity,
                    &s.space,
                    chrono::Utc::now().timestamp(),
                    self.valid_until(),
                )
            })
            .map(|s| s.space.clone())
            .collect())
    }
    pub fn readable_spaces(&self) -> Result<Vec<SpaceRef>, AppError> {
        if !self.is_current() {
            return Err(AppError::Forbidden);
        }
        Ok(self
            .admission
            .spaces
            .iter()
            .filter(|s| {
                self.admission.can_read_space(
                    &self.identity,
                    &s.space,
                    chrono::Utc::now().timestamp(),
                    self.window.valid_until(),
                )
            })
            .map(|s| s.space.clone())
            .collect())
    }
    pub fn permit(&self) -> Result<SpaceWritePermit, AppError> {
        if !self.is_current() || !self.may_write {
            return Err(AppError::Forbidden);
        }
        Ok(SpaceWritePermit::from_admission(
            self.user,
            &self.identity,
            &self.admission,
            self.window.valid_until(),
        )?
        .restrict_deadline(self.window.monotonic_deadline()))
    }
    pub fn require_project_write(&self, links: &ConfirmedProjectSpaces) -> Result<(), AppError> {
        self.require_read(links)?;
        let permit = self.permit()?;
        if links.spaces.iter().any(|s| permit.allows(s)) {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}

/// A central HTTP principal lives only for this request. No user/global token map,
/// serialization, or automatic propagation into spawned jobs is permitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestOperation {
    Read,
    Write,
}
#[derive(Debug, Clone)]
pub struct CentralRequestContext {
    catalog_origin: Option<Arc<pdlc_auth_core::service_bridge::CatalogReadOrigin>>,
    user: UserId,
    identity_window: SpaceOwnerAuthority,
    global_admin: bool,
    may_write: bool,
    operation: RequestOperation,
    spaces: Option<SpaceActor>,
}
impl CentralRequestContext {
    pub fn verified(
        user: UserId,
        actor: &VerifiedCentralToken,
        spaces: Option<SpaceActor>,
        operation: RequestOperation,
    ) -> Result<Self, AppError> {
        if !actor.lifetime.is_current() {
            return Err(AppError::Forbidden);
        }
        let identity = CentralIdentity {
            issuer: actor.issuer.clone(),
            subject: actor.context.user_id.clone(),
        };
        if spaces
            .as_ref()
            .is_some_and(|s| s.user() != user || s.identity() != &identity || !s.is_current())
        {
            return Err(AppError::Forbidden);
        }
        let validity = AdmissionValidity::confirmed(
            actor.lifetime.source_observed_at,
            actor.lifetime.local_valid_until(),
            i64::MAX,
        )
        .ok_or(AppError::Forbidden)?;
        let identity_window = SpaceOwnerAuthority::confirmed(identity, validity)
            .ok_or(AppError::Forbidden)?
            .restrict_deadline(actor.lifetime.monotonic_deadline());
        // Capture once from the verified source request; no bearer value or
        // repeated token introspection is needed by the read-only catalog relay.
        let catalog_origin =
            pdlc_auth_core::service_bridge::CatalogReadOrigin::verified(actor, "task-tracker")
                .ok()
                .and_then(|origin| match spaces.as_ref() {
                    Some(space) => origin
                        .restrict(space.admission.validity, space.window.monotonic_deadline())
                        .ok(),
                    None => Some(origin),
                })
                .map(Arc::new);
        Ok(Self {
            catalog_origin,
            user,
            identity_window,
            global_admin: matches!(
                actor.subject_role.as_deref(),
                Some("admin" | "platform_admin")
            ),
            may_write: actor.context.allows_service("task-tracker", "PATCH"),
            operation,
            spaces,
        })
    }
    pub fn user(&self) -> UserId {
        self.user
    }
    pub fn is_current(&self) -> bool {
        self.identity_window.is_current() && self.spaces.as_ref().is_none_or(SpaceActor::is_current)
    }
    pub fn is_global_admin(&self) -> bool {
        self.is_current() && self.global_admin
    }
    pub fn is_mutation(&self) -> bool {
        self.operation == RequestOperation::Write
    }
    pub fn allows_mutation(&self) -> bool {
        self.is_current() && self.operation == RequestOperation::Write && self.may_write
    }
    pub fn catalog_origin(
        &self,
    ) -> Result<&pdlc_auth_core::service_bridge::CatalogReadOrigin, AppError> {
        if !self.is_current() {
            return Err(AppError::Forbidden);
        }
        self.catalog_origin.as_deref().ok_or(AppError::Forbidden)
    }
    pub fn spaces(&self) -> Option<&SpaceActor> {
        self.spaces.as_ref()
    }
    pub fn monotonic_deadline(&self) -> std::time::Instant {
        self.spaces
            .as_ref()
            .map_or(self.identity_window.monotonic_deadline(), |s| {
                s.window
                    .monotonic_deadline()
                    .min(self.identity_window.monotonic_deadline())
            })
    }
    pub fn valid_until(&self) -> i64 {
        self.spaces
            .as_ref()
            .map_or(self.identity_window.valid_until(), |s| {
                s.valid_until().min(self.identity_window.valid_until())
            })
    }
}
/// All request-owned readers use the same active-policy boundary. A missing
/// frame is an error after activation, including calls through old service APIs.
pub async fn request_space_actor(
    user: UserId,
    projects: &dyn domain::ProjectRepository,
) -> Result<Option<SpaceActor>, AppError> {
    if let Some(frame) = current_request_for(user)? {
        if let Some(actor) = frame.spaces() {
            return Ok(Some(actor.clone()));
        }
    }
    if projects.space_admission_enabled().await? {
        return Err(AppError::Forbidden);
    }
    Ok(None)
}
tokio::task_local! {static REQUEST_CONTEXT:Arc<CentralRequestContext>;}
pub async fn with_request_context<F: std::future::Future>(
    context: Arc<CentralRequestContext>,
    future: F,
) -> F::Output {
    REQUEST_CONTEXT.scope(context, future).await
}
pub fn current_request() -> Result<Option<Arc<CentralRequestContext>>, AppError> {
    let value = REQUEST_CONTEXT.try_with(Arc::clone).ok();
    if value.as_ref().is_some_and(|s| !s.is_current()) {
        return Err(AppError::Forbidden);
    }
    Ok(value)
}
pub fn current_request_for(user: UserId) -> Result<Option<Arc<CentralRequestContext>>, AppError> {
    let value = current_request()?;
    if value.as_ref().is_some_and(|s| s.user() != user) {
        return Err(AppError::Forbidden);
    }
    Ok(value)
}

#[cfg(test)]
pub(crate) mod request_context_tests {
    use super::*;
    pub(crate) fn context(
        user: UserId,
        admin: bool,
        operation: RequestOperation,
    ) -> Arc<CentralRequestContext> {
        let now = chrono::Utc::now().timestamp();
        Arc::new(CentralRequestContext {
            user,
            identity_window: SpaceOwnerAuthority::confirmed(
                CentralIdentity {
                    issuer: "https://auth.example.test".into(),
                    subject: user.to_string(),
                },
                AdmissionValidity::confirmed(now, now + 30, i64::MAX).unwrap(),
            )
            .unwrap(),
            global_admin: admin,
            may_write: true,
            operation,
            spaces: None,
            catalog_origin: None,
        })
    }
    #[tokio::test]
    async fn request_principal_is_bound_to_user_and_does_not_leak_into_jobs() {
        let user = UserId::new();
        let frame = context(user, false, RequestOperation::Read);
        with_request_context(frame, async move {
            let current = current_request_for(user).unwrap().unwrap();
            assert!(!current.is_global_admin());
            assert!(!current.allows_mutation());
            assert!(current_request_for(UserId::new()).is_err());
            assert!(
                tokio::spawn(async move { current_request_for(user).unwrap().is_none() })
                    .await
                    .unwrap()
            );
        })
        .await;
        assert!(current_request_for(user).unwrap().is_none());
    }
    #[tokio::test]
    async fn concurrent_requests_keep_separate_principals_and_capabilities() {
        let a = UserId::new();
        let b = UserId::new();
        let (left, right) = tokio::join!(
            with_request_context(context(a, true, RequestOperation::Write), async move {
                tokio::task::yield_now().await;
                let c = current_request_for(a).unwrap().unwrap();
                assert!(c.is_global_admin() && c.allows_mutation());
                assert!(current_request_for(b).is_err());
                a
            }),
            with_request_context(context(b, false, RequestOperation::Read), async move {
                tokio::task::yield_now().await;
                assert!(!current_request_for(b).unwrap().unwrap().is_global_admin());
                assert!(current_request_for(a).is_err());
                b
            })
        );
        assert_eq!(left, a);
        assert_eq!(right, b);
    }
    #[tokio::test]
    async fn mutation_without_scope_is_not_downgraded_to_read() {
        let user = UserId::new();
        let mut frame = (*context(user, false, RequestOperation::Write)).clone();
        frame.may_write = false;
        assert!(frame.is_mutation());
        assert!(!frame.allows_mutation());
        with_request_context(Arc::new(frame), async move {
            assert!(matches!(
                crate::authz::Authz::request_is_mutation(user),
                Err(AppError::Forbidden)
            ));
        })
        .await;
    }
    #[test]
    fn archived_admission_cannot_borrow_hidden_active_space() {
        let user = UserId::new();
        let now = chrono::Utc::now().timestamp();
        let validity = AdmissionValidity::confirmed(now, now + 30, i64::MAX).unwrap();
        let identity = CentralIdentity {
            issuer: "https://auth.example.test".into(),
            subject: user.to_string(),
        };
        let a = SpaceRef {
            registry_instance_id: uuid::Uuid::new_v4(),
            space_id: uuid::Uuid::new_v4(),
        };
        let b = SpaceRef {
            space_id: uuid::Uuid::new_v4(),
            ..a.clone()
        };
        let admission = SpaceAdmission {
            schema_version: 1,
            identity: identity.clone(),
            identity_active: true,
            policy_revision: 1,
            policy: SpaceAccessPolicy::Selected {
                spaces: vec![a.clone()],
            },
            validity,
            spaces: vec![
                ConfirmedSpace {
                    space: a.clone(),
                    revision: 1,
                    state: SpaceState::Archived,
                },
                ConfirmedSpace {
                    space: b.clone(),
                    revision: 1,
                    state: SpaceState::Active,
                },
            ],
        };
        let mut actor = SpaceActor {
            user,
            identity: identity.clone(),
            window: SpaceOwnerAuthority::confirmed(identity, validity).unwrap(),
            admission,
            may_write: true,
        };
        let links = ConfirmedProjectSpaces {
            project: ProjectRef {
                tracker_instance_id: uuid::Uuid::new_v4(),
                project_id: uuid::Uuid::new_v4(),
            },
            revision: 1,
            spaces: vec![a.clone(), b.clone()],
            validity,
        };
        // A fresh repository read cannot reset the upstream window.
        let mut fresh_links = links.clone();
        fresh_links.validity = AdmissionValidity::confirmed(now, now + 60, i64::MAX).unwrap();
        let bounded = actor.confirm_links(fresh_links).unwrap();
        assert_eq!(bounded.validity, validity);
        let mut short_links = links.clone();
        short_links.validity = AdmissionValidity::confirmed(now, now + 5, i64::MAX).unwrap();
        assert_eq!(
            actor
                .confirm_links(short_links)
                .unwrap()
                .validity
                .valid_until,
            now + 5
        );
        let expired_actor = actor.clone().restrict_deadline(std::time::Instant::now());
        assert!(expired_actor.clone().confirm_links(links.clone()).is_err());
        let renewed = expired_actor
            .restrict_deadline(std::time::Instant::now() + std::time::Duration::from_secs(60));
        assert!(
            !renewed.is_current(),
            "a downstream service cannot renew the original deadline"
        );
        actor.require_read(&links).unwrap();
        assert_eq!(actor.readable_spaces().unwrap(), vec![a.clone()]);
        assert!(actor.require_project_write(&links).is_err());
        actor.admission.policy = SpaceAccessPolicy::Selected {
            spaces: vec![b.clone()],
        };
        actor.require_project_write(&links).unwrap();
        actor.may_write = false;
        actor.require_read(&links).unwrap();
        assert!(actor.require_project_write(&links).is_err());
        actor.admission.policy = SpaceAccessPolicy::Selected { spaces: vec![] };
        assert!(actor.require_read(&links).is_err());
    }
}

/// Background delivery addresses only the recipient's active stored central
/// binding. It never materializes a user, guesses an ID or grants product roles.
pub async fn recipient_read(
    user: UserId,
    users: &dyn domain::UserRepository,
    projects: &dyn domain::ProjectRepository,
    reader: &pdlc_auth_core::service_bridge::RegisteredSpaceReader,
) -> Result<pdlc_auth_core::service_bridge::RecipientSpaceRead, AppError> {
    let subject = users
        .central_subject(user)
        .await?
        .and_then(|s| s.parse::<uuid::Uuid>().ok())
        .filter(|id| !id.is_nil())
        .ok_or(AppError::Forbidden)?;
    let refs = projects.space_catalog_refs().await?;
    admission_client()
        .await?
        .confirm_recipient_many(&CENTRAL_BRIDGE, reader, subject, &refs)
        .await
        .map_err(|_| AppError::Unavailable("notification_recipient_not_confirmed".into()))
}
