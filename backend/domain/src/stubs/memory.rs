use async_trait::async_trait;
use std::sync::{Arc, Mutex};

#[cfg(test)]
#[path = "memory/tests.rs"]
mod tests;

use crate::{
    AuditLog, AuditLogRepository, Board, BoardRepository, Comment, CommentRepository, EventBus,
    Issue, IssueQuery, IssueRepository, IssueStatusHistory, IssueStatusHistoryRepository,
    IssueVote, IssueWatcher, Notification, NotificationRepository, NotificationUserSettings,
    Project, ProjectComponent, ProjectComponentRepository, ProjectMember, ProjectMemberRepository,
    ProjectQuery, ProjectRepository, ProjectVersion, ProjectVersionRepository, Sprint,
    SprintRepository, Status, StatusRepository, SystemSetting, SystemSettingRepository,
    TransitionGuard, UnitOfWork, User, UserNotificationSettingsRepository, UserRepository,
    VoteRepository, WatcherRepository, Worklog, WorklogRepository,
};
use shared::{
    AppError, BoardId, CommentId, CustomFieldId, IssueId, NotificationId, ProjectComponentId,
    ProjectId, ProjectKey, ProjectVersionId, SprintId, StatusId, UserId, WorklogId,
};

#[derive(Default)]
pub struct MemoryUserRepository {
    users: Arc<Mutex<Vec<User>>>,
}

#[async_trait]
impl UserRepository for MemoryUserRepository {
    async fn rotate_refresh_token(
        &self,
        user_id: UserId,
        expected_hash: &str,
        new_hash: &str,
    ) -> Result<(), AppError> {
        let mut users = self.users.lock().unwrap();
        let user = users
            .iter_mut()
            .find(|u| u.id == user_id)
            .ok_or_else(|| AppError::not_found("user", user_id))?;
        if user.refresh_token_hash.as_deref() != Some(expected_hash) {
            return Err(AppError::Unauthorized);
        }
        user.refresh_token_hash = Some(new_hash.to_string().into());
        user.updated_at = shared::now();
        Ok(())
    }

    async fn get_by_id(&self, id: UserId) -> Result<User, AppError> {
        let users = self.users.lock().unwrap();
        users
            .iter()
            .find(|u| u.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("user", id))
    }

    async fn get_by_email(&self, email: &str) -> Result<User, AppError> {
        let users = self.users.lock().unwrap();
        users
            .iter()
            .find(|u| u.email.as_ref() == email)
            .cloned()
            .ok_or_else(|| AppError::not_found("user", email))
    }

    async fn get_by_refresh_token(&self, token_hash: &str) -> Result<User, AppError> {
        let users = self.users.lock().unwrap();
        users
            .iter()
            .find(|u| {
                u.refresh_token_hash
                    .as_ref()
                    .is_some_and(|h| h.as_ref() == token_hash)
            })
            .cloned()
            .ok_or_else(|| AppError::not_found("user", "refresh"))
    }

    async fn save(&self, user: &User) -> Result<UserId, AppError> {
        let mut users = self.users.lock().unwrap();
        if let Some(idx) = users.iter().position(|u| u.id == user.id) {
            users[idx] = user.clone();
        } else {
            users.push(user.clone());
        }
        Ok(user.id)
    }

    async fn list(&self) -> Result<Vec<User>, AppError> {
        let users = self.users.lock().unwrap();
        Ok(users.clone())
    }
}

#[derive(Default)]
pub struct MemoryProjectRepository {
    projects: Arc<Mutex<Vec<Project>>>,
    issue_counters: Arc<Mutex<std::collections::HashMap<ProjectId, u32>>>,
}

#[async_trait]
impl ProjectRepository for MemoryProjectRepository {
    async fn get_by_id(&self, id: ProjectId) -> Result<Project, AppError> {
        let projects = self.projects.lock().unwrap();
        projects
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("project", id))
    }

    async fn get_by_key(&self, key: &ProjectKey) -> Result<Project, AppError> {
        let projects = self.projects.lock().unwrap();
        projects
            .iter()
            .find(|p| &p.key == key)
            .cloned()
            .ok_or_else(|| AppError::not_found("project", key))
    }

    async fn list(&self, query: ProjectQuery) -> Result<Vec<Project>, AppError> {
        let projects = self.projects.lock().unwrap();
        Ok(projects
            .iter()
            .filter(|p| query.owner_id.is_none_or(|owner| p.owner_id == owner))
            .cloned()
            .collect())
    }

    async fn save(&self, project: &Project) -> Result<ProjectId, AppError> {
        let mut projects = self.projects.lock().unwrap();
        if let Some(idx) = projects.iter().position(|p| p.id == project.id) {
            projects[idx] = project.clone();
        } else {
            if projects.iter().any(|p| p.key == project.key) {
                return Err(AppError::conflict(format!(
                    "project key {} already exists",
                    project.key
                )));
            }
            projects.push(project.clone());
        }
        Ok(project.id)
    }

    async fn save_with_board(
        &self,
        project: &Project,
        _board: &Board,
    ) -> Result<ProjectId, AppError> {
        // In-memory saves are trivially atomic; board storage lives in
        // MemoryBoardRepository, which tests register explicitly.
        self.save(project).await
    }

    async fn next_issue_number(&self, project_id: ProjectId) -> Result<u32, AppError> {
        // Monotonic per-project counter so sequential and concurrent creation
        // never yields duplicate keys (mirrors the SQL repo MAX(number)+1 retry).
        let mut counters = self.issue_counters.lock().unwrap();
        let next = counters.entry(project_id).or_insert(0);
        *next += 1;
        Ok(*next)
    }

    async fn delete(&self, id: ProjectId) -> Result<(), AppError> {
        let mut projects = self.projects.lock().unwrap();
        if let Some(idx) = projects.iter().position(|p| p.id == id) {
            projects.remove(idx);
            Ok(())
        } else {
            Err(AppError::not_found("project", id))
        }
    }
}

#[derive(Default)]
pub struct MemoryIssueRepository {
    issues: Arc<Mutex<Vec<Issue>>>,
    /// Shared with `MemoryIssueStatusHistoryRepository` when wired via
    /// `with_shared_history`, so atomic writes are visible to reports.
    history: Arc<Mutex<Vec<IssueStatusHistory>>>,
    history_project_ids: Arc<Mutex<Vec<ProjectId>>>,
    custom_field_values: Arc<Mutex<Vec<crate::CustomFieldValue>>>,
}

impl MemoryIssueRepository {
    pub fn with_shared_stores(
        history: Arc<Mutex<Vec<IssueStatusHistory>>>,
        project_ids: Arc<Mutex<Vec<ProjectId>>>,
        custom_field_values: Arc<Mutex<Vec<crate::CustomFieldValue>>>,
    ) -> Self {
        Self {
            issues: Arc::new(Mutex::new(Vec::new())),
            history,
            history_project_ids: project_ids,
            custom_field_values,
        }
    }

    /// Wire the issue repo to the same store the history repository reads,
    /// so `change_status_atomic` history entries appear in reports.
    pub fn with_shared_history(
        history: Arc<Mutex<Vec<IssueStatusHistory>>>,
        project_ids: Arc<Mutex<Vec<ProjectId>>>,
    ) -> Self {
        Self::with_shared_stores(history, project_ids, Arc::new(Mutex::new(Vec::new())))
    }

    pub fn with_shared_custom_fields(
        custom_field_values: Arc<Mutex<Vec<crate::CustomFieldValue>>>,
    ) -> Self {
        Self::with_shared_stores(
            Arc::new(Mutex::new(Vec::new())),
            Arc::new(Mutex::new(Vec::new())),
            custom_field_values,
        )
    }
}

#[async_trait]
impl IssueRepository for MemoryIssueRepository {
    async fn change_status_atomic(
        &self,
        issue_id: IssueId,
        project_id: ProjectId,
        from_status_id: StatusId,
        to_status_id: StatusId,
        actor_id: UserId,
        guard: &TransitionGuard,
    ) -> Result<(), AppError> {
        // Single mutex acquisition = critical section: count and write are
        // serialized, so concurrent movers cannot both pass the check.
        let mut issues = self.issues.lock().unwrap();
        let target_count = issues
            .iter()
            .filter(|i| {
                i.project_id == project_id && i.status_id == to_status_id && i.deleted_at.is_none()
            })
            .count() as u64;
        let guard = TransitionGuard {
            target_count,
            ..guard.clone()
        };
        guard.ensure_wip_ok()?;
        let issue = issues
            .iter_mut()
            .find(|i| i.id == issue_id && i.deleted_at.is_none())
            .ok_or_else(|| AppError::not_found("issue", issue_id))?;
        if issue.project_id != project_id {
            return Err(AppError::invalid_input(
                "issue does not belong to this project",
            ));
        }
        if issue.status_id != from_status_id {
            return Err(AppError::conflict("issue status changed concurrently"));
        }
        issue.status_id = to_status_id;
        issue.updated_at = shared::now();
        drop(issues);
        self.history.lock().unwrap().push(IssueStatusHistory {
            id: shared::IssueStatusHistoryId::new(),
            issue_id,
            from_status_id: Some(from_status_id),
            to_status_id,
            changed_by_id: actor_id,
            changed_at: shared::now(),
        });
        self.history_project_ids.lock().unwrap().push(project_id);
        Ok(())
    }

    async fn get_by_id(&self, id: IssueId) -> Result<Issue, AppError> {
        let issues = self.issues.lock().unwrap();
        issues
            .iter()
            .find(|i| i.id == id && i.deleted_at.is_none())
            .cloned()
            .ok_or_else(|| AppError::not_found("issue", id))
    }

    async fn get_by_id_include_deleted(&self, id: IssueId) -> Result<Issue, AppError> {
        let issues = self.issues.lock().unwrap();
        issues
            .iter()
            .find(|i| i.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("issue", id))
    }

    async fn get_by_key(&self, key: &shared::IssueKey) -> Result<Issue, AppError> {
        let issues = self.issues.lock().unwrap();
        issues
            .iter()
            .find(|i| &i.key == key && i.deleted_at.is_none())
            .cloned()
            .ok_or_else(|| AppError::not_found("issue", key))
    }

    async fn list(&self, query: IssueQuery) -> Result<Vec<Issue>, AppError> {
        let issues = self.issues.lock().unwrap();
        let mut result: Vec<Issue> = issues
            .iter()
            // Soft-delete filtering: by default exclude trashed issues.
            // `include_deleted` returns everything; `deleted_only` returns only trashed.
            .filter(|i| {
                if query.deleted_only {
                    i.deleted_at.is_some()
                } else if query.include_deleted {
                    true
                } else {
                    i.deleted_at.is_none()
                }
            })
            .filter(|i| query.project_id.is_none_or(|pid| i.project_id == pid))
            .filter(|i| {
                query
                    .accessible_project_ids
                    .as_ref()
                    .is_none_or(|ids| ids.contains(&i.project_id))
            })
            .filter(|i| query.status_id.is_none_or(|sid| i.status_id == sid))
            .filter(|i| {
                query
                    .assignee_id
                    .is_none_or(|aid| i.assignee_id == Some(aid))
            })
            .filter(|i| {
                query
                    .priority
                    .as_deref()
                    .is_none_or(|priority| i.priority.as_str() == priority)
            })
            .filter(|i| query.sprint_id.is_none_or(|spid| i.sprint_id == Some(spid)))
            .filter(|i| {
                query.search_text.as_ref().is_none_or(|q| {
                    i.summary
                        .as_ref()
                        .to_lowercase()
                        .contains(&q.to_lowercase())
                        || i.key.to_string().to_lowercase().contains(&q.to_lowercase())
                        || i.description.as_ref().is_some_and(|description| {
                            description
                                .as_ref()
                                .to_lowercase()
                                .contains(&q.to_lowercase())
                        })
                })
            })
            .filter(|i| {
                query
                    .jql
                    .as_ref()
                    .is_none_or(|expr| jql_matches_issue(expr, i, query.jql_user_id))
            })
            .cloned()
            .collect();
        match query.sort_by.as_deref() {
            Some("created") => result.sort_by(|a, b| {
                compare_with_order(
                    a.created_at,
                    b.created_at,
                    a.id.to_string(),
                    b.id.to_string(),
                    query.sort_order.as_deref(),
                )
            }),
            Some("updated") => result.sort_by(|a, b| {
                compare_with_order(
                    a.updated_at,
                    b.updated_at,
                    a.id.to_string(),
                    b.id.to_string(),
                    query.sort_order.as_deref(),
                )
            }),
            _ => result.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap()),
        }
        let offset = query.offset as usize;
        let limit = query.limit as usize;
        Ok(result.into_iter().skip(offset).take(limit).collect())
    }

    async fn save(&self, issue: &Issue) -> Result<IssueId, AppError> {
        let mut issues = self.issues.lock().unwrap();
        if let Some(idx) = issues.iter().position(|i| i.id == issue.id) {
            issues[idx] = issue.clone();
        } else {
            issues.push(issue.clone());
        }
        Ok(issue.id)
    }

    async fn create_with_initial_data(
        &self,
        issue: &Issue,
        status_history: &IssueStatusHistory,
        custom_field_values: &[(CustomFieldId, serde_json::Value)],
    ) -> Result<IssueId, AppError> {
        let mut issues = self.issues.lock().unwrap();
        if issues.iter().any(|i| i.key == issue.key) {
            return Err(AppError::conflict("duplicate entry"));
        }
        issues.push(issue.clone());
        drop(issues);

        self.history.lock().unwrap().push(status_history.clone());
        self.history_project_ids
            .lock()
            .unwrap()
            .push(issue.project_id);

        let mut values = self.custom_field_values.lock().unwrap();
        for (field_id, value) in custom_field_values {
            values.push(crate::CustomFieldValue {
                issue_id: issue.id,
                field_id: *field_id,
                value: value.clone(),
            });
        }
        Ok(issue.id)
    }

    async fn delete(&self, id: IssueId) -> Result<(), AppError> {
        let mut issues = self.issues.lock().unwrap();
        if let Some(idx) = issues.iter().position(|i| i.id == id) {
            if issues[idx].deleted_at.is_some() {
                return Err(AppError::invalid_input("issue already deleted"));
            }
            issues[idx].deleted_at = Some(shared::now());
            Ok(())
        } else {
            Err(AppError::not_found("issue", id))
        }
    }

    async fn restore(&self, id: IssueId) -> Result<(), AppError> {
        let mut issues = self.issues.lock().unwrap();
        if let Some(idx) = issues.iter().position(|i| i.id == id) {
            if issues[idx].deleted_at.is_none() {
                return Err(AppError::invalid_input("issue is not deleted"));
            }
            issues[idx].deleted_at = None;
            Ok(())
        } else {
            Err(AppError::not_found("issue", id))
        }
    }

    async fn purge(&self, id: IssueId) -> Result<(), AppError> {
        let mut issues = self.issues.lock().unwrap();
        if let Some(idx) = issues.iter().position(|i| i.id == id) {
            if issues[idx].deleted_at.is_none() {
                return Err(AppError::invalid_input(
                    "issue is not deleted; soft-delete before purging",
                ));
            }
            issues.remove(idx);
            Ok(())
        } else {
            Err(AppError::not_found("issue", id))
        }
    }
}

fn compare_with_order<T: Ord>(
    a: T,
    b: T,
    a_tie: String,
    b_tie: String,
    sort_order: Option<&str>,
) -> std::cmp::Ordering {
    match sort_order {
        Some("asc") => a.cmp(&b).then_with(|| a_tie.cmp(&b_tie)),
        _ => b.cmp(&a).then_with(|| b_tie.cmp(&a_tie)),
    }
}

fn jql_matches_issue(expr: &crate::jql::Expr, issue: &Issue, current_user: Option<UserId>) -> bool {
    use crate::jql::Expr;
    match expr {
        Expr::And(left, right) => {
            jql_matches_issue(left, issue, current_user)
                && jql_matches_issue(right, issue, current_user)
        }
        Expr::Or(left, right) => {
            jql_matches_issue(left, issue, current_user)
                || jql_matches_issue(right, issue, current_user)
        }
        Expr::Not(inner) => !jql_matches_issue(inner, issue, current_user),
        Expr::IsEmpty { field, negated } => {
            let empty = jql_field_empty(*field, issue);
            if *negated { !empty } else { empty }
        }
        Expr::Clause {
            field,
            operator,
            values,
        } => jql_clause_matches(*field, *operator, values, issue, current_user),
    }
}

fn jql_clause_matches(
    field: crate::jql::Field,
    operator: crate::jql::BinaryOperator,
    values: &[crate::jql::Value],
    issue: &Issue,
    current_user: Option<UserId>,
) -> bool {
    use crate::jql::BinaryOperator;
    let Some(mut field_values) = jql_field_values(field, issue) else {
        return false;
    };
    if field_values.is_empty() {
        return false;
    }
    let Some(mut values) = values
        .iter()
        .map(|value| jql_value(value, current_user))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    if values.is_empty() {
        return false;
    }
    if jql_field_is_case_insensitive(field) {
        field_values = field_values
            .into_iter()
            .map(|value| value.to_lowercase())
            .collect();
        values = values
            .into_iter()
            .map(|value| value.to_lowercase())
            .collect();
    }
    match operator {
        BinaryOperator::Equals => one_value(&values)
            .is_some_and(|wanted| field_values.iter().any(|actual| actual == wanted)),
        BinaryOperator::NotEquals => one_value(&values)
            .is_some_and(|wanted| field_values.iter().all(|actual| actual != wanted)),
        BinaryOperator::Contains => one_value(&values).is_some_and(|wanted| {
            let wanted = wanted.to_lowercase();
            field_values
                .iter()
                .any(|actual| actual.to_lowercase().contains(&wanted))
        }),
        BinaryOperator::NotContains => one_value(&values).is_some_and(|wanted| {
            let wanted = wanted.to_lowercase();
            field_values
                .iter()
                .all(|actual| !actual.to_lowercase().contains(&wanted))
        }),
        BinaryOperator::In => field_values.iter().any(|actual| values.contains(actual)),
        BinaryOperator::NotIn => field_values.iter().all(|actual| !values.contains(actual)),
        BinaryOperator::LessThan => one_value(&values)
            .is_some_and(|wanted| field_values.iter().any(|actual| actual < wanted)),
        BinaryOperator::LessThanOrEqual => one_value(&values)
            .is_some_and(|wanted| field_values.iter().any(|actual| actual <= wanted)),
        BinaryOperator::GreaterThan => one_value(&values)
            .is_some_and(|wanted| field_values.iter().any(|actual| actual > wanted)),
        BinaryOperator::GreaterThanOrEqual => one_value(&values)
            .is_some_and(|wanted| field_values.iter().any(|actual| actual >= wanted)),
    }
}

fn jql_field_is_case_insensitive(field: crate::jql::Field) -> bool {
    use crate::jql::Field;
    matches!(
        field,
        Field::Key
            | Field::Project
            | Field::ProjectKey
            | Field::Status
            | Field::StatusCategory
            | Field::IssueType
            | Field::Priority
            | Field::Labels
    )
}

fn one_value(values: &[String]) -> Option<&String> {
    (values.len() == 1).then(|| &values[0])
}

fn jql_value(value: &crate::jql::Value, current_user: Option<UserId>) -> Option<String> {
    match value {
        crate::jql::Value::Text(value) => Some(value.clone()),
        crate::jql::Value::Function(name) if name.eq_ignore_ascii_case("currentUser") => {
            current_user.map(|id| id.to_string())
        }
        crate::jql::Value::Function(_) => None,
    }
}

fn jql_field_empty(field: crate::jql::Field, issue: &Issue) -> bool {
    use crate::jql::Field;
    match field {
        Field::Assignee => issue.assignee_id.is_none(),
        Field::Sprint => issue.sprint_id.is_none(),
        Field::Description => issue.description.is_none(),
        Field::DueDate => issue.due_date.is_none(),
        _ => false,
    }
}

fn jql_field_values(field: crate::jql::Field, issue: &Issue) -> Option<Vec<String>> {
    use crate::jql::Field;
    match field {
        Field::Key => Some(vec![issue.key.to_string()]),
        Field::Summary => Some(vec![issue.summary.as_ref().to_string()]),
        Field::Description => issue
            .description
            .as_ref()
            .map(|value| vec![value.as_ref().to_string()]),
        Field::Text => {
            let mut values = vec![issue.key.to_string(), issue.summary.as_ref().to_string()];
            if let Some(description) = issue.description.as_ref() {
                values.push(description.as_ref().to_string());
            }
            Some(values)
        }
        Field::Project | Field::ProjectKey => Some(vec![issue.key.project_key.to_string()]),
        Field::Status => Some(vec![issue.status_id.to_string()]),
        Field::IssueType => Some(vec![format!("{:?}", issue.issue_type)]),
        Field::Assignee => issue.assignee_id.map(|id| vec![id.to_string()]),
        Field::Reporter => Some(vec![issue.reporter_id.to_string()]),
        Field::Priority => Some(vec![issue.priority.as_str().to_string()]),
        Field::Sprint => issue.sprint_id.map(|id| vec![id.to_string()]),
        Field::Created => Some(vec![issue.created_at.to_rfc3339()]),
        Field::Updated => Some(vec![issue.updated_at.to_rfc3339()]),
        Field::DueDate => issue.due_date.map(|value| vec![value.to_rfc3339()]),
        Field::StatusCategory | Field::Labels => None,
    }
}

#[derive(Default)]
pub struct MemoryBoardRepository {
    boards: Arc<Mutex<Vec<Board>>>,
}

#[async_trait]
impl BoardRepository for MemoryBoardRepository {
    async fn get_by_id(&self, id: BoardId) -> Result<Board, AppError> {
        let boards = self.boards.lock().unwrap();
        boards
            .iter()
            .find(|b| b.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("board", id))
    }

    async fn get_default_by_project(&self, project_id: ProjectId) -> Result<Board, AppError> {
        let boards = self.boards.lock().unwrap();
        boards
            .iter()
            .find(|b| b.project_id == project_id)
            .cloned()
            .ok_or_else(|| AppError::not_found("board", project_id))
    }

    async fn get_default_by_project_key(&self, key: &ProjectKey) -> Result<Board, AppError> {
        let boards = self.boards.lock().unwrap();
        boards
            .iter()
            .find(|b| {
                // best-effort project key lookup by scanning projects not available here
                b.columns.iter().any(|_| true) // placeholder; key check omitted
            })
            .cloned()
            .ok_or_else(|| AppError::not_found("board", key))
    }

    async fn save(&self, board: &Board) -> Result<(), AppError> {
        let mut boards = self.boards.lock().unwrap();
        if let Some(idx) = boards.iter().position(|b| b.id == board.id) {
            boards[idx] = board.clone();
        } else {
            boards.push(board.clone());
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct MemorySprintRepository {
    sprints: Arc<Mutex<Vec<Sprint>>>,
}

#[async_trait]
impl SprintRepository for MemorySprintRepository {
    async fn get_active_by_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Option<Sprint>, AppError> {
        let sprints = self.sprints.lock().unwrap();
        Ok(sprints
            .iter()
            .find(|s| s.project_id == project_id && matches!(s.state, crate::SprintState::Active))
            .cloned())
    }

    async fn get_by_id(&self, id: SprintId) -> Result<Sprint, AppError> {
        let sprints = self.sprints.lock().unwrap();
        sprints
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("sprint", id))
    }

    async fn list_by_project(&self, project_id: ProjectId) -> Result<Vec<Sprint>, AppError> {
        let sprints = self.sprints.lock().unwrap();
        Ok(sprints
            .iter()
            .filter(|s| s.project_id == project_id)
            .cloned()
            .collect())
    }

    async fn save(&self, sprint: &Sprint) -> Result<SprintId, AppError> {
        let mut sprints = self.sprints.lock().unwrap();
        if let Some(idx) = sprints.iter().position(|s| s.id == sprint.id) {
            sprints[idx] = sprint.clone();
        } else {
            sprints.push(sprint.clone());
        }
        Ok(sprint.id)
    }
}

pub struct MemoryStatusRepository {
    statuses: Arc<Mutex<Vec<Status>>>,
}

impl MemoryStatusRepository {
    pub fn new(statuses: Vec<Status>) -> Self {
        Self {
            statuses: Arc::new(Mutex::new(statuses)),
        }
    }
}

impl Default for MemoryStatusRepository {
    fn default() -> Self {
        Self::new(vec![])
    }
}

#[async_trait]
impl StatusRepository for MemoryStatusRepository {
    async fn get_by_id(&self, id: shared::StatusId) -> Result<Status, AppError> {
        self.statuses
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("status", id))
    }

    async fn list_all(&self) -> Result<Vec<Status>, AppError> {
        Ok(self.statuses.lock().unwrap().clone())
    }

    async fn get_default(&self) -> Result<Status, AppError> {
        self.statuses
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.is_default)
            .cloned()
            .ok_or_else(|| AppError::not_found("status", "default"))
    }
}

pub struct MemoryUnitOfWork {
    repos: crate::Repositories,
}

impl MemoryUnitOfWork {
    pub fn new(repos: crate::Repositories) -> Self {
        Self { repos }
    }
}

#[async_trait]
impl UnitOfWork for MemoryUnitOfWork {
    async fn with_transaction<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a> FnOnce(
                &'a crate::Repositories,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<T, AppError>> + Send + 'a>,
            > + Send
            + 'static,
        T: Send + 'static,
    {
        f(&self.repos).await
    }
}

#[derive(Default)]
pub struct MemoryEventBus {
    events: Arc<Mutex<Vec<crate::ProjectEvent>>>,
}

#[async_trait]
impl EventBus for MemoryEventBus {
    async fn publish(&self, event: crate::ProjectEvent) -> Result<(), AppError> {
        let mut events = self.events.lock().unwrap();
        events.push(event);
        Ok(())
    }
}

impl MemoryEventBus {
    pub fn drained(&self) -> Vec<crate::ProjectEvent> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}

pub struct MemoryCommentRepository {
    comments: Arc<Mutex<Vec<Comment>>>,
}

impl MemoryCommentRepository {
    pub fn new() -> Self {
        Self {
            comments: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for MemoryCommentRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CommentRepository for MemoryCommentRepository {
    async fn list_by_issue_page(
        &self,
        issue_id: IssueId,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<Comment>, AppError> {
        let all = self.list_by_issue(issue_id).await?;
        Ok(all
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect())
    }
    async fn get_by_id(&self, id: CommentId) -> Result<Comment, AppError> {
        let comments = self.comments.lock().unwrap();
        comments
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("comment", id))
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<Comment>, AppError> {
        let comments = self.comments.lock().unwrap();
        Ok(comments
            .iter()
            .filter(|c| c.issue_id == issue_id)
            .cloned()
            .collect())
    }

    async fn save(&self, comment: &Comment) -> Result<CommentId, AppError> {
        let mut comments = self.comments.lock().unwrap();
        if let Some(idx) = comments.iter().position(|c| c.id == comment.id) {
            comments[idx] = comment.clone();
        } else {
            comments.push(comment.clone());
        }
        Ok(comment.id)
    }

    async fn delete(&self, id: CommentId) -> Result<(), AppError> {
        let mut comments = self.comments.lock().unwrap();
        comments.retain(|c| c.id != id);
        Ok(())
    }
}

pub struct MemoryWorklogRepository {
    worklogs: Arc<Mutex<Vec<Worklog>>>,
}

impl MemoryWorklogRepository {
    pub fn new() -> Self {
        Self {
            worklogs: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for MemoryWorklogRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WorklogRepository for MemoryWorklogRepository {
    async fn get_by_id(&self, id: WorklogId) -> Result<Worklog, AppError> {
        let worklogs = self.worklogs.lock().unwrap();
        worklogs
            .iter()
            .find(|w| w.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("worklog", id))
    }

    async fn list_by_issue_page(
        &self,
        issue_id: IssueId,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<Worklog>, AppError> {
        let worklogs = self.worklogs.lock().unwrap();
        let mut items: Vec<_> = worklogs
            .iter()
            .filter(|w| w.issue_id == issue_id)
            .cloned()
            .collect();
        items.sort_by_key(|w| w.started_at);
        Ok(items
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect())
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<Worklog>, AppError> {
        self.list_by_issue_page(issue_id, u64::MAX, 0).await
    }

    async fn save(&self, worklog: &Worklog) -> Result<WorklogId, AppError> {
        let mut worklogs = self.worklogs.lock().unwrap();
        if let Some(idx) = worklogs.iter().position(|w| w.id == worklog.id) {
            worklogs[idx] = worklog.clone();
        } else {
            worklogs.push(worklog.clone());
        }
        Ok(worklog.id)
    }

    async fn delete(&self, id: WorklogId) -> Result<(), AppError> {
        let mut worklogs = self.worklogs.lock().unwrap();
        worklogs.retain(|w| w.id != id);
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryProjectMemberRepository {
    members: Arc<Mutex<Vec<ProjectMember>>>,
}

#[async_trait]
impl ProjectMemberRepository for MemoryProjectMemberRepository {
    async fn list_by_project(&self, project_id: ProjectId) -> Result<Vec<ProjectMember>, AppError> {
        Ok(self
            .members
            .lock()
            .unwrap()
            .iter()
            .filter(|m| m.project_id == project_id)
            .cloned()
            .collect())
    }

    async fn list_by_user(&self, user_id: UserId) -> Result<Vec<ProjectMember>, AppError> {
        Ok(self
            .members
            .lock()
            .unwrap()
            .iter()
            .filter(|m| m.user_id == user_id)
            .cloned()
            .collect())
    }

    async fn get(&self, project_id: ProjectId, user_id: UserId) -> Result<ProjectMember, AppError> {
        self.members
            .lock()
            .unwrap()
            .iter()
            .find(|m| m.project_id == project_id && m.user_id == user_id)
            .cloned()
            .ok_or_else(|| AppError::not_found("project member", project_id))
    }

    async fn save(&self, member: &ProjectMember) -> Result<(), AppError> {
        let mut members = self.members.lock().unwrap();
        let idx = members
            .iter()
            .position(|m| m.project_id == member.project_id && m.user_id == member.user_id);
        if let Some(i) = idx {
            members[i] = member.clone();
        } else {
            members.push(member.clone());
        }
        Ok(())
    }

    async fn delete(&self, project_id: ProjectId, user_id: UserId) -> Result<(), AppError> {
        let mut members = self.members.lock().unwrap();
        let idx = members
            .iter()
            .position(|m| m.project_id == project_id && m.user_id == user_id);
        if let Some(i) = idx {
            members.remove(i);
        }
        Ok(())
    }
}

pub struct MemoryAttachmentRepository {
    attachments: Arc<Mutex<Vec<crate::Attachment>>>,
}

impl MemoryAttachmentRepository {
    pub fn new() -> Self {
        Self {
            attachments: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for MemoryAttachmentRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::AttachmentRepository for MemoryAttachmentRepository {
    async fn get_by_id(&self, id: shared::AttachmentId) -> Result<crate::Attachment, AppError> {
        let items = self.attachments.lock().unwrap();
        items
            .iter()
            .find(|a| a.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("attachment", id))
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<crate::Attachment>, AppError> {
        let items = self.attachments.lock().unwrap();
        Ok(items
            .iter()
            .filter(|a| a.issue_id == issue_id)
            .cloned()
            .collect())
    }

    async fn save(&self, attachment: &crate::Attachment) -> Result<shared::AttachmentId, AppError> {
        let mut items = self.attachments.lock().unwrap();
        if let Some(idx) = items.iter().position(|a| a.id == attachment.id) {
            items[idx] = attachment.clone();
        } else {
            items.push(attachment.clone());
        }
        Ok(attachment.id)
    }

    async fn delete(&self, id: shared::AttachmentId) -> Result<(), AppError> {
        let mut items = self.attachments.lock().unwrap();
        items.retain(|a| a.id != id);
        Ok(())
    }
}

pub struct MemoryLabelRepository {
    labels: Arc<Mutex<Vec<crate::Label>>>,
    issue_labels: Arc<Mutex<Vec<(IssueId, shared::LabelId)>>>,
}

impl MemoryLabelRepository {
    pub fn new() -> Self {
        Self {
            labels: Arc::new(Mutex::new(Vec::new())),
            issue_labels: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for MemoryLabelRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::LabelRepository for MemoryLabelRepository {
    async fn get_by_id(&self, id: shared::LabelId) -> Result<crate::Label, AppError> {
        self.labels
            .lock()
            .unwrap()
            .iter()
            .find(|l| l.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("label", id))
    }

    async fn list_by_project(&self, project_id: ProjectId) -> Result<Vec<crate::Label>, AppError> {
        Ok(self
            .labels
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.project_id == project_id)
            .cloned()
            .collect())
    }

    async fn save(&self, label: &crate::Label) -> Result<shared::LabelId, AppError> {
        let mut labels = self.labels.lock().unwrap();
        if let Some(idx) = labels.iter().position(|l| l.id == label.id) {
            labels[idx] = label.clone();
        } else {
            labels.push(label.clone());
        }
        Ok(label.id)
    }

    async fn delete(&self, id: shared::LabelId) -> Result<(), AppError> {
        self.labels.lock().unwrap().retain(|l| l.id != id);
        self.issue_labels
            .lock()
            .unwrap()
            .retain(|(_, lid)| *lid != id);
        Ok(())
    }

    async fn list_ids_by_issue(&self, issue_id: IssueId) -> Result<Vec<shared::LabelId>, AppError> {
        Ok(self
            .issue_labels
            .lock()
            .unwrap()
            .iter()
            .filter(|(iid, _)| *iid == issue_id)
            .map(|(_, lid)| *lid)
            .collect())
    }

    async fn list_issue_ids_by_label(
        &self,
        label_id: shared::LabelId,
    ) -> Result<Vec<IssueId>, AppError> {
        Ok(self
            .issue_labels
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, lid)| *lid == label_id)
            .map(|(iid, _)| *iid)
            .collect())
    }

    async fn list_by_issues(
        &self,
        issue_ids: &[IssueId],
    ) -> Result<std::collections::HashMap<IssueId, Vec<crate::Label>>, AppError> {
        let wanted = issue_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let labels = self.labels.lock().unwrap();
        let labels_by_id = labels
            .iter()
            .cloned()
            .map(|label| (label.id, label))
            .collect::<std::collections::HashMap<_, _>>();
        let mut result = std::collections::HashMap::<IssueId, Vec<crate::Label>>::new();
        for issue_id in issue_ids {
            result.entry(*issue_id).or_default();
        }
        for (issue_id, label_id) in self.issue_labels.lock().unwrap().iter() {
            if !wanted.contains(issue_id) {
                continue;
            }
            if let Some(label) = labels_by_id.get(label_id) {
                result.entry(*issue_id).or_default().push(label.clone());
            }
        }
        for labels in result.values_mut() {
            labels.sort_by(|a, b| a.name.as_ref().cmp(b.name.as_ref()));
        }
        Ok(result)
    }

    async fn attach(&self, issue_id: IssueId, label_id: shared::LabelId) -> Result<(), AppError> {
        let mut il = self.issue_labels.lock().unwrap();
        if !il.contains(&(issue_id, label_id)) {
            il.push((issue_id, label_id));
        }
        Ok(())
    }

    async fn detach(&self, issue_id: IssueId, label_id: shared::LabelId) -> Result<(), AppError> {
        self.issue_labels
            .lock()
            .unwrap()
            .retain(|(iid, lid)| !(*iid == issue_id && *lid == label_id));
        Ok(())
    }
}

pub struct MemoryIssueLinkRepository {
    links: Arc<Mutex<Vec<crate::IssueLink>>>,
}

impl MemoryIssueLinkRepository {
    pub fn new() -> Self {
        Self {
            links: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for MemoryIssueLinkRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl crate::IssueLinkRepository for MemoryIssueLinkRepository {
    async fn get_by_id(&self, id: shared::IssueLinkId) -> Result<crate::IssueLink, AppError> {
        self.links
            .lock()
            .unwrap()
            .iter()
            .find(|l| l.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("issue link", id))
    }

    async fn save(&self, link: &crate::IssueLink) -> Result<shared::IssueLinkId, AppError> {
        let mut links = self.links.lock().unwrap();
        if let Some(idx) = links.iter().position(|l| l.id == link.id) {
            links[idx] = link.clone();
        } else {
            links.push(link.clone());
        }
        Ok(link.id)
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<crate::IssueLink>, AppError> {
        Ok(self
            .links
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.source_id == issue_id || l.target_id == issue_id)
            .cloned()
            .collect())
    }

    async fn delete(&self, id: shared::IssueLinkId) -> Result<(), AppError> {
        self.links.lock().unwrap().retain(|l| l.id != id);
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryNotificationRepository {
    notifications: Arc<Mutex<Vec<Notification>>>,
    settings: Arc<Mutex<Vec<NotificationUserSettings>>>,
}

#[async_trait]
impl NotificationRepository for MemoryNotificationRepository {
    async fn mark_read_batch(&self, ids: &[shared::NotificationId]) -> Result<(), AppError> {
        let mut all = self.notifications.lock().unwrap();
        for n in all.iter_mut() {
            if ids.contains(&n.id) && !n.is_read {
                n.is_read = true;
                n.read_at = Some(shared::now());
            }
        }
        Ok(())
    }
    async fn save(&self, notification: &Notification) -> Result<NotificationId, AppError> {
        let mut notifications = self.notifications.lock().unwrap();
        if let Some(index) = notifications
            .iter()
            .position(|existing| existing.id == notification.id)
        {
            notifications[index] = notification.clone();
        } else {
            notifications.push(notification.clone());
        }
        Ok(notification.id)
    }

    async fn list_unread(&self, recipient_id: UserId) -> Result<Vec<Notification>, AppError> {
        let mut notifications: Vec<_> = self
            .notifications
            .lock()
            .unwrap()
            .iter()
            .filter(|notification| {
                notification.recipient_id == recipient_id && !notification.is_read
            })
            .cloned()
            .collect();
        notifications.sort_by_key(|notification| notification.created_at);
        Ok(notifications)
    }

    async fn list_all_unread(&self) -> Result<Vec<Notification>, AppError> {
        let mut notifications: Vec<_> = self
            .notifications
            .lock()
            .unwrap()
            .iter()
            .filter(|notification| !notification.is_read)
            .cloned()
            .collect();
        notifications.sort_by_key(|notification| notification.created_at);
        Ok(notifications)
    }

    async fn mark_read(&self, id: NotificationId, recipient_id: UserId) -> Result<(), AppError> {
        let mut notifications = self.notifications.lock().unwrap();
        let notification = notifications
            .iter_mut()
            .find(|notification| notification.id == id && notification.recipient_id == recipient_id)
            .ok_or_else(|| AppError::not_found("notification", id))?;
        if !notification.is_read {
            notification.is_read = true;
            notification.read_at = Some(shared::now());
        }
        Ok(())
    }

    async fn mark_all_read(&self, recipient_id: UserId) -> Result<(), AppError> {
        let now = shared::now();
        for notification in self.notifications.lock().unwrap().iter_mut() {
            if notification.recipient_id == recipient_id && !notification.is_read {
                notification.is_read = true;
                notification.read_at = Some(now);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl UserNotificationSettingsRepository for MemoryNotificationRepository {
    async fn get_settings(&self, user_id: UserId) -> Result<NotificationUserSettings, AppError> {
        self.settings
            .lock()
            .unwrap()
            .iter()
            .find(|settings| settings.user_id == user_id)
            .cloned()
            .ok_or_else(|| AppError::not_found("notification settings", user_id))
    }

    async fn save_settings(&self, settings: &NotificationUserSettings) -> Result<(), AppError> {
        let mut all_settings = self.settings.lock().unwrap();
        if let Some(index) = all_settings
            .iter()
            .position(|existing| existing.user_id == settings.user_id)
        {
            all_settings[index] = settings.clone();
        } else {
            all_settings.push(settings.clone());
        }
        Ok(())
    }

    async fn mark_email_digest_sent(
        &self,
        user_id: UserId,
        sent_at: shared::Timestamp,
    ) -> Result<(), AppError> {
        let mut all_settings = self.settings.lock().unwrap();
        let settings = all_settings
            .iter_mut()
            .find(|settings| settings.user_id == user_id)
            .ok_or_else(|| AppError::not_found("notification settings", user_id))?;
        settings.last_email_digest_at = Some(sent_at);
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryIssueStatusHistoryRepository {
    history: Arc<Mutex<Vec<IssueStatusHistory>>>,
    /// Parallel vector of project_id per entry (index-aligned with `history`).
    project_ids: Arc<Mutex<Vec<ProjectId>>>,
}

/// Shared handle pair so `MemoryIssueRepository::with_shared_history` and the
/// history repository observe the same entries (atomic writes + reports).
pub type SharedHistoryStore = (
    Arc<Mutex<Vec<IssueStatusHistory>>>,
    Arc<Mutex<Vec<ProjectId>>>,
);

impl MemoryIssueStatusHistoryRepository {
    pub fn store(&self) -> SharedHistoryStore {
        (self.history.clone(), self.project_ids.clone())
    }
}

#[async_trait]
impl IssueStatusHistoryRepository for MemoryIssueStatusHistoryRepository {
    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<IssueStatusHistory>, AppError> {
        let history = self.history.lock().unwrap();
        Ok(history
            .iter()
            .filter(|h| h.issue_id == issue_id)
            .cloned()
            .collect())
    }

    async fn list_by_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<IssueStatusHistory>, AppError> {
        let history = self.history.lock().unwrap();
        let project_ids = self.project_ids.lock().unwrap();
        Ok(history
            .iter()
            .zip(project_ids.iter())
            .filter(|(_, pid)| **pid == project_id)
            .map(|(h, _)| h.clone())
            .collect())
    }

    async fn save(&self, entry: &IssueStatusHistory) -> Result<(), AppError> {
        let mut history = self.history.lock().unwrap();
        let mut project_ids = self.project_ids.lock().unwrap();
        if let Some(idx) = history.iter().position(|h| h.id == entry.id) {
            history[idx] = entry.clone();
            // project_ids stays aligned by index
        } else {
            history.push(entry.clone());
            // Project ID is not stored on IssueStatusHistory directly; caller must
            // set it via set_project. For simplicity, we store ProjectId::nil() and
            // rely on the caller using the variant below.
            project_ids.push(ProjectId::nil());
        }
        Ok(())
    }

    async fn save_for_project(
        &self,
        entry: &IssueStatusHistory,
        project_id: ProjectId,
    ) -> Result<(), AppError> {
        self.save_for_project_impl(entry, project_id).await
    }
}

impl MemoryIssueStatusHistoryRepository {
    /// Save a history entry with its associated project_id for `list_by_project`.
    pub async fn save_for_project_impl(
        &self,
        entry: &IssueStatusHistory,
        project_id: ProjectId,
    ) -> Result<(), AppError> {
        let mut history = self.history.lock().unwrap();
        let mut project_ids = self.project_ids.lock().unwrap();
        match history.iter().position(|h| h.id == entry.id) {
            Some(idx) => {
                history[idx] = entry.clone();
                project_ids[idx] = project_id;
            }
            None => {
                history.push(entry.clone());
                project_ids.push(project_id);
            }
        }
        Ok(())
    }

    pub fn save_with_project(&self, entry: &IssueStatusHistory, project_id: ProjectId) {
        let mut history = self.history.lock().unwrap();
        let mut project_ids = self.project_ids.lock().unwrap();
        if let Some(idx) = history.iter().position(|h| h.id == entry.id) {
            history[idx] = entry.clone();
            project_ids[idx] = project_id;
        } else {
            history.push(entry.clone());
            project_ids.push(project_id);
        }
    }
}

#[derive(Default)]
pub struct MemoryAuditLogRepository {
    entries: Arc<Mutex<Vec<AuditLog>>>,
}

#[async_trait]
impl AuditLogRepository for MemoryAuditLogRepository {
    async fn save(&self, entry: &AuditLog) -> Result<(), AppError> {
        self.entries.lock().unwrap().push(entry.clone());
        Ok(())
    }

    async fn list(
        &self,
        actor_id: Option<UserId>,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<AuditLog>, AppError> {
        let mut entries: Vec<_> = self
            .entries
            .lock()
            .unwrap()
            .iter()
            .filter(|entry| actor_id.is_none_or(|actor| entry.actor_id == actor))
            .cloned()
            .collect();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.created_at));
        let offset = offset.min(entries.len() as u64) as usize;
        entries.truncate(offset + limit as usize);
        entries.drain(..offset);
        Ok(entries)
    }
}

#[derive(Default)]
pub struct MemorySystemSettingRepository {
    settings: Arc<Mutex<Vec<SystemSetting>>>,
}

#[async_trait]
impl SystemSettingRepository for MemorySystemSettingRepository {
    async fn get(&self, key: &str) -> Result<SystemSetting, AppError> {
        self.settings
            .lock()
            .unwrap()
            .iter()
            .find(|setting| setting.key.as_ref() == key)
            .cloned()
            .ok_or_else(|| AppError::not_found("system setting", key))
    }

    async fn list(&self) -> Result<Vec<SystemSetting>, AppError> {
        let mut settings = self.settings.lock().unwrap().clone();
        settings.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(settings)
    }

    async fn save(&self, setting: &SystemSetting) -> Result<(), AppError> {
        let mut settings = self.settings.lock().unwrap();
        if let Some(index) = settings
            .iter()
            .position(|existing| existing.key == setting.key)
        {
            settings[index] = setting.clone();
        } else {
            settings.push(setting.clone());
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryWatcherRepository {
    watchers: Arc<Mutex<Vec<(IssueId, UserId)>>>,
}

#[async_trait]
impl WatcherRepository for MemoryWatcherRepository {
    async fn add(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError> {
        let mut watchers = self.watchers.lock().unwrap();
        if !watchers.contains(&(issue_id, user_id)) {
            watchers.push((issue_id, user_id));
        }
        Ok(())
    }

    async fn remove(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError> {
        self.watchers
            .lock()
            .unwrap()
            .retain(|(iid, uid)| !(*iid == issue_id && *uid == user_id));
        Ok(())
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<IssueWatcher>, AppError> {
        Ok(self
            .watchers
            .lock()
            .unwrap()
            .iter()
            .filter(|(iid, _)| *iid == issue_id)
            .map(|(iid, uid)| IssueWatcher {
                issue_id: *iid,
                user_id: *uid,
            })
            .collect())
    }

    async fn is_watching(&self, issue_id: IssueId, user_id: UserId) -> Result<bool, AppError> {
        Ok(self.watchers.lock().unwrap().contains(&(issue_id, user_id)))
    }

    async fn list_by_user(&self, user_id: UserId) -> Result<Vec<IssueWatcher>, AppError> {
        Ok(self
            .watchers
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, uid)| *uid == user_id)
            .map(|(iid, uid)| IssueWatcher {
                issue_id: *iid,
                user_id: *uid,
            })
            .collect())
    }
}

#[derive(Default)]
pub struct MemoryVoteRepository {
    votes: Arc<Mutex<Vec<IssueVote>>>,
}

#[async_trait]
impl VoteRepository for MemoryVoteRepository {
    async fn add(&self, issue_id: IssueId, user_id: UserId) -> Result<IssueVote, AppError> {
        let mut votes = self.votes.lock().unwrap();
        if let Some(existing) = votes
            .iter()
            .find(|v| v.issue_id == issue_id && v.user_id == user_id)
        {
            return Ok(existing.clone());
        }
        let vote = IssueVote {
            issue_id,
            user_id,
            voted_at: shared::now(),
        };
        votes.push(vote.clone());
        Ok(vote)
    }

    async fn remove(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError> {
        self.votes
            .lock()
            .unwrap()
            .retain(|v| !(v.issue_id == issue_id && v.user_id == user_id));
        Ok(())
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<IssueVote>, AppError> {
        Ok(self
            .votes
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v.issue_id == issue_id)
            .cloned()
            .collect())
    }

    async fn count_by_issue(&self, issue_id: IssueId) -> Result<u64, AppError> {
        Ok(self
            .votes
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v.issue_id == issue_id)
            .count() as u64)
    }

    async fn has_voted(&self, issue_id: IssueId, user_id: UserId) -> Result<bool, AppError> {
        Ok(self
            .votes
            .lock()
            .unwrap()
            .iter()
            .any(|v| v.issue_id == issue_id && v.user_id == user_id))
    }
}

#[derive(Default)]
pub struct MemoryCustomFieldRepository {
    fields: Arc<Mutex<Vec<crate::CustomField>>>,
    values: Arc<Mutex<Vec<crate::CustomFieldValue>>>,
}

impl MemoryCustomFieldRepository {
    pub fn with_shared_values(values: Arc<Mutex<Vec<crate::CustomFieldValue>>>) -> Self {
        Self {
            fields: Arc::new(Mutex::new(Vec::new())),
            values,
        }
    }

    pub fn value_store(&self) -> Arc<Mutex<Vec<crate::CustomFieldValue>>> {
        self.values.clone()
    }
}

#[async_trait]
impl crate::CustomFieldRepository for MemoryCustomFieldRepository {
    async fn get_by_id(&self, id: shared::CustomFieldId) -> Result<crate::CustomField, AppError> {
        self.fields
            .lock()
            .unwrap()
            .iter()
            .find(|f| f.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("custom field", id))
    }

    async fn list_by_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<crate::CustomField>, AppError> {
        Ok(self
            .fields
            .lock()
            .unwrap()
            .iter()
            .filter(|f| f.project_id == project_id)
            .cloned()
            .collect())
    }

    async fn save(&self, field: &crate::CustomField) -> Result<shared::CustomFieldId, AppError> {
        let mut fields = self.fields.lock().unwrap();
        if let Some(idx) = fields.iter().position(|f| f.id == field.id) {
            fields[idx] = field.clone();
        } else {
            fields.push(field.clone());
        }
        Ok(field.id)
    }

    async fn delete(&self, id: shared::CustomFieldId) -> Result<(), AppError> {
        self.fields.lock().unwrap().retain(|f| f.id != id);
        self.values.lock().unwrap().retain(|v| v.field_id != id);
        Ok(())
    }

    async fn set_value(
        &self,
        issue_id: IssueId,
        field_id: shared::CustomFieldId,
        value: &serde_json::Value,
    ) -> Result<(), AppError> {
        let mut values = self.values.lock().unwrap();
        if let Some(idx) = values
            .iter()
            .position(|v| v.issue_id == issue_id && v.field_id == field_id)
        {
            values[idx].value = value.clone();
        } else {
            values.push(crate::CustomFieldValue {
                issue_id,
                field_id,
                value: value.clone(),
            });
        }
        Ok(())
    }

    async fn delete_value(
        &self,
        issue_id: IssueId,
        field_id: shared::CustomFieldId,
    ) -> Result<(), AppError> {
        self.values
            .lock()
            .unwrap()
            .retain(|v| !(v.issue_id == issue_id && v.field_id == field_id));
        Ok(())
    }

    async fn get_values_for_issue(
        &self,
        issue_id: IssueId,
    ) -> Result<Vec<crate::CustomFieldValue>, AppError> {
        Ok(self
            .values
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v.issue_id == issue_id)
            .cloned()
            .collect())
    }

    async fn delete_values_for_issue(&self, issue_id: IssueId) -> Result<(), AppError> {
        self.values
            .lock()
            .unwrap()
            .retain(|v| v.issue_id != issue_id);
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryProjectComponentRepository {
    components: Arc<Mutex<Vec<ProjectComponent>>>,
}

#[async_trait]
impl ProjectComponentRepository for MemoryProjectComponentRepository {
    async fn get_by_id(&self, id: ProjectComponentId) -> Result<ProjectComponent, AppError> {
        self.components
            .lock()
            .unwrap()
            .iter()
            .find(|c| c.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("component", id))
    }
    async fn list_by_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectComponent>, AppError> {
        Ok(self
            .components
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.project_id == project_id)
            .cloned()
            .collect())
    }
    async fn save(&self, component: &ProjectComponent) -> Result<ProjectComponentId, AppError> {
        let mut components = self.components.lock().unwrap();
        if let Some(idx) = components.iter().position(|c| c.id == component.id) {
            components[idx] = component.clone();
        } else {
            components.push(component.clone());
        }
        Ok(component.id)
    }
    async fn delete(&self, id: ProjectComponentId) -> Result<(), AppError> {
        self.components.lock().unwrap().retain(|c| c.id != id);
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryProjectVersionRepository {
    versions: Arc<Mutex<Vec<ProjectVersion>>>,
}

#[async_trait]
impl ProjectVersionRepository for MemoryProjectVersionRepository {
    async fn get_by_id(&self, id: ProjectVersionId) -> Result<ProjectVersion, AppError> {
        self.versions
            .lock()
            .unwrap()
            .iter()
            .find(|v| v.id == id)
            .cloned()
            .ok_or_else(|| AppError::not_found("version", id))
    }
    async fn list_by_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectVersion>, AppError> {
        Ok(self
            .versions
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v.project_id == project_id)
            .cloned()
            .collect())
    }
    async fn save(&self, version: &ProjectVersion) -> Result<ProjectVersionId, AppError> {
        let mut versions = self.versions.lock().unwrap();
        if let Some(idx) = versions.iter().position(|v| v.id == version.id) {
            versions[idx] = version.clone();
        } else {
            versions.push(version.clone());
        }
        Ok(version.id)
    }
    async fn delete(&self, id: ProjectVersionId) -> Result<(), AppError> {
        self.versions.lock().unwrap().retain(|v| v.id != id);
        Ok(())
    }
}
