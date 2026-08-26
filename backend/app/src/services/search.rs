use async_trait::async_trait;
use std::sync::Arc;

use crate::authz::Authz;
use crate::context::SearchFilters;
use crate::dto::IssueDto;
use domain::{IssueQuery, IssueRepository, ProjectRepository};
use shared::{AppError, ProjectKey, UserId};

pub struct SearchServiceImpl {
    issues: Arc<dyn IssueRepository>,
    projects: Arc<dyn ProjectRepository>,
    users: Arc<dyn domain::UserRepository>,
    statuses: Arc<dyn domain::StatusRepository>,
    authz: Authz,
}

impl SearchServiceImpl {
    pub fn new(
        issues: Arc<dyn IssueRepository>,
        projects: Arc<dyn ProjectRepository>,
        users: Arc<dyn domain::UserRepository>,
        statuses: Arc<dyn domain::StatusRepository>,
        authz: Authz,
    ) -> Self {
        Self {
            issues,
            projects,
            users,
            statuses,
            authz,
        }
    }
}

#[async_trait]
impl crate::context::SearchService for SearchServiceImpl {
    async fn search(
        &self,
        filters: SearchFilters,
        requester: UserId,
    ) -> Result<Vec<IssueDto>, AppError> {
        let mut query = IssueQuery::default();
        // Search is a list endpoint: keep responses bounded and reject a
        // zero/oversized page instead of silently loading every issue.
        if let Some(limit) = filters.limit {
            if !(1..=100).contains(&limit) {
                return Err(AppError::invalid_input("limit must be between 1 and 100"));
            }
            query.limit = limit;
        } else {
            query.limit = 50;
        }
        query.offset = filters.offset.unwrap_or(0);
        if let Some(q) = filters.q.as_deref().filter(|s| !s.is_empty()) {
            query.search_text = Some(q.to_string());
        }
        if let Some(priority) = filters.priority.as_deref().filter(|s| !s.is_empty()) {
            // DB stores canonical Title-Case values; accept any casing.
            let canonical = ["lowest", "low", "medium", "high", "highest"]
                .iter()
                .find(|p| p.eq_ignore_ascii_case(priority))
                .map(|p| {
                    let mut c = p.chars();
                    match c.next() {
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        None => String::new(),
                    }
                });
            match canonical {
                Some(p) => query.priority = Some(p),
                // Unknown priority: provably empty result set.
                None => return Ok(Vec::new()),
            }
        }
        if let Some(status) = filters.status.as_deref().filter(|s| !s.is_empty()) {
            // The UI filters by status name; issues store status ids.
            // Status names are human-cased ("To Do"); URL filters use
            // snake_case ("to_do") — normalize both sides.
            let norm = |v: &str| {
                v.replace('_', " ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join("")
                    .to_lowercase()
            };
            let wanted = norm(status);
            let all = self.statuses.list_all().await?;
            let matched = all.iter().find(|s| norm(s.name.as_ref()) == wanted);
            match matched {
                Some(s) => query.status_id = Some(s.id),
                None => {
                    // Unknown status name: provably empty result set.
                    return Ok(Vec::new());
                }
            }
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
            self.authz
                .require_project_access(project.id, requester)
                .await?;
            query.project_id = Some(project.id);
        } else {
            // Cross-project search must never leak issues from projects the
            // requester does not own or hold membership in.
            query.accessible_project_ids =
                Some(self.authz.accessible_project_ids(requester).await?);
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
