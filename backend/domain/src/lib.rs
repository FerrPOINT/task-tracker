pub mod email_port;
pub mod entities;
pub mod events;
pub mod jql;
pub mod repositories;
pub mod stubs;
pub mod value_objects;

pub use email_port::*;
pub use entities::*;
pub use events::*;
pub use repositories::*;
pub use stubs::*;
pub use value_objects::*;

use serde::{Deserialize, Serialize};
pub use shared::{IssueTypeId, ProjectId, SprintId, StatusId, UserId, WorkflowTransitionId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueQuery {
    pub project_id: Option<ProjectId>,
    /// Authorization scope for cross-project queries (search/dashboard):
    /// `Some(ids)` restricts results to these projects; `None` leaves the
    /// query unrestricted (single-project board/backlog paths that already
    /// authorize via `project_id`).
    pub accessible_project_ids: Option<Vec<ProjectId>>,
    pub sprint_id: Option<SprintId>,
    pub status_id: Option<StatusId>,
    pub assignee_id: Option<UserId>,
    pub priority: Option<String>,
    pub search_text: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub jql: Option<crate::jql::Expr>,
    pub jql_user_id: Option<shared::UserId>,
    pub limit: u64,
    pub offset: u64,
    /// When `true`, include soft-deleted (trashed) issues in the result set.
    /// Defaults to `false` so normal board/backlog/search/dashboard queries
    /// automatically exclude trashed issues.
    pub include_deleted: bool,
    /// When `true`, return *only* soft-deleted (trashed) issues. Mutually
    /// exclusive with `include_deleted`; used by the trash listing query.
    pub deleted_only: bool,
}

impl Default for IssueQuery {
    fn default() -> Self {
        Self {
            project_id: None,
            accessible_project_ids: None,
            sprint_id: None,
            status_id: None,
            assignee_id: None,
            priority: None,
            search_text: None,
            sort_by: None,
            sort_order: None,
            jql: None,
            jql_user_id: None,
            limit: 1000,
            offset: 0,
            include_deleted: false,
            deleted_only: false,
        }
    }
}

impl IssueQuery {
    pub fn project(project_id: ProjectId) -> Self {
        Self {
            project_id: Some(project_id),
            ..Default::default()
        }
    }

    pub fn assignee(assignee_id: UserId) -> Self {
        Self {
            assignee_id: Some(assignee_id),
            ..Default::default()
        }
    }

    pub fn with_sort(mut self, field: &str, order: &str) -> Self {
        self.sort_by = Some(field.to_string());
        self.sort_order = Some(order.to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusDto {
    pub id: String,
    pub name: String,
    pub category: String,
    pub position: i32,
    pub is_default: bool,
}
