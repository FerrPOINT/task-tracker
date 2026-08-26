use async_trait::async_trait;
use std::str::FromStr;
use std::sync::Arc;

use crate::authz::Authz;
use crate::dto::ProjectMemberDto;
use domain::{ProjectMember, ProjectMemberRepository, ProjectRole, UserRepository};
use shared::{AppError, ProjectId, UserId};

#[async_trait]
pub trait ProjectMemberService: Send + Sync {
    async fn list(
        &self,
        project_id: ProjectId,
        requester: UserId,
    ) -> Result<Vec<ProjectMemberDto>, AppError>;
    async fn add(
        &self,
        cmd: crate::commands::AddProjectMemberCommand,
        requester: UserId,
    ) -> Result<ProjectMemberDto, AppError>;
    async fn remove(
        &self,
        project_id: ProjectId,
        user_id: UserId,
        requester: UserId,
    ) -> Result<(), AppError>;
}

pub struct ProjectMemberServiceImpl {
    members: Arc<dyn ProjectMemberRepository>,
    users: Arc<dyn UserRepository>,
    authz: Authz,
}

impl ProjectMemberServiceImpl {
    pub fn new(
        members: Arc<dyn ProjectMemberRepository>,
        users: Arc<dyn UserRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            members,
            users,
            authz,
        }
    }
}

#[async_trait]
impl ProjectMemberService for ProjectMemberServiceImpl {
    async fn list(
        &self,
        project_id: ProjectId,
        requester: UserId,
    ) -> Result<Vec<ProjectMemberDto>, AppError> {
        self.authz
            .require_project_access(project_id, requester)
            .await?;
        let members = self.members.list_by_project(project_id).await?;
        let mut dtos = Vec::with_capacity(members.len());
        for m in members {
            let role = m.role.as_str().to_string();
            dtos.push(ProjectMemberDto {
                project_id: m.project_id.to_string(),
                user_id: m.user_id.to_string(),
                role,
                joined_at: m.joined_at,
            });
        }
        Ok(dtos)
    }

    async fn add(
        &self,
        cmd: crate::commands::AddProjectMemberCommand,
        requester: UserId,
    ) -> Result<ProjectMemberDto, AppError> {
        self.authz.require_owner(cmd.project_id, requester).await?;
        let role = ProjectRole::from_str(&cmd.role).unwrap_or_default();
        let _ = self.users.get_by_id(cmd.user_id).await?;
        // Re-adding an existing member upserts the role (repo save is idempotent)
        // and preserves the original joined_at.
        let joined_at = match self.members.get(cmd.project_id, cmd.user_id).await {
            Ok(existing) => existing.joined_at,
            Err(_) => shared::now(),
        };
        let member = ProjectMember {
            project_id: cmd.project_id,
            user_id: cmd.user_id,
            role,
            joined_at,
        };
        self.members.save(&member).await?;
        Ok(ProjectMemberDto {
            project_id: member.project_id.to_string(),
            user_id: member.user_id.to_string(),
            role: member.role.as_str().to_string(),
            joined_at: member.joined_at,
        })
    }

    async fn remove(
        &self,
        project_id: ProjectId,
        user_id: UserId,
        requester: UserId,
    ) -> Result<(), AppError> {
        self.authz.require_owner(project_id, requester).await?;
        self.members.delete(project_id, user_id).await
    }
}
