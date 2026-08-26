#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub q: Option<String>,
    pub project_key: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub assignee_id: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub jql: Option<String>,
    pub user_id: Option<String>,
}
