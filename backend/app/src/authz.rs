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

    async fn scoped_actor(
        &self,
        user: UserId,
    ) -> Result<Option<crate::space_access::SpaceActor>, AppError> {
        crate::space_access::request_space_actor(user, self.projects.as_ref()).await
    }
    async fn check_scoped_project(
        &self,
        project: ProjectId,
        user: UserId,
        write: bool,
    ) -> Result<bool, AppError> {
        if let Some(actor) = self.scoped_actor(user).await? {
            let links = self.projects.project_spaces(project).await?;
            if write {
                actor.require_project_write(&links)?;
            } else {
                actor.require_read(&links)?;
            }
            return Ok(true);
        }
        Ok(false)
    }
    pub(crate) fn request_is_mutation(user: UserId) -> Result<bool, AppError> {
        let frame = crate::space_access::current_request_for(user)?;
        if let Some(frame) = frame {
            if frame.is_mutation() && !frame.allows_mutation() {
                return Err(AppError::Forbidden);
            }
            Ok(frame.is_mutation())
        } else {
            Ok(true)
        }
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
        if self.check_scoped_project(project_id, user, false).await? {
            return Ok(());
        }
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
        let write = Self::request_is_mutation(user)?;
        if !self.check_scoped_project(project_id, user, write).await? {
            self.require_project_access(project_id, user).await?;
        }
        // Read visibility never establishes an edit role, including central mode.
        if !self.is_owner(project_id, user).await? {
            self.require_member(project_id, user).await?;
        }
        if write {
            self.projects.require_writable(project_id).await
        } else {
            Ok(())
        }
    }

    /// Verify the user is the project owner.
    ///
    /// Used for project deletion, member management, and settings updates.
    pub async fn require_owner(&self, project_id: ProjectId, user: UserId) -> Result<(), AppError> {
        let write = Self::request_is_mutation(user)?;
        self.check_scoped_project(project_id, user, write).await?;
        if write {
            self.projects.require_writable(project_id).await?;
        }
        if self.is_owner(project_id, user).await? {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }

    // ---- helpers ----

    /// All projects the user may read: owned projects plus memberships.
    ///
    /// Used to scope cross-project queries (global search, dashboard) so they
    /// never return issues from projects the requester has no access to.
    pub async fn accessible_project_ids(&self, user: UserId) -> Result<Vec<ProjectId>, AppError> {
        if let Some(actor) = self.scoped_actor(user).await? {
            let refs = actor.readable_spaces()?;
            let projects = self
                .projects
                .list_space_accessible_page(&refs, 0, 0)
                .await?;
            actor.readable_spaces()?;
            return Ok(projects.into_iter().map(|p| p.id).collect());
        }
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
