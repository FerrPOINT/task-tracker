#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub q: Option<String>,
    pub project_key: Option<String>,
    pub priority: Option<String>,
    pub assignee_id: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub jql: Option<String>,
    pub user_id: Option<String>,
}