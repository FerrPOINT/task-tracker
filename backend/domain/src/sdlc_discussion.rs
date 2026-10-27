//! Own-task discussion uses ordinary Tracker comments, not a second message store.

use crate::sdlc::{MachineFence, Principal};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::AppError;
use uuid::Uuid;

pub type CommentFence = MachineFence;

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
