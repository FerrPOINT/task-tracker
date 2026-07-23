use serde::{Deserialize, Serialize};
use shared::{IssueId, StatusId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IssueEvent {
    Created {
        issue_id: IssueId,
        reporter_id: UserId,
    },
    StatusChanged {
        issue_id: IssueId,
        from: StatusId,
        to: StatusId,
    },
    Assigned {
        issue_id: IssueId,
        assignee_id: Option<UserId>,
    },
    CommentAdded {
        issue_id: IssueId,
        comment_id: shared::CommentId,
        author_id: UserId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProjectEvent {
    Created {
        project_id: shared::ProjectId,
        owner_id: UserId,
    },
}
