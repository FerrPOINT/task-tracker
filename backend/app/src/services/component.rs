use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use domain::ProjectRepository;
use shared::{AppError, ProjectKey, UserId};

pub struct ComponentServiceImpl {
    components: Arc<dyn domain::ProjectComponentRepository>,
    projects: Arc<dyn ProjectRepository>,
    authz: Authz,
}

impl ComponentServiceImpl {
    pub fn new(
        components: Arc<dyn domain::ProjectComponentRepository>,
        projects: Arc<dyn ProjectRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            components,
            projects,
            authz,
        }
    }

    fn to_dto(c: &domain::ProjectComponent) -> crate::context::ComponentDto {
        crate::context::ComponentDto {
            id: c.id.to_string(),
            project_id: c.project_id.to_string(),
            name: c.name.as_ref().to_string(),
            description: c.description.as_ref().map(|d| d.as_ref().to_string()),
            created_at: c.created_at.to_rfc3339(),
        }
    }
}

#[async_trait]
impl crate::context::ComponentService for ComponentServiceImpl {
    async fn create(
        &self,
        project_key: &ProjectKey,
        name: &str,
        description: Option<&str>,
        requester: UserId,
    ) -> Result<crate::context::ComponentDto, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        self.authz
            .require_project_edit(project.id, requester)
            .await?;
        if name.trim().is_empty() {
            return Err(AppError::invalid_input("component name must not be empty"));
        }
        let component = domain::ProjectComponent {
            id: shared::ProjectComponentId::new(),
            project_id: project.id,
            name: name.trim().to_string().into(),
            description: description.map(|d| d.to_string().into()),
            created_at: shared::now(),
        };
        self.components.save(&component).await?;
        Ok(Self::to_dto(&component))
    }

    async fn list_by_project(
        &self,
        project_key: &ProjectKey,
        requester: UserId,
    ) -> Result<Vec<crate::context::ComponentDto>, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        self.authz
            .require_project_access(project.id, requester)
            .await?;
        let items = self.components.list_by_project(project.id).await?;
        Ok(items.iter().map(Self::to_dto).collect())
    }

    async fn update(
        &self,
        id: shared::ProjectComponentId,
        name: &str,
        description: Option<&str>,
        requester: UserId,
    ) -> Result<crate::context::ComponentDto, AppError> {
        let mut component = self.components.get_by_id(id).await?;
        self.authz
            .require_project_edit(component.project_id, requester)
            .await?;
        if !name.trim().is_empty() {
            component.name = name.trim().to_string().into();
        }
        component.description = description.map(|d| d.to_string().into());
        self.components.save(&component).await?;
        Ok(Self::to_dto(&component))
    }

    async fn delete(
        &self,
        id: shared::ProjectComponentId,
        requester: UserId,
    ) -> Result<(), AppError> {
        let component = self.components.get_by_id(id).await?;
        self.authz
            .require_project_edit(component.project_id, requester)
            .await?;
        self.components.delete(id).await?;
        Ok(())
    }
}
