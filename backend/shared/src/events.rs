use serde::Serialize;

/// Real-time event delivered over SSE at `/api/v1/events`.
///
/// Events are coarse-grained invalidation signals: clients react by
/// refetching the affected queries rather than applying patches.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TrackerEvent {
    IssueCreated {
        issue_id: String,
        project_key: String,
    },
    IssueUpdated {
        issue_id: String,
        project_key: String,
    },
    IssueMoved {
        issue_id: String,
        project_key: String,
    },
    IssueDeleted {
        issue_id: String,
        project_key: String,
    },
    IssueCommented {
        issue_id: String,
        project_key: String,
    },
    WorklogLogged {
        issue_id: String,
        project_key: String,
    },
    SprintChanged {
        project_key: String,
    },
    NotificationCreated {
        recipient_id: String,
    },
}

impl TrackerEvent {
    pub fn project_key(&self) -> &str {
        match self {
            TrackerEvent::IssueCreated { project_key, .. }
            | TrackerEvent::IssueUpdated { project_key, .. }
            | TrackerEvent::IssueMoved { project_key, .. }
            | TrackerEvent::IssueDeleted { project_key, .. }
            | TrackerEvent::IssueCommented { project_key, .. }
            | TrackerEvent::WorklogLogged { project_key, .. }
            | TrackerEvent::SprintChanged { project_key } => project_key,
            TrackerEvent::NotificationCreated { .. } => "",
        }
    }
}
