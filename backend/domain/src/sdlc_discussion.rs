//! Own-task discussion uses ordinary Tracker comments, not a second message store.

use crate::sdlc::{MachineFence, Principal, safe_version};
use crate::sdlc_lifecycle::DeliveryFence;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::AppError;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommentFence {
    Pm {
        fence: MachineFence,
        lease_id: Uuid,
        #[serde(deserialize_with = "safe_version")]
        expected_lease_version: i64,
    },
    Delivery {
        fence: DeliveryFence,
    },
}

impl CommentFence {
    pub fn assignment_id(&self) -> Uuid {
        match self {
            Self::Pm { fence, .. } => fence.assignment_id,
            Self::Delivery { fence } => fence.assignment_id,
        }
    }
    pub fn execution_id(&self) -> Uuid {
        match self {
            Self::Pm { fence, .. } => fence.execution_id,
            Self::Delivery { fence } => fence.execution_id,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PublishStageComment {
    pub fence: CommentFence,
    pub body: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StageCommentReceipt {
    pub comment_id: Uuid,
    pub task_id: Uuid,
    pub author_id: Uuid,
    pub author_subject: String,
    pub agent_id: Uuid,
    pub fence: CommentFence,
    pub requirement_revision: Option<i64>,
    pub body_sha256: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct DiscussionComment {
    pub id: Uuid,
    pub author_id: Uuid,
    pub author_name: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct TaskDiscussion {
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub description: String,
    pub comments: Vec<DiscussionComment>,
    /// Upper bound captured on the first page. New comments require a fresh scan.
    pub snapshot: Option<Uuid>,
    pub next_after: Option<Uuid>,
}

#[derive(Clone, Debug, Default, Deserialize, utoipa::IntoParams)]
#[serde(deny_unknown_fields)]
pub struct DiscussionQuery {
    pub limit: Option<u16>,
    pub after: Option<Uuid>,
    pub snapshot: Option<Uuid>,
}

#[async_trait]
pub trait DiscussionRepository: Send + Sync {
    async fn discussion(
        &self,
        task: Uuid,
        actor: &Principal,
        query: DiscussionQuery,
    ) -> Result<TaskDiscussion, AppError>;
    async fn publish_stage_comment(
        &self,
        task: Uuid,
        actor: &Principal,
        command: PublishStageComment,
    ) -> Result<(StageCommentReceipt, bool), AppError>;
    async fn stage_comment_operation(
        &self,
        task: Uuid,
        actor: &Principal,
        key: &str,
    ) -> Result<StageCommentReceipt, AppError>;
}
