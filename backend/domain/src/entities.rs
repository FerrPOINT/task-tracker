use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::{
    AttachmentId, BoardId, CommentId, IssueId, IssueKey, IssueType, LabelId, Priority, ProjectId,
    ProjectKey, SprintId, StatusId, UserId,
};
use crate::value_objects::{ArcStr, RichText};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub email: ArcStr,
    pub username: ArcStr,
    pub display_name: ArcStr,
    pub password_hash: ArcStr,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub key: ProjectKey,
    pub name: ArcStr,
    pub description: Option<ArcStr>,
    pub owner_id: UserId,
    pub default_board_id: BoardId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: IssueId,
    pub project_id: ProjectId,
    pub key: IssueKey,
    pub issue_type: IssueType,
    pub status_id: StatusId,
    pub summary: ArcStr,
    pub description: Option<crate::RichText>,
    pub assignee_id: Option<UserId>,
    pub reporter_id: UserId,
    pub priority: Priority,
    pub labels: Vec<LabelId>,
    pub sprint_id: Option<SprintId>,
    pub position: f64,
    pub due_date: Option<DateTime<Utc>>,
    pub original_estimate_seconds: Option<i64>,
    pub remaining_estimate_seconds: Option<i64>,
    pub time_spent_seconds: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip)]
    pub events: Vec<crate::IssueEvent>,
}

impl Issue {
    pub fn create(
        project: &Project,
        number: u32,
        issue_type: IssueType,
        status_id: StatusId,
        summary: impl Into<ArcStr>,
        description: Option<crate::RichText>,
        reporter_id: UserId,
        priority: Priority,
    ) -> Self {
        let now = Utc::now();
        let mut issue = Self {
            id: IssueId::new(),
            project_id: project.id,
            key: IssueKey::new(project.key.clone(), number),
            issue_type,
            status_id,
            summary: summary.into(),
            description,
            assignee_id: None,
            reporter_id,
            priority,
            labels: Vec::new(),
            sprint_id: None,
            position: 0.0,
            due_date: None,
            original_estimate_seconds: None,
            remaining_estimate_seconds: None,
            time_spent_seconds: 0,
            created_at: now,
            updated_at: now,
            events: Vec::new(),
        };
        issue.events.push(crate::IssueEvent::Created {
            issue_id: issue.id,
            reporter_id,
        });
        issue
    }

    pub fn assign(&mut self, assignee_id: Option<UserId>) {
        if self.assignee_id != assignee_id {
            self.assignee_id = assignee_id;
            self.updated_at = Utc::now();
            self.events.push(crate::IssueEvent::Assigned {
                issue_id: self.id,
                assignee_id,
            });
        }
    }

    pub fn change_status(&mut self, to: StatusId) {
        if self.status_id != to {
            let from = self.status_id;
            self.status_id = to;
            self.updated_at = Utc::now();
            self.events.push(crate::IssueEvent::StatusChanged {
                issue_id: self.id,
                from,
                to,
            });
        }
    }

    pub fn set_position(&mut self, position: f64) {
        if (self.position - position).abs() > f64::EPSILON {
            self.position = position;
            self.updated_at = Utc::now();
        }
    }

    pub fn take_events(&mut self) -> Vec<crate::IssueEvent> {
        std::mem::take(&mut self.events)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: CommentId,
    pub issue_id: IssueId,
    pub author_id: UserId,
    pub body: crate::RichText,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: AttachmentId,
    pub issue_id: IssueId,
    pub author_id: UserId,
    pub file_name: ArcStr,
    pub content_type: ArcStr,
    pub size_bytes: i64,
    pub storage_key: ArcStr,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: LabelId,
    pub project_id: ProjectId,
    pub name: ArcStr,
    pub color: ArcStr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sprint {
    pub id: SprintId,
    pub project_id: ProjectId,
    pub name: ArcStr,
    pub goal: Option<ArcStr>,
    pub state: SprintState,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub velocity: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SprintState {
    #[default]
    Future,
    Active,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub id: BoardId,
    pub project_id: ProjectId,
    pub name: ArcStr,
    pub columns: Vec<BoardColumn>,
}

impl Default for Board {
    fn default() -> Self {
        Self {
            id: BoardId::new(),
            project_id: ProjectId::nil(),
            name: "Board".into(),
            columns: vec![
                BoardColumn {
                    id: StatusId::nil(),
                    name: "To Do".into(),
                    category: ColumnCategory::Todo,
                    wip_limit: None,
                    position: 0,
                },
                BoardColumn {
                    id: StatusId::nil(),
                    name: "In Progress".into(),
                    category: ColumnCategory::InProgress,
                    wip_limit: None,
                    position: 1,
                },
                BoardColumn {
                    id: StatusId::nil(),
                    name: "Done".into(),
                    category: ColumnCategory::Done,
                    wip_limit: None,
                    position: 2,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardColumn {
    pub id: StatusId,
    pub name: ArcStr,
    pub category: ColumnCategory,
    pub wip_limit: Option<i64>,
    pub position: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColumnCategory {
    #[default]
    Todo,
    InProgress,
    Done,
}
