use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use crate::dto::ProjectDto;
use domain::{
    AttachmentRepository, Board, FileStorage, IssueQuery, IssueRepository, ProjectRepository,
    StatusCategory, StatusRepository,
};
use shared::{AppError, BoardId, ProjectId, ProjectKey, UserId};

fn normalize_project_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 200 {
        return Err(AppError::invalid_input(
            "project name must contain 1–200 characters",
        ));
    }
    Ok(name.to_owned())
}

pub struct ProjectServiceImpl {
    projects: Arc<dyn ProjectRepository>,
    issues: Arc<dyn IssueRepository>,
    attachments: Arc<dyn AttachmentRepository>,
    storage: Arc<dyn FileStorage>,
    users: Arc<dyn domain::UserRepository>,
    statuses: Arc<dyn StatusRepository>,
    authz: Authz,
}

impl ProjectServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        projects: Arc<dyn ProjectRepository>,
        issues: Arc<dyn IssueRepository>,
        users: Arc<dyn domain::UserRepository>,
        boards: Arc<dyn domain::BoardRepository>,
        statuses: Arc<dyn StatusRepository>,
        attachments: Arc<dyn AttachmentRepository>,
        storage: Arc<dyn FileStorage>,
        authz: Authz,
    ) -> Self {
        // `boards` is no longer stored: project creation persists the default
        // board atomically inside `ProjectRepository::save_with_board`.
        let _ = boards;
        Self {
            projects,
            issues,
            attachments,
            storage,
            users,
            statuses,
            authz,
        }
    }

    async fn persist_new(
        &self,
        cmd: crate::commands::CreateProjectCommand,
        spaces: Option<(
            &[shared::space_access::SpaceRef],
            &domain::project_spaces::SpaceWritePermit,
        )>,
    ) -> Result<ProjectDto, AppError> {
        if std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_some() {
            crate::authz::require_central_administrator()?;
        }
        let name = normalize_project_name(&cmd.name)?;
        let owner = self.users.get_by_id(cmd.owner_id).await?;
        let board_id = BoardId::new();
        let project = domain::Project {
            id: ProjectId::new(),
            key: cmd.key,
            name: name.into(),
            description: cmd.description.map(Into::into),
            owner_id: owner.id,
            default_board_id: board_id,
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        let board = Board {
            id: board_id,
            project_id: project.id,
            name: "Board".into(),
            columns: super::helpers::default_board_columns(),
        };
        // Project + default board must land atomically: a project whose
        // default_board_id points at a nonexistent board breaks board reads
        // and issue creation.
        if let Some((spaces, permit)) = spaces {
            self.projects
                .save_with_board_and_spaces(&project, &board, spaces, permit)
                .await?;
        } else {
            self.projects.save_with_board(&project, &board).await?;
        }
        Ok(ProjectDto::from_project(
            project,
            owner.display_name.as_ref().to_string(),
            0,
            0,
            0,
        ))
    }

    async fn project_dtos(
        &self,
        projects: Vec<domain::Project>,
    ) -> Result<Vec<ProjectDto>, AppError> {
        if projects.is_empty() {
            return Ok(Vec::new());
        }
        let statuses = self.statuses.list_all().await?;
        // One user-list fetch instead of a per-project owner lookup (N+1).
        let owner_names: std::collections::HashMap<_, _> = self
            .users
            .list()
            .await?
            .into_iter()
            .map(|u| (u.id, u.display_name.as_ref().to_string()))
            .collect();
        let counts = self
            .issues
            .status_category_counts(
                &projects.iter().map(|p| p.id).collect::<Vec<_>>(),
                &statuses,
            )
            .await?;
        let mut dtos = Vec::new();
        for project in projects {
            let [todo, in_progress, done] = counts.get(&project.id).copied().unwrap_or([0, 0, 0]);
            let owner_name = owner_names
                .get(&project.owner_id)
                .cloned()
                .unwrap_or_default();
            dtos.push(ProjectDto::from_project(
                project,
                owner_name,
                todo,
                in_progress,
                done,
            ));
        }
        Ok(dtos)
    }

    async fn count_by_status_categories(
        &self,
        project_id: ProjectId,
        statuses: &[domain::Status],
    ) -> Result<(i64, i64, i64), AppError> {
        let mut todo = 0;
        let mut in_progress = 0;
        let mut done = 0;
        for status in statuses {
            let count = self
                .issues
                .count_by_project_status(project_id, status.id)
                .await? as i64;
            match status.category {
                StatusCategory::Todo => todo += count,
                StatusCategory::InProgress => in_progress += count,
                StatusCategory::Done => done += count,
            }
        }
        Ok((todo, in_progress, done))
    }
}

#[async_trait]
impl crate::context::ProjectService for ProjectServiceImpl {
    async fn lifecycle(
        &self,
        key: &ProjectKey,
        actor: &crate::space_access::SpaceActor,
    ) -> Result<shared::project_spaces::ProjectLifecycle, AppError> {
        let project = self.projects.get_by_key(key).await?;
        let links = self.projects.project_spaces(project.id).await?;
        actor.require_read(&links)?;
        let life = self.projects.project_lifecycle(project.id).await?;
        if life.project != links.project {
            return Err(AppError::Unavailable(
                "project_lifecycle_identity_mismatch".into(),
            ));
        }
        actor.require_read(&links)?;
        Ok(life)
    }
    async fn change_lifecycle(
        &self,
        key: &ProjectKey,
        change: domain::project_spaces::ChangeProjectLifecycle,
        actor: &crate::space_access::SpaceActor,
    ) -> Result<shared::project_spaces::ProjectLifecycle, AppError> {
        change.validate()?;
        let project = self.projects.get_by_key(key).await?;
        if project.owner_id != actor.user() {
            return Err(AppError::Forbidden);
        }
        let links = self.projects.project_spaces(project.id).await?;
        actor.require_project_write(&links)?;
        // Restoration has a dedicated owner port; the generic writable gate
        // intentionally forbids editing an archived project's ordinary resources.
        self.projects
            .change_project_lifecycle(project.id, &change, &actor.permit()?)
            .await
    }
    async fn create(
        &self,
        cmd: crate::commands::CreateProjectCommand,
    ) -> Result<ProjectDto, AppError> {
        if self.projects.space_admission_enabled().await? {
            return Err(AppError::invalid_input("project_spaces_required_use_v2"));
        }
        self.persist_new(cmd, None).await
    }

    async fn create_in_spaces(
        &self,
        cmd: crate::commands::CreateProjectCommand,
        spaces: Vec<shared::space_access::SpaceRef>,
        actor: crate::space_access::SpaceActor,
    ) -> Result<ProjectDto, AppError> {
        if cmd.owner_id != actor.user() {
            return Err(AppError::Forbidden);
        }
        let permit = actor.permit()?;
        permit.require_set(&spaces)?;
        self.persist_new(cmd, Some((&spaces, &permit))).await
    }
    async fn spaces(
        &self,
        key: &ProjectKey,
        actor: &crate::space_access::SpaceActor,
    ) -> Result<shared::space_access::ConfirmedProjectSpaces, AppError> {
        let project = self.projects.get_by_key(key).await?;
        let links = self.projects.project_spaces(project.id).await?;
        actor.confirm_links(links)
    }
    async fn space_operation(
        &self,
        key: &ProjectKey,
        operation: uuid::Uuid,
        actor: &crate::space_access::SpaceActor,
    ) -> Result<domain::project_spaces::ProjectSpaceOperationReceipt, AppError> {
        if operation.is_nil() {
            return Err(AppError::invalid_input("invalid_project_space_operation"));
        }
        let project = self.projects.get_by_key(key).await?;
        let links = actor.confirm_links(self.projects.project_spaces(project.id).await?)?;
        let stored = self
            .projects
            .project_space_operation(project.id, operation, actor.identity())
            .await?
            .ok_or_else(|| AppError::not_found("project_space_operation", operation))?;
        actor.require_read(&links)?;
        let receipt = domain::project_spaces::ProjectSpaceOperationReceipt {
            schema_version: 1,
            project: stored.project,
            operation_id: stored.operation_id,
            applied: true,
            applied_revision: stored.applied_revision,
            change: stored.change,
            validity: links.validity,
        };
        if receipt.project != links.project || receipt.operation_id != operation || !receipt.valid()
        {
            return Err(AppError::Unavailable(
                "invalid_project_space_receipt".into(),
            ));
        }
        Ok(receipt)
    }
    async fn replace_spaces(
        &self,
        key: &ProjectKey,
        change: domain::project_spaces::ReplaceProjectSpaces,
        actor: &crate::space_access::SpaceActor,
    ) -> Result<shared::space_access::ConfirmedProjectSpaces, AppError> {
        change.validate()?;
        let project = self.projects.get_by_key(key).await?;
        if project.owner_id != actor.user() {
            return Err(AppError::Forbidden);
        }
        let previous = self.projects.project_spaces(project.id).await?;
        actor.require_read(&previous)?;
        let permit = actor.permit()?;
        if !permit.is_current() {
            return Err(AppError::Forbidden);
        }
        if !previous.spaces.iter().any(|space| permit.allows(space)) {
            return Err(AppError::Forbidden);
        }
        let links = self
            .projects
            .replace_project_spaces(project.id, &change, &permit)
            .await?;
        actor.confirm_links(links)
    }

    async fn visible_catalog(
        &self,
        query: domain::project_spaces::ProjectCatalogQuery,
        space: Option<&shared::space_access::SpaceRef>,
        actor: &crate::space_access::SpaceActor,
    ) -> Result<(Vec<ProjectDto>, u64), AppError> {
        query.validate()?;
        let allowed = actor.catalog_spaces(space, query.working_only)?;
        let page = self
            .projects
            .list_space_accessible_catalog(&allowed, &query)
            .await?;
        let dtos = self.project_dtos(page.projects).await?;
        actor.readable_spaces()?;
        Ok((dtos, page.total))
    }
    async fn list(
        &self,
        query: crate::commands::ProjectQueryDto,
        requester: UserId,
    ) -> Result<Vec<ProjectDto>, AppError> {
        let limit = if query.limit == 0 {
            50
        } else {
            query.limit.clamp(1, 100)
        };
        let policy_active = self.projects.space_admission_enabled().await?;
        let projects = if policy_active {
            let frame =
                crate::space_access::current_request_for(requester)?.ok_or(AppError::Forbidden)?;
            let actor = frame.spaces().ok_or(AppError::Forbidden)?;
            let projects = self
                .projects
                .list_space_accessible_page(&actor.readable_spaces()?, limit, query.offset)
                .await?;
            actor.readable_spaces()?;
            projects
        } else if std::env::var_os("TT_AUTH__CENTRAL_JWKS_URI").is_some() {
            self.projects
                .list(domain::ProjectQuery {
                    owner_id: None,
                    limit,
                    offset: query.offset,
                })
                .await?
        } else {
            let accessible = self.authz.accessible_project_ids(requester).await?;
            self.projects
                .list_accessible_page(&accessible, limit, query.offset)
                .await?
        };
        self.project_dtos(projects).await
    }

    async fn get_by_key(
        &self,
        key: &ProjectKey,
        requester: UserId,
    ) -> Result<ProjectDto, AppError> {
        let project = self.projects.get_by_key(key).await?;
        // Project metadata (key, name, owner, counts) is member-visible only;
        // an authenticated outsider must not enumerate other projects.
        self.authz
            .require_project_access(project.id, requester)
            .await?;
        let statuses = self.statuses.list_all().await?;
        let (todo, in_progress, done) = self
            .count_by_status_categories(project.id, &statuses)
            .await?;
        let owner_name = self
            .users
            .get_by_id(project.owner_id)
            .await
            .map(|u| u.display_name.as_ref().to_string())?;
        Ok(ProjectDto::from_project(
            project,
            owner_name,
            todo,
            in_progress,
            done,
        ))
    }

    async fn update(
        &self,
        key: &ProjectKey,
        cmd: crate::commands::UpdateProjectCommand,
        requester_id: UserId,
    ) -> Result<ProjectDto, AppError> {
        let project = self.projects.get_by_key(key).await?;
        self.authz.require_owner(project.id, requester_id).await?;
        let mut project = project;
        if let Some(name) = cmd.name {
            project.name = normalize_project_name(&name)?.into();
            project.updated_at = shared::now();
        }
        if let Some(description) = cmd.description {
            project.description = description.map(Into::into);
            project.updated_at = shared::now();
        }
        self.projects.save(&project).await?;
        let statuses = self.statuses.list_all().await?;
        let (todo, in_progress, done) = self
            .count_by_status_categories(project.id, &statuses)
            .await?;
        let owner_name = self
            .users
            .get_by_id(project.owner_id)
            .await
            .map(|u| u.display_name.as_ref().to_string())?;
        Ok(ProjectDto::from_project(
            project,
            owner_name,
            todo,
            in_progress,
            done,
        ))
    }

    async fn delete(&self, key: &ProjectKey, requester_id: UserId) -> Result<(), AppError> {
        let project = self.projects.get_by_key(key).await?;
        self.authz.require_owner(project.id, requester_id).await?;
        if self.projects.namespace_binding(project.id).await?.is_some() {
            return Err(AppError::conflict("managed_project_cannot_be_deleted"));
        }
        let issues = self
            .issues
            .list_unbounded(IssueQuery {
                project_id: Some(project.id),
                include_deleted: true,
                ..Default::default()
            })
            .await?;
        let mut attachment_keys = Vec::new();
        for issue in &issues {
            for attachment in self.attachments.list_by_issue(issue.id).await? {
                attachment_keys.push((
                    issue.id.to_string(),
                    attachment.storage_key.as_ref().to_string(),
                ));
            }
        }
        self.projects.delete(project.id).await?;
        // Files are external to the database transaction. Best-effort cleanup
        // happens only after the DB graph is gone.
        for (issue_id, storage_key) in attachment_keys {
            if let Err(error) = self.storage.delete(&issue_id, &storage_key).await {
                tracing::warn!(
                    issue_id = %issue_id,
                    storage_key = %storage_key,
                    error = %error,
                    "failed to delete project attachment"
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod name_validation_tests {
    use super::normalize_project_name;

    #[test]
    fn project_names_are_nonempty_bounded_and_normalized() {
        assert!(normalize_project_name("").is_err());
        assert!(normalize_project_name(" \t\n").is_err());
        assert!(normalize_project_name(&"я".repeat(201)).is_err());
        assert_eq!(
            normalize_project_name(&"я".repeat(200)).unwrap(),
            "я".repeat(200)
        );
        assert_eq!(
            normalize_project_name("  Проект QA  ").unwrap(),
            "Проект QA"
        );
    }
}
