use async_trait::async_trait;
use std::sync::Arc;

use domain::{IssueRepository, ProjectMemberRepository, ProjectRepository};
use shared::{AppError, IssueId, ProjectId, ProjectKey, UserId};

pub struct CustomFieldServiceImpl {
    fields: Arc<dyn domain::CustomFieldRepository>,
    projects: Arc<dyn ProjectRepository>,
    issues: Arc<dyn IssueRepository>,
    members: Arc<dyn ProjectMemberRepository>,
}

impl CustomFieldServiceImpl {
    pub fn new(
        fields: Arc<dyn domain::CustomFieldRepository>,
        projects: Arc<dyn ProjectRepository>,
        issues: Arc<dyn IssueRepository>,
        members: Arc<dyn ProjectMemberRepository>,
    ) -> Self {
        Self {
            fields,
            projects,
            issues,
            members,
        }
    }

    /// Verify the requester is the project owner or a member.
    async fn check_membership(
        &self,
        project_id: ProjectId,
        requester: UserId,
    ) -> Result<(), AppError> {
        let project = self.projects.get_by_id(project_id).await?;
        if project.owner_id == requester {
            return Ok(());
        }
        match self.members.get(project_id, requester).await {
            Ok(_) => Ok(()),
            Err(AppError::NotFound(_)) => Err(AppError::Forbidden),
            Err(e) => Err(e),
        }
    }

    fn to_dto(f: &domain::CustomField) -> crate::context::CustomFieldDto {
        crate::context::CustomFieldDto {
            id: f.id.to_string(),
            project_id: f.project_id.to_string(),
            name: f.name.as_ref().to_string(),
            field_type: f.field_type.as_str().to_string(),
            options: f.options.iter().map(|o| o.as_ref().to_string()).collect(),
            is_required: f.is_required,
            created_at: f.created_at.to_rfc3339(),
        }
    }
}

#[async_trait]
impl crate::context::CustomFieldService for CustomFieldServiceImpl {
    async fn create_field(
        &self,
        project_key: &ProjectKey,
        name: &str,
        field_type: &str,
        options: &[String],
        is_required: bool,
        requester: UserId,
    ) -> Result<crate::context::CustomFieldDto, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        self.check_membership(project.id, requester).await?;
        if name.trim().is_empty() {
            return Err(AppError::invalid_input("field name must not be empty"));
        }
        let ft: domain::CustomFieldType = field_type.parse().map_err(AppError::invalid_input)?;
        let field = domain::CustomField {
            id: shared::CustomFieldId::new(),
            project_id: project.id,
            name: name.trim().to_string().into(),
            field_type: ft,
            options: options
                .iter()
                .map(|s| s.trim().to_string().into())
                .collect(),
            is_required,
            created_at: shared::now(),
        };
        self.fields.save(&field).await?;
        Ok(Self::to_dto(&field))
    }

    async fn list_fields(
        &self,
        project_key: &ProjectKey,
    ) -> Result<Vec<crate::context::CustomFieldDto>, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        let items = self.fields.list_by_project(project.id).await?;
        Ok(items.iter().map(Self::to_dto).collect())
    }

    async fn update_field(
        &self,
        field_id: shared::CustomFieldId,
        name: &str,
        field_type: &str,
        options: &[String],
        is_required: bool,
        requester: UserId,
    ) -> Result<crate::context::CustomFieldDto, AppError> {
        let field = self.fields.get_by_id(field_id).await?;
        self.check_membership(field.project_id, requester).await?;
        let mut field = field;
        if !name.trim().is_empty() {
            field.name = name.trim().to_string().into();
        }
        field.field_type = field_type.parse().map_err(AppError::invalid_input)?;
        field.options = options
            .iter()
            .map(|s| s.trim().to_string().into())
            .collect();
        field.is_required = is_required;
        self.fields.save(&field).await?;
        Ok(Self::to_dto(&field))
    }

    async fn delete_field(
        &self,
        field_id: shared::CustomFieldId,
        _requester: UserId,
    ) -> Result<(), AppError> {
        self.fields.delete(field_id).await?;
        Ok(())
    }

    async fn set_value(
        &self,
        issue_id: IssueId,
        field_id: shared::CustomFieldId,
        value: serde_json::Value,
        _requester: UserId,
    ) -> Result<(), AppError> {
        // Validate the issue and field exist.
        let _issue = self.issues.get_by_id(issue_id).await?;
        let _field = self.fields.get_by_id(field_id).await?;
        self.fields.set_value(issue_id, field_id, &value).await?;
        Ok(())
    }

    async fn get_values_for_issue(
        &self,
        issue_id: IssueId,
    ) -> Result<Vec<crate::context::CustomFieldValueDto>, AppError> {
        let values = self.fields.get_values_for_issue(issue_id).await?;
        Ok(values
            .into_iter()
            .map(|v| crate::context::CustomFieldValueDto {
                field_id: v.field_id.to_string(),
                value: v.value,
            })
            .collect())
    }
}
