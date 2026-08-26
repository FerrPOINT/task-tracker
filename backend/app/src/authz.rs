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
        // MVP: members can edit. Same as access.
        self.require_project_access(project_id, user).await
    }

    /// Verify the user is the project owner.
    ///
    /// Used for project deletion, member management, and settings updates.
    pub async fn require_owner(&self, project_id: ProjectId, user: UserId) -> Result<(), AppError> {
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
        let mut ids: Vec<ProjectId> = self
            .projects
            .list(domain::ProjectQuery {
                owner_id: Some(user),
                limit: 1000,
                offset: 0,
            })
            .await?
            .into_iter()
            .map(|p| p.id)
            .collect();
        for m in self.members.list_by_user(user).await? {
            if !ids.contains(&m.project_id) {
                ids.push(m.project_id);
            }
        }
        Ok(ids)
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
