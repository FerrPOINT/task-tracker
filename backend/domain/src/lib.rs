pub mod entities;
pub mod events;
pub mod repositories;
pub mod value_objects;

pub use entities::*;
pub use events::*;
pub use repositories::*;
pub use value_objects::*;

use serde::{Deserialize, Serialize};
use shared::{ProjectId, SprintId, StatusId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueQuery {
    pub project_id: Option<ProjectId>,
    pub sprint_id: Option<SprintId>,
    pub status_id: Option<StatusId>,
    pub assignee_id: Option<UserId>,
    pub search_text: Option<String>,
    pub limit: u64,
    pub offset: u64,
}

impl Default for IssueQuery {
    fn default() -> Self {
        Self {
            project_id: None,
            sprint_id: None,
            status_id: None,
            assignee_id: None,
            search_text: None,
            limit: 1000,
            offset: 0,
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
}
