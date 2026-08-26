use async_trait::async_trait;
use std::sync::Arc;

use domain::StatusRepository;
use shared::AppError;

pub struct StatusServiceImpl {
    statuses: Arc<dyn StatusRepository>,
}

impl StatusServiceImpl {
    pub fn new(statuses: Arc<dyn StatusRepository>) -> Self {
        Self { statuses }
    }
}

#[async_trait]
impl crate::context::StatusService for StatusServiceImpl {
    async fn list_statuses(&self) -> Result<Vec<domain::Status>, AppError> {
        self.statuses.list_all().await
    }
}
