use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use shared::{
    AttachmentId, AuditLogId, BoardId, CommentId, IssueId, IssueKey, IssueLinkId,
    IssueStatusHistoryId, IssueType, IssueTypeId, LabelId, NotificationId, Priority, ProjectId,
    ProjectKey, SprintId, StatusId, Timestamp, UserId, WorkflowTransitionId, WorklogId,
};
use std::str::FromStr;

pub use crate::value_objects::{ArcStr, RichText};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub email: ArcStr,
    pub username: ArcStr,
    pub display_name: ArcStr,
    pub password_hash: ArcStr,
    pub refresh_token_hash: Option<ArcStr>,
    pub is_system_admin: bool,
    pub is_active: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl User {
    pub fn clear_refresh_token(&mut self) {
        self.refresh_token_hash = None;
        self.updated_at = shared::now();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub key: ProjectKey,
    pub name: ArcStr,
    pub description: Option<ArcStr>,
    pub owner_id: UserId,
    pub default_board_id: BoardId,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: IssueId,
    pub project_id: ProjectId,
    pub key: IssueKey,
    pub issue_type: IssueType,
    pub status_id: StatusId,
    pub summary: ArcStr,
    pub description: Option<RichText>,
    pub assignee_id: Option<UserId>,
    pub reporter_id: UserId,
    pub priority: Priority,
    pub labels: Vec<LabelId>,
    pub sprint_id: Option<SprintId>,
    pub position: f64,
    pub due_date: Option<Timestamp>,
    pub original_estimate_seconds: Option<i64>,
    pub remaining_estimate_seconds: Option<i64>,
    pub time_spent_seconds: i64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    /// Soft-delete timestamp: `None` for live issues, `Some(ts)` once moved to trash.
    /// Soft-deleted issues are filtered out of all normal queries unless
    /// `IssueQuery::include_deleted` is set.
    pub deleted_at: Option<Timestamp>,
    pub component_id: Option<shared::ProjectComponentId>,
    pub affected_version_id: Option<shared::ProjectVersionId>,
    pub fix_version_id: Option<shared::ProjectVersionId>,
    #[serde(skip)]
    pub events: Vec<crate::IssueEvent>,
}

impl Issue {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        project: &Project,
        number: u32,
        issue_type: IssueType,
        status_id: StatusId,
        summary: impl Into<ArcStr>,
        description: Option<RichText>,
        reporter_id: UserId,
        priority: Priority,
    ) -> Self {
        let now = shared::now();
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
            deleted_at: None,
            component_id: None,
            affected_version_id: None,
            fix_version_id: None,
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
            self.updated_at = shared::now();
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
            self.updated_at = shared::now();
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
            self.updated_at = shared::now();
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
    pub body: RichText,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
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
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: LabelId,
    pub project_id: ProjectId,
    pub name: ArcStr,
    pub color: ArcStr,
}

/// Field type for a project-level custom field definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CustomFieldType {
    Text,
    Number,
    Select,
    MultiSelect,
    Date,
}

impl std::str::FromStr for CustomFieldType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "text" => Ok(Self::Text),
            "number" => Ok(Self::Number),
            "select" => Ok(Self::Select),
            "multi-select" => Ok(Self::MultiSelect),
            "date" => Ok(Self::Date),
            other => Err(format!("unknown custom field type: {other}")),
        }
    }
}

impl CustomFieldType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Number => "number",
            Self::Select => "select",
            Self::MultiSelect => "multi-select",
            Self::Date => "date",
        }
    }
}

/// Project-level custom field definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomField {
    pub id: shared::CustomFieldId,
    pub project_id: ProjectId,
    pub name: ArcStr,
    pub field_type: CustomFieldType,
    /// Select / multi-select option labels. Empty for other field types.
    pub options: Vec<ArcStr>,
    pub is_required: bool,
    pub created_at: Timestamp,
}

/// Issue-level value for a custom field. `value` is JSONB; the shape depends
/// on the field type (string, number, array of strings, ISO date string, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomFieldValue {
    pub issue_id: IssueId,
    pub field_id: shared::CustomFieldId,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sprint {
    pub id: SprintId,
    pub project_id: ProjectId,
    pub name: ArcStr,
    pub goal: Option<ArcStr>,
    pub state: SprintState,
    pub start_date: Option<Timestamp>,
    pub end_date: Option<Timestamp>,
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

impl FromStr for SprintState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "future" => Ok(Self::Future),
            "active" => Ok(Self::Active),
            "closed" => Ok(Self::Closed),
            _ => Err(format!("unknown sprint state: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectComponent {
    pub id: shared::ProjectComponentId,
    pub project_id: ProjectId,
    pub name: ArcStr,
    pub description: Option<ArcStr>,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVersion {
    pub id: shared::ProjectVersionId,
    pub project_id: ProjectId,
    pub name: ArcStr,
    pub description: Option<ArcStr>,
    pub released: bool,
    pub release_date: Option<Timestamp>,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueTypeEntity {
    pub id: IssueTypeId,
    pub name: ArcStr,
    pub description: Option<ArcStr>,
    pub icon: Option<ArcStr>,
    pub color: Option<ArcStr>,
    pub is_subtask: bool,
    pub hierarchy_level: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub id: StatusId,
    pub name: ArcStr,
    pub category: StatusCategory,
    pub position: i32,
    pub is_default: bool,
    pub is_closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTransition {
    pub id: WorkflowTransitionId,
    pub name: Option<ArcStr>,
    pub from_status_id: StatusId,
    pub to_status_id: StatusId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueStatusHistory {
    pub id: IssueStatusHistoryId,
    pub issue_id: IssueId,
    pub from_status_id: Option<StatusId>,
    pub to_status_id: StatusId,
    pub changed_by_id: UserId,
    pub changed_at: Timestamp,
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
                    category: StatusCategory::Todo,
                    wip_limit: None,
                    position: 0,
                },
                BoardColumn {
                    id: StatusId::nil(),
                    name: "In Progress".into(),
                    category: StatusCategory::InProgress,
                    wip_limit: None,
                    position: 1,
                },
                BoardColumn {
                    id: StatusId::nil(),
                    name: "Done".into(),
                    category: StatusCategory::Done,
                    wip_limit: None,
                    position: 2,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusCategory {
    #[default]
    Todo,
    InProgress,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardColumn {
    pub id: StatusId,
    pub name: ArcStr,
    pub category: StatusCategory,
    pub wip_limit: Option<i64>,
    pub position: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worklog {
    pub id: WorklogId,
    pub issue_id: IssueId,
    pub author_id: UserId,
    pub started_at: Timestamp,
    pub duration_seconds: i64,
    pub description: Option<ArcStr>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone)]
pub struct ProjectMember {
    pub project_id: ProjectId,
    pub user_id: UserId,
    pub role: ProjectRole,
    pub joined_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProjectRole {
    #[default]
    Member,
    Admin,
    Owner,
}

impl ProjectRole {
    pub fn as_str(&self) -> &str {
        match self {
            ProjectRole::Member => "member",
            ProjectRole::Admin => "admin",
            ProjectRole::Owner => "owner",
        }
    }
}

impl std::str::FromStr for ProjectRole {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "admin" => Ok(ProjectRole::Admin),
            "owner" => Ok(ProjectRole::Owner),
            _ => Ok(ProjectRole::Member),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueLink {
    pub id: IssueLinkId,
    pub source_id: IssueId,
    pub target_id: IssueId,
    pub link_type: LinkType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkType {
    Blocks,
    Duplicates,
    Relates,
}

impl LinkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LinkType::Blocks => "blocks",
            LinkType::Duplicates => "duplicates",
            LinkType::Relates => "relates",
        }
    }
}

impl std::str::FromStr for LinkType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "blocks" => Ok(LinkType::Blocks),
            "duplicates" => Ok(LinkType::Duplicates),
            "relates" => Ok(LinkType::Relates),
            other => Err(format!("unknown link type: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    pub id: NotificationId,
    pub recipient_id: UserId,
    pub event_type: ArcStr,
    pub entity_type: ArcStr,
    pub entity_id: Option<uuid::Uuid>,
    pub actor_id: Option<UserId>,
    pub title: ArcStr,
    pub body: Option<ArcStr>,
    pub is_read: bool,
    pub read_at: Option<Timestamp>,
    pub action_url: Option<ArcStr>,
    pub metadata: serde_json::Value,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationUserSettings {
    pub user_id: UserId,
    pub email_frequency: ArcStr,
    pub disabled_event_types: Vec<ArcStr>,
    pub notify_own_changes: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: AuditLogId,
    pub actor_id: UserId,
    pub action: ArcStr,
    pub entity_type: ArcStr,
    pub entity_id: Option<uuid::Uuid>,
    pub metadata: serde_json::Value,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemSetting {
    pub key: ArcStr,
    pub value: serde_json::Value,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueWatcher {
    pub issue_id: IssueId,
    pub user_id: UserId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueVote {
    pub issue_id: IssueId,
    pub user_id: UserId,
    pub voted_at: Timestamp,
}
