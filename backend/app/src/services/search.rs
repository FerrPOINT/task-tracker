use async_trait::async_trait;
use std::sync::Arc;

use crate::context::SearchFilters;
use crate::dto::IssueDto;
use domain::{IssueQuery, IssueRepository, ProjectRepository};
use shared::{AppError, ProjectKey, UserId};

pub struct SearchServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
}

impl SearchServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        users: Arc<dyn domain::UserRepository>,
    ) -> Self {
        Self {
            issues,
            projects,
            users,
        }
    }
}

#[async_trait]
impl crate::context::SearchService for SearchServiceImpl {
    async fn search(&self, filters: SearchFilters) -> Result<Vec<IssueDto>, AppError> {
        let mut query = IssueQuery::default();
        if let Some(q) = filters.q.as_deref().filter(|s| !s.is_empty()) {
            query.search_text = Some(q.to_string());
        }
        if let Some(priority) = filters.priority.as_deref().filter(|s| !s.is_empty()) {
            query.priority = Some(priority.to_string());
        }
        if let Some(sort_by) = filters.sort_by.as_deref() {
            query.sort_by = Some(sort_by.to_string());
            query.sort_order = filters.sort_order.clone();
        }
        if let Some(project_key) = filters.project_key.as_deref().filter(|s| !s.is_empty()) {
            let key: ProjectKey = project_key
                .parse()
                .map_err(|e: String| AppError::invalid_input(e))?;
            let project = self.projects.get_by_key(&key).await?;
            query.project_id = Some(project.id);
        }
        if let Some(assignee_id) = filters.assignee_id.as_deref().filter(|s| !s.is_empty()) {
            let uuid = uuid::Uuid::parse_str(assignee_id)
                .map_err(|e| AppError::invalid_input(e.to_string()))?;
            query.assignee_id = Some(UserId::from_uuid(uuid));
        }
        if let Some(jql_str) = filters.jql.as_deref().filter(|s| !s.is_empty()) {
            let expr =
                domain::jql::parse(jql_str).map_err(|e| AppError::invalid_input(e.to_string()))?;
            query.jql = Some(expr);
            if let Some(uid_str) = filters.user_id.as_deref().filter(|s| !s.is_empty()) {
                let uuid = uuid::Uuid::parse_str(uid_str)
                    .map_err(|e| AppError::invalid_input(e.to_string()))?;
                query.jql_user_id = Some(UserId::from_uuid(uuid));
            }
        }
        let issues = self.issues.list(query).await?;
        super::helpers::build_issue_dtos_with_projects(
            Arc::clone(&self.projects),
            Arc::clone(&self.users),
            issues,
        )
        .await
    }
}
