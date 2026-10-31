//! Original native request clock. This is not a human impersonation/read proof.
pub use pdlc_auth_core::service_bridge::ConfirmedSpaceRead as OwnerSpaceRead;
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
    owner_identity: Arc<tokio::sync::OnceCell<SpaceOwnerAuthority>>,
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
            owner_identity: Arc::new(tokio::sync::OnceCell::new()),
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
    /// Identity remains mandatory for a human-origin execution even when both
    /// optional visibility policies are off. This grants no apps or data access.
    pub async fn require_known_owner(&self, owner: uuid::Uuid) -> Result<(), AppError> {
        if owner.is_nil() || owner.to_string() == self.identity.subject || !self.window.is_current()
        {
            return Err(AppError::Forbidden);
        }
        self.owner_identity
            .get_or_try_init(|| async {
                let (actor, _) = crate::space_access::registered_admission_reader().await?;
                let confirmation = crate::space_access::CENTRAL_BRIDGE
                    .confirm_known_identity(&actor, owner)
                    .await
                    .map_err(|_| {
                        AppError::Unavailable("current owner identity unavailable".into())
                    })?
                    .filter(|confirmation| confirmation.active)
                    .ok_or(AppError::Forbidden)?;
                SpaceOwnerAuthority::confirmed(confirmation.identity, confirmation.validity)
                    .map(|window| {
                        window
                            .restrict_deadline(actor.lifetime.monotonic_deadline())
                            .restrict_deadline(self.window.monotonic_deadline())
                    })
                    .ok_or(AppError::Forbidden)
            })
            .await?;
        self.confirmed_owner(owner)
    }
    /// Locked repository code may only recheck the already resolved owner.
    /// It must never initiate identity HTTP while holding the transaction.
    pub fn confirmed_owner(&self, owner: uuid::Uuid) -> Result<(), AppError> {
        let proof = self.owner_identity.get().ok_or(AppError::Forbidden)?;
        if !self.window.is_current()
            || !proof.is_current()
            || proof.identity().issuer != self.identity.issuer
            || proof.identity().subject != owner.to_string()
        {
            return Err(AppError::Forbidden);
        }
        Ok(())
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
    /// Read-only data restriction for the immutable task owner. The caller still
    /// verifies its own native capability and retains both original clocks.
    pub async fn owner_space_read(
        &self,
        owner: uuid::Uuid,
        spaces: &[shared::space_access::SpaceRef],
    ) -> Result<pdlc_auth_core::service_bridge::ConfirmedSpaceRead, AppError> {
        if owner.is_nil() || !self.is_current() {
            return Err(AppError::Forbidden);
        }
        let client = crate::space_access::admission_client().await?;
        let result = if let Some(proof) = self.owner_access.get() {
            self.check_owner(proof, owner, &["task-tracker"])?;
            client
                .confirm_owner_many_read(
                    &crate::space_access::CENTRAL_BRIDGE,
                    proof,
                    "task-tracker",
                    spaces,
                )
                .await
        } else {
            // Space policy can be active independently of application policy.
            let reader = crate::space_access::notification_reader().await?;
            client
                .confirm_recipient_many(
                    &crate::space_access::CENTRAL_BRIDGE,
                    &reader,
                    owner,
                    spaces,
                )
                .await
        };
        result.map_err(|error| match error {
            pdlc_auth_core::service_bridge::SpaceAdmissionError::Invalid
            | pdlc_auth_core::service_bridge::SpaceAdmissionError::Expired => AppError::Forbidden,
            _ => AppError::Unavailable("current task owner space admission unavailable".into()),
        })
    }
    pub fn is_current(&self) -> bool {
        self.window.is_current()
            && self
                .owner_identity
                .get()
                .is_none_or(|proof| proof.is_current())
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
        let validity = if let Some(owner) = self.owner_identity.get() {
            if !owner.is_current() {
                return Err(AppError::Forbidden);
            }
            validity
                .attenuate(
                    owner
                        .bounded_validity()
                        .ok_or(AppError::Forbidden)?
                        .valid_until,
                )
                .ok_or(AppError::Forbidden)?
        } else {
            validity
        };
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
        let native = self
            .owner_identity
            .get()
            .map_or(self.window.monotonic_deadline(), |owner| {
                self.window
                    .monotonic_deadline()
                    .min(owner.monotonic_deadline())
            });
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
