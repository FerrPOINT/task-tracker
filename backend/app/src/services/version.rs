use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use domain::ProjectRepository;
use shared::{AppError, ProjectKey, UserId};

pub struct VersionServiceImpl {
    versions: Arc<dyn domain::ProjectVersionRepository>,
    projects: Arc<dyn ProjectRepository>,
    authz: Authz,
}

impl VersionServiceImpl {
    pub fn new(
        versions: Arc<dyn domain::ProjectVersionRepository>,
        projects: Arc<dyn ProjectRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            versions,
            projects,
            authz,
        }
    }

    fn to_dto(v: &domain::ProjectVersion) -> crate::context::VersionDto {
        crate::context::VersionDto {
            id: v.id.to_string(),
            project_id: v.project_id.to_string(),
            name: v.name.as_ref().to_string(),
            description: v.description.as_ref().map(|d| d.as_ref().to_string()),
            released: v.released,
            release_date: v.release_date.map(|d| d.to_rfc3339()),
            created_at: v.created_at.to_rfc3339(),
        }
    }
}

#[async_trait]
impl crate::context::VersionService for VersionServiceImpl {
    async fn create(
        &self,
        project_key: &ProjectKey,
        name: &str,
        description: Option<&str>,
        released: bool,
        release_date: Option<chrono::DateTime<chrono::FixedOffset>>,
        requester: UserId,
    ) -> Result<crate::context::VersionDto, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        self.authz
            .require_project_edit(project.id, requester)
            .await?;
        if name.trim().is_empty() {
            return Err(AppError::invalid_input("version name must not be empty"));
        }
        let version = domain::ProjectVersion {
            id: shared::ProjectVersionId::new(),
            project_id: project.id,
            name: name.trim().to_string().into(),
            description: description.map(|d| d.to_string().into()),
            released,
            release_date,
            created_at: shared::now(),
        };
        self.versions.save(&version).await?;
        Ok(Self::to_dto(&version))
    }

    async fn list_by_project(
        &self,
        project_key: &ProjectKey,
        requester: UserId,
    ) -> Result<Vec<crate::context::VersionDto>, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        self.authz
            .require_project_access(project.id, requester)
            .await?;
        let items = self.versions.list_by_project(project.id).await?;
        Ok(items.iter().map(Self::to_dto).collect())
    }

    async fn update(
        &self,
        id: shared::ProjectVersionId,
        name: &str,
        description: Option<&str>,
        released: bool,
        release_date: Option<Option<chrono::DateTime<chrono::FixedOffset>>>,
        requester: UserId,
    ) -> Result<crate::context::VersionDto, AppError> {
        let mut version = self.versions.get_by_id(id).await?;
        self.authz
            .require_project_edit(version.project_id, requester)
            .await?;
        if !name.trim().is_empty() {
            version.name = name.trim().to_string().into();
        }
        version.description = description.map(|d| d.to_string().into());
        version.released = released;
        if let Some(rd) = release_date {
            version.release_date = rd;
        }
        self.versions.save(&version).await?;
        Ok(Self::to_dto(&version))
    }

    async fn delete(
        &self,
        id: shared::ProjectVersionId,
        requester: UserId,
    ) -> Result<(), AppError> {
        let version = self.versions.get_by_id(id).await?;
        self.authz
            .require_project_edit(version.project_id, requester)
            .await?;
        self.versions.delete(id).await?;
        Ok(())
    }
}
