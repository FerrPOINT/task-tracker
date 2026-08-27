use chrono::{DateTime, FixedOffset};
use domain::{Issue, Project, Sprint, SprintState, User};

use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "dto/tests.rs"]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDto {
    pub id: String,
    pub email: String,
    pub username: String,
    pub display_name: String,
    pub is_system_admin: bool,
}

impl From<User> for UserDto {
    fn from(user: User) -> Self {
        Self {
            id: user.id.to_string(),
            email: user.email.as_ref().to_string(),
            username: user.username.as_ref().to_string(),
            display_name: user.display_name.as_ref().to_string(),
            is_system_admin: user.is_system_admin,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDto {
    pub id: String,
    pub key: String,
    pub name: String,
    pub description: String,
    pub owner_id: String,
    pub created_at: DateTime<FixedOffset>,
    pub todo_count: i64,
    pub in_progress_count: i64,
    pub done_count: i64,
}

impl ProjectDto {
    pub fn from_project(project: Project, todo: i64, in_progress: i64, done: i64) -> Self {
        Self {
            id: project.id.to_string(),
            key: project.key.to_string(),
            name: project.name.as_ref().to_string(),
            description: project
                .description
                .as_ref()
                .map(|s| s.as_ref().to_string())
                .unwrap_or_default(),
            owner_id: project.owner_id.to_string(),
            created_at: project.created_at,
            todo_count: todo,
            in_progress_count: in_progress,
            done_count: done,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDto {
    pub id: String,
    pub key: String,
    pub summary: String,
    pub description: String,
    pub project_key: String,
    pub project_name: String,
    pub status: String,
    pub status_id: String,
    pub issue_type: String,
    pub assignee_id: Option<String>,
    pub assignee_name: Option<String>,
    pub reporter_id: String,
    pub reporter_name: Option<String>,
    pub priority: String,
    pub labels: Vec<String>,
    pub due_date: Option<DateTime<FixedOffset>>,
    pub original_estimate_seconds: Option<i64>,
    pub remaining_estimate_seconds: Option<i64>,
    pub time_spent_seconds: i64,
    pub position: f64,
    pub sprint_id: Option<String>,
    pub component_id: Option<String>,
    pub affected_version_id: Option<String>,
    pub fix_version_id: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

impl IssueDto {
    pub fn from_issue(
        issue: Issue,
        project_name: String,
        status_name: String,
        assignee_name: Option<String>,
        reporter_name: Option<String>,
    ) -> Self {
        Self {
            id: issue.id.to_string(),
            key: issue.key.to_string(),
            summary: issue.summary.as_ref().to_string(),
            description: issue
                .description
                .as_ref()
                .map(|d| d.as_ref().to_string())
                .unwrap_or_default(),
            project_key: issue.key.project_key.to_string(),
            project_name,
            status: status_name,
            status_id: issue.status_id.to_string(),
            issue_type: format!("{:?}", issue.issue_type).to_lowercase(),
            assignee_id: issue.assignee_id.map(|id| id.to_string()),
            assignee_name,
            reporter_id: issue.reporter_id.to_string(),
            reporter_name,
            priority: issue.priority.as_str().to_string(),
            labels: issue.labels.iter().map(|l| l.to_string()).collect(),
            due_date: issue.due_date,
            original_estimate_seconds: issue.original_estimate_seconds,
            remaining_estimate_seconds: issue.remaining_estimate_seconds,
            time_spent_seconds: issue.time_spent_seconds,
            position: issue.position,
            sprint_id: issue.sprint_id.map(|id| id.to_string()),
            component_id: issue.component_id.map(|id| id.to_string()),
            affected_version_id: issue.affected_version_id.map(|id| id.to_string()),
            fix_version_id: issue.fix_version_id.map(|id| id.to_string()),
            created_at: issue.created_at,
            updated_at: issue.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardColumnDto {
    pub id: String,
    pub name: String,
    pub wip_limit: Option<i64>,
    pub issue_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SprintDto {
    pub id: String,
    pub name: String,
    pub goal: String,
    pub state: String,
    pub velocity: i64,
    pub remaining_days: Option<i64>,
    pub issue_ids: Vec<String>,
    pub start_date: Option<DateTime<FixedOffset>>,
    pub end_date: Option<DateTime<FixedOffset>>,
}

impl SprintDto {
    pub fn from_sprint(sprint: Sprint, issue_ids: Vec<String>) -> Self {
        Self {
            id: sprint.id.to_string(),
            name: sprint.name.as_ref().to_string(),
            goal: sprint
                .goal
                .as_ref()
                .map(|s| s.as_ref().to_string())
                .unwrap_or_default(),
            state: match sprint.state {
                SprintState::Future => "future".to_string(),
                SprintState::Active => "active".to_string(),
                SprintState::Closed => "closed".to_string(),
            },
            velocity: sprint.velocity.unwrap_or(0),
            remaining_days: None,
            issue_ids,
            start_date: sprint.start_date,
            end_date: sprint.end_date,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardDto {
    pub project_id: String,
    pub project_key: String,
    pub columns: Vec<BoardColumnDto>,
    pub issues: Vec<IssueDto>,
    pub sprint: SprintDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacklogDto {
    pub project_id: String,
    pub project_key: String,
    /// Total backlog size before the response-window cap was applied.
    pub backlog_total: usize,
    pub sprint: SprintDto,
    pub sprint_issues: Vec<IssueDto>,
    pub backlog_issues: Vec<IssueDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardDto {
    pub assigned_issues: Vec<IssueDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthDto {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub user: UserDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentDto {
    pub id: String,
    pub issue_id: String,
    pub author_id: String,
    pub author_name: Option<String>,
    pub body: String,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

impl CommentDto {
    pub fn from_comment(comment: domain::Comment, author_name: Option<String>) -> Self {
        Self {
            id: comment.id.to_string(),
            issue_id: comment.issue_id.to_string(),
            author_id: comment.author_id.to_string(),
            author_name,
            body: comment.body.as_ref().to_string(),
            created_at: comment.created_at,
            updated_at: comment.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorklogDto {
    pub id: String,
    pub issue_id: String,
    pub author_id: String,
    pub author_name: Option<String>,
    pub started_at: DateTime<FixedOffset>,
    pub duration_seconds: i64,
    pub description: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

impl WorklogDto {
    pub fn from_worklog(worklog: domain::Worklog, author_name: Option<String>) -> Self {
        Self {
            id: worklog.id.to_string(),
            issue_id: worklog.issue_id.to_string(),
            author_id: worklog.author_id.to_string(),
            author_name,
            started_at: worklog.started_at,
            duration_seconds: worklog.duration_seconds,
            description: worklog.description.as_ref().map(|d| d.as_ref().to_string()),
            created_at: worklog.created_at,
            updated_at: worklog.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMemberDto {
    pub project_id: String,
    pub user_id: String,
    pub role: String,
    pub joined_at: DateTime<FixedOffset>,
}
