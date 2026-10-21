//! Centralized authorization policy layer.
//!
//! Single-tenant, self-hosted tracker. MVP policy:
//! - Any authenticated user can **view** project-scoped resources (members and
//!   owners alike).
//! - Project **members** can **modify** project-scoped resources.
//! - Only the project **owner** can delete the project, manage members, or
//!   change project settings.
//!
//! All checks return [`AppError::Forbidden`] on failure. A non-existent
//! project surfaces as [`AppError::NotFound`] (from the repository) before the
//! membership check runs.

use std::sync::Arc;

use domain::{ProjectMemberRepository, ProjectRepository};
use shared::{AppError, ProjectId, UserId};

/// Centralized authorization helper. Owns cheap `Arc<dyn …>` clones so it can
/// be cheaply copied into any service that needs it.
#[derive(Clone)]
pub struct Authz {
    members: Arc<dyn ProjectMemberRepository>,
    projects: Arc<dyn ProjectRepository>,
}

impl Authz {
    pub fn new(
        members: Arc<dyn ProjectMemberRepository>,
        projects: Arc<dyn ProjectRepository>,
    ) -> Self {
        Self { members, projects }
    }

    /// Verify the user is the project owner **or** a member.
    ///
    /// Used for read paths (board, backlog, issues, comments, etc.). Any
    /// project member or the owner can view.
    pub async fn require_project_access(
        &self,
        project_id: ProjectId,
        user: UserId,
    ) -> Result<(), AppError> {
        if std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_some() {
            self.projects.get_by_id(project_id).await?;
            return Ok(());
        }
        if self.is_owner(project_id, user).await? {
            return Ok(());
        }
        self.require_member(project_id, user).await
    }

    /// Verify the user is the project owner **or** a member.
    ///
    /// For the MVP any member can edit. This is a distinct gate so it can be
    /// tightened independently of [`require_project_access`].
    pub async fn require_project_edit(
        &self,
        project_id: ProjectId,
        user: UserId,
    ) -> Result<(), AppError> {
        if std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_some() {
            let role = pdlc_auth_core::human_request::role_for(user.as_uuid())
                .map_err(|_| AppError::Forbidden)?;
            if role.is_none() {
                return Err(AppError::Forbidden);
            }
        }
        self.require_project_access(project_id, user).await?;
        self.projects.require_writable(project_id).await
    }

    /// Verify the user is the project owner.
    ///
    /// Used for project deletion, member management, and settings updates.
    pub async fn require_owner(&self, project_id: ProjectId, user: UserId) -> Result<(), AppError> {
        self.projects.require_writable(project_id).await?;
        if std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_some() {
            self.projects.get_by_id(project_id).await?;
            return if central_administrator(user)? {
                Ok(())
            } else {
                Err(AppError::Forbidden)
            };
        }
        if self.is_owner(project_id, user).await? {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }

    /// Shared catalog settings require platform administration in central mode.
    pub async fn require_configuration_edit(
        &self,
        project: ProjectId,
        user: UserId,
    ) -> Result<(), AppError> {
        if std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_some() {
            self.require_owner(project, user).await
        } else {
            self.require_project_edit(project, user).await
        }
    }

    // ---- helpers ----

    /// All projects the user may read: owned projects plus memberships.
    ///
    /// Used to scope cross-project queries (global search, dashboard) so they
    /// never return issues from projects the requester has no access to.
    pub async fn accessible_project_ids(&self, user: UserId) -> Result<Vec<ProjectId>, AppError> {
        if std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_some() {
            return Ok(self
                .projects
                .list(domain::ProjectQuery {
                    owner_id: None,
                    limit: 0,
                    offset: 0,
                })
                .await?
                .into_iter()
                .map(|project| project.id)
                .collect());
        }
        let mut seen: std::collections::HashSet<ProjectId> = self
            .projects
            .list(domain::ProjectQuery {
                owner_id: Some(user),
                limit: 0,
                offset: 0,
            })
            .await?
            .into_iter()
            .map(|p| p.id)
            .collect();
        for m in self.members.list_by_user(user).await? {
            seen.insert(m.project_id);
        }
        Ok(seen.into_iter().collect())
    }

    async fn is_owner(&self, project_id: ProjectId, user: UserId) -> Result<bool, AppError> {
        let project = self.projects.get_by_id(project_id).await?;
        Ok(project.owner_id == user)
    }

    async fn require_member(&self, project_id: ProjectId, user: UserId) -> Result<(), AppError> {
        match self.members.get(project_id, user).await {
            Ok(_) => Ok(()),
            Err(AppError::NotFound(_)) => Err(AppError::Forbidden),
            Err(e) => Err(e),
        }
    }
}

/// Only the current request may authorize a central human; saved local flags are history.
pub fn central_administrator(user: UserId) -> Result<bool, AppError> {
    pdlc_auth_core::human_request::role_for(user.as_uuid())
        .map(|role| role == Some(pdlc_auth_core::application_access::HumanRole::Admin))
        .map_err(|error| match error {
            pdlc_auth_core::human_request::HumanRequestError::Expired => {
                AppError::Unavailable("Current human authority expired".into())
            }
            _ => AppError::Forbidden,
        })
}
pub fn require_central_administrator() -> Result<(), AppError> {
    let role = pdlc_auth_core::human_request::current_role().map_err(|_| AppError::Forbidden)?;
    if role == Some(pdlc_auth_core::application_access::HumanRole::Admin) {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

#[cfg(test)]
mod central_role_tests {
    use super::*;
    #[tokio::test]
    async fn only_the_bound_current_administrator_can_manage_structure() {
        let actor = UserId::new();
        let other = UserId::new();
        assert!(!central_administrator(actor).unwrap());
        for (role, expected) in [("user", false), ("admin", true), ("operator", false)] {
            let context = pdlc_auth_core::AuthContext {
                user_id: "central-identity".into(),
                role: Some(role.into()),
                email: None,
                scopes: Default::default(),
                session_id: Some("synthetic-session".into()),
                token: String::new(),
            };
            pdlc_auth_core::human_request::with_human_request(
                actor.as_uuid(),
                &context,
                None,
                async {
                    assert_eq!(central_administrator(actor).unwrap(), expected);
                    assert!(central_administrator(other).is_err());
                    assert_eq!(require_central_administrator().is_ok(), expected);
                },
            )
            .await
            .unwrap();
        }
    }
}
