//! A human identity administrator may read metadata, not project content or writes.
use domain::identity_memberships::IdentityMembershipPage;
use pdlc_auth_core::service_bridge::VerifiedCentralToken;
use shared::{
    AppError,
    space_access::{AdmissionValidity, CentralIdentity},
    space_lifecycle::SpaceOwnerAuthority,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct IdentityManagementConfig {
    pub enabled: bool,
    pub registered_machines: Vec<String>,
}
impl IdentityManagementConfig {
    pub fn from_env() -> Result<Self, AppError> {
        let enabled = match std::env::var("TT_USER_MANAGEMENT_READ_ENABLED").as_deref() {
            Ok("true") => true,
            Ok("false") | Err(_) => false,
            _ => {
                return Err(AppError::Unavailable(
                    "invalid_identity_management_configuration".into(),
                ));
            }
        };
        let mut registered_machines = Vec::new();
        for key in [
            "TT_NAMESPACE__MACHINE_SUBJECTS",
            "TT_SPACES__ADMISSION_READER_SUBJECTS",
            "TT_USER_MANAGEMENT_MACHINE_SUBJECTS",
        ] {
            registered_machines.extend(
                std::env::var(key)
                    .unwrap_or_default()
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
            );
        }
        Ok(Self {
            enabled,
            registered_machines,
        })
    }
}

pub async fn page(
    repos: &domain::Repositories,
    actor: &VerifiedCentralToken,
    config: &IdentityManagementConfig,
    subject: Uuid,
    limit: u64,
    offset: u64,
) -> Result<IdentityMembershipPage, AppError> {
    if !config.enabled {
        return Err(AppError::Unavailable(
            "identity_membership_projection_disabled".into(),
        ));
    }
    let ctx = &actor.context;
    if !actor.lifetime.is_current() || !permitted(actor, config) {
        return Err(AppError::Forbidden);
    }
    let validity = AdmissionValidity::confirmed(
        actor.lifetime.source_observed_at,
        actor.lifetime.local_valid_until(),
        i64::MAX,
    )
    .ok_or(AppError::Unauthorized)?;
    let proof = SpaceOwnerAuthority::confirmed(
        CentralIdentity {
            issuer: actor.issuer.clone(),
            subject: ctx.user_id.clone(),
        },
        validity,
    )
    .ok_or(AppError::Unauthorized)?
    .restrict_deadline(actor.lifetime.monotonic_deadline());
    let result = repos
        .members
        .identity_memberships(subject, limit, offset)
        .await?;
    if !proof.is_current() {
        return Err(AppError::Unavailable(
            "identity_membership_authority_expired".into(),
        ));
    }
    Ok(result)
}

fn permitted(actor: &VerifiedCentralToken, config: &IdentityManagementConfig) -> bool {
    let ctx = &actor.context;
    matches!(
        actor.subject_role.as_deref(),
        Some("admin" | "platform_admin")
    ) && ctx.session_id.as_ref().is_some_and(|sid| !sid.is_empty())
        && !pdlc_auth_core::is_personal_token(&ctx.token)
        && ctx.allows_service("task-tracker", "GET")
        && !config
            .registered_machines
            .iter()
            .any(|subject| subject == &ctx.user_id)
}
