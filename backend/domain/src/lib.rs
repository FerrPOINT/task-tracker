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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IssueQuery {
    pub project_id: Option<ProjectId>,
    pub sprint_id: Option<SprintId>,
    pub status_id: Option<StatusId>,
    pub assignee_id: Option<UserId>,
    pub search_text: Option<String>,
    pub limit: u64,
    pub offset: u64,
}
