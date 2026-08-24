use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use domain::{
    Attachment, AttachmentRepository, Board, BoardColumn, BoardRepository, Comment,
    CommentRepository, Issue, IssueLink, IssueLinkRepository, IssueQuery, IssueRepository,
    IssueTypeEntity, IssueTypeRepository, Label, LabelRepository, LinkType, Project, ProjectMember,
    ProjectMemberRepository, ProjectRepository, ProjectRole, SavedFilter, SavedFilterRepository,
    Sprint, SprintRepository, SprintState, Status, StatusCategory, StatusRepository, User,
    UserRepository, WorkflowTransition, WorkflowTransitionId, WorkflowTransitionRepository,
    Worklog, WorklogRepository,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use shared::{
    AppError, AttachmentId, BoardId, CommentId, IssueId, IssueKey, IssueLinkId, IssueType,
    IssueTypeId, LabelId, Priority, ProjectId, ProjectKey, SavedFilterId, SprintId, StatusId,
    UserId, WorklogId,
};
use uuid::Uuid;

use crate::entities::{
    attachment, board, comment, issue, issue_label, issue_link, issue_type, label, project,
    project_member, saved_filter, sprint, status, user, workflow_transition, worklog,
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
    pub issue_links: Arc<dyn IssueLinkRepository>,
    pub saved_filters: Arc<dyn SavedFilterRepository>,
}

impl SeaOrmRepositories {
    pub fn new(db: DatabaseConnection) -> Self {
        let db = Arc::new(db);
        Self {
            users: Arc::new(UserRepo { db: db.clone() }),
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
            issue_links: Arc::new(IssueLinkRepo { db: db.clone() }),
            saved_filters: Arc::new(SavedFilterRepo { db }),
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
    async fn search_by_jql(
        &self,
        compiled: &crate::jql::CompiledJql,
        limit: u64,
        offset: u64,
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
        params.push(sea_orm::Value::Unsigned(Some(limit as u32)));
        params.push(sea_orm::Value::Unsigned(Some(offset as u32)));

        let sql = format!(
            "SELECT i.* FROM issues i JOIN projects p ON i.project_id = p.id \
             WHERE {} ORDER BY i.created_at DESC LIMIT ${} OFFSET ${}",
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
            .one(&*self.db)
            .await
            .map_err(AppError::database)?;
        model
            .map(map_issue)
            .ok_or_else(|| AppError::not_found("issue", key))
    }

    async fn list(&self, query: IssueQuery) -> Result<Vec<Issue>, AppError> {
        if let Some(jql_expr) = &query.jql {
            let user_id = query.jql_user_id.unwrap_or_default();
            let compiled = crate::jql::compile(jql_expr, user_id)
                .map_err(|e| AppError::invalid_input(e.to_string()))?;
            return self
                .search_by_jql(&compiled, query.limit, query.offset)
                .await;
        }
        let mut select = issue::Entity::find();
        if let Some(pid) = query.project_id {
            select = select.filter(issue::Column::ProjectId.eq(pid.as_uuid()));
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
            let pattern = format!("%{}%", q);
            select = select.filter(
                sea_orm::Condition::any()
                    .add(issue::Column::Summary.like(&pattern))
                    .add(issue::Column::Key.like(&pattern))
                    .add(issue::Column::Description.like(&pattern)),
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
            position: Set(issue.position),
            due_date: Set(issue.due_date),
            original_estimate_seconds: Set(issue.original_estimate_seconds),
            remaining_estimate_seconds: Set(issue.remaining_estimate_seconds),
            time_spent_seconds: Set(issue.time_spent_seconds),
            created_at: Set(issue.created_at),
            updated_at: Set(shared::now()),
        };
        if exists {
            active.update(&*self.db).await.map_err(AppError::database)?;
        } else {
            active.insert(&*self.db).await.map_err(AppError::database)?;
        }
        Ok(issue.id)
    }
    async fn delete(&self, id: IssueId) -> Result<(), AppError> {
        let res = issue::Entity::delete_by_id(id.as_uuid())
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        if res.rows_affected == 0 {
            return Err(AppError::not_found("issue", id));
        }
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
        created_at: m.created_at,
        updated_at: m.updated_at,
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
        issue_links: sea.issue_links,
        saved_filters: sea.saved_filters,
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

struct IssueLinkRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl IssueLinkRepository for IssueLinkRepo {
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

struct SavedFilterRepo {
    db: Arc<DatabaseConnection>,
}

#[async_trait]
impl SavedFilterRepository for SavedFilterRepo {
    async fn get_by_id(&self, id: SavedFilterId) -> Result<SavedFilter, AppError> {
        let model = saved_filter::Entity::find_by_id(id.as_uuid())
            .one(&*self.db)
            .await
            .map_err(AppError::database)?
            .ok_or_else(|| AppError::not_found("saved_filter", id))?;
        Ok(map_saved_filter(model))
    }

    async fn list_by_owner(&self, owner_id: UserId) -> Result<Vec<SavedFilter>, AppError> {
        let models = saved_filter::Entity::find()
            .filter(saved_filter::Column::OwnerId.eq(owner_id.as_uuid()))
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_saved_filter).collect())
    }

    async fn list_public(&self) -> Result<Vec<SavedFilter>, AppError> {
        let models = saved_filter::Entity::find()
            .filter(saved_filter::Column::IsPublic.eq(true))
            .all(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(models.into_iter().map(map_saved_filter).collect())
    }

    async fn save(&self, filter: &SavedFilter) -> Result<SavedFilterId, AppError> {
        let model = saved_filter::ActiveModel {
            id: sea_orm::ActiveValue::Set(filter.id.as_uuid()),
            name: sea_orm::ActiveValue::Set(filter.name.as_ref().to_string()),
            jql: sea_orm::ActiveValue::Set(filter.jql.clone()),
            owner_id: sea_orm::ActiveValue::Set(filter.owner_id.as_uuid()),
            is_public: sea_orm::ActiveValue::Set(filter.is_public),
            created_at: sea_orm::ActiveValue::Set(filter.created_at),
            updated_at: sea_orm::ActiveValue::Set(filter.updated_at),
        };
        let result = saved_filter::Entity::insert(model)
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(SavedFilterId::from_uuid(result.last_insert_id))
    }

    async fn delete(&self, id: SavedFilterId) -> Result<(), AppError> {
        saved_filter::Entity::delete_by_id(id.as_uuid())
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(())
    }
}

fn map_saved_filter(m: saved_filter::Model) -> SavedFilter {
    SavedFilter {
        id: SavedFilterId::from_uuid(m.id),
        name: m.name.into(),
        jql: m.jql,
        owner_id: UserId::from_uuid(m.owner_id),
        is_public: m.is_public,
        created_at: m.created_at,
        updated_at: m.updated_at,
    }
}
