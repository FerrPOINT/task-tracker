use async_trait::async_trait;
use std::sync::Arc;

use crate::context::VoteService;
use domain::{IssueRepository, VoteRepository};
use shared::{AppError, IssueId, UserId};

pub struct VoteServiceImpl {
    votes: Arc<dyn domain::VoteRepository>,
    issues: Arc<dyn IssueRepository>,
}

impl VoteServiceImpl {
    pub fn new(votes: Arc<dyn domain::VoteRepository>, issues: Arc<dyn IssueRepository>) -> Self {
        Self { votes, issues }
    }
}

#[async_trait]
impl crate::context::VoteService for VoteServiceImpl {
    async fn vote(
        &self,
        issue_id: IssueId,
        user_id: UserId,
    ) -> Result<crate::context::VoteDto, AppError> {
        // Verify the issue exists
        self.issues.get_by_id(issue_id).await?;
        let vote = self.votes.add(issue_id, user_id).await?;
        Ok(crate::context::VoteDto {
            user_id: vote.user_id.to_string(),
            username: String::new(),
            display_name: String::new(),
            voted_at: vote.voted_at.to_rfc3339(),
        })
    }

    async fn unvote(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError> {
        self.votes.remove(issue_id, user_id).await?;
        Ok(())
    }

    async fn list_votes(
        &self,
        issue_id: IssueId,
    ) -> Result<Vec<crate::context::VoteDto>, AppError> {
        let votes = self.votes.list_by_issue(issue_id).await?;
        Ok(votes
            .into_iter()
            .map(|v| crate::context::VoteDto {
                user_id: v.user_id.to_string(),
                username: String::new(),
                display_name: String::new(),
                voted_at: v.voted_at.to_rfc3339(),
            })
            .collect())
    }

    async fn count_votes(&self, issue_id: IssueId) -> Result<u64, AppError> {
        self.votes.count_by_issue(issue_id).await
    }

    async fn has_voted(&self, issue_id: IssueId, user_id: UserId) -> Result<bool, AppError> {
        self.votes.has_voted(issue_id, user_id).await
    }
}