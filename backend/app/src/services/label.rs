use async_trait::async_trait;
use std::sync::Arc;

use domain::{IssueRepository, ProjectMemberRepository, ProjectRepository};
use shared::{AppError, IssueId, ProjectId, ProjectKey, UserId};

pub struct LabelServiceImpl {
    labels: Arc<dyn domain::LabelRepository>,
    projects: Arc<dyn ProjectRepository>,
    issues: Arc<dyn IssueRepository>,
    members: Arc<dyn ProjectMemberRepository>,
}

impl LabelServiceImpl {
    pub fn new(
        labels: Arc<dyn domain::LabelRepository>,
        projects: Arc<dyn ProjectRepository>,
        issues: Arc<dyn IssueRepository>,
        members: Arc<dyn ProjectMemberRepository>,
    ) -> Self {
        Self {
            labels,
            projects,
            issues,
            members,
        }
    }

    fn to_dto(l: &domain::Label) -> crate::context::LabelDto {
        crate::context::LabelDto {
            id: l.id.to_string(),
            project_id: l.project_id.to_string(),
            name: l.name.as_ref().to_string(),
            color: l.color.as_ref().to_string(),
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
}

#[async_trait]
impl crate::context::LabelService for LabelServiceImpl {
    async fn create(
        &self,
        project_key: &ProjectKey,
        name: &str,
        color: &str,
        requester: UserId,
    ) -> Result<crate::context::LabelDto, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        self.check_membership(project.id, requester).await?;
        if name.trim().is_empty() {
            return Err(AppError::invalid_input("label name must not be empty"));
        }
        let label = domain::Label {
            id: shared::LabelId::new(),
            project_id: project.id,
            name: name.trim().to_string().into(),
            color: color.to_string().into(),
        };
        self.labels.save(&label).await?;
        Ok(Self::to_dto(&label))
    }

    async fn list_by_project(
        &self,
        project_key: &ProjectKey,
    ) -> Result<Vec<crate::context::LabelDto>, AppError> {
        let project = self.projects.get_by_key(project_key).await?;
        let items = self.labels.list_by_project(project.id).await?;
        Ok(items.iter().map(Self::to_dto).collect())
    }

    async fn update(
        &self,
        label_id: shared::LabelId,
        name: &str,
        color: &str,
        requester: UserId,
    ) -> Result<crate::context::LabelDto, AppError> {
        let label = self.labels.get_by_id(label_id).await?;
        self.check_membership(label.project_id, requester).await?;
        let mut label = label;
        if !name.trim().is_empty() {
            label.name = name.trim().to_string().into();
        }
        label.color = color.to_string().into();
        self.labels.save(&label).await?;
        Ok(Self::to_dto(&label))
    }

    async fn delete(&self, label_id: shared::LabelId, requester: UserId) -> Result<(), AppError> {
        let label = self.labels.get_by_id(label_id).await?;
        self.check_membership(label.project_id, requester).await?;
        self.labels.delete(label_id).await?;
        Ok(())
    }

    async fn list_for_issue(
        &self,
        issue_id: IssueId,
    ) -> Result<Vec<crate::context::LabelDto>, AppError> {
        let ids = self.labels.list_ids_by_issue(issue_id).await?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let l = self.labels.get_by_id(id).await?;
            out.push(Self::to_dto(&l));
        }
        Ok(out)
    }

    async fn attach(
        &self,
        issue_id: IssueId,
        label_id: shared::LabelId,
        _requester: UserId,
    ) -> Result<(), AppError> {
        let _issue = self.issues.get_by_id(issue_id).await?;
        let _label = self.labels.get_by_id(label_id).await?;
        self.labels.attach(issue_id, label_id).await?;
        Ok(())
    }

    async fn detach(
        &self,
        issue_id: IssueId,
        label_id: shared::LabelId,
        _requester: UserId,
    ) -> Result<(), AppError> {
        self.labels.detach(issue_id, label_id).await?;
        Ok(())
    }
}
