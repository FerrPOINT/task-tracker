//! Canonical project-to-space full-set replacement.
use serde::{Deserialize, Serialize};
use shared::{AppError, space_access::SpaceRef};

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplaceProjectSpaces {
    pub operation_id: uuid::Uuid,
    #[schema(minimum = 1)]
    pub expected_revision: i64,
    #[schema(min_items = 1, max_items = 1000)]
    pub spaces: Vec<SpaceRef>,
}
impl ReplaceProjectSpaces {
    pub fn validate(&self) -> Result<(), AppError> {
        validate_space_set(&self.spaces)?;
        if self.operation_id.is_nil() || self.expected_revision < 1 {
            return Err(AppError::invalid_input("invalid_project_space_revision"));
        }
        Ok(())
    }
}

pub fn validate_space_set(spaces: &[SpaceRef]) -> Result<(), AppError> {
    let mut seen = std::collections::HashSet::new();
    if spaces.is_empty()
        || spaces.len() > 1000
        || !spaces
            .iter()
            .all(|space| space.valid() && seen.insert(space))
    {
        return Err(AppError::invalid_input("project_requires_distinct_spaces"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_duplicate_or_nil_space_set_is_never_a_project_default() {
        let a = SpaceRef {
            registry_instance_id: uuid::Uuid::new_v4(),
            space_id: uuid::Uuid::new_v4(),
        };
        assert!(validate_space_set(&[]).is_err());
        assert!(validate_space_set(&[a.clone(), a.clone()]).is_err());
        assert!(
            validate_space_set(&[SpaceRef {
                space_id: uuid::Uuid::nil(),
                ..a.clone()
            }])
            .is_err()
        );
        assert!(validate_space_set(&[a]).is_ok());
    }
}

/// Internal write authority from a verified server admission; never an HTTP payload.
#[derive(Debug, Clone)]
pub struct SpaceWritePermit {
    actor: shared::UserId,
    identity: shared::space_access::CentralIdentity,
    validity: shared::space_access::AdmissionValidity,
    active_spaces: Vec<SpaceRef>,
    policy_revision: i64,
    deadline: std::time::Instant,
}
impl SpaceWritePermit {
    pub fn from_admission(
        actor: shared::UserId,
        identity: &shared::space_access::CentralIdentity,
        admission: &shared::space_access::SpaceAdmission,
        token_expiry: i64,
    ) -> Result<Self, AppError> {
        let started = std::time::Instant::now();
        let wall = chrono::Utc::now();
        let now = wall.timestamp();
        if !admission.valid() || &admission.identity != identity || !admission.identity_active {
            return Err(AppError::Forbidden);
        }
        let validity = admission
            .validity
            .attenuate(token_expiry)
            .ok_or(AppError::Forbidden)?;
        if !validity.valid_at(now, token_expiry) {
            return Err(AppError::Forbidden);
        }
        let expires =
            chrono::DateTime::from_timestamp(validity.valid_until, 0).ok_or(AppError::Forbidden)?;
        let remaining = expires
            .signed_duration_since(wall)
            .to_std()
            .map_err(|_| AppError::Forbidden)?;
        let deadline = started.checked_add(remaining).ok_or(AppError::Forbidden)?;
        let active_spaces = admission
            .spaces
            .iter()
            .filter(|item| admission.has_active_space(identity, &item.space, now, token_expiry))
            .map(|item| item.space.clone())
            .collect();
        Ok(Self {
            actor,
            identity: identity.clone(),
            validity,
            active_spaces,
            policy_revision: admission.policy_revision,
            deadline,
        })
    }
    /// A request scope can shorten this permit, never renew it after a later clone.
    pub fn restrict_deadline(mut self, deadline: std::time::Instant) -> Self {
        self.deadline = self.deadline.min(deadline);
        self
    }
    pub fn policy_revision(&self) -> i64 {
        self.policy_revision
    }
    pub fn active_spaces(&self) -> Result<&[SpaceRef], AppError> {
        if !self.is_current() {
            return Err(AppError::Forbidden);
        }
        Ok(&self.active_spaces)
    }
    pub fn valid_until(&self) -> i64 {
        self.validity.valid_until
    }
    pub fn actor(&self) -> shared::UserId {
        self.actor
    }
    pub fn identity(&self) -> &shared::space_access::CentralIdentity {
        &self.identity
    }
    pub fn is_current(&self) -> bool {
        std::time::Instant::now() < self.deadline
            && self
                .validity
                .valid_at(chrono::Utc::now().timestamp(), i64::MAX)
    }
    pub fn allows(&self, space: &SpaceRef) -> bool {
        self.is_current() && self.active_spaces.contains(space)
    }
    pub fn require_set(&self, spaces: &[SpaceRef]) -> Result<(), AppError> {
        validate_space_set(spaces)?;
        if spaces.iter().all(|space| self.allows(space)) {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}

#[cfg(test)]
mod permit_tests {
    use super::*;
    use shared::space_access::*;
    #[test]
    fn archived_or_hidden_active_space_never_supplies_write_authority() {
        let user = shared::UserId::new();
        let identity = CentralIdentity {
            issuer: "https://auth.example.test".into(),
            subject: "central-subject".into(),
        };
        let a = SpaceRef {
            registry_instance_id: uuid::Uuid::new_v4(),
            space_id: uuid::Uuid::new_v4(),
        };
        let b = SpaceRef {
            space_id: uuid::Uuid::new_v4(),
            ..a.clone()
        };
        let now = chrono::Utc::now().timestamp();
        let mut admission = SpaceAdmission {
            inheritance: None,
            schema_version: 1,
            identity: identity.clone(),
            identity_active: true,
            policy_revision: 1,
            policy: SpaceAccessPolicy::Selected {
                spaces: vec![a.clone()],
            },
            validity: AdmissionValidity::confirmed(now, now + 30, i64::MAX).unwrap(),
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
        let permit =
            SpaceWritePermit::from_admission(user, &identity, &admission, now + 30).unwrap();
        assert!(!permit.allows(&a));
        assert!(!permit.allows(&b));
        assert!(permit.require_set(&[]).is_err());
        assert!(permit.require_set(&[b.clone()]).is_err());
        admission.policy = SpaceAccessPolicy::Selected {
            spaces: vec![b.clone()],
        };
        let permit =
            SpaceWritePermit::from_admission(user, &identity, &admission, now + 30).unwrap();
        assert!(permit.require_set(&[b.clone()]).is_ok());
        assert!(!permit.allows(&a));
        assert!(SpaceWritePermit::from_admission(user, &identity, &admission, now).is_err());
        admission.identity_active = false;
        assert!(SpaceWritePermit::from_admission(user, &identity, &admission, now + 30).is_err());
    }
}

/// Delivery status contains durable metadata, never an admission or write grant.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectSpaceProjectionStatus {
    pub operation_id: uuid::Uuid,
    pub project: shared::space_access::ProjectRef,
    pub revision: i64,
    pub confirmed: bool,
    pub last_error: Option<String>,
}

/// Internal canonical read page. Authorization is applied by the API before this Space query.
pub struct CanonicalProjectContextPage {
    pub projects: Vec<shared::project_spaces::ProjectSpaceReadContext>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangeProjectLifecycle {
    pub operation_id: uuid::Uuid,
    pub expected_revision: i64,
    pub archived: bool,
}
impl ChangeProjectLifecycle {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.operation_id.is_nil() || self.expected_revision < 1 {
            Err(AppError::invalid_input("invalid_project_lifecycle_command"))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod lifecycle_command_tests {
    use super::*;
    #[test]
    fn lifecycle_command_cannot_supply_actor_authority_or_skip_revision() {
        let mut command = ChangeProjectLifecycle {
            operation_id: uuid::Uuid::new_v4(),
            expected_revision: 1,
            archived: true,
        };
        command.validate().unwrap();
        let mut body = serde_json::to_value(&command).unwrap();
        body["actor"] = serde_json::json!({"role":"admin"});
        assert!(serde_json::from_value::<ChangeProjectLifecycle>(body).is_err());
        command.expected_revision = 0;
        assert!(command.validate().is_err());
        command.expected_revision = 1;
        command.operation_id = uuid::Uuid::nil();
        assert!(command.validate().is_err());
    }
}

/// Committed metadata from this owner's operation table; no authorization TTL is manufactured here.
pub struct StoredProjectSpaceOperation {
    pub project: shared::space_access::ProjectRef,
    pub operation_id: uuid::Uuid,
    pub applied_revision: i64,
    pub change: ReplaceProjectSpaces,
}
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectSpaceOperationReceipt {
    pub schema_version: u32,
    pub project: shared::space_access::ProjectRef,
    pub operation_id: uuid::Uuid,
    pub applied: bool,
    pub applied_revision: i64,
    pub change: ReplaceProjectSpaces,
    pub validity: shared::space_access::AdmissionValidity,
}
impl ProjectSpaceOperationReceipt {
    pub fn valid(&self) -> bool {
        self.schema_version == 1
            && self.project.valid()
            && !self.operation_id.is_nil()
            && self.applied
            && self.change.operation_id == self.operation_id
            && self.change.validate().is_ok()
            && self.applied_revision >= self.change.expected_revision
            && self.validity.valid()
    }
}

/// Internal catalog query; it does not alter the legacy ProjectQuery wire contract.
#[derive(Debug, Clone)]
pub struct ProjectCatalogQuery {
    pub working_only: bool,
    pub limit: u64,
    pub offset: u64,
    pub search: String,
    pub project_key: Option<shared::ProjectKey>,
}
impl ProjectCatalogQuery {
    pub fn validate(&self) -> Result<(), AppError> {
        if !(1..=100).contains(&self.limit)
            || self.offset > i64::MAX as u64
            || self.search.chars().count() > 200
            || self.project_key.as_ref().is_some_and(|key| !key.is_valid())
        {
            return Err(AppError::invalid_input("invalid_project_catalog_query"));
        }
        Ok(())
    }
}
pub struct VisibleProjectPage {
    pub projects: Vec<crate::Project>,
    pub total: u64,
}
