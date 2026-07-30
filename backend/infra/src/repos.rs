use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use domain::{
    Board, BoardColumn, BoardRepository, ColumnCategory, Comment, CommentRepository, Issue,
    IssueQuery, IssueRepository, Project, ProjectMember, ProjectMemberRepository,
    ProjectRepository, ProjectRole, Sprint, SprintRepository, SprintState, User, UserRepository,
    Worklog, WorklogRepository,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use shared::{
    AppError, BoardId, CommentId, IssueId, IssueKey, IssueType, LabelId, Priority, ProjectId,
    ProjectKey, SprintId, StatusId, UserId, WorklogId,
};
use uuid::Uuid;

use crate::entities::{board, comment, issue, project, project_member, sprint, user, worklog};

pub struct SeaOrmRepositories {
    pub users: Arc<dyn UserRepository>,
    pub projects: Arc<dyn ProjectRepository>,
    pub issues: Arc<dyn IssueRepository>,
    pub boards: Arc<dyn BoardRepository>,
    pub sprints: Arc<dyn SprintRepository>,
    pub comments: Arc<dyn CommentRepository>,
    pub worklogs: Arc<dyn WorklogRepository>,
    pub members: Arc<dyn ProjectMemberRepository>,
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
        project::Entity::insert(active)
            .exec(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(project.id)
    }

    async fn next_issue_number(&self, project_id: ProjectId) -> Result<u32, AppError> {
        let count = issue::Entity::find()
            .filter(issue::Column::ProjectId.eq(project_id.as_uuid()))
            .count(&*self.db)
            .await
            .map_err(AppError::database)?;
        Ok(count as u32 + 1)
    }
}

struct IssueRepo {
    db: Arc<DatabaseConnection>,
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

    async fn save(&self, sprint: &Sprint) -> Result<SprintId, AppError> {
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
        active.insert(&*self.db).await.map_err(AppError::database)?;
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
                            "Todo" | "todo" => ColumnCategory::Todo,
                            "InProgress" | "in_progress" => ColumnCategory::InProgress,
                            "Done" | "done" => ColumnCategory::Done,
                            _ => ColumnCategory::Todo,
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
        let active = project_member::ActiveModel {
            project_id: Set(member.project_id.as_uuid()),
            user_id: Set(member.user_id.as_uuid()),
            role: Set(member.role.as_str().to_string()),
            joined_at: Set(member.joined_at),
        };
        active.insert(&*self.db).await.map_err(AppError::database)?;
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
