//! Original native request clock. This is not a human impersonation/read proof.
use pdlc_auth_core::{
    application_access::VerifiedApplicationAccess,
    application_access_client::{ApplicationAccessClient, ApplicationAccessError},
    service_bridge::VerifiedCentralToken,
};
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
    owner_access: Arc<tokio::sync::OnceCell<VerifiedApplicationAccess>>,
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
            owner_access: Arc::new(tokio::sync::OnceCell::new()),
        })
    }
    /// The aggregate supplies the immutable human owner after its native task/
    /// lease authorization. A URL, submitted owner field or machine role cannot.
    pub async fn require_owner(
        &self,
        owner: uuid::Uuid,
        required: &[&str],
    ) -> Result<(), AppError> {
        if !application_policy_required().await? {
            return Ok(());
        }
        if owner.is_nil() || owner.to_string() == self.identity.subject || !self.window.is_current()
        {
            return Err(AppError::Forbidden);
        }
        let proof =
            self.owner_access
                .get_or_try_init(|| async {
                    let mut decisions = owner_client().await?.resolve(&[owner]).await.map_err(
                        |error| match error {
                            ApplicationAccessError::Forbidden => AppError::Forbidden,
                            _ => AppError::Unavailable(
                                "current task owner admission unavailable".into(),
                            ),
                        },
                    )?;
                    let proof = decisions.remove(&owner).ok_or(AppError::Forbidden)?;
                    self.check_owner(&proof, owner, required)?;
                    Ok::<_, AppError>(proof)
                })
                .await?;
        self.check_owner(proof, owner, required)
    }
    fn check_owner(
        &self,
        proof: &VerifiedApplicationAccess,
        owner: uuid::Uuid,
        required: &[&str],
    ) -> Result<(), AppError> {
        if !self.window.is_current()
            || required.is_empty()
            || required
                .iter()
                .any(|key| !pdlc_auth_core::application_access::valid_application_key(key))
            || !proof.decision.enforced
            || proof.decision.issuer != self.identity.issuer
            || proof.decision.subject != owner.to_string()
            || proof.decision.subject == self.identity.subject
            || !proof.permits(required, false)
        {
            return Err(AppError::Forbidden);
        }
        Ok(())
    }
    pub fn is_current(&self) -> bool {
        self.window.is_current()
            && self
                .owner_access
                .get()
                .is_none_or(|proof| proof.is_current())
    }
    pub fn identity(&self) -> &CentralIdentity {
        &self.identity
    }
    pub fn validity(&self) -> Result<AdmissionValidity, AppError> {
        let validity = self.window.bounded_validity().ok_or(AppError::Forbidden)?;
        let Some(owner) = self.owner_access.get() else {
            return Ok(validity);
        };
        if !owner.is_current() {
            return Err(AppError::Forbidden);
        }
        // Wall first: scheduling delays may shorten, never renew, the owner window.
        let wall = chrono::Utc::now();
        let remaining =
            chrono::Duration::from_std(owner.remaining()).map_err(|_| AppError::Forbidden)?;
        let until = wall
            .checked_add_signed(remaining)
            .ok_or(AppError::Forbidden)?
            .min(owner.decision.expires_at);
        validity
            .attenuate(until.timestamp())
            .ok_or(AppError::Forbidden)
    }
    pub fn deadline(&self) -> std::time::Instant {
        let native = self.window.monotonic_deadline();
        self.owner_access
            .get()
            .map_or(native, |proof| native.min(proof.monotonic_deadline()))
    }
    pub fn may_write(&self) -> bool {
        self.may_write && self.is_current()
    }
    pub fn scopes(&self) -> &std::collections::HashSet<String> {
        &self.scopes
    }
}
pub async fn application_policy_required() -> Result<bool, AppError> {
    super::space_access::CENTRAL_BRIDGE
        .application_access_required()
        .await
        .map_err(|_| AppError::Unavailable("application policy configuration unavailable".into()))
}
static OWNER_CLIENT: tokio::sync::OnceCell<ApplicationAccessClient> =
    tokio::sync::OnceCell::const_new();
async fn owner_client() -> Result<&'static ApplicationAccessClient, AppError> {
    OWNER_CLIENT
        .get_or_try_init(|| async {
            let issuer = std::env::var("TT_AUTH__CENTRAL_ISSUER")
                .map_err(|_| AppError::Unavailable("current owner reader not configured".into()))?;
            let credential = std::env::var("TT_AUTH__CENTRAL_APPLICATION_ACCESS_READER_TOKEN")
                .map_err(|_| AppError::Unavailable("current owner reader not configured".into()))?;
            ApplicationAccessClient::new(&issuer, credential, std::time::Duration::from_secs(5))
                .map_err(|_| {
                    AppError::Unavailable("current owner reader configuration invalid".into())
                })
        })
        .await
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
