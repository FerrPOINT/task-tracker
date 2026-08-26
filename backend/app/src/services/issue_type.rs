use async_trait::async_trait;
use std::sync::Arc;

use domain::{IssueTypeEntity, IssueTypeRepository};
use shared::AppError;

pub struct IssueTypeServiceImpl {
    issue_types: Arc<dyn IssueTypeRepository>,
}

impl IssueTypeServiceImpl {
    pub fn new(issue_types: Arc<dyn IssueTypeRepository>) -> Self {
        Self { issue_types }
    }
}

#[async_trait]
impl crate::context::IssueTypeService for IssueTypeServiceImpl {
    async fn list_issue_types(&self) -> Result<Vec<IssueTypeEntity>, AppError> {
        self.issue_types.list_all().await
    }
}
