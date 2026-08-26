use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use domain::{IssueRepository, ProjectRepository};
use shared::{AppError, IssueId, ProjectKey, UserId};

pub struct CustomFieldServiceImpl {
    fields: Arc<dyn domain::CustomFieldRepository>,
    projects: Arc<dyn ProjectRepository>,
    issues: Arc<dyn IssueRepository>,
    authz: Authz,
}

impl CustomFieldServiceImpl {
    pub fn new(
        fields: Arc<dyn domain::CustomFieldRepository>,
        projects: Arc<dyn ProjectRepository>,
        issues: Arc<dyn IssueRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            fields,
            projects,
            issues,
            authz,
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

/// Validate that a JSON value is acceptable for the given custom field type.
fn validate_custom_field_value(
    field: &domain::CustomField,
    value: &serde_json::Value,
) -> Result<(), AppError> {
    use domain::CustomFieldType;
    match field.field_type {
        CustomFieldType::Text => {
            if !value.is_string() {
                return Err(AppError::invalid_input(
                    "expected a string value for text field",
                ));
            }
        }
        CustomFieldType::Number => {
            if !value.is_number() {
                return Err(AppError::invalid_input(
                    "expected a number value for number field",
                ));
            }
        }
        CustomFieldType::Date => {
            let s = value
                .as_str()
                .ok_or_else(|| AppError::invalid_input("expected a date string for date field"))?;
            chrono::DateTime::parse_from_rfc3339(s)
                .map_err(|_| AppError::invalid_input("invalid RFC 3339 date for date field"))?;
        }
        CustomFieldType::Select => {
            let s = value.as_str().ok_or_else(|| {
                AppError::invalid_input("expected a string value for select field")
            })?;
            if !field.options.iter().any(|opt| opt.as_ref() == s) {
                return Err(AppError::invalid_input(
                    "value is not one of the allowed options",
                ));
            }
        }
        CustomFieldType::MultiSelect => {
            let arr = value.as_array().ok_or_else(|| {
                AppError::invalid_input("expected an array for multi-select field")
            })?;
            for item in arr {
                let s = item
                    .as_str()
                    .ok_or_else(|| AppError::invalid_input("multi-select items must be strings"))?;
                if !field.options.iter().any(|opt| opt.as_ref() == s) {
                    return Err(AppError::invalid_input(
                        "value is not one of the allowed options",
                    ));
                }
            }
        }
    }
    Ok(())
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
        self.authz
            .require_project_edit(project.id, requester)
            .await?;
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
        requester: UserId,
    ) -> Result<Vec<crate::context::CustomFieldDto>, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        self.authz
            .require_project_access(project.id, requester)
            .await?;
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
        self.authz
            .require_project_edit(field.project_id, requester)
            .await?;
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
        requester: UserId,
    ) -> Result<(), AppError> {
        let field = self.fields.get_by_id(field_id).await?;
        self.authz
            .require_project_edit(field.project_id, requester)
            .await?;
        self.fields.delete(field_id).await?;
        Ok(())
    }

    async fn set_value(
        &self,
        issue_id: IssueId,
        field_id: shared::CustomFieldId,
        value: serde_json::Value,
        requester: UserId,
    ) -> Result<(), AppError> {
        let issue = self.issues.get_by_id(issue_id).await?;
        self.authz
            .require_project_edit(issue.project_id, requester)
            .await?;
        let field = self.fields.get_by_id(field_id).await?;
        // A field defined in another project must not be settable here.
        if field.project_id != issue.project_id {
            return Err(AppError::invalid_input(
                "custom field belongs to a different project",
            ));
        }
        // Validate the value matches the field type.
        validate_custom_field_value(&field, &value)?;
        self.fields.set_value(issue_id, field_id, &value).await?;
        Ok(())
    }

    async fn get_values_for_issue(
        &self,
        issue_id: IssueId,
        requester: UserId,
    ) -> Result<Vec<crate::context::CustomFieldValueDto>, AppError> {
        let issue = self.issues.get_by_id(issue_id).await?;
        self.authz
            .require_project_access(issue.project_id, requester)
            .await?;
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
