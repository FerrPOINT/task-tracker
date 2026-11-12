//! Original native request clock. This is not a human impersonation/read proof.
use pdlc_auth_core::service_bridge::VerifiedCentralToken;
use shared::{
    AppError,
    space_access::{AdmissionValidity, CentralIdentity},
    space_lifecycle::SpaceOwnerAuthority,
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct NativeRequest {
    identity: CentralIdentity,
    window: SpaceOwnerAuthority,
    may_write: bool,
    scopes: std::collections::HashSet<String>,
}
impl NativeRequest {
    /// The API must first classify the independently registered native protocol.
    /// Neither a frontend selector nor a local shadow admin role enters this value.
    pub fn verified(
        actor: &VerifiedCentralToken,
        registered: bool,
        mutation: bool,
    ) -> Result<Self, AppError> {
        if !registered
            || actor.subject_role.as_deref() != Some("service_account")
            || !actor.lifetime.is_current()
            || actor.context.user_id.trim().is_empty()
            || !actor
                .context
                .allows_service("task-tracker", if mutation { "POST" } else { "GET" })
        {
            return Err(AppError::Forbidden);
        }
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
        let validity = AdmissionValidity {
            observed_at: actor.lifetime.source_observed_at,
            valid_until: expiry,
        };
        let window = SpaceOwnerAuthority::confirmed(identity.clone(), validity)
            .ok_or(AppError::Forbidden)?
            .restrict_deadline(actor.lifetime.monotonic_deadline());
        Ok(Self {
            identity,
            window,
            may_write: mutation,
            scopes: actor.context.scopes.clone(),
        })
    }
    pub fn is_current(&self) -> bool {
        self.window.is_current()
    }
    pub fn identity(&self) -> &CentralIdentity {
        &self.identity
    }
    pub fn validity(&self) -> Result<AdmissionValidity, AppError> {
        self.window.bounded_validity().ok_or(AppError::Forbidden)
    }
    pub fn deadline(&self) -> std::time::Instant {
        self.window.monotonic_deadline()
    }
    pub fn may_write(&self) -> bool {
        self.may_write && self.is_current()
    }
    pub fn scopes(&self) -> &std::collections::HashSet<String> {
        &self.scopes
    }
}
tokio::task_local! { static CURRENT:Arc<NativeRequest>; }
pub async fn with_request<T>(
    scope: NativeRequest,
    future: impl std::future::Future<Output = T>,
) -> T {
    CURRENT.scope(Arc::new(scope), future).await
}
pub fn current() -> Result<Option<Arc<NativeRequest>>, AppError> {
    match CURRENT.try_with(Clone::clone) {
        Ok(scope) if scope.is_current() => Ok(Some(scope)),
        Ok(_) => Err(AppError::Forbidden),
        Err(_) => Ok(None),
    }
}
