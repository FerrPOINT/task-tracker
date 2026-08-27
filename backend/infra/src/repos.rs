use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use domain::{
    Attachment, AttachmentRepository, AuditLog, AuditLogRepository, Board, BoardColumn,
    BoardRepository, Comment, CommentRepository, CustomField, CustomFieldRepository,
    CustomFieldType, CustomFieldValue, Issue, IssueLink, IssueLinkRepository, IssueQuery,
    IssueRepository, IssueStatusHistory, IssueStatusHistoryRepository, IssueTypeEntity,
    IssueTypeRepository, IssueVote, IssueWatcher, Label, LabelRepository, LinkType, Notification,
    NotificationRepository, NotificationUserSettings, Project, ProjectComponent,
    ProjectComponentRepository, ProjectMember, ProjectMemberRepository, ProjectRepository,
    ProjectRole, ProjectVersion, ProjectVersionRepository, Sprint, SprintRepository, SprintState,
    Status, StatusCategory, StatusRepository, SystemSetting, SystemSettingRepository, User,
    UserNotificationSettingsRepository, UserRepository, VoteRepository, WatcherRepository,
    WorkflowTransition, WorkflowTransitionId, WorkflowTransitionRepository, Worklog,
    WorklogRepository,
};
use sea_orm::sea_query::extension::postgres::PgExpr as _;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::Expr,
};
use shared::{
    AppError, AttachmentId, AuditLogId, BoardId, CommentId, CustomFieldId, IssueId, IssueKey,
    IssueLinkId, IssueStatusHistoryId, IssueType, IssueTypeId, LabelId, NotificationId, Priority,
    ProjectComponentId, ProjectId, ProjectKey, ProjectVersionId, SprintId, StatusId, UserId,
    WorklogId,
};
use uuid::Uuid;

use crate::entities::{
    attachment, audit_log, board, comment, custom_field, issue, issue_custom_field_value,
    issue_label, issue_link, issue_status_history, issue_type, issue_vote, issue_watcher, label,
    notification, notification_user_settings, project, project_component, project_member,
    project_version, sprint, status, system_setting, user, workflow_transition, worklog,
};

fn map_status(m: status::Model) -> Status {
    Status {
        id: StatusId::from_uuid(m.id),
        name: domain::ArcStr::from(m.name),
        category: match m.category.as_str() {
            "inprogress" => StatusCategory::InProgress,
            "done" => StatusCategory::Done,
            _ => StatusCategory::Todo,
        },
        position: m.position,
        is_default: m.is_default,
        is_closed: m.is_closed,
    }
}

fn map_transition(m: workflow_transition::Model) -> WorkflowTransition {
    WorkflowTransition {
        id: WorkflowTransitionId::from_uuid(m.id),
        name: m.name.map(domain::ArcStr::from),
        from_status_id: StatusId::from_uuid(m.from_status_id),
        to_status_id: StatusId::from_uuid(m.to_status_id),
    }
}

fn map_issue_type(m: issue_type::Model) -> IssueTypeEntity {
    IssueTypeEntity {
        id: IssueTypeId::from_uuid(m.id),
        name: domain::ArcStr::from(m.name),
        description: m.description.map(domain::ArcStr::from),
        icon: m.icon.map(domain::ArcStr::from),
        color: m.color.map(domain::ArcStr::from),
        is_subtask: m.is_subtask,
        hierarchy_level: m.hierarchy_level,
    }
}
pub struct SeaOrmRepositories {
    pub users: Arc<dyn UserRepository>,
    pub audit_logs: Arc<dyn AuditLogRepository>,
    pub system_settings: Arc<dyn SystemSettingRepository>,
    pub projects: Arc<dyn ProjectRepository>,
    pub issues: Arc<dyn IssueRepository>,
    pub boards: Arc<dyn BoardRepository>,
    pub sprints: Arc<dyn SprintRepository>,
    pub comments: Arc<dyn CommentRepository>,
    pub worklogs: Arc<dyn WorklogRepository>,
    pub members: Arc<dyn ProjectMemberRepository>,
    pub statuses: Arc<dyn StatusRepository>,
    pub transitions: Arc<dyn WorkflowTransitionRepository>,
    pub issue_types: Arc<dyn IssueTypeRepository>,
    pub attachments: Arc<dyn AttachmentRepository>,
    pub labels: Arc<dyn LabelRepository>,
    pub components: Arc<dyn domain::ProjectComponentRepository>,
    pub versions: Arc<dyn domain::ProjectVersionRepository>,
    pub custom_fields: Arc<dyn CustomFieldRepository>,
    pub issue_links: Arc<dyn IssueLinkRepository>,
    pub notifications: Arc<dyn NotificationRepository>,
    pub notification_settings: Arc<dyn UserNotificationSettingsRepository>,
    pub issue_status_history: Arc<dyn IssueStatusHistoryRepository>,
    pub watchers: Arc<dyn WatcherRepository>,
    pub votes: Arc<dyn VoteRepository>,
}

impl SeaOrmRepositories {
    pub fn new(db: DatabaseConnection) -> Self {
        let db = Arc::new(db);
        Self {
            users: Arc::new(UserRepo { db: db.clone() }),
            audit_logs: Arc::new(AuditLogRepo { db: db.clone() }),
            system_settings: Arc::new(SystemSettingRepo { db: db.clone() }),
            projects: Arc::new(ProjectRepo { db: db.clone() }),
            issues: Arc::new(IssueRepo { db: db.clone() }),
            boards: Arc::new(BoardRepo { db: db.clone() }),
            sprints: Arc::new(SprintRepo { db: db.clone() }),
            comments: Arc::new(CommentRepo { db: db.clone() }),
            worklogs: Arc::new(WorklogRepo { db: db.clone() }),
            members: Arc::new(ProjectMemberRepo { db: db.clone() }),
            statuses: Arc::new(StatusRepo { db: db.clone() }),
            transitions: Arc::new(TransitionRepo { db: db.clone() }),
            issue_types: Arc::new(IssueTypeRepo { db: db.clone() }),
            attachments: Arc::new(AttachmentRepo { db: db.clone() }),
            labels: Arc::new(LabelRepo { db: db.clone() }),
            components: Arc::new(ProjectComponentRepo { db: db.clone() }),
            versions: Arc::new(ProjectVersionRepo { db: db.clone() }),
            custom_fields: Arc::new(CustomFieldRepo { db: db.clone() }),
            issue_links: Arc::new(IssueLinkRepo { db: db.clone() }),
            notifications: Arc::new(NotificationRepo { db: db.clone() }),
            notification_settings: Arc::new(NotificationUserSettingsRepo { db: db.clone() }),
            issue_status_history: Arc::new(IssueStatusHistoryRepo { db: db.clone() }),
            watchers: Arc::new(WatcherRepo { db: db.clone() }),
            votes: Arc::new(VoteRepo { db }),
        }
    }
}

struct UserRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl UserRepository for UserRepo {
    async fn get_by_id(&self, id: UserId) -> Result<User, AppError> {
        let model = user::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_user)
            .ok_or_else(|| AppError::not_found("user", id))
    }

    async fn get_by_email(&self, email: &str) -> Result<User, AppError> {
        let model = user::Entity::find()
            .filter(user::Column::Email.eq(email))
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_user)
            .ok_or_else(|| AppError::not_found("user", email))
    }

    async fn get_by_refresh_token(&self, token_hash: &str) -> Result<User, AppError> {
        let model = user::Entity::find()
            .filter(user::Column::RefreshTokenHash.eq(token_hash))
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_user)
            .ok_or_else(|| AppError::not_found("user", "refresh"))
    }

    async fn save(&self, user: &User) -> Result<UserId, AppError> {
        let active = user::ActiveModel {
            id: Set(user.id.as_uuid()),
            email: Set(user.email.as_ref().to_string()),
            username: Set(user.username.as_ref().to_string()),
            display_name: Set(user.display_name.as_ref().to_string()),
            password_hash: Set(user.password_hash.as_ref().to_string()),
            refresh_token_hash: Set(user
                .refresh_token_hash
                .as_ref()
                .map(|h| h.as_ref().to_string())),
            is_system_admin: Set(user.is_system_admin),
            is_active: Set(user.is_active),
            created_at: Set(user.created_at),
            updated_at: Set(shared::now()),
        };
        let exists = user::Entity::find_by_id(user.id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?
            .is_some();
        if exists {
            active.update(&*self.db).await.map_err(AppError::database)?;
        } else {
            active.insert(&*self.db).await.map_err(AppError::database)?;
        }
        Ok(user.id)
    }

    async fn list(&self) -> Result<Vec<User>, AppError> {
        let models = user::Entity::find()
            .order_by_asc(user::Column::DisplayName)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_user).collect())
    }
}

struct ProjectRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl ProjectRepository for ProjectRepo {
    async fn get_by_id(&self, id: ProjectId) -> Result<Project, AppError> {
        let model = project::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_project)
            .ok_or_else(|| AppError::not_found("project", id))
    }

    async fn get_by_key(&self, key: &ProjectKey) -> Result<Project, AppError> {
        let model = project::Entity::find()
            .filter(project::Column::Key.eq(key.as_str()))
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_project)
            .ok_or_else(|| AppError::not_found("project", key))
    }

    async fn list(&self, _query: domain::ProjectQuery) -> Result<Vec<Project>, AppError> {
        let models = project::Entity::find()
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_project).collect())
    }

    async fn save(&self, project: &Project) -> Result<ProjectId, AppError> {
        let active = project::ActiveModel {
            id: Set(project.id.as_uuid()),
            key: Set(project.key.to_string()),
            name: Set(project.name.as_ref().to_string()),
            description: Set(project.description.as_ref().map(|d| d.as_ref().to_string())),
            owner_id: Set(project.owner_id.as_uuid()),
            default_board_id: Set(project.default_board_id.as_uuid()),
            created_at: Set(project.created_at),
            updated_at: Set(shared::now()),
        };
        let exists = project::Entity::find_by_id(project.id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?
            .is_some();
        if exists {
            active.update(&*self.db).await.map_err(AppError::database)?;
        } else {
            project::Entity::insert(active)
                .exec(&*self.db)
                .await
                .map_err(AppError::database)?;
        }
        Ok(project.id)
    }

    async fn save_with_board(
        &self,
        project: &Project,
        board: &Board,
    ) -> Result<ProjectId, AppError> {
        let txn = self.db.as_ref().begin().await.map_err(AppError::database)?;
        let active = project::ActiveModel {
            id: Set(project.id.as_uuid()),
            key: Set(project.key.to_string()),
            name: Set(project.name.as_ref().to_string()),
            description: Set(project.description.as_ref().map(|d| d.as_ref().to_string())),
            owner_id: Set(project.owner_id.as_uuid()),
            default_board_id: Set(project.default_board_id.as_uuid()),
            created_at: Set(project.created_at),
            updated_at: Set(shared::now()),
        };
        project::Entity::insert(active)
            .exec(&txn)
            .await
            .map_err(AppError::database)?;
        let columns = serde_json::to_value(
            board
                .columns
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "id": c.id.as_uuid().to_string(),
                        "name": c.name.as_ref(),
                        "category": format!("{:?}", c.category),
                        "wip_limit": c.wip_limit,
                        "position": c.position,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default();
        board::Entity::insert(board::ActiveModel {
            id: Set(board.id.as_uuid()),
            project_id: Set(board.project_id.as_uuid()),
            name: Set(board.name.as_ref().to_string()),
            columns: Set(columns),
        })
        .exec(&txn)
        .await
        .map_err(AppError::database)?;
        txn.commit().await.map_err(AppError::database)?;
        Ok(project.id)
    }

    async fn delete(&self, id: ProjectId) -> Result<(), AppError> {
        let res = project::Entity::delete_by_id(id.as_uuid())
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        if res.rows_affected == 0 {
            return Err(AppError::not_found("project", id));
        }
        Ok(())
    }

    async fn next_issue_number(&self, project_id: ProjectId) -> Result<u32, AppError> {
        // MAX(number) parsed from issue keys, so deleted issues never cause key reuse
        // and concurrent counters can only collide on truly parallel inserts (handled by retry).
        let keys = issue::Entity::find()
            .filter(issue::Column::ProjectId.eq(project_id.as_uuid()))
            .select_only()
            .column(issue::Column::Key)
            .into_tuple::<String>()
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        let max = keys
            .iter()
            .filter_map(|k| k.rsplit('-').next())
            .filter_map(|suffix| suffix.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        Ok(max + 1)
    }
}

struct IssueRepo {
    db: Arc<DatabaseConnection>,
}

impl IssueRepo {
    /// `deleted_filter`: "exclude" = only live issues, "only" = only trashed,
    /// "include" = both.
    async fn search_by_jql(
        &self,
        compiled: &crate::jql::CompiledJql,
        limit: u64,
        offset: u64,
        deleted_filter: &str,
        accessible_project_ids: Option<&[ProjectId]>,
    ) -> Result<Vec<Issue>, AppError> {
        use sea_orm::FromQueryResult;
        let mut params: Vec<sea_orm::Value> = compiled
            .parameters
            .iter()
            .map(|p| match p {
                crate::jql::JqlParameter::Text(s) => {
                    sea_orm::Value::String(Some(Box::new(s.clone())))
                }
                crate::jql::JqlParameter::Uuid(u) => sea_orm::Value::Uuid(Some(Box::new(*u))),
            })
            .collect();
        let deleted_clause = match deleted_filter {
            "only" => " AND i.deleted_at IS NOT NULL",
            "include" => "",
            _ => " AND i.deleted_at IS NULL",
        };
        // The scope parameter must be appended BEFORE limit/offset so the
        // positional placeholders stay in binding order ($n scope, $n+1
        // limit, $n+2 offset). Appending it after limit/offset silently
        // bound the uuid array to OFFSET ("argument of OFFSET must be type
        // bigint, not type uuid[]").
        let mut scope_clause = String::new();
        match accessible_project_ids {
            Some([]) => {
                // No accessible projects: the result set is provably empty.
                scope_clause = " AND FALSE".to_string();
            }
            Some(ids) => {
                scope_clause = format!(" AND i.project_id = ANY(${})", params.len() + 1);
                params.push(sea_orm::Value::Array(
                    sea_orm::sea_query::ArrayType::Uuid,
                    Some(Box::new(
                        ids.iter()
                            .map(|id| sea_orm::sea_query::Value::Uuid(Some(Box::new(id.as_uuid()))))
                            .collect(),
                    )),
                ));
            }
            None => {}
        }
        params.push(sea_orm::Value::Unsigned(Some(limit as u32)));
        params.push(sea_orm::Value::Unsigned(Some(offset as u32)));
        let sql = format!(
            "SELECT i.* FROM issues i JOIN projects p ON i.project_id = p.id \
             WHERE {}{deleted_clause}{scope_clause} ORDER BY i.created_at DESC LIMIT ${} OFFSET ${}",
            compiled.predicate,
            params.len() - 1,
            params.len()
        );

        let stmt = sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            sql,
            params,
        );
        let rows = <issue::Model as FromQueryResult>::find_by_statement(stmt)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(rows.into_iter().map(map_issue).collect())
    }
}

#[async_trait]
impl IssueRepository for IssueRepo {
    async fn get_by_id(&self, id: IssueId) -> Result<Issue, AppError> {
        let model = issue::Entity::find_by_id(id.as_uuid())
            .filter(issue::Column::DeletedAt.is_null())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_issue)
            .ok_or_else(|| AppError::not_found("issue", id))
    }

    async fn get_by_id_include_deleted(&self, id: IssueId) -> Result<Issue, AppError> {
        let model = issue::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_issue)
            .ok_or_else(|| AppError::not_found("issue", id))
    }

    async fn get_by_key(&self, key: &IssueKey) -> Result<Issue, AppError> {
        let model = issue::Entity::find()
            .filter(issue::Column::Key.eq(key.to_string()))
            .filter(issue::Column::DeletedAt.is_null())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_issue)
            .ok_or_else(|| AppError::not_found("issue", key))
    }

    async fn list(&self, query: IssueQuery) -> Result<Vec<Issue>, AppError> {
        let deleted_filter = if query.deleted_only {
            "only"
        } else if query.include_deleted {
            "include"
        } else {
            "exclude"
        };
        if let Some(jql_expr) = &query.jql {
            let user_id = query.jql_user_id.unwrap_or_default();
            let compiled = crate::jql::compile(jql_expr, user_id)
                .map_err(|e| AppError::invalid_input(e.to_string()))?;
            return self
                .search_by_jql(
                    &compiled,
                    query.limit,
                    query.offset,
                    deleted_filter,
                    query.accessible_project_ids.as_deref(),
                )
                .await;
        }
        let mut select = issue::Entity::find();
        // Soft-delete filtering.
        match deleted_filter {
            "only" => {
                select = select.filter(issue::Column::DeletedAt.is_not_null());
            }
            "exclude" => {
                select = select.filter(issue::Column::DeletedAt.is_null());
            }
            _ => {}
        }
        if let Some(pid) = query.project_id {
            select = select.filter(issue::Column::ProjectId.eq(pid.as_uuid()));
        }
        if let Some(ids) = query.accessible_project_ids.as_ref() {
            if ids.is_empty() {
                // No accessible projects: the result set is provably empty.
                return Ok(vec![]);
            }
            let mut cond = sea_orm::Condition::any();
            for id in ids {
                cond = cond.add(issue::Column::ProjectId.eq(id.as_uuid()));
            }
            select = select.filter(cond);
        }
        if let Some(sid) = query.status_id {
            select = select.filter(issue::Column::StatusId.eq(sid.as_uuid()));
        }
        if let Some(aid) = query.assignee_id {
            select = select.filter(issue::Column::AssigneeId.eq(aid.as_uuid()));
        }
        if let Some(spid) = query.sprint_id {
            select = select.filter(issue::Column::SprintId.eq(spid.as_uuid()));
        }
        if let Some(priority) = query.priority.as_deref().filter(|s| !s.is_empty()) {
            select = select.filter(issue::Column::Priority.eq(priority));
        }
        if let Some(sort_by) = query.sort_by.as_deref() {
            let order = query.sort_order.as_deref().unwrap_or("asc");
            let col: issue::Column = match sort_by {
                "created" => issue::Column::CreatedAt,
                "updated" => issue::Column::UpdatedAt,
                "priority" => issue::Column::Priority,
                _ => issue::Column::CreatedAt,
            };
            select = match order {
                "desc" => select.order_by_desc(col),
                _ => select.order_by_asc(col),
            };
        }
        if let Some(q) = query.search_text.as_deref().filter(|s| !s.is_empty()) {
            // Escape LIKE metacharacters so user input matches literally.
            let escaped = q
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            let pattern = format!("%{}%", escaped);
            select = select.filter(
                sea_orm::Condition::any()
                    .add(Expr::col(issue::Column::Summary).ilike(&pattern))
                    .add(Expr::col(issue::Column::Key).ilike(&pattern))
                    .add(Expr::col(issue::Column::Description).ilike(&pattern)),
            );
        }
        let models = select
            .limit(query.limit)
            .offset(query.offset)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_issue).collect())
    }

    async fn save(&self, issue: &Issue) -> Result<IssueId, AppError> {
        let exists = issue::Entity::find_by_id(issue.id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?
            .is_some();
        let labels = issue
            .labels
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>();
        let active = issue::ActiveModel {
            id: Set(issue.id.as_uuid()),
            project_id: Set(issue.project_id.as_uuid()),
            key: Set(issue.key.to_string()),
            issue_type: Set(format!("{:?}", issue.issue_type)),
            status_id: Set(issue.status_id.as_uuid()),
            summary: Set(issue.summary.as_ref().to_string()),
            description: Set(issue.description.as_ref().map(|d| d.as_ref().to_string())),
            assignee_id: Set(issue.assignee_id.map(|id| id.as_uuid())),
            reporter_id: Set(issue.reporter_id.as_uuid()),
            priority: Set(format!("{:?}", issue.priority)),
            labels: Set(serde_json::to_value(labels).unwrap_or_default()),
            sprint_id: Set(issue.sprint_id.map(|id| id.as_uuid())),
            component_id: Set(issue.component_id.map(|id| id.as_uuid())),
            affected_version_id: Set(issue.affected_version_id.map(|id| id.as_uuid())),
            fix_version_id: Set(issue.fix_version_id.map(|id| id.as_uuid())),
            position: Set(issue.position),
            due_date: Set(issue.due_date),
            original_estimate_seconds: Set(issue.original_estimate_seconds),
            remaining_estimate_seconds: Set(issue.remaining_estimate_seconds),
            time_spent_seconds: Set(issue.time_spent_seconds),
            created_at: Set(issue.created_at),
            updated_at: Set(shared::now()),
            deleted_at: Set(issue.deleted_at),
        };
        if exists {
            active.update(&*self.db).await.map_err(AppError::database)?;
        } else {
            active.insert(&*self.db).await.map_err(AppError::database)?;
        }
        Ok(issue.id)
    }

    async fn delete(&self, id: IssueId) -> Result<(), AppError> {
        // Soft-delete: set deleted_at. Only live issues can be soft-deleted.
        let res = issue::Entity::update_many()
            .col_expr(issue::Column::DeletedAt, Expr::current_timestamp().into())
            .filter(issue::Column::Id.eq(id.as_uuid()))
            .filter(issue::Column::DeletedAt.is_null())
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        if res.rows_affected == 0 {
            // Either the issue doesn't exist or it's already deleted.
            return Err(AppError::not_found("issue", id));
        }
        Ok(())
    }

    async fn restore(&self, id: IssueId) -> Result<(), AppError> {
        // Clear deleted_at. Only trashed issues can be restored.
        let res = issue::Entity::update_many()
            .col_expr(
                issue::Column::DeletedAt,
                Expr::value(None::<chrono::DateTime<chrono::FixedOffset>>),
            )
            .filter(issue::Column::Id.eq(id.as_uuid()))
            .filter(issue::Column::DeletedAt.is_not_null())
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        if res.rows_affected == 0 {
            // Distinguish "not found" from "not deleted" for a better error message.
            let exists = issue::Entity::find_by_id(id.as_uuid())
                .one(&*self.db)
                .await
                .map_err(AppError::database)?;
            if exists.is_some() {
                return Err(AppError::invalid_input(
                    "issue is not deleted; nothing to restore",
                ));
            }
            return Err(AppError::not_found("issue", id));
        }
        Ok(())
    }

    async fn purge(&self, id: IssueId) -> Result<(), AppError> {
        // Permanent delete: only works on already soft-deleted (trashed) issues.
        // All child deletions and the issue row itself run in a single
        // transaction so a mid-cascade failure cannot leave the trashed issue
        // alive with partially destroyed data.
        let txn = self.db.as_ref().begin().await.map_err(AppError::database)?;
        // Verify the issue exists and is trashed.
        let exists = issue::Entity::find()
            .filter(issue::Column::Id.eq(id.as_uuid()))
            .filter(issue::Column::DeletedAt.is_not_null())
            .one(&txn)
            .await
            .map_err(AppError::database)?;
        if exists.is_none() {
            return Err(AppError::not_found("issue", id));
        }
        comment::Entity::delete_many()
            .filter(comment::Column::IssueId.eq(id.as_uuid()))
            .exec(&txn)
            .await
            .map_err(AppError::database)?;
        worklog::Entity::delete_many()
            .filter(worklog::Column::IssueId.eq(id.as_uuid()))
            .exec(&txn)
            .await
            .map_err(AppError::database)?;
        attachment::Entity::delete_many()
            .filter(attachment::Column::IssueId.eq(id.as_uuid()))
            .exec(&txn)
            .await
            .map_err(AppError::database)?;
        issue_label::Entity::delete_many()
            .filter(issue_label::Column::IssueId.eq(id.as_uuid()))
            .exec(&txn)
            .await
            .map_err(AppError::database)?;
        issue_link::Entity::delete_many()
            .filter(
                sea_orm::Condition::any()
                    .add(issue_link::Column::SourceId.eq(id.as_uuid()))
                    .add(issue_link::Column::TargetId.eq(id.as_uuid())),
            )
            .exec(&txn)
            .await
            .map_err(AppError::database)?;
        issue_status_history::Entity::delete_many()
            .filter(issue_status_history::Column::IssueId.eq(id.as_uuid()))
            .exec(&txn)
            .await
            .map_err(AppError::database)?;
        issue_custom_field_value::Entity::delete_many()
            .filter(issue_custom_field_value::Column::IssueId.eq(id.as_uuid()))
            .exec(&txn)
            .await
            .map_err(AppError::database)?;
        issue_vote::Entity::delete_many()
            .filter(issue_vote::Column::IssueId.eq(id.as_uuid()))
            .exec(&txn)
            .await
            .map_err(AppError::database)?;
        issue_watcher::Entity::delete_many()
            .filter(issue_watcher::Column::IssueId.eq(id.as_uuid()))
            .exec(&txn)
            .await
            .map_err(AppError::database)?;
        // Finally, hard-delete the issue row.
        issue::Entity::delete_many()
            .filter(issue::Column::Id.eq(id.as_uuid()))
            .filter(issue::Column::DeletedAt.is_not_null())
            .exec(&txn)
            .await
            .map_err(AppError::database)?;
        txn.commit().await.map_err(AppError::database)?;
        Ok(())
    }
}

struct BoardRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl BoardRepository for BoardRepo {
    async fn get_by_id(&self, id: BoardId) -> Result<Board, AppError> {
        let model = board::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_board)
            .ok_or_else(|| AppError::not_found("board", id))
    }

    async fn get_default_by_project(&self, project_id: ProjectId) -> Result<Board, AppError> {
        let model = board::Entity::find()
            .filter(board::Column::ProjectId.eq(project_id.as_uuid()))
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_board)
            .ok_or_else(|| AppError::not_found("board", project_id))
    }

    async fn get_default_by_project_key(
        &self,
        project_key: &ProjectKey,
    ) -> Result<Board, AppError> {
        let project = project::Entity::find()
            .filter(project::Column::Key.eq(project_key.as_str()))
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        let project_id = project
            .map(|p| p.id)
            .ok_or_else(|| AppError::not_found("project", project_key))?;
        let model = board::Entity::find()
            .filter(board::Column::ProjectId.eq(project_id))
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_board)
            .ok_or_else(|| AppError::not_found("board", project_key))
    }

    async fn save(&self, board: &Board) -> Result<(), AppError> {
        let columns = serde_json::to_value(
            board
                .columns
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "id": c.id.as_uuid().to_string(),
                        "name": c.name.as_ref(),
                        "category": format!("{:?}", c.category),
                        "wip_limit": c.wip_limit,
                        "position": c.position,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default();
        let active = board::ActiveModel {
            id: Set(board.id.as_uuid()),
            project_id: Set(board.project_id.as_uuid()),
            name: Set(board.name.as_ref().to_string()),
            columns: Set(columns),
        };
        active.insert(&*self.db).await.map_err(AppError::database)?;
        Ok(())
    }
}

struct SprintRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl SprintRepository for SprintRepo {
    async fn get_active_by_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Option<Sprint>, AppError> {
        let model = sprint::Entity::find()
            .filter(sprint::Column::ProjectId.eq(project_id.as_uuid()))
            // Only an Active sprint is "the active sprint"; without this a
            // future or closed sprint could be picked arbitrarily.
            .filter(sprint::Column::State.eq("Active"))
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(model.map(map_sprint))
    }

    async fn get_by_id(&self, id: SprintId) -> Result<Sprint, AppError> {
        let model = sprint::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_sprint)
            .ok_or_else(|| AppError::not_found("sprint", id))
    }

    async fn list_by_project(&self, project_id: ProjectId) -> Result<Vec<Sprint>, AppError> {
        let models = sprint::Entity::find()
            .filter(sprint::Column::ProjectId.eq(project_id.as_uuid()))
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_sprint).collect())
    }

    async fn save(&self, sprint: &Sprint) -> Result<SprintId, AppError> {
        let exists = sprint::Entity::find_by_id(sprint.id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?
            .is_some();
        let active = sprint::ActiveModel {
            id: Set(sprint.id.as_uuid()),
            project_id: Set(sprint.project_id.as_uuid()),
            name: Set(sprint.name.as_ref().to_string()),
            goal: Set(sprint.goal.as_ref().map(|g| g.as_ref().to_string())),
            state: Set(format!("{:?}", sprint.state)),
            start_date: Set(sprint.start_date),
            end_date: Set(sprint.end_date),
            velocity: Set(sprint.velocity),
        };
        if exists {
            active.update(&*self.db).await.map_err(AppError::database)?;
        } else {
            active.insert(&*self.db).await.map_err(AppError::database)?;
        }
        Ok(sprint.id)
    }
}

fn map_user(m: user::Model) -> User {
    User {
        id: UserId::from_uuid(m.id),
        email: m.email.into(),
        username: m.username.into(),
        display_name: m.display_name.into(),
        password_hash: m.password_hash.into(),
        refresh_token_hash: m.refresh_token_hash.map(|h| h.into()),
        is_system_admin: m.is_system_admin,
        is_active: m.is_active,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

fn map_project(m: project::Model) -> Project {
    Project {
        id: ProjectId::from_uuid(m.id),
        key: ProjectKey::new(m.key),
        name: m.name.into(),
        description: m.description.map(|d| d.into()),
        owner_id: UserId::from_uuid(m.owner_id),
        default_board_id: BoardId::from_uuid(m.default_board_id),
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

fn map_issue(m: issue::Model) -> Issue {
    Issue {
        id: IssueId::from_uuid(m.id),
        project_id: ProjectId::from_uuid(m.project_id),
        key: IssueKey::parse(&m.key)
            .unwrap_or_else(|_| IssueKey::new(ProjectKey::new("UNKNOWN"), 0)),
        issue_type: IssueType::from_str(&m.issue_type).unwrap_or_default(),
        status_id: StatusId::from_uuid(m.status_id),
        summary: m.summary.into(),
        description: m.description.map(domain::value_objects::RichText::new),
        assignee_id: m.assignee_id.map(UserId::from_uuid),
        reporter_id: UserId::from_uuid(m.reporter_id),
        priority: Priority::from_str(&m.priority).unwrap_or_default(),
        labels: m
            .labels
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| LabelId::from_str(s).ok()))
                    .flatten()
                    .collect()
            })
            .unwrap_or_default(),
        sprint_id: m.sprint_id.map(SprintId::from_uuid),
        position: m.position,
        due_date: m.due_date,
        original_estimate_seconds: m.original_estimate_seconds,
        remaining_estimate_seconds: m.remaining_estimate_seconds,
        time_spent_seconds: m.time_spent_seconds,
        component_id: m.component_id.map(shared::ProjectComponentId::from_uuid),
        affected_version_id: m
            .affected_version_id
            .map(shared::ProjectVersionId::from_uuid),
        fix_version_id: m.fix_version_id.map(shared::ProjectVersionId::from_uuid),
        created_at: m.created_at,
        updated_at: m.updated_at,
        deleted_at: m.deleted_at,
        events: Vec::new(),
    }
}

fn map_board(m: board::Model) -> Board {
    let columns = m
        .columns
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let id = Uuid::parse_str(v.get("id")?.as_str()?).ok()?;
                    let name = v.get("name")?.as_str()?;
                    let category = v.get("category")?.as_str()?;
                    Some(BoardColumn {
                        id: StatusId::from_uuid(id),
                        name: name.into(),
                        category: match category {
                            "Todo" | "todo" => StatusCategory::Todo,
                            "InProgress" | "in_progress" => StatusCategory::InProgress,
                            "Done" | "done" => StatusCategory::Done,
                            _ => StatusCategory::Todo,
                        },
                        wip_limit: v.get("wip_limit").and_then(|x| x.as_i64()),
                        position: v.get("position").and_then(|x| x.as_i64()).unwrap_or(0) as i32,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Board {
        id: BoardId::from_uuid(m.id),
        project_id: ProjectId::from_uuid(m.project_id),
        name: m.name.into(),
        columns,
    }
}

fn map_sprint(m: sprint::Model) -> Sprint {
    Sprint {
        id: SprintId::from_uuid(m.id),
        project_id: ProjectId::from_uuid(m.project_id),
        name: m.name.into(),
        goal: m.goal.map(|g| g.into()),
        state: SprintState::from_str(&m.state).unwrap_or_default(),
        start_date: m.start_date,
        end_date: m.end_date,
        velocity: m.velocity,
    }
}

pub fn to_domain_repositories(sea: SeaOrmRepositories) -> domain::Repositories {
    domain::Repositories {
        users: sea.users,
        audit_logs: sea.audit_logs,
        system_settings: sea.system_settings,
        projects: sea.projects,
        issues: sea.issues,
        boards: sea.boards,
        sprints: sea.sprints,
        comments: sea.comments,
        worklogs: sea.worklogs,
        members: sea.members,
        statuses: sea.statuses,
        transitions: sea.transitions,
        issue_types: sea.issue_types,
        attachments: sea.attachments,
        labels: sea.labels,
        custom_fields: sea.custom_fields,
        issue_links: sea.issue_links,
        notifications: sea.notifications,
        notification_settings: sea.notification_settings,
        issue_status_history: sea.issue_status_history,
        watchers: sea.watchers,
        votes: sea.votes,
        components: sea.components,
        versions: sea.versions,
    }
}

struct AttachmentRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl AttachmentRepository for AttachmentRepo {
    async fn get_by_id(&self, id: AttachmentId) -> Result<Attachment, AppError> {
        let model = attachment::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_attachment)
            .ok_or_else(|| AppError::not_found("attachment", id))
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<Attachment>, AppError> {
        let models = attachment::Entity::find()
            .filter(attachment::Column::IssueId.eq(issue_id.as_uuid()))
            .order_by_asc(attachment::Column::CreatedAt)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_attachment).collect())
    }

    async fn save(&self, attachment: &Attachment) -> Result<AttachmentId, AppError> {
        let active = attachment::ActiveModel {
            id: Set(attachment.id.as_uuid()),
            issue_id: Set(attachment.issue_id.as_uuid()),
            author_id: Set(attachment.author_id.as_uuid()),
            file_name: Set(attachment.file_name.as_ref().to_string()),
            content_type: Set(attachment.content_type.as_ref().to_string()),
            size_bytes: Set(attachment.size_bytes),
            storage_key: Set(attachment.storage_key.as_ref().to_string()),
            created_at: Set(attachment.created_at),
        };
        active.insert(&*self.db).await.map_err(AppError::database)?;
        Ok(attachment.id)
    }

    async fn delete(&self, id: AttachmentId) -> Result<(), AppError> {
        attachment::Entity::delete_by_id(id.as_uuid())
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }
}

fn map_attachment(m: attachment::Model) -> Attachment {
    Attachment {
        id: AttachmentId::from_uuid(m.id),
        issue_id: IssueId::from_uuid(m.issue_id),
        author_id: UserId::from_uuid(m.author_id),
        file_name: m.file_name.into(),
        content_type: m.content_type.into(),
        size_bytes: m.size_bytes,
        storage_key: m.storage_key.into(),
        created_at: m.created_at,
    }
}

struct LabelRepo {
    db: Arc<DatabaseConnection>,
}

fn map_label(m: label::Model) -> Label {
    Label {
        id: LabelId::from_uuid(m.id),
        project_id: ProjectId::from_uuid(m.project_id),
        name: m.name.into(),
        color: m.color.into(),
    }
}

#[async_trait]
impl LabelRepository for LabelRepo {
    async fn get_by_id(&self, id: LabelId) -> Result<Label, AppError> {
        let model = label::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_label)
            .ok_or_else(|| AppError::not_found("label", id))
    }

    async fn list_by_project(&self, project_id: ProjectId) -> Result<Vec<Label>, AppError> {
        let models = label::Entity::find()
            .filter(label::Column::ProjectId.eq(project_id.as_uuid()))
            .order_by_asc(label::Column::Name)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_label).collect())
    }

    async fn save(&self, label: &Label) -> Result<LabelId, AppError> {
        let existing = label::Entity::find_by_id(label.id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        let active = label::ActiveModel {
            id: Set(label.id.as_uuid()),
            project_id: Set(label.project_id.as_uuid()),
            name: Set(label.name.as_ref().to_string()),
            color: Set(label.color.as_ref().to_string()),
            created_at: Set(existing
                .as_ref()
                .map(|m| m.created_at)
                .unwrap_or_else(|| chrono::Utc::now().fixed_offset())),
        };
        // Explicit insert/update branch: a new entity with a client-generated UUID
        // must INSERT, not UPDATE-by-id (which matches zero rows).
        let saved = if existing.is_some() {
            active.update(&*self.db).await.map_err(AppError::database)?
        } else {
            active.insert(&*self.db).await.map_err(AppError::database)?
        };
        Ok(LabelId::from_uuid(saved.id))
    }

    async fn delete(&self, id: LabelId) -> Result<(), AppError> {
        label::Entity::delete_by_id(id.as_uuid())
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }

    async fn list_ids_by_issue(&self, issue_id: IssueId) -> Result<Vec<LabelId>, AppError> {
        let models = issue_label::Entity::find()
            .filter(issue_label::Column::IssueId.eq(issue_id.as_uuid()))
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models
            .into_iter()
            .map(|m| LabelId::from_uuid(m.label_id))
            .collect())
    }

    async fn attach(&self, issue_id: IssueId, label_id: LabelId) -> Result<(), AppError> {
        let existing = issue_label::Entity::find()
            .filter(issue_label::Column::IssueId.eq(issue_id.as_uuid()))
            .filter(issue_label::Column::LabelId.eq(label_id.as_uuid()))
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        if existing.is_some() {
            return Ok(());
        }
        let active = issue_label::ActiveModel {
            issue_id: Set(issue_id.as_uuid()),
            label_id: Set(label_id.as_uuid()),
        };
        active.insert(&*self.db).await.map_err(AppError::database)?;
        Ok(())
    }

    async fn detach(&self, issue_id: IssueId, label_id: LabelId) -> Result<(), AppError> {
        issue_label::Entity::delete_many()
            .filter(issue_label::Column::IssueId.eq(issue_id.as_uuid()))
            .filter(issue_label::Column::LabelId.eq(label_id.as_uuid()))
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }
}

struct CustomFieldRepo {
    db: Arc<DatabaseConnection>,
}

fn map_custom_field(m: custom_field::Model) -> CustomField {
    let options = match m.options {
        serde_json::Value::Array(values) => values
            .into_iter()
            .filter_map(|value| value.as_str().map(Into::into))
            .collect(),
        _ => Vec::new(),
    };
    CustomField {
        id: CustomFieldId::from_uuid(m.id),
        project_id: ProjectId::from_uuid(m.project_id),
        name: m.name.into(),
        field_type: m.field_type.parse().unwrap_or(CustomFieldType::Text),
        options,
        is_required: m.is_required,
        created_at: m.created_at,
    }
}

#[async_trait]
impl CustomFieldRepository for CustomFieldRepo {
    async fn get_by_id(&self, id: CustomFieldId) -> Result<CustomField, AppError> {
        custom_field::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?
            .map(map_custom_field)
            .ok_or_else(|| AppError::not_found("custom field", id))
    }

    async fn list_by_project(&self, project_id: ProjectId) -> Result<Vec<CustomField>, AppError> {
        let models = custom_field::Entity::find()
            .filter(custom_field::Column::ProjectId.eq(project_id.as_uuid()))
            .order_by_asc(custom_field::Column::CreatedAt)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_custom_field).collect())
    }

    async fn save(&self, field: &CustomField) -> Result<CustomFieldId, AppError> {
        let existing = custom_field::Entity::find_by_id(field.id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        let active = custom_field::ActiveModel {
            id: Set(field.id.as_uuid()),
            project_id: Set(field.project_id.as_uuid()),
            name: Set(field.name.as_ref().to_string()),
            field_type: Set(field.field_type.as_str().to_string()),
            options: Set(serde_json::json!(field.options)),
            is_required: Set(field.is_required),
            created_at: Set(field.created_at),
        };
        let saved = if existing.is_some() {
            active.update(&*self.db).await.map_err(AppError::database)?
        } else {
            active.insert(&*self.db).await.map_err(AppError::database)?
        };
        Ok(CustomFieldId::from_uuid(saved.id))
    }

    async fn delete(&self, id: CustomFieldId) -> Result<(), AppError> {
        custom_field::Entity::delete_by_id(id.as_uuid())
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }

    async fn set_value(
        &self,
        issue_id: IssueId,
        field_id: CustomFieldId,
        value: &serde_json::Value,
    ) -> Result<(), AppError> {
        let existing = issue_custom_field_value::Entity::find()
            .filter(issue_custom_field_value::Column::IssueId.eq(issue_id.as_uuid()))
            .filter(issue_custom_field_value::Column::FieldId.eq(field_id.as_uuid()))
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        let active = issue_custom_field_value::ActiveModel {
            issue_id: Set(issue_id.as_uuid()),
            field_id: Set(field_id.as_uuid()),
            value: Set(value.clone()),
        };
        if existing.is_some() {
            active.update(&*self.db).await.map_err(AppError::database)?;
        } else {
            active.insert(&*self.db).await.map_err(AppError::database)?;
        }
        Ok(())
    }

    async fn get_values_for_issue(
        &self,
        issue_id: IssueId,
    ) -> Result<Vec<CustomFieldValue>, AppError> {
        let models = issue_custom_field_value::Entity::find()
            .filter(issue_custom_field_value::Column::IssueId.eq(issue_id.as_uuid()))
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models
            .into_iter()
            .map(|m| CustomFieldValue {
                issue_id: IssueId::from_uuid(m.issue_id),
                field_id: CustomFieldId::from_uuid(m.field_id),
                value: m.value,
            })
            .collect())
    }

    async fn delete_values_for_issue(&self, issue_id: IssueId) -> Result<(), AppError> {
        issue_custom_field_value::Entity::delete_many()
            .filter(issue_custom_field_value::Column::IssueId.eq(issue_id.as_uuid()))
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }
}

struct IssueLinkRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl IssueLinkRepository for IssueLinkRepo {
    async fn get_by_id(&self, id: IssueLinkId) -> Result<IssueLink, AppError> {
        let model = issue_link::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(|m| IssueLink {
                id: IssueLinkId::from_uuid(m.id),
                source_id: IssueId::from_uuid(m.source_id),
                target_id: IssueId::from_uuid(m.target_id),
                link_type: m.link_type.parse().unwrap_or(LinkType::Relates),
            })
            .ok_or_else(|| AppError::not_found("issue link", id))
    }

    async fn save(&self, link: &IssueLink) -> Result<IssueLinkId, AppError> {
        let active = issue_link::ActiveModel {
            id: Set(link.id.as_uuid()),
            source_id: Set(link.source_id.as_uuid()),
            target_id: Set(link.target_id.as_uuid()),
            link_type: Set(link.link_type.as_str().to_string()),
            created_at: Set(chrono::Utc::now().fixed_offset()),
        };
        active.insert(&*self.db).await.map_err(AppError::database)?;
        Ok(link.id)
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<IssueLink>, AppError> {
        let models = issue_link::Entity::find()
            .filter(
                sea_orm::Condition::any()
                    .add(issue_link::Column::SourceId.eq(issue_id.as_uuid()))
                    .add(issue_link::Column::TargetId.eq(issue_id.as_uuid())),
            )
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models
            .into_iter()
            .map(|m| IssueLink {
                id: IssueLinkId::from_uuid(m.id),
                source_id: IssueId::from_uuid(m.source_id),
                target_id: IssueId::from_uuid(m.target_id),
                link_type: m.link_type.parse().unwrap_or(LinkType::Relates),
            })
            .collect())
    }

    async fn delete(&self, id: IssueLinkId) -> Result<(), AppError> {
        issue_link::Entity::delete_by_id(id.as_uuid())
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }
}

struct CommentRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl CommentRepository for CommentRepo {
    async fn get_by_id(&self, id: CommentId) -> Result<Comment, AppError> {
        let model = comment::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_comment)
            .ok_or_else(|| AppError::not_found("comment", id))
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<Comment>, AppError> {
        let models = comment::Entity::find()
            .filter(comment::Column::IssueId.eq(issue_id.as_uuid()))
            .order_by_asc(comment::Column::CreatedAt)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_comment).collect())
    }

    async fn save(&self, comment_item: &Comment) -> Result<CommentId, AppError> {
        let exists = comment::Entity::find_by_id(comment_item.id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?
            .is_some();
        let active = comment::ActiveModel {
            id: Set(comment_item.id.as_uuid()),
            issue_id: Set(comment_item.issue_id.as_uuid()),
            author_id: Set(comment_item.author_id.as_uuid()),
            body: Set(comment_item.body.as_ref().to_string()),
            created_at: Set(comment_item.created_at),
            updated_at: Set(shared::now()),
        };
        if exists {
            active.update(&*self.db).await.map_err(AppError::database)?;
        } else {
            active.insert(&*self.db).await.map_err(AppError::database)?;
        }
        Ok(comment_item.id)
    }

    async fn delete(&self, id: CommentId) -> Result<(), AppError> {
        comment::Entity::delete_by_id(id.as_uuid())
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }
}

fn map_comment(m: comment::Model) -> Comment {
    Comment {
        id: CommentId::from_uuid(m.id),
        issue_id: IssueId::from_uuid(m.issue_id),
        author_id: UserId::from_uuid(m.author_id),
        body: domain::value_objects::RichText::new(m.body),
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

struct WorklogRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl WorklogRepository for WorklogRepo {
    async fn get_by_id(&self, id: WorklogId) -> Result<Worklog, AppError> {
        let model = worklog::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_worklog)
            .ok_or_else(|| AppError::not_found("worklog", id))
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<Worklog>, AppError> {
        let models = worklog::Entity::find()
            .filter(worklog::Column::IssueId.eq(issue_id.as_uuid()))
            .order_by_asc(worklog::Column::StartedAt)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_worklog).collect())
    }

    async fn save(&self, worklog_item: &Worklog) -> Result<WorklogId, AppError> {
        let exists = worklog::Entity::find_by_id(worklog_item.id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?
            .is_some();
        let active = worklog::ActiveModel {
            id: Set(worklog_item.id.as_uuid()),
            issue_id: Set(worklog_item.issue_id.as_uuid()),
            author_id: Set(worklog_item.author_id.as_uuid()),
            started_at: Set(worklog_item.started_at),
            duration_seconds: Set(worklog_item.duration_seconds),
            description: Set(worklog_item
                .description
                .as_ref()
                .map(|d| d.as_ref().to_string())),
            created_at: Set(worklog_item.created_at),
            updated_at: Set(shared::now()),
        };
        if exists {
            active.update(&*self.db).await.map_err(AppError::database)?;
        } else {
            active.insert(&*self.db).await.map_err(AppError::database)?;
        }
        Ok(worklog_item.id)
    }

    async fn delete(&self, id: WorklogId) -> Result<(), AppError> {
        worklog::Entity::delete_by_id(id.as_uuid())
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }
}

fn map_worklog(m: worklog::Model) -> Worklog {
    Worklog {
        id: WorklogId::from_uuid(m.id),
        issue_id: IssueId::from_uuid(m.issue_id),
        author_id: UserId::from_uuid(m.author_id),
        started_at: m.started_at,
        duration_seconds: m.duration_seconds,
        description: m.description.map(|d| d.into()),
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}

struct ProjectMemberRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl ProjectMemberRepository for ProjectMemberRepo {
    async fn list_by_project(&self, project_id: ProjectId) -> Result<Vec<ProjectMember>, AppError> {
        let models = project_member::Entity::find()
            .filter(project_member::Column::ProjectId.eq(project_id.as_uuid()))
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_project_member).collect())
    }

    async fn list_by_user(&self, user_id: UserId) -> Result<Vec<ProjectMember>, AppError> {
        let models = project_member::Entity::find()
            .filter(project_member::Column::UserId.eq(user_id.as_uuid()))
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_project_member).collect())
    }

    async fn get(&self, project_id: ProjectId, user_id: UserId) -> Result<ProjectMember, AppError> {
        let model = project_member::Entity::find_by_id((project_id.as_uuid(), user_id.as_uuid()))
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_project_member)
            .ok_or_else(|| AppError::not_found("project member", project_id))
    }

    async fn save(&self, member: &ProjectMember) -> Result<(), AppError> {
        // Upsert: re-adding an existing member updates the role instead of failing.
        let insert = sea_orm::sea_query::Query::insert()
            .into_table(project_member::Entity)
            .columns([
                project_member::Column::ProjectId,
                project_member::Column::UserId,
                project_member::Column::Role,
                project_member::Column::JoinedAt,
            ])
            .values_panic([
                member.project_id.as_uuid().into(),
                member.user_id.as_uuid().into(),
                member.role.as_str().into(),
                member.joined_at.into(),
            ])
            .on_conflict(
                sea_orm::sea_query::OnConflict::columns([
                    project_member::Column::ProjectId,
                    project_member::Column::UserId,
                ])
                .update_columns([project_member::Column::Role])
                .to_owned(),
            )
            .to_owned();
        self.db
            .execute(sea_orm::Statement::from_sql_and_values(
                self.db.get_database_backend(),
                insert.to_string(sea_orm::sea_query::PostgresQueryBuilder),
                [],
            ))
            .await
            .map_err(AppError::database)?;
        Ok(())
    }

    async fn delete(&self, project_id: ProjectId, user_id: UserId) -> Result<(), AppError> {
        project_member::Entity::delete_by_id((project_id.as_uuid(), user_id.as_uuid()))
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }
}

fn map_project_member(m: project_member::Model) -> ProjectMember {
    ProjectMember {
        project_id: ProjectId::from_uuid(m.project_id),
        user_id: UserId::from_uuid(m.user_id),
        role: ProjectRole::from_str(&m.role).unwrap_or_default(),
        joined_at: m.joined_at,
    }
}

struct StatusRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl StatusRepository for StatusRepo {
    async fn list_all(&self) -> Result<Vec<Status>, AppError> {
        let models = status::Entity::find()
            .order_by_asc(status::Column::Position)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_status).collect())
    }

    async fn get_default(&self) -> Result<Status, AppError> {
        let model = status::Entity::find()
            .filter(status::Column::IsDefault.eq(true))
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_status)
            .ok_or_else(|| AppError::not_found("default status", "default"))
    }

    async fn get_by_id(&self, id: StatusId) -> Result<Status, AppError> {
        let model = status::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_status)
            .ok_or_else(|| AppError::not_found("status", id))
    }
}

struct TransitionRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl WorkflowTransitionRepository for TransitionRepo {
    async fn list_all(&self) -> Result<Vec<WorkflowTransition>, AppError> {
        let models = workflow_transition::Entity::find()
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_transition).collect())
    }

    async fn is_allowed(
        &self,
        from_status_id: StatusId,
        to_status_id: StatusId,
    ) -> Result<bool, AppError> {
        let from_uuid = from_status_id.as_uuid();
        let to_uuid = to_status_id.as_uuid();
        if from_uuid == to_uuid {
            return Ok(true);
        }
        let count = workflow_transition::Entity::find()
            .filter(workflow_transition::Column::FromStatusId.eq(from_uuid))
            .filter(workflow_transition::Column::ToStatusId.eq(to_uuid))
            .count(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(count > 0)
    }
}

struct IssueTypeRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl IssueTypeRepository for IssueTypeRepo {
    async fn list_all(&self) -> Result<Vec<IssueTypeEntity>, AppError> {
        let models = issue_type::Entity::find()
            .order_by_asc(issue_type::Column::HierarchyLevel)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_issue_type).collect())
    }

    async fn get_by_id(&self, id: IssueTypeId) -> Result<IssueTypeEntity, AppError> {
        let model = issue_type::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_issue_type)
            .ok_or_else(|| AppError::not_found("issue type", id))
    }
}

struct NotificationRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait::async_trait]
impl NotificationRepository for NotificationRepo {
    async fn save(&self, notification: &Notification) -> Result<NotificationId, AppError> {
        let model = notification::ActiveModel {
            id: sea_orm::ActiveValue::Set(notification.id.as_uuid()),
            recipient_id: sea_orm::ActiveValue::Set(notification.recipient_id.as_uuid()),
            event_type: sea_orm::ActiveValue::Set(notification.event_type.as_ref().to_string()),
            entity_type: sea_orm::ActiveValue::Set(notification.entity_type.as_ref().to_string()),
            entity_id: sea_orm::ActiveValue::Set(notification.entity_id),
            actor_id: sea_orm::ActiveValue::Set(notification.actor_id.map(|id| id.as_uuid())),
            title: sea_orm::ActiveValue::Set(notification.title.as_ref().to_string()),
            body: sea_orm::ActiveValue::Set(
                notification.body.as_ref().map(|s| s.as_ref().to_string()),
            ),
            is_read: sea_orm::ActiveValue::Set(notification.is_read),
            read_at: sea_orm::ActiveValue::Set(notification.read_at),
            action_url: sea_orm::ActiveValue::Set(
                notification
                    .action_url
                    .as_ref()
                    .map(|s| s.as_ref().to_string()),
            ),
            metadata: sea_orm::ActiveValue::Set(notification.metadata.clone()),
            created_at: sea_orm::ActiveValue::Set(notification.created_at),
        };
        notification::Entity::insert(model)
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(notification.id)
    }

    async fn list_unread(&self, recipient_id: UserId) -> Result<Vec<Notification>, AppError> {
        let models = notification::Entity::find()
            .filter(notification::Column::RecipientId.eq(recipient_id.as_uuid()))
            .filter(notification::Column::IsRead.eq(false))
            .order_by_asc(notification::Column::CreatedAt)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_notification).collect())
    }

    async fn list_all_unread(&self) -> Result<Vec<Notification>, AppError> {
        let models = notification::Entity::find()
            .filter(notification::Column::IsRead.eq(false))
            .order_by_asc(notification::Column::CreatedAt)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_notification).collect())
    }

    async fn mark_read(&self, id: NotificationId, recipient_id: UserId) -> Result<(), AppError> {
        let result = notification::Entity::update_many()
            .col_expr(notification::Column::IsRead, Expr::value(true))
            .col_expr(
                notification::Column::ReadAt,
                Expr::current_timestamp().into(),
            )
            .filter(notification::Column::Id.eq(id.as_uuid()))
            .filter(notification::Column::RecipientId.eq(recipient_id.as_uuid()))
            .filter(notification::Column::IsRead.eq(false))
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        if result.rows_affected == 0 {
            return Err(AppError::not_found("notification", id));
        }
        Ok(())
    }

    async fn mark_all_read(&self, recipient_id: UserId) -> Result<(), AppError> {
        notification::Entity::update_many()
            .col_expr(notification::Column::IsRead, Expr::value(true))
            .col_expr(
                notification::Column::ReadAt,
                Expr::current_timestamp().into(),
            )
            .filter(notification::Column::RecipientId.eq(recipient_id.as_uuid()))
            .filter(notification::Column::IsRead.eq(false))
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }
}

fn map_notification(m: notification::Model) -> Notification {
    Notification {
        id: NotificationId::from_uuid(m.id),
        recipient_id: UserId::from_uuid(m.recipient_id),
        event_type: m.event_type.into(),
        entity_type: m.entity_type.into(),
        entity_id: m.entity_id,
        actor_id: m.actor_id.map(UserId::from_uuid),
        title: m.title.into(),
        body: m.body.map(Into::into),
        is_read: m.is_read,
        read_at: m.read_at,
        action_url: m.action_url.map(Into::into),
        metadata: m.metadata,
        created_at: m.created_at,
    }
}

struct NotificationUserSettingsRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait::async_trait]
impl UserNotificationSettingsRepository for NotificationUserSettingsRepo {
    async fn get_settings(&self, user_id: UserId) -> Result<NotificationUserSettings, AppError> {
        let model = notification_user_settings::Entity::find_by_id(user_id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?
            .ok_or_else(|| AppError::not_found("notification settings", user_id))?;
        Ok(map_notification_user_settings(model))
    }

    async fn save_settings(&self, settings: &NotificationUserSettings) -> Result<(), AppError> {
        let model = notification_user_settings::ActiveModel {
            user_id: sea_orm::ActiveValue::Set(settings.user_id.as_uuid()),
            email_frequency: sea_orm::ActiveValue::Set(settings.email_frequency.to_string()),
            disabled_event_types: sea_orm::ActiveValue::Set(serde_json::Value::Array(
                settings
                    .disabled_event_types
                    .iter()
                    .map(|event_type| serde_json::Value::String(event_type.to_string()))
                    .collect(),
            )),
            notify_own_changes: sea_orm::ActiveValue::Set(settings.notify_own_changes),
        };
        notification_user_settings::Entity::insert(model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(notification_user_settings::Column::UserId)
                    .update_columns([
                        notification_user_settings::Column::EmailFrequency,
                        notification_user_settings::Column::DisabledEventTypes,
                        notification_user_settings::Column::NotifyOwnChanges,
                    ])
                    .to_owned(),
            )
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }
}

fn map_notification_user_settings(
    m: notification_user_settings::Model,
) -> NotificationUserSettings {
    let disabled_event_types = serde_json::from_value::<Vec<String>>(m.disabled_event_types)
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect();
    NotificationUserSettings {
        user_id: UserId::from_uuid(m.user_id),
        email_frequency: m.email_frequency.into(),
        disabled_event_types,
        notify_own_changes: m.notify_own_changes,
    }
}

struct IssueStatusHistoryRepo {
    db: Arc<DatabaseConnection>,
}

fn map_issue_status_history(m: issue_status_history::Model) -> IssueStatusHistory {
    IssueStatusHistory {
        id: IssueStatusHistoryId::from_uuid(m.id),
        issue_id: IssueId::from_uuid(m.issue_id),
        from_status_id: m.from_status_id.map(StatusId::from_uuid),
        to_status_id: StatusId::from_uuid(m.to_status_id),
        changed_by_id: UserId::from_uuid(m.changed_by_id),
        changed_at: m
            .created_at
            .with_timezone(&chrono::FixedOffset::east_opt(0).unwrap()),
    }
}

#[async_trait]
impl IssueStatusHistoryRepository for IssueStatusHistoryRepo {
    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<IssueStatusHistory>, AppError> {
        let models = issue_status_history::Entity::find()
            .filter(issue_status_history::Column::IssueId.eq(issue_id.as_uuid()))
            .order_by_asc(issue_status_history::Column::CreatedAt)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_issue_status_history).collect())
    }

    async fn list_by_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<IssueStatusHistory>, AppError> {
        // Fetch issue IDs belonging to the project, then filter history by those IDs.
        let issue_ids: Vec<Uuid> = issue::Entity::find()
            .filter(issue::Column::ProjectId.eq(project_id.as_uuid()))
            .select_only()
            .column(issue::Column::Id)
            .into_tuple()
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        if issue_ids.is_empty() {
            return Ok(vec![]);
        }
        let models = issue_status_history::Entity::find()
            .filter(issue_status_history::Column::IssueId.is_in(issue_ids))
            .order_by_asc(issue_status_history::Column::CreatedAt)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_issue_status_history).collect())
    }

    async fn save(&self, entry: &IssueStatusHistory) -> Result<(), AppError> {
        let model = issue_status_history::ActiveModel {
            id: Set(entry.id.as_uuid()),
            issue_id: Set(entry.issue_id.as_uuid()),
            from_status_id: Set(entry.from_status_id.map(|s| s.as_uuid())),
            to_status_id: Set(entry.to_status_id.as_uuid()),
            changed_by_id: Set(entry.changed_by_id.as_uuid()),
            created_at: Set(entry.changed_at),
        };
        issue_status_history::Entity::insert(model)
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }
}

struct AuditLogRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl AuditLogRepository for AuditLogRepo {
    async fn save(&self, entry: &AuditLog) -> Result<(), AppError> {
        audit_log::ActiveModel {
            id: Set(entry.id.as_uuid()),
            actor_id: Set(entry.actor_id.as_uuid()),
            action: Set(entry.action.to_string()),
            entity_type: Set(entry.entity_type.to_string()),
            entity_id: Set(entry.entity_id),
            metadata: Set(entry.metadata.clone()),
            created_at: Set(entry.created_at),
        }
        .insert(&*self.db)
        .await
        .map_err(AppError::database)?;
        Ok(())
    }

    async fn list(
        &self,
        actor_id: Option<UserId>,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<AuditLog>, AppError> {
        let mut query = audit_log::Entity::find().order_by_desc(audit_log::Column::CreatedAt);
        if let Some(actor_id) = actor_id {
            query = query.filter(audit_log::Column::ActorId.eq(actor_id.as_uuid()));
        }
        let models = query
            .offset(offset)
            .limit(limit)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_audit_log).collect())
    }
}

fn map_audit_log(m: audit_log::Model) -> AuditLog {
    AuditLog {
        id: AuditLogId::from_uuid(m.id),
        actor_id: UserId::from_uuid(m.actor_id),
        action: m.action.into(),
        entity_type: m.entity_type.into(),
        entity_id: m.entity_id,
        metadata: m.metadata,
        created_at: m.created_at,
    }
}

struct SystemSettingRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl SystemSettingRepository for SystemSettingRepo {
    async fn get(&self, key: &str) -> Result<SystemSetting, AppError> {
        let model = system_setting::Entity::find_by_id(key)
            .one(&*self.db)
            .await
            .map_err(AppError::database)?
            .ok_or_else(|| AppError::not_found("system setting", key))?;
        Ok(map_system_setting(model))
    }

    async fn list(&self) -> Result<Vec<SystemSetting>, AppError> {
        let models = system_setting::Entity::find()
            .order_by_asc(system_setting::Column::Key)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_system_setting).collect())
    }

    async fn save(&self, setting: &SystemSetting) -> Result<(), AppError> {
        system_setting::Entity::insert(system_setting::ActiveModel {
            key: Set(setting.key.to_string()),
            value: Set(setting.value.clone()),
            updated_at: Set(setting.updated_at),
        })
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(system_setting::Column::Key)
                .update_columns([
                    system_setting::Column::Value,
                    system_setting::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec(&*self.db)
        .await
        .map_err(AppError::database)?;
        Ok(())
    }
}

fn map_system_setting(m: system_setting::Model) -> SystemSetting {
    SystemSetting {
        key: m.key.into(),
        value: m.value,
        updated_at: m.updated_at,
    }
}

struct WatcherRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl WatcherRepository for WatcherRepo {
    async fn add(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError> {
        let existing = issue_watcher::Entity::find()
            .filter(issue_watcher::Column::IssueId.eq(issue_id.as_uuid()))
            .filter(issue_watcher::Column::UserId.eq(user_id.as_uuid()))
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        if existing.is_some() {
            return Ok(());
        }
        let active = issue_watcher::ActiveModel {
            issue_id: Set(issue_id.as_uuid()),
            user_id: Set(user_id.as_uuid()),
        };
        active.insert(&*self.db).await.map_err(AppError::database)?;
        Ok(())
    }

    async fn remove(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError> {
        issue_watcher::Entity::delete_many()
            .filter(issue_watcher::Column::IssueId.eq(issue_id.as_uuid()))
            .filter(issue_watcher::Column::UserId.eq(user_id.as_uuid()))
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<IssueWatcher>, AppError> {
        let models = issue_watcher::Entity::find()
            .filter(issue_watcher::Column::IssueId.eq(issue_id.as_uuid()))
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models
            .into_iter()
            .map(|m| IssueWatcher {
                issue_id: IssueId::from_uuid(m.issue_id),
                user_id: UserId::from_uuid(m.user_id),
            })
            .collect())
    }

    async fn is_watching(&self, issue_id: IssueId, user_id: UserId) -> Result<bool, AppError> {
        let count = issue_watcher::Entity::find()
            .filter(issue_watcher::Column::IssueId.eq(issue_id.as_uuid()))
            .filter(issue_watcher::Column::UserId.eq(user_id.as_uuid()))
            .count(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(count > 0)
    }

    async fn list_by_user(&self, user_id: UserId) -> Result<Vec<IssueWatcher>, AppError> {
        let models = issue_watcher::Entity::find()
            .filter(issue_watcher::Column::UserId.eq(user_id.as_uuid()))
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models
            .into_iter()
            .map(|m| IssueWatcher {
                issue_id: IssueId::from_uuid(m.issue_id),
                user_id: UserId::from_uuid(m.user_id),
            })
            .collect())
    }
}

struct VoteRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl VoteRepository for VoteRepo {
    async fn add(&self, issue_id: IssueId, user_id: UserId) -> Result<IssueVote, AppError> {
        let existing = issue_vote::Entity::find()
            .filter(issue_vote::Column::IssueId.eq(issue_id.as_uuid()))
            .filter(issue_vote::Column::UserId.eq(user_id.as_uuid()))
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        if let Some(m) = existing {
            return Ok(IssueVote {
                issue_id: IssueId::from_uuid(m.issue_id),
                user_id: UserId::from_uuid(m.user_id),
                voted_at: m.voted_at,
            });
        }
        let now = chrono::Utc::now().fixed_offset();
        let active = issue_vote::ActiveModel {
            issue_id: Set(issue_id.as_uuid()),
            user_id: Set(user_id.as_uuid()),
            voted_at: Set(now),
        };
        active.insert(&*self.db).await.map_err(AppError::database)?;
        Ok(IssueVote {
            issue_id,
            user_id,
            voted_at: now,
        })
    }

    async fn remove(&self, issue_id: IssueId, user_id: UserId) -> Result<(), AppError> {
        issue_vote::Entity::delete_many()
            .filter(issue_vote::Column::IssueId.eq(issue_id.as_uuid()))
            .filter(issue_vote::Column::UserId.eq(user_id.as_uuid()))
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<IssueVote>, AppError> {
        let models = issue_vote::Entity::find()
            .filter(issue_vote::Column::IssueId.eq(issue_id.as_uuid()))
            .order_by_asc(issue_vote::Column::VotedAt)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models
            .into_iter()
            .map(|m| IssueVote {
                issue_id: IssueId::from_uuid(m.issue_id),
                user_id: UserId::from_uuid(m.user_id),
                voted_at: m.voted_at,
            })
            .collect())
    }

    async fn count_by_issue(&self, issue_id: IssueId) -> Result<u64, AppError> {
        issue_vote::Entity::find()
            .filter(issue_vote::Column::IssueId.eq(issue_id.as_uuid()))
            .count(&*self.db)
            .await
            .map_err(AppError::database)
    }

    async fn has_voted(&self, issue_id: IssueId, user_id: UserId) -> Result<bool, AppError> {
        let count = issue_vote::Entity::find()
            .filter(issue_vote::Column::IssueId.eq(issue_id.as_uuid()))
            .filter(issue_vote::Column::UserId.eq(user_id.as_uuid()))
            .count(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(count > 0)
    }
}

// ---------------------------------------------------------------------------
// SeaORM repository: project_components
// ---------------------------------------------------------------------------

struct ProjectComponentRepo {
    db: Arc<DatabaseConnection>,
}

fn map_project_component(m: project_component::Model) -> ProjectComponent {
    ProjectComponent {
        id: ProjectComponentId::from_uuid(m.id),
        project_id: ProjectId::from_uuid(m.project_id),
        name: m.name.into(),
        description: m.description.map(domain::ArcStr::from),
        created_at: m.created_at,
    }
}

#[async_trait]
impl ProjectComponentRepository for ProjectComponentRepo {
    async fn get_by_id(&self, id: ProjectComponentId) -> Result<ProjectComponent, AppError> {
        let model = project_component::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_project_component)
            .ok_or_else(|| AppError::not_found("component", id))
    }

    async fn list_by_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectComponent>, AppError> {
        let models = project_component::Entity::find()
            .filter(project_component::Column::ProjectId.eq(project_id.as_uuid()))
            .order_by_asc(project_component::Column::Name)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_project_component).collect())
    }

    async fn save(&self, component: &ProjectComponent) -> Result<ProjectComponentId, AppError> {
        let existing = project_component::Entity::find_by_id(component.id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        let active = project_component::ActiveModel {
            id: Set(component.id.as_uuid()),
            project_id: Set(component.project_id.as_uuid()),
            name: Set(component.name.as_ref().to_string()),
            description: Set(component
                .description
                .as_ref()
                .map(|d| d.as_ref().to_string())),
            created_at: Set(existing
                .as_ref()
                .map(|m| m.created_at)
                .unwrap_or_else(|| component.created_at)),
        };
        let saved = if existing.is_some() {
            active.update(&*self.db).await.map_err(AppError::database)?
        } else {
            active.insert(&*self.db).await.map_err(AppError::database)?
        };
        Ok(ProjectComponentId::from_uuid(saved.id))
    }

    async fn delete(&self, id: ProjectComponentId) -> Result<(), AppError> {
        project_component::Entity::delete_by_id(id.as_uuid())
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SeaORM repository: project_versions
// ---------------------------------------------------------------------------

struct ProjectVersionRepo {
    db: Arc<DatabaseConnection>,
}

fn map_project_version(m: project_version::Model) -> ProjectVersion {
    ProjectVersion {
        id: ProjectVersionId::from_uuid(m.id),
        project_id: ProjectId::from_uuid(m.project_id),
        name: m.name.into(),
        description: m.description.map(domain::ArcStr::from),
        released: m.released,
        release_date: m.release_date,
        created_at: m.created_at,
    }
}

#[async_trait]
impl ProjectVersionRepository for ProjectVersionRepo {
    async fn get_by_id(&self, id: ProjectVersionId) -> Result<ProjectVersion, AppError> {
        let model = project_version::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_project_version)
            .ok_or_else(|| AppError::not_found("version", id))
    }

    async fn list_by_project(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectVersion>, AppError> {
        let models = project_version::Entity::find()
            .filter(project_version::Column::ProjectId.eq(project_id.as_uuid()))
            .order_by_asc(project_version::Column::Name)
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_project_version).collect())
    }

    async fn save(&self, version: &ProjectVersion) -> Result<ProjectVersionId, AppError> {
        let existing = project_version::Entity::find_by_id(version.id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        let active = project_version::ActiveModel {
            id: Set(version.id.as_uuid()),
            project_id: Set(version.project_id.as_uuid()),
            name: Set(version.name.as_ref().to_string()),
            description: Set(version.description.as_ref().map(|d| d.as_ref().to_string())),
            released: Set(version.released),
            release_date: Set(version.release_date),
            created_at: Set(existing
                .as_ref()
                .map(|m| m.created_at)
                .unwrap_or_else(|| version.created_at)),
        };
        let saved = if existing.is_some() {
            active.update(&*self.db).await.map_err(AppError::database)?
        } else {
            active.insert(&*self.db).await.map_err(AppError::database)?
        };
        Ok(ProjectVersionId::from_uuid(saved.id))
    }

    async fn delete(&self, id: ProjectVersionId) -> Result<(), AppError> {
        project_version::Entity::delete_by_id(id.as_uuid())
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }
}
