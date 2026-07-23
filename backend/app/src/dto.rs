use chrono::{DateTime, Utc};
use domain::{Issue, Project, Sprint, SprintState, User};
use serde::{Deserialize, Serialize};
use shared::{IssueId, IssueKey, ProjectId, ProjectKey, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDto {
    pub id: String,
    pub email: String,
    pub name: String,
}

impl From<User> for UserDto {
    fn from(user: User) -> Self {
        Self {
            id: user.id.to_string(),
            email: user.email.as_ref().to_string(),
            name: user.display_name.as_ref().to_string(),
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
    pub created_at: DateTime<Utc>,
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
    pub due_date: Option<DateTime<Utc>>,
    pub original_estimate_seconds: Option<i64>,
    pub remaining_estimate_seconds: Option<i64>,
    pub time_spent_seconds: i64,
    pub position: f64,
    pub sprint_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl IssueDto {
    pub fn from_issue(issue: Issue, project_name: String, status_name: String) -> Self {
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
            assignee_name: None,
            reporter_id: issue.reporter_id.to_string(),
            reporter_name: None,
            priority: issue.priority.as_str().to_string(),
            labels: issue.labels.iter().map(|l| l.to_string()).collect(),
            due_date: issue.due_date,
            original_estimate_seconds: issue.original_estimate_seconds,
            remaining_estimate_seconds: issue.remaining_estimate_seconds,
            time_spent_seconds: issue.time_spent_seconds,
            position: issue.position,
            sprint_id: issue.sprint_id.map(|id| id.to_string()),
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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardDto {
    pub columns: Vec<BoardColumnDto>,
    pub issues: Vec<IssueDto>,
    pub sprint: SprintDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacklogDto {
    pub sprint: SprintDto,
    pub sprint_issues: Vec<IssueDto>,
    pub backlog_issues: Vec<IssueDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthDto {
    pub token: String,
    pub user: UserDto,
}
