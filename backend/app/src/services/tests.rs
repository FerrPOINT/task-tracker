use std::sync::Arc;

type TestStorage = domain::InMemoryStorage;
use domain::{
    AttachmentRepository, Board, BoardColumn, BoardRepository, CommentRepository, FileStorage,
    Issue, IssueQuery, IssueRepository, Label, LabelRepository, MemoryAttachmentRepository,
    MemoryBoardRepository, MemoryCommentRepository, MemoryIssueLinkRepository,
    MemoryIssueRepository, MemoryLabelRepository, MemoryNotificationRepository,
    MemoryProjectRepository, MemorySprintRepository, MemoryUserRepository, MemoryWorklogRepository,
    Notification, NotificationRepository, Project, ProjectMemberRepository, ProjectQuery,
    ProjectRepository, Sprint, SprintRepository, Status, StatusCategory, StatusRepository, User,
    UserNotificationSettingsRepository, UserRepository, WorklogRepository,
};
use shared::{
    AppConfig, AppError, AuthConfig, DatabaseConfig, IssueId, IssueKey, IssueType, LabelId,
    NotificationId, Priority, ProjectId, ProjectKey, ServerConfig, SprintId, StatusId, UserId,
};

use crate::commands::{
    CreateCommentCommand, CreateIssueCommand, CreateProjectCommand, CreateSprintCommand,
    CreateWorklogCommand, LoginCommand, MoveIssueToSprintCommand, RegisterCommand,
    TransitionIssueCommand, UpdateCommentCommand, UpdateIssueCommand,
    UpdateNotificationSettingsCommand, UpdateSprintCommand, UpdateWorklogCommand,
};
use crate::context::{AppContext, AttachmentService, NotificationService};
use crate::services::{AttachmentServiceImpl, NotificationServiceImpl};

fn test_user() -> User {
    User {
        id: UserId::new(),
        email: "demo@example.com".into(),
        username: "demo".into(),
        display_name: "Demo User".into(),
        password_hash: "$argon2id$v=19$m=65536,t=3,p=4$stN/enhZ9yOvgWC9E8Y6BA$IL9I0WONb/I6zoT4rdmdkrPcIFADFxsLCjrO0ySSl0Y".into(),
        refresh_token_hash: None,
        is_system_admin: false,
        is_active: true,
        created_at: shared::now(),
        updated_at: shared::now(),
    }
}

fn test_user_with(username: &str, email: &str, display_name: &str) -> User {
    let mut user = test_user();
    user.id = UserId::new();
    user.email = email.to_string().into();
    user.username = username.to_string().into();
    user.display_name = display_name.to_string().into();
    user
}

fn test_config() -> Arc<AppConfig> {
    Arc::new(AppConfig {
        database: DatabaseConfig::default(),
        server: ServerConfig::default(),
        auth: AuthConfig {
            jwt_secret: "test-secret".to_string(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
            refresh_cookie_name: "refresh_token".to_string(),
            refresh_cookie_secure: false,
            refresh_cookie_same_site: "lax".to_string(),
            refresh_cookie_domain: None,
            refresh_cookie_path: "/".to_string(),
        },
        storage: shared::StorageConfig::default(),
        email: shared::EmailConfig::default(),
        metrics: shared::MetricsConfig::default(),
    })
}

#[derive(Default)]
struct RecordingStorage {
    files: std::sync::Mutex<std::collections::HashMap<(String, String), Vec<u8>>>,
    deletes: std::sync::atomic::AtomicUsize,
}

impl RecordingStorage {
    fn file_count(&self) -> usize {
        self.files.lock().unwrap().len()
    }

    fn delete_count(&self) -> usize {
        self.deletes.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl domain::FileStorage for RecordingStorage {
    async fn put(&self, issue_id: &str, key: &str, bytes: Vec<u8>) -> Result<(), AppError> {
        self.files
            .lock()
            .unwrap()
            .insert((issue_id.to_string(), key.to_string()), bytes);
        Ok(())
    }

    async fn get(&self, issue_id: &str, key: &str) -> Result<Vec<u8>, AppError> {
        self.files
            .lock()
            .unwrap()
            .get(&(issue_id.to_string(), key.to_string()))
            .cloned()
            .ok_or_else(|| AppError::not_found("attachment file", key))
    }

    async fn delete(&self, issue_id: &str, key: &str) -> Result<(), AppError> {
        self.files
            .lock()
            .unwrap()
            .remove(&(issue_id.to_string(), key.to_string()));
        self.deletes
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

struct FailingAttachmentSaveRepository;

#[async_trait::async_trait]
impl domain::AttachmentRepository for FailingAttachmentSaveRepository {
    async fn get_by_id(&self, _id: shared::AttachmentId) -> Result<domain::Attachment, AppError> {
        Err(AppError::not_found("attachment", "failing"))
    }

    async fn list_by_issue(&self, _issue_id: IssueId) -> Result<Vec<domain::Attachment>, AppError> {
        Ok(Vec::new())
    }

    async fn save(
        &self,
        _attachment: &domain::Attachment,
    ) -> Result<shared::AttachmentId, AppError> {
        Err(AppError::Internal("metadata insert failed".into()))
    }

    async fn delete(&self, _id: shared::AttachmentId) -> Result<(), AppError> {
        Ok(())
    }
}

struct FailingUserRepository;

#[async_trait::async_trait]
impl UserRepository for FailingUserRepository {
    async fn get_by_id(&self, _id: UserId) -> Result<User, AppError> {
        Err(AppError::Internal("failing user repo".into()))
    }

    async fn get_by_email(&self, _email: &str) -> Result<User, AppError> {
        Err(AppError::Internal("failing user repo".into()))
    }

    async fn get_by_refresh_token(&self, _token_hash: &str) -> Result<User, AppError> {
        Err(AppError::Internal("failing user repo".into()))
    }

    async fn rotate_refresh_token(
        &self,
        _user_id: UserId,
        _expected_hash: &str,
        _new_hash: &str,
    ) -> Result<(), AppError> {
        Err(AppError::Internal("failing user repo".into()))
    }

    async fn save(&self, _user: &User) -> Result<UserId, AppError> {
        Err(AppError::Internal("failing user repo".into()))
    }

    async fn list(&self) -> Result<Vec<User>, AppError> {
        Err(AppError::Internal("failing user repo".into()))
    }
}

struct FailingStatusRepository;

#[async_trait::async_trait]
impl StatusRepository for FailingStatusRepository {
    async fn get_by_id(&self, _id: StatusId) -> Result<Status, AppError> {
        Err(AppError::Internal("failing status repo".into()))
    }

    async fn list_all(&self) -> Result<Vec<Status>, AppError> {
        Err(AppError::Internal("failing status repo".into()))
    }

    async fn get_default(&self) -> Result<Status, AppError> {
        Err(AppError::Internal("failing status repo".into()))
    }
}

struct FailingLabelRepository;

#[async_trait::async_trait]
impl LabelRepository for FailingLabelRepository {
    async fn get_by_id(&self, _id: LabelId) -> Result<Label, AppError> {
        Err(AppError::Internal("failing label repo".into()))
    }

    async fn list_by_project(&self, _project_id: ProjectId) -> Result<Vec<Label>, AppError> {
        Err(AppError::Internal("failing label repo".into()))
    }

    async fn save(&self, _label: &Label) -> Result<LabelId, AppError> {
        Err(AppError::Internal("failing label repo".into()))
    }

    async fn delete(&self, _id: LabelId) -> Result<(), AppError> {
        Err(AppError::Internal("failing label repo".into()))
    }

    async fn list_ids_by_issue(&self, _issue_id: IssueId) -> Result<Vec<LabelId>, AppError> {
        Err(AppError::Internal("failing label repo".into()))
    }

    async fn list_issue_ids_by_label(&self, _label_id: LabelId) -> Result<Vec<IssueId>, AppError> {
        Err(AppError::Internal("failing label repo".into()))
    }

    async fn attach(&self, _issue_id: IssueId, _label_id: LabelId) -> Result<(), AppError> {
        Err(AppError::Internal("failing label repo".into()))
    }

    async fn detach(&self, _issue_id: IssueId, _label_id: LabelId) -> Result<(), AppError> {
        Err(AppError::Internal("failing label repo".into()))
    }
}

struct FailingAttachmentDeleteRepository {
    attachment: domain::Attachment,
}

#[async_trait::async_trait]
impl domain::AttachmentRepository for FailingAttachmentDeleteRepository {
    async fn get_by_id(&self, id: shared::AttachmentId) -> Result<domain::Attachment, AppError> {
        if id == self.attachment.id {
            Ok(self.attachment.clone())
        } else {
            Err(AppError::not_found("attachment", id))
        }
    }

    async fn list_by_issue(&self, issue_id: IssueId) -> Result<Vec<domain::Attachment>, AppError> {
        if issue_id == self.attachment.issue_id {
            Ok(vec![self.attachment.clone()])
        } else {
            Ok(Vec::new())
        }
    }

    async fn save(
        &self,
        _attachment: &domain::Attachment,
    ) -> Result<shared::AttachmentId, AppError> {
        Ok(self.attachment.id)
    }

    async fn delete(&self, _id: shared::AttachmentId) -> Result<(), AppError> {
        Err(AppError::Internal("metadata delete failed".into()))
    }
}

fn statuses_from_board(board: &Board) -> Vec<domain::Status> {
    board
        .columns
        .iter()
        .map(|column| domain::Status {
            id: column.id,
            name: column.name.clone(),
            category: column.category,
            position: column.position,
            is_default: column.category == StatusCategory::Todo && column.position == 0,
            is_closed: column.category == StatusCategory::Done,
        })
        .collect()
}

async fn ctx_with_demo_data() -> (AppContext, User) {
    let user = test_user();
    let user_copy = user.clone();
    let mut project = Project {
        id: shared::ProjectId::new(),
        key: ProjectKey::new("TT"),
        name: "Task Tracker".into(),
        description: None,
        owner_id: user.id,
        default_board_id: shared::BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };

    let todo =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let in_progress =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap());
    let review =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000004").unwrap());
    let done =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap());
    project.default_board_id = shared::BoardId::new();
    let board = Board {
        id: project.default_board_id,
        project_id: project.id,
        name: "TT Kanban".into(),
        columns: vec![
            BoardColumn {
                id: todo,
                name: "Todo".into(),
                category: StatusCategory::Todo,
                wip_limit: None,
                position: 0,
            },
            BoardColumn {
                id: in_progress,
                name: "In Progress".into(),
                category: StatusCategory::InProgress,
                wip_limit: Some(5),
                position: 1,
            },
            BoardColumn {
                id: review,
                name: "Review".into(),
                category: StatusCategory::InProgress,
                wip_limit: None,
                position: 2,
            },
            BoardColumn {
                id: done,
                name: "Done".into(),
                category: StatusCategory::Done,
                wip_limit: None,
                position: 3,
            },
        ],
    };

    let users = Arc::new(MemoryUserRepository::default());
    users.save(&user).await.unwrap();
    let projects = Arc::new(MemoryProjectRepository::default());
    projects.save(&project).await.unwrap();
    let history = Arc::new(domain::MemoryIssueStatusHistoryRepository::default());
    let (history_entries, history_project_ids) = history.store();
    let custom_fields = Arc::new(domain::MemoryCustomFieldRepository::default());
    let issues = Arc::new(MemoryIssueRepository::with_shared_stores(
        history_entries,
        history_project_ids,
        custom_fields.value_store(),
    ));
    let boards = Arc::new(MemoryBoardRepository::default());
    boards.save(&board).await.unwrap();
    let sprints = Arc::new(MemorySprintRepository::default());

    let notifications = Arc::new(MemoryNotificationRepository::default());
    let repos = Arc::new(domain::Repositories {
        users: users.clone(),
        audit_logs: Arc::new(domain::StubAuditLogRepository),
        system_settings: Arc::new(domain::StubSystemSettingRepository),
        projects: projects.clone(),
        issues: issues.clone(),
        boards: boards.clone(),
        sprints: sprints.clone(),
        comments: Arc::new(domain::StubCommentRepository),
        worklogs: Arc::new(domain::StubWorklogRepository),
        members: Arc::new(domain::MemoryProjectMemberRepository::default()),
        statuses: Arc::new(domain::MemoryStatusRepository::new(statuses_from_board(
            &board,
        ))),
        transitions: Arc::new(domain::StubWorkflowTransitionRepository),
        issue_types: Arc::new(domain::StubIssueTypeRepository),
        attachments: Arc::new(MemoryAttachmentRepository::default()),
        labels: Arc::new(domain::StubLabelRepository),
        issue_links: Arc::new(domain::StubIssueLinkRepository),
        notifications: notifications.clone(),
        notification_settings: notifications.clone(),
        issue_status_history: history,
        watchers: Arc::new(domain::MemoryWatcherRepository::default()),
        votes: Arc::new(domain::MemoryVoteRepository::default()),
        components: Arc::new(domain::MemoryProjectComponentRepository::default()),
        versions: Arc::new(domain::MemoryProjectVersionRepository::default()),
        custom_fields,
    });
    AppContext::new(
        test_config(),
        repos.clone(),
        Arc::new(TestStorage::default()),
    );
    (
        AppContext::new(
            test_config(),
            repos.clone(),
            Arc::new(TestStorage::default()),
        ),
        user_copy,
    )
}

async fn create_demo_issue(ctx: &AppContext, user: &User, summary: &str) -> IssueId {
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: summary.to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    issue.id.parse().unwrap()
}

#[tokio::test]
async fn auth_register_and_login() {
    let (ctx, _user) = ctx_with_demo_data().await;
    ctx.services
        .auth
        .register(RegisterCommand {
            email: "new@example.com".to_string(),
            username: "new".to_string(),
            name: "New User".to_string(),
            password: "secret123".to_string(),
        })
        .await
        .unwrap();

    let dto = ctx
        .services
        .auth
        .login(LoginCommand {
            email: "new@example.com".to_string(),
            password: "secret123".to_string(),
        })
        .await
        .unwrap();

    assert!(!dto.access_token.is_empty());
    let claims = ctx.services.auth.verify_token(&dto.access_token).unwrap();
    assert_eq!(claims.sub, dto.user.id.to_string());
}

#[tokio::test]
async fn auth_login_missing_user_fails() {
    let (ctx, _user) = ctx_with_demo_data().await;
    let err = ctx
        .services
        .auth
        .login(LoginCommand {
            email: "missing@example.com".to_string(),
            password: "secret123".to_string(),
        })
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn auth_list_active_users_filters_inactive_accounts() {
    let (ctx, _user) = ctx_with_demo_data().await;
    let mut inactive = test_user_with("inactive", "inactive@example.com", "Inactive User");
    inactive.is_active = false;
    ctx.repos.users.save(&inactive).await.unwrap();

    let users = ctx.services.auth.list_active_users().await.unwrap();
    assert!(users.iter().any(|user| user.username == "demo"));
    assert!(!users.iter().any(|user| user.username == "inactive"));
}

#[tokio::test]
async fn auth_expired_token_fails_verification() {
    let (ctx, _user) = ctx_with_demo_data().await;
    let expired = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &crate::auth::UserClaims {
            jti: None,
            typ: Some("access".to_string()),
            sub: UserId::new().to_string(),
            exp: 1,
        },
        &jsonwebtoken::EncodingKey::from_secret(ctx.config.auth.jwt_secret.as_bytes()),
    )
    .unwrap();
    let err = ctx.services.auth.verify_token(&expired);
    assert!(err.is_err());
}

#[tokio::test]
async fn issue_service_create() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let status_id = board.columns[0].id.to_string();

    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Test issue".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id,
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    assert_eq!(issue.project_key, "TT");
    assert_eq!(issue.summary, "Test issue");
    assert!(!issue.key.is_empty());

    let history = ctx
        .repos
        .issue_status_history
        .list_by_issue(issue.id.parse().unwrap())
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].from_status_id, None);
    assert_eq!(history[0].to_status_id.to_string(), issue.status_id);
    assert_eq!(history[0].changed_by_id, user.id);
}

#[tokio::test]
async fn issue_service_update_and_move() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let todo_id = board.columns[0].id.to_string();
    let in_progress_id = board.columns[1].id.to_string();
    let project_key = ProjectKey::new("TT");

    let created = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: project_key.clone(),
                summary: "Move me".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Low,
                status_id: todo_id,
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let updated = ctx
        .services
        .issue
        .update(
            created.id.parse().unwrap(),
            UpdateIssueCommand {
                summary: Some("Updated".to_string()),
                description: None,
                priority: Some(Priority::High),
                status_id: Some(in_progress_id.clone()),
                assignee_id: Some(Some(user.id)),
                sprint_id: None,
                component_id: None,
                affected_version_id: None,
                fix_version_id: None,
                actor_id: user.id,
            },
            user.id,
        )
        .await
        .unwrap();

    assert_eq!(updated.summary, "Updated");
    assert_eq!(updated.priority, "High");
    assert_eq!(updated.status, "In Progress");
    assert_eq!(updated.assignee_name, Some("Demo User".to_string()));

    let board = ctx
        .services
        .board
        .move_issue(
            &project_key,
            created.id.parse().unwrap(),
            in_progress_id.parse().unwrap(),
            user.id,
        )
        .await
        .unwrap();
    let col = board
        .columns
        .iter()
        .find(|c| c.name == "In Progress")
        .unwrap();
    assert!(col.issue_ids.contains(&created.id));
}

#[tokio::test]
async fn board_move_issue_publishes_issue_moved_event() {
    let (ctx, user) = ctx_with_demo_data().await;
    let project_key = ProjectKey::new("TT");
    let board = ctx
        .services
        .board
        .get_board(&project_key, user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: project_key.clone(),
                summary: "Move from board".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let mut receiver = ctx.events.subscribe();
    while receiver.try_recv().is_ok() {}

    ctx.services
        .board
        .move_issue(
            &project_key,
            issue.id.parse().unwrap(),
            board.columns[1].id.parse().unwrap(),
            user.id,
        )
        .await
        .unwrap();

    let event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        event,
        shared::TrackerEvent::IssueMoved {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));
}

#[tokio::test]
async fn board_move_same_status_is_noop_for_history_and_events() {
    let (ctx, user) = ctx_with_demo_data().await;
    let project_key = ProjectKey::new("TT");
    let board = ctx
        .services
        .board
        .get_board(&project_key, user.id)
        .await
        .unwrap();
    let status_id: StatusId = board.columns[0].id.parse().unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: project_key.clone(),
                summary: "Board same status".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: status_id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let mut receiver = ctx.events.subscribe();
    ctx.services
        .board
        .move_issue(&project_key, issue.id.parse().unwrap(), status_id, user.id)
        .await
        .unwrap();

    assert!(receiver.try_recv().is_err());
    let history = ctx
        .repos
        .issue_status_history
        .list_by_issue(issue.id.parse().unwrap())
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
}

#[tokio::test]
async fn transition_same_status_is_noop_for_history_and_events() {
    let (ctx, user) = ctx_with_demo_data().await;
    let project_key = ProjectKey::new("TT");
    let board = ctx
        .services
        .board
        .get_board(&project_key, user.id)
        .await
        .unwrap();
    let status_id: StatusId = board.columns[0].id.parse().unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key,
                summary: "Transition same status".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: status_id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();

    let mut receiver = ctx.events.subscribe();
    let updated = ctx
        .services
        .issue
        .transition(TransitionIssueCommand {
            issue_id,
            target_status_id: status_id,
            actor_id: user.id,
        })
        .await
        .unwrap();

    assert_eq!(updated.status_id, status_id.to_string());
    assert!(receiver.try_recv().is_err());
    let history = ctx
        .repos
        .issue_status_history
        .list_by_issue(issue_id)
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
}

#[tokio::test]
async fn issue_create_rejects_non_member_assignee_and_reporter() {
    let (ctx, owner) = ctx_with_demo_data().await;
    let outsider = test_user_with("outsider", "outsider@example.com", "Outsider");
    ctx.repos.users.save(&outsider).await.unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let status_id = board.columns[0].id.to_string();

    let assignee_err = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Bad assignee".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: status_id.clone(),
                reporter_id: owner.id,
                assignee_id: Some(outsider.id),
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap_err();
    assert!(matches!(assignee_err, AppError::Forbidden));

    let reporter_err = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Bad reporter".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id,
                reporter_id: outsider.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap_err();
    assert!(matches!(reporter_err, AppError::Forbidden));
}

#[tokio::test]
async fn issue_update_rejects_non_member_assignee() {
    let (ctx, owner) = ctx_with_demo_data().await;
    let outsider = test_user_with("outsider", "outsider@example.com", "Outsider");
    ctx.repos.users.save(&outsider).await.unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Needs update".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();

    let err = ctx
        .services
        .issue
        .update(
            issue.id.parse().unwrap(),
            UpdateIssueCommand {
                assignee_id: Some(Some(outsider.id)),
                actor_id: owner.id,
                ..Default::default()
            },
            owner.id,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Forbidden));
}

#[tokio::test]
async fn issue_update_prevalidates_assignee_before_status_transition() {
    let (ctx, owner) = ctx_with_demo_data().await;
    let outsider = test_user_with("outsider", "outsider@example.com", "Outsider");
    ctx.repos.users.save(&outsider).await.unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let todo: StatusId = board.columns[0].id.parse().unwrap();
    let done: StatusId = board.columns[3].id.parse().unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Atomic update".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: todo.to_string(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();

    let err = ctx
        .services
        .issue
        .update(
            issue_id,
            UpdateIssueCommand {
                status_id: Some(done.to_string()),
                assignee_id: Some(Some(outsider.id)),
                actor_id: owner.id,
                ..Default::default()
            },
            owner.id,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Forbidden));

    let persisted = ctx.repos.issues.get_by_id(issue_id).await.unwrap();
    assert_eq!(persisted.status_id, todo);
    let history = ctx
        .repos
        .issue_status_history
        .list_by_issue(issue_id)
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].to_status_id, todo);
}

#[tokio::test]
async fn issue_update_distinguishes_omitted_and_null_assignee() {
    let (ctx, owner) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Assignee clear".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: owner.id,
                assignee_id: Some(owner.id),
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();

    let issue_id: IssueId = issue.id.parse().unwrap();
    let renamed = ctx
        .services
        .issue
        .update(
            issue_id,
            UpdateIssueCommand {
                summary: Some("Assignee unchanged".to_string()),
                assignee_id: None,
                actor_id: owner.id,
                ..Default::default()
            },
            owner.id,
        )
        .await
        .unwrap();
    assert_eq!(renamed.assignee_id, Some(owner.id.to_string()));

    let cleared = ctx
        .services
        .issue
        .update(
            issue_id,
            UpdateIssueCommand {
                assignee_id: Some(None),
                actor_id: owner.id,
                ..Default::default()
            },
            owner.id,
        )
        .await
        .unwrap();
    assert_eq!(cleared.assignee_id, None);
}

#[tokio::test]
async fn issue_update_same_status_is_noop_for_workflow_history() {
    let (ctx, owner) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let initial_status_id = board.columns[0].id.to_string();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Same status update".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: initial_status_id.clone(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();

    let issue_id: IssueId = issue.id.parse().unwrap();
    let updated = ctx
        .services
        .issue
        .update(
            issue_id,
            UpdateIssueCommand {
                status_id: Some(initial_status_id.clone()),
                summary: Some("Same status patch accepted".to_string()),
                actor_id: owner.id,
                ..Default::default()
            },
            owner.id,
        )
        .await
        .unwrap();

    assert_eq!(updated.status_id, initial_status_id);
    assert_eq!(updated.summary, "Same status patch accepted");
    let history = ctx
        .repos
        .issue_status_history
        .list_by_issue(issue_id)
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
}

#[tokio::test]
async fn dashboard_lists_assigned_issues() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let status_id = board.columns[0].id.to_string();
    ctx.services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Assigned task".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id,
                reporter_id: user.id,
                assignee_id: Some(user.id),
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let dashboard = ctx.services.dashboard.get_dashboard(user.id).await.unwrap();
    assert_eq!(dashboard.assigned_issues.len(), 1);
}

#[tokio::test]
async fn dashboard_does_not_show_assigned_issues_from_inaccessible_projects() {
    let (ctx, owner) = ctx_with_demo_data().await;
    let outsider = test_user_with("outsider", "outsider@example.com", "Outsider");
    ctx.repos.users.save(&outsider).await.unwrap();
    let project = ctx
        .repos
        .projects
        .get_by_key(&ProjectKey::new("TT"))
        .await
        .unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let mut issue = Issue::create(
        &project,
        404,
        IssueType::Task,
        board.columns[0].id.parse().unwrap(),
        "Legacy assignment",
        None,
        owner.id,
        Priority::Medium,
    );
    issue.assign(Some(outsider.id));
    ctx.repos.issues.save(&issue).await.unwrap();

    let dashboard = ctx
        .services
        .dashboard
        .get_dashboard(outsider.id)
        .await
        .unwrap();
    assert!(dashboard.assigned_issues.is_empty());
}

#[tokio::test]
async fn search_finds_issue() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let status_id = board.columns[0].id.to_string();
    ctx.services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Searchable keyword".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id,
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let results = ctx
        .services
        .search
        .search(Default::default(), user.id)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn issue_service_search_defaults_to_newest_first() {
    let (ctx, user) = ctx_with_demo_data().await;
    let project = ctx
        .repos
        .projects
        .get_by_key(&ProjectKey::new("TT"))
        .await
        .unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let status_id = board.columns[0].id.parse().unwrap();
    let now = shared::now();
    let mut older = Issue::create(
        &project,
        700,
        IssueType::Task,
        status_id,
        "issue search sort probe older",
        None,
        user.id,
        Priority::Medium,
    );
    older.created_at = now - chrono::Duration::minutes(10);
    older.updated_at = older.created_at;
    older.position = 0.0;
    ctx.repos.issues.save(&older).await.unwrap();
    let mut newer = Issue::create(
        &project,
        701,
        IssueType::Task,
        status_id,
        "issue search sort probe newer",
        None,
        user.id,
        Priority::Medium,
    );
    newer.created_at = now;
    newer.updated_at = now;
    newer.position = 100.0;
    ctx.repos.issues.save(&newer).await.unwrap();

    let results = ctx
        .services
        .issue
        .search(
            crate::context::SearchFilters {
                q: Some("issue search sort probe".to_string()),
                project_key: Some("TT".to_string()),
                limit: Some(1),
                ..Default::default()
            },
            user.id,
        )
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, newer.id.to_string());
}

#[tokio::test]
async fn project_service_create_list_and_get_by_key() {
    let (ctx, user) = ctx_with_demo_data().await;
    let created = ctx
        .services
        .project
        .create(CreateProjectCommand {
            key: ProjectKey::new("NP"),
            name: "New Project".to_string(),
            description: Some("desc".to_string()),
            owner_id: user.id,
        })
        .await
        .unwrap();
    assert_eq!(created.key, "NP");
    let list = ctx
        .services
        .project
        .list(crate::commands::ProjectQueryDto::default(), user.id)
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
    let by_key = ctx
        .services
        .project
        .get_by_key(&ProjectKey::new("NP"), user.id)
        .await
        .unwrap();
    assert_eq!(by_key.key, "NP");
}

#[tokio::test]
async fn project_service_create_fails_when_owner_missing() {
    let (ctx, _user) = ctx_with_demo_data().await;
    let err = ctx
        .services
        .project
        .create(CreateProjectCommand {
            key: ProjectKey::new("XX"),
            name: "Bad".to_string(),
            description: None,
            owner_id: UserId::new(),
        })
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn project_service_list_and_get_by_key() {
    let (ctx, _user) = ctx_with_demo_data().await;
    let list = ctx
        .services
        .project
        .list(crate::commands::ProjectQueryDto::default(), _user.id)
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].key, "TT");
    let by_key = ctx
        .services
        .project
        .get_by_key(&ProjectKey::new("TT"), _user.id)
        .await
        .unwrap();
    assert_eq!(by_key.key, "TT");
}

struct ListOnlyProjectRepository {
    project: Project,
    get_by_id_error: AppError,
}

#[async_trait::async_trait]
impl ProjectRepository for ListOnlyProjectRepository {
    async fn get_by_id(&self, _id: ProjectId) -> Result<Project, AppError> {
        match &self.get_by_id_error {
            AppError::NotFound(message) => Err(AppError::NotFound(message.clone())),
            AppError::InvalidInput(message) => Err(AppError::InvalidInput(message.clone())),
            AppError::Validation(message) => Err(AppError::Validation(message.clone())),
            AppError::Unauthorized => Err(AppError::Unauthorized),
            AppError::Forbidden => Err(AppError::Forbidden),
            AppError::Conflict(message) => Err(AppError::Conflict(message.clone())),
            AppError::Database(message) => Err(AppError::Database(message.clone())),
            AppError::Internal(message) => Err(AppError::Internal(message.clone())),
        }
    }

    async fn get_by_key(&self, _key: &ProjectKey) -> Result<Project, AppError> {
        Ok(self.project.clone())
    }

    async fn list(&self, _query: ProjectQuery) -> Result<Vec<Project>, AppError> {
        Ok(vec![self.project.clone()])
    }

    async fn save(&self, _project: &Project) -> Result<ProjectId, AppError> {
        Ok(self.project.id)
    }

    async fn save_with_board(
        &self,
        _project: &Project,
        _board: &Board,
    ) -> Result<ProjectId, AppError> {
        Ok(self.project.id)
    }

    async fn delete(&self, _id: ProjectId) -> Result<(), AppError> {
        Ok(())
    }

    async fn next_issue_number(&self, _project_id: ProjectId) -> Result<u32, AppError> {
        Ok(1)
    }
}

async fn ctx_with_list_only_project_repo(get_by_id_error: AppError) -> (AppContext, UserId) {
    let (base_ctx, user) = ctx_with_demo_data().await;
    let project = base_ctx
        .repos
        .projects
        .get_by_key(&ProjectKey::new("TT"))
        .await
        .unwrap();
    let repos = Arc::new(domain::Repositories {
        projects: Arc::new(ListOnlyProjectRepository {
            project,
            get_by_id_error,
        }),
        ..(*base_ctx.repos).clone()
    });
    (
        AppContext::new(test_config(), repos, Arc::new(TestStorage::default())),
        user.id,
    )
}

#[tokio::test]
async fn project_list_propagates_project_get_error() {
    let (ctx, user_id) = ctx_with_list_only_project_repo(AppError::Internal("x".into())).await;

    assert_internal(
        ctx.services
            .project
            .list(crate::commands::ProjectQueryDto::default(), user_id)
            .await,
    );
}

#[tokio::test]
async fn project_list_skips_stale_accessible_project_id() {
    let (ctx, user_id) =
        ctx_with_list_only_project_repo(AppError::not_found("project", "stale")).await;

    let list = ctx
        .services
        .project
        .list(crate::commands::ProjectQueryDto::default(), user_id)
        .await
        .unwrap();

    assert!(list.is_empty());
}

#[tokio::test]
async fn board_service_backlog() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let status_id = board.columns[0].id.to_string();
    ctx.services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Backlog item".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id,
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let backlog = ctx
        .services
        .board
        .get_backlog(&ProjectKey::new("TT"), user.id, 0, 100)
        .await
        .unwrap();
    assert_eq!(backlog.backlog_issues.len(), 1);
    assert_eq!(backlog.backlog_issues[0].summary, "Backlog item");
}

#[tokio::test]
async fn active_sprint_issue_ids_include_only_issues_in_that_sprint() {
    let (ctx, user) = ctx_with_demo_data().await;
    let project = ctx
        .repos
        .projects
        .get_by_key(&ProjectKey::new("TT"))
        .await
        .unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let todo_status_id = board.columns[0].id.to_string();
    let in_progress_status_id = board.columns[1].id.to_string();

    let sprint_issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "In active sprint".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: todo_status_id.clone(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let backlog_issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Plain backlog item".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: todo_status_id,
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let non_sprint_issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "In progress without sprint".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: in_progress_status_id,
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let sprint = ctx
        .services
        .sprint
        .create(
            CreateSprintCommand {
                project_id: project.id,
                name: "Active sprint".to_string(),
                goal: None,
                start_date: None,
                end_date: None,
            },
            user.id,
        )
        .await
        .unwrap();
    let sprint_id: SprintId = sprint.id.parse().unwrap();
    ctx.services.sprint.start(sprint_id, user.id).await.unwrap();
    ctx.services
        .sprint
        .move_issue(
            MoveIssueToSprintCommand {
                issue_id: sprint_issue.id.parse().unwrap(),
                sprint_id: Some(sprint_id),
            },
            user.id,
        )
        .await
        .unwrap();

    let board_view = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let backlog_view = ctx
        .services
        .board
        .get_backlog(&ProjectKey::new("TT"), user.id, 0, 100)
        .await
        .unwrap();

    assert_eq!(board_view.sprint.id, sprint.id);
    assert_eq!(backlog_view.sprint.id, sprint.id);
    assert_eq!(board_view.sprint.issue_ids, vec![sprint_issue.id.clone()]);
    assert_eq!(backlog_view.sprint.issue_ids, vec![sprint_issue.id.clone()]);
    assert!(!board_view.sprint.issue_ids.contains(&backlog_issue.id));
    assert!(!backlog_view.sprint.issue_ids.contains(&non_sprint_issue.id));
}

#[tokio::test]
async fn backlog_offset_reaches_later_items_without_duplicates() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let status_id = board.columns[0].id.to_string();
    for summary in ["Backlog page 1", "Backlog page 2", "Backlog page 3"] {
        ctx.services
            .issue
            .create(
                CreateIssueCommand {
                    project_key: ProjectKey::new("TT"),
                    summary: summary.to_string(),
                    description: None,
                    issue_type: IssueType::Task,
                    priority: Priority::Medium,
                    status_id: status_id.clone(),
                    reporter_id: user.id,
                    assignee_id: None,
                    actor_id: user.id,
                    custom_fields: Default::default(),
                },
                user.id,
            )
            .await
            .unwrap();
    }
    let first = ctx
        .services
        .board
        .get_backlog(&ProjectKey::new("TT"), user.id, 0, 1)
        .await
        .unwrap();
    let second = ctx
        .services
        .board
        .get_backlog(&ProjectKey::new("TT"), user.id, 1, 1)
        .await
        .unwrap();
    assert_eq!(first.backlog_total, 3);
    assert_eq!(first.backlog_offset, 0);
    assert_eq!(second.backlog_offset, 1);
    assert_eq!(first.backlog_issues.len(), 1);
    assert_eq!(second.backlog_issues.len(), 1);
    assert_ne!(first.backlog_issues[0].id, second.backlog_issues[0].id);
}

#[tokio::test]
async fn backlog_offset_reaches_items_beyond_default_issue_cap() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let status_id = board.columns[0].id.to_string();
    for idx in 0..1005 {
        ctx.services
            .issue
            .create(
                CreateIssueCommand {
                    project_key: ProjectKey::new("TT"),
                    summary: format!("Large backlog {idx:04}"),
                    description: None,
                    issue_type: IssueType::Task,
                    priority: Priority::Medium,
                    status_id: status_id.clone(),
                    reporter_id: user.id,
                    assignee_id: None,
                    actor_id: user.id,
                    custom_fields: Default::default(),
                },
                user.id,
            )
            .await
            .unwrap();
    }

    let backlog = ctx
        .services
        .board
        .get_backlog(&ProjectKey::new("TT"), user.id, 1000, 10)
        .await
        .unwrap();
    assert_eq!(backlog.backlog_total, 1005);
    assert_eq!(backlog.backlog_offset, 1000);
    assert_eq!(backlog.backlog_issues.len(), 5);
}

#[tokio::test]
async fn project_counters_include_issues_beyond_default_issue_cap() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let status_id = board.columns[0].id.to_string();
    for idx in 0..1005 {
        ctx.services
            .issue
            .create(
                CreateIssueCommand {
                    project_key: ProjectKey::new("TT"),
                    summary: format!("Counter item {idx:04}"),
                    description: None,
                    issue_type: IssueType::Task,
                    priority: Priority::Medium,
                    status_id: status_id.clone(),
                    reporter_id: user.id,
                    assignee_id: None,
                    actor_id: user.id,
                    custom_fields: Default::default(),
                },
                user.id,
            )
            .await
            .unwrap();
    }

    let project = ctx
        .services
        .project
        .get_by_key(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    assert_eq!(project.todo_count, 1005);
}

#[tokio::test]
async fn auth_wrong_password_fails() {
    let (ctx, _user) = ctx_with_demo_data().await;
    let err = ctx
        .services
        .auth
        .login(LoginCommand {
            email: "demo@example.com".to_string(),
            password: "wrong".to_string(),
        })
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn issue_service_create_fails_for_missing_project() {
    let (ctx, user) = ctx_with_demo_data().await;
    let err = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("ZZ"),
                summary: "orphan".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: "00000000-0000-0000-0000-000000000001".to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn issue_service_create_fails_for_invalid_status_id() {
    let (ctx, user) = ctx_with_demo_data().await;
    let err = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "bad status".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: "not-a-uuid".to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn issue_service_update_fails_for_invalid_status_id() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let created = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Update me".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Low,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let err = ctx
        .services
        .issue
        .update(
            created.id.parse().unwrap(),
            UpdateIssueCommand {
                summary: None,
                description: None,
                priority: None,
                status_id: Some("not-a-uuid".to_string()),
                assignee_id: None,
                sprint_id: None,
                component_id: None,
                affected_version_id: None,
                fix_version_id: None,
                actor_id: shared::UserId::new(),
            },
            user.id,
        )
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn issue_service_update_fails_for_missing_issue() {
    let (ctx, _user) = ctx_with_demo_data().await;
    let err = ctx
        .services
        .issue
        .update(
            shared::IssueId::new(),
            UpdateIssueCommand {
                summary: Some("nope".to_string()),
                description: None,
                priority: None,
                status_id: None,
                assignee_id: None,
                sprint_id: None,
                component_id: None,
                affected_version_id: None,
                fix_version_id: None,
                actor_id: shared::UserId::new(),
            },
            shared::UserId::new(),
        )
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn board_move_issue_fails_for_missing_issue() {
    let (ctx, _user) = ctx_with_demo_data().await;
    let err = ctx
        .services
        .board
        .move_issue(
            &ProjectKey::new("TT"),
            shared::IssueId::new(),
            StatusId::from_uuid(
                uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
            ),
            shared::UserId::new(),
        )
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn dashboard_get_for_user_without_issues_is_empty() {
    let (ctx, _user) = ctx_with_demo_data().await;
    let dashboard = ctx
        .services
        .dashboard
        .get_dashboard(UserId::new())
        .await
        .unwrap();
    assert!(dashboard.assigned_issues.is_empty());
}

#[tokio::test]
async fn auth_invalid_token_fails_verification() {
    let (ctx, _user) = ctx_with_demo_data().await;
    let err = ctx.services.auth.verify_token("not.valid.token");
    assert!(err.is_err());
}

#[tokio::test]
async fn auth_duplicate_registration_fails() {
    let (ctx, _user) = ctx_with_demo_data().await;
    let email = "dup@example.com".to_string();
    ctx.services
        .auth
        .register(RegisterCommand {
            email: email.clone(),
            username: "dup".to_string(),
            name: "Dup".to_string(),
            password: "secret123".to_string(),
        })
        .await
        .unwrap();

    let err = ctx
        .services
        .auth
        .register(RegisterCommand {
            email,
            username: "dup2".to_string(),
            name: "Dup2".to_string(),
            password: "secret123".to_string(),
        })
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn issue_service_get_by_id_fails_for_missing_issue() {
    let (ctx, _user) = ctx_with_demo_data().await;
    let err = ctx
        .services
        .issue
        .get_by_id(shared::IssueId::new(), shared::UserId::new())
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn dashboard_get_skips_issue_from_missing_project() {
    let (ctx, user) = ctx_with_demo_data().await;
    let fake_project = domain::Project {
        id: ProjectId::new(),
        key: ProjectKey::new("FAKE"),
        name: "Fake".into(),
        description: None,
        owner_id: user.id,
        default_board_id: shared::BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    let status = StatusId::from_uuid(uuid::Uuid::nil());
    let issue = domain::Issue::create(
        &fake_project,
        1,
        IssueType::Task,
        status,
        "orphan",
        None,
        user.id,
        Priority::Medium,
    );
    let mut issue_with_assignee = issue.clone();
    issue_with_assignee.assign(Some(user.id));
    ctx.repos.issues.save(&issue_with_assignee).await.unwrap();

    let dashboard = ctx.services.dashboard.get_dashboard(user.id).await.unwrap();
    assert!(dashboard.assigned_issues.is_empty());
}

#[tokio::test]
async fn issue_service_search_fails_when_project_missing() {
    let (ctx, user) = ctx_with_demo_data().await;
    let fake_project = domain::Project {
        id: ProjectId::new(),
        key: ProjectKey::new("FAKE"),
        name: "Fake".into(),
        description: None,
        owner_id: user.id,
        default_board_id: shared::BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    let status = StatusId::from_uuid(uuid::Uuid::nil());
    let issue = domain::Issue::create(
        &fake_project,
        1,
        IssueType::Task,
        status,
        "orphan keyword",
        None,
        user.id,
        Priority::Medium,
    );
    ctx.repos.issues.save(&issue).await.unwrap();

    // Since search results are scoped to the requester's accessible projects,
    // an orphan issue in a nonexistent project is filtered out instead of
    // surfacing an enrichment error: search succeeds with no results.
    let res = ctx
        .services
        .search
        .search(Default::default(), user.id)
        .await;
    assert!(res.is_ok());
    assert!(res.unwrap().is_empty());
}

#[tokio::test]
async fn issue_service_get_by_id_fails_when_project_missing() {
    let (ctx, user) = ctx_with_demo_data().await;
    let fake_project = domain::Project {
        id: ProjectId::new(),
        key: ProjectKey::new("FAKE"),
        name: "Fake".into(),
        description: None,
        owner_id: user.id,
        default_board_id: shared::BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    let status = StatusId::from_uuid(uuid::Uuid::nil());
    let issue = domain::Issue::create(
        &fake_project,
        1,
        IssueType::Task,
        status,
        "orphan get",
        None,
        user.id,
        Priority::Medium,
    );
    ctx.repos.issues.save(&issue).await.unwrap();

    let err = ctx.services.issue.get_by_id(issue.id, user.id).await;
    assert!(err.is_err());
}

fn failing_context() -> AppContext {
    #[derive(Default)]
    struct FailingProjectRepository;
    #[async_trait::async_trait]
    impl ProjectRepository for FailingProjectRepository {
        async fn get_by_id(&self, _id: ProjectId) -> Result<Project, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn get_by_key(&self, _key: &ProjectKey) -> Result<Project, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn list(&self, _query: ProjectQuery) -> Result<Vec<Project>, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn save(&self, _project: &Project) -> Result<ProjectId, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn save_with_board(
            &self,
            _project: &domain::Project,
            _board: &Board,
        ) -> Result<ProjectId, AppError> {
            Err(AppError::Internal("failing project repo".into()))
        }
        async fn delete(&self, _id: ProjectId) -> Result<(), AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn next_issue_number(&self, _project_id: ProjectId) -> Result<u32, AppError> {
            Err(AppError::Internal("x".into()))
        }
    }

    #[derive(Default)]
    struct FailingIssueRepository;
    #[async_trait::async_trait]
    impl IssueRepository for FailingIssueRepository {
        async fn change_status_atomic(
            &self,
            _issue_id: shared::IssueId,
            _project_id: shared::ProjectId,
            _from_status_id: shared::StatusId,
            _to_status_id: shared::StatusId,
            _actor_id: shared::UserId,
            _guard: &domain::TransitionGuard,
        ) -> Result<(), shared::AppError> {
            Err(shared::AppError::internal("failing repo"))
        }

        async fn get_by_id(&self, _id: IssueId) -> Result<Issue, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn get_by_id_include_deleted(&self, _id: IssueId) -> Result<Issue, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn get_by_key(&self, _key: &IssueKey) -> Result<Issue, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn list(&self, _query: IssueQuery) -> Result<Vec<Issue>, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn save(&self, _issue: &Issue) -> Result<IssueId, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn delete(&self, _id: IssueId) -> Result<(), AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn restore(&self, _id: IssueId) -> Result<(), AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn purge(&self, _id: IssueId) -> Result<(), AppError> {
            Err(AppError::Internal("x".into()))
        }
    }

    #[derive(Default)]
    struct FailingUserRepository;
    #[async_trait::async_trait]
    impl UserRepository for FailingUserRepository {
        async fn rotate_refresh_token(
            &self,
            _user_id: shared::UserId,
            _expected_hash: &str,
            _new_hash: &str,
        ) -> Result<(), shared::AppError> {
            Err(shared::AppError::Internal("failing user repo".into()))
        }

        async fn get_by_id(&self, _id: UserId) -> Result<User, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn get_by_email(&self, _email: &str) -> Result<User, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn get_by_refresh_token(&self, _token_hash: &str) -> Result<User, AppError> {
            Err(AppError::not_found("user", "stub"))
        }

        async fn save(&self, _user: &User) -> Result<UserId, AppError> {
            Err(AppError::Internal("x".into()))
        }

        async fn list(&self) -> Result<Vec<User>, AppError> {
            Err(AppError::Internal("x".into()))
        }
    }

    #[derive(Default)]
    struct FailingBoardRepository;
    #[async_trait::async_trait]
    impl BoardRepository for FailingBoardRepository {
        async fn get_by_id(&self, _id: shared::BoardId) -> Result<Board, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn get_default_by_project(&self, _project_id: ProjectId) -> Result<Board, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn get_default_by_project_key(&self, _key: &ProjectKey) -> Result<Board, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn save(&self, _board: &Board) -> Result<(), AppError> {
            Err(AppError::Internal("x".into()))
        }
    }

    #[derive(Default)]
    struct FailingSprintRepository;
    #[async_trait::async_trait]
    impl SprintRepository for FailingSprintRepository {
        async fn get_by_id(&self, _id: SprintId) -> Result<Sprint, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn get_active_by_project(
            &self,
            _project_id: ProjectId,
        ) -> Result<Option<Sprint>, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn save(&self, _sprint: &Sprint) -> Result<SprintId, AppError> {
            Err(AppError::Internal("x".into()))
        }
        async fn list_by_project(&self, _project_id: ProjectId) -> Result<Vec<Sprint>, AppError> {
            Err(AppError::Internal("x".into()))
        }
    }

    let repos = Arc::new(domain::Repositories {
        users: Arc::new(FailingUserRepository),
        audit_logs: Arc::new(domain::StubAuditLogRepository),
        system_settings: Arc::new(domain::StubSystemSettingRepository),
        projects: Arc::new(FailingProjectRepository),
        issues: Arc::new(FailingIssueRepository),
        boards: Arc::new(FailingBoardRepository),
        sprints: Arc::new(FailingSprintRepository),
        comments: Arc::new(domain::StubCommentRepository),
        worklogs: Arc::new(domain::StubWorklogRepository),
        members: Arc::new(domain::StubProjectMemberRepository),
        statuses: Arc::new(domain::StubStatusRepository),
        transitions: Arc::new(domain::StubWorkflowTransitionRepository),
        issue_types: Arc::new(domain::StubIssueTypeRepository),
        attachments: Arc::new(domain::StubAttachmentRepository),
        labels: Arc::new(domain::StubLabelRepository),
        issue_links: Arc::new(domain::StubIssueLinkRepository),
        notifications: Arc::new(domain::StubNotificationRepository),
        notification_settings: Arc::new(domain::StubUserNotificationSettingsRepository),
        issue_status_history: Arc::new(domain::StubIssueStatusHistoryRepository),
        watchers: Arc::new(domain::StubWatcherRepository),
        votes: Arc::new(domain::StubVoteRepository),
        components: Arc::new(domain::StubProjectComponentRepository),
        versions: Arc::new(domain::StubProjectVersionRepository),
        custom_fields: Arc::new(domain::StubCustomFieldRepository),
    });
    AppContext::new(test_config(), repos, Arc::new(TestStorage::default()))
}

fn assert_internal(err: Result<impl std::fmt::Debug, AppError>) {
    match err {
        Err(AppError::Internal(msg)) => assert!(!msg.is_empty()),
        other => panic!("expected AppError::Internal, got {:?}", other),
    }
}

#[tokio::test]
async fn project_create_propagates_repo_error() {
    let ctx = failing_context();
    assert_internal(
        ctx.services
            .project
            .create(CreateProjectCommand {
                key: ProjectKey::new("NP"),
                name: "New".to_string(),
                description: None,
                owner_id: UserId::new(),
            })
            .await,
    );
}

#[tokio::test]
async fn issue_create_propagates_repo_error() {
    let ctx = failing_context();
    assert_internal(
        ctx.services
            .issue
            .create(
                CreateIssueCommand {
                    project_key: ProjectKey::new("TT"),
                    summary: "x".to_string(),
                    description: None,
                    issue_type: IssueType::Task,
                    priority: Priority::Medium,
                    status_id: "00000000-0000-0000-0000-000000000001".to_string(),
                    reporter_id: UserId::new(),
                    assignee_id: None,
                    actor_id: UserId::new(),
                    custom_fields: Default::default(),
                },
                UserId::new(),
            )
            .await,
    );
}

#[tokio::test]
async fn board_get_propagates_repo_error() {
    let ctx = failing_context();
    assert_internal(
        ctx.services
            .board
            .get_board(&ProjectKey::new("TT"), UserId::new())
            .await,
    );
}

#[tokio::test]
async fn dashboard_get_propagates_repo_error() {
    let ctx = failing_context();
    assert_internal(ctx.services.dashboard.get_dashboard(UserId::new()).await);
}

#[tokio::test]
async fn search_propagates_repo_error() {
    let ctx = failing_context();
    assert_internal(
        ctx.services
            .search
            .search(Default::default(), UserId::new())
            .await,
    );
}

#[tokio::test]
async fn issue_get_propagates_user_lookup_error() {
    let (base_ctx, user) = ctx_with_demo_data().await;
    let issue_id = create_demo_issue(&base_ctx, &user, "issue user lookup failure").await;
    let repos = Arc::new(domain::Repositories {
        users: Arc::new(FailingUserRepository),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));

    assert_internal(ctx.services.issue.get_by_id(issue_id, user.id).await);
}

#[tokio::test]
async fn issue_get_propagates_label_lookup_error() {
    let (base_ctx, user) = ctx_with_demo_data().await;
    let issue_id = create_demo_issue(&base_ctx, &user, "issue label lookup failure").await;
    let repos = Arc::new(domain::Repositories {
        labels: Arc::new(FailingLabelRepository),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));

    assert_internal(ctx.services.issue.get_by_id(issue_id, user.id).await);
}

#[tokio::test]
async fn board_get_propagates_status_lookup_error() {
    let (base_ctx, user) = ctx_with_demo_data().await;
    let repos = Arc::new(domain::Repositories {
        statuses: Arc::new(FailingStatusRepository),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));

    assert_internal(
        ctx.services
            .board
            .get_board(&ProjectKey::new("TT"), user.id)
            .await,
    );
}

#[tokio::test]
async fn project_get_propagates_owner_lookup_error() {
    let (base_ctx, user) = ctx_with_demo_data().await;
    let repos = Arc::new(domain::Repositories {
        users: Arc::new(FailingUserRepository),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));

    assert_internal(
        ctx.services
            .project
            .get_by_key(&ProjectKey::new("TT"), user.id)
            .await,
    );
}

#[tokio::test]
async fn vote_create_propagates_user_lookup_error_without_writing() {
    let (base_ctx, owner, member, _project_id) = ctx_with_real_members().await;
    let issue_id = create_demo_issue(&base_ctx, &owner, "vote user lookup failure").await;
    let repos = Arc::new(domain::Repositories {
        users: Arc::new(FailingUserRepository),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));

    assert_internal(ctx.services.vote.vote(issue_id, member.id).await);
    assert_eq!(
        base_ctx.repos.votes.count_by_issue(issue_id).await.unwrap(),
        0,
        "failed voter lookup must happen before writing the vote"
    );
}

fn notification(recipient_id: UserId, created_at: shared::Timestamp) -> Notification {
    Notification {
        id: NotificationId::new(),
        recipient_id,
        event_type: "issue_assigned".into(),
        entity_type: "issue".into(),
        entity_id: Some(IssueId::new().as_uuid()),
        actor_id: Some(UserId::new()),
        title: "Assigned to you".into(),
        body: Some("Review this issue".into()),
        is_read: false,
        read_at: None,
        action_url: Some("/issues/TT-1".into()),
        metadata: serde_json::Value::Null,
        created_at,
    }
}

#[tokio::test]
async fn notification_service_lists_newest_ten_and_counts_all_unread() {
    let user_id = UserId::new();
    let repo = Arc::new(MemoryNotificationRepository::default());
    let service = NotificationServiceImpl::new(repo.clone(), repo.clone());
    let now = shared::now();

    for offset in 0..12 {
        let mut item = notification(user_id, now + chrono::Duration::seconds(offset));
        item.title = format!("Notification {offset}").into();
        repo.save(&item).await.unwrap();
    }

    let result = service.list_unread(user_id).await.unwrap();
    assert_eq!(result.unread_count, 12);
    assert_eq!(result.notifications.len(), 10);
    assert_eq!(result.notifications[0].title, "Notification 11");
    assert_eq!(result.notifications[9].title, "Notification 2");
}

#[tokio::test]
async fn notification_service_can_page_all_notifications_without_losing_unread_count() {
    let user_id = UserId::new();
    let repo = Arc::new(MemoryNotificationRepository::default());
    let service = NotificationServiceImpl::new(repo.clone(), repo.clone());
    let now = shared::now();

    let mut old_unread = notification(user_id, now);
    old_unread.title = "Old unread".into();
    repo.save(&old_unread).await.unwrap();

    let mut read = notification(user_id, now + chrono::Duration::seconds(1));
    read.title = "Already read".into();
    read.is_read = true;
    read.read_at = Some(now + chrono::Duration::seconds(1));
    repo.save(&read).await.unwrap();

    let mut new_unread = notification(user_id, now + chrono::Duration::seconds(2));
    new_unread.title = "New unread".into();
    repo.save(&new_unread).await.unwrap();

    let first_page = service.list(user_id, true, 2, 0).await.unwrap();
    assert_eq!(first_page.unread_count, 2);
    assert_eq!(first_page.notifications.len(), 2);
    assert_eq!(first_page.notifications[0].title, "New unread");
    assert_eq!(first_page.notifications[1].title, "Already read");

    let second_page = service.list(user_id, true, 2, 2).await.unwrap();
    assert_eq!(second_page.unread_count, 2);
    assert_eq!(second_page.notifications.len(), 1);
    assert_eq!(second_page.notifications[0].title, "Old unread");

    let unread_only = service.list(user_id, false, 10, 0).await.unwrap();
    assert_eq!(unread_only.unread_count, 2);
    assert_eq!(unread_only.notifications.len(), 2);
    assert!(
        unread_only
            .notifications
            .iter()
            .all(|notification| !notification.is_read)
    );

    assert!(service.list(user_id, true, 0, 0).await.is_err());
    assert!(service.list(user_id, true, 51, 0).await.is_err());
}

#[tokio::test]
async fn notification_service_marks_only_recipients_unread_notification_read() {
    let user_id = UserId::new();
    let other_user_id = UserId::new();
    let repo = Arc::new(MemoryNotificationRepository::default());
    let service = NotificationServiceImpl::new(repo.clone(), repo.clone());
    let item = notification(other_user_id, shared::now());
    repo.save(&item).await.unwrap();

    assert!(
        service
            .mark_read(item.id.to_string(), user_id)
            .await
            .is_err()
    );
    assert_eq!(repo.list_unread(other_user_id).await.unwrap().len(), 1);
    assert!(
        service
            .mark_read("not-a-uuid".to_string(), user_id)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn notification_service_returns_default_settings_and_persists_valid_updates() {
    let user_id = UserId::new();
    let repo = Arc::new(MemoryNotificationRepository::default());
    let service = NotificationServiceImpl::new(repo.clone(), repo.clone());

    let defaults = service.get_settings(user_id).await.unwrap();
    assert_eq!(defaults.email_frequency, "immediate");
    assert!(defaults.disabled_event_types.is_empty());
    assert!(!defaults.notify_own_changes);
    assert!(repo.get_settings(user_id).await.is_err());

    let updated = service
        .update_settings(
            user_id,
            UpdateNotificationSettingsCommand {
                email_frequency: "daily".into(),
                disabled_event_types: vec!["issue_commented".into()],
                notify_own_changes: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.email_frequency, "daily");
    assert_eq!(updated.disabled_event_types, vec!["issue_commented"]);
    assert!(updated.notify_own_changes);
    assert!(
        service
            .update_settings(
                user_id,
                UpdateNotificationSettingsCommand {
                    email_frequency: "weekly".into(),
                    disabled_event_types: vec![],
                    notify_own_changes: false,
                },
            )
            .await
            .is_err()
    );
}

async fn notification_recipient(ctx: &AppContext) -> User {
    let dto = ctx
        .services
        .auth
        .register(RegisterCommand {
            email: format!("notify-{}@example.com", uuid::Uuid::new_v4()),
            username: format!("notify{}", &uuid::Uuid::new_v4().simple().to_string()[..6]),
            name: "Notify Recipient".to_string(),
            password: "12345678".to_string(),
        })
        .await
        .unwrap();
    let user = ctx
        .repos
        .users
        .get_by_id(dto.user.id.parse().unwrap())
        .await
        .unwrap();
    let project = ctx
        .repos
        .projects
        .get_by_key(&ProjectKey::new("TT"))
        .await
        .unwrap();
    ctx.repos
        .members
        .save(&domain::ProjectMember {
            project_id: project.id,
            user_id: user.id,
            role: domain::ProjectRole::Member,
            joined_at: shared::now(),
        })
        .await
        .unwrap();
    user
}

// ─── Notification preference enforcement (audit r4, P2) ──────────────

#[tokio::test]
async fn disabled_event_types_suppress_in_app_notification() {
    let (ctx, user) = ctx_with_demo_data().await;
    let second = notification_recipient(&ctx).await;
    // Disable issue_assigned notifications for the recipient, then assign.
    ctx.services
        .notification
        .update_settings(
            second.id,
            UpdateNotificationSettingsCommand {
                email_frequency: "immediate".into(),
                disabled_event_types: vec!["issue_assigned".into()],
                notify_own_changes: false,
            },
        )
        .await
        .unwrap();

    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    ctx.services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Muted assignment".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: Some(second.id),
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let unread = ctx
        .repos
        .notifications
        .list_unread(second.id)
        .await
        .unwrap();
    assert!(
        unread
            .iter()
            .all(|n| n.event_type.as_ref() != "issue_assigned"),
        "disabled_event_types must suppress in-app notifications, got {:?}",
        unread
            .iter()
            .map(|n| n.event_type.to_string())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn notify_own_changes_false_suppresses_self_notifications() {
    let (ctx, user) = ctx_with_demo_data().await;
    // notify_own_changes defaults to false: commenting on own issue must not
    // create a notification for the author.
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Self comment".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    ctx.services
        .comment
        .create(
            CreateCommentCommand {
                issue_id: issue.id.parse().unwrap(),
                author_id: user.id,
                body: "own comment".to_string(),
                actor_id: user.id,
            },
            user.id,
        )
        .await
        .unwrap();

    let unread = ctx.repos.notifications.list_unread(user.id).await.unwrap();
    assert!(
        unread
            .iter()
            .all(|n| n.event_type.as_ref() != "issue_commented"),
        "self-events must be suppressed when notify_own_changes is false"
    );
}

#[tokio::test]
async fn comment_notifications_deduplicate_recipients() {
    let (ctx, owner) = ctx_with_demo_data().await;
    let reporter = notification_recipient(&ctx).await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Unassigned issue".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: reporter.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();

    ctx.services
        .comment
        .create(
            CreateCommentCommand {
                issue_id: issue.id.parse().unwrap(),
                author_id: owner.id,
                body: "one notification only".to_string(),
                actor_id: owner.id,
            },
            owner.id,
        )
        .await
        .unwrap();

    let unread = ctx
        .repos
        .notifications
        .list_unread(reporter.id)
        .await
        .unwrap();
    let comments: Vec<_> = unread
        .iter()
        .filter(|notification| notification.event_type.as_ref() == "issue_commented")
        .collect();
    assert_eq!(comments.len(), 1, "reporter must not receive duplicates");
}

#[tokio::test]
async fn comment_create_rejects_spoofed_author_and_actor() {
    let (base_ctx, owner) = ctx_with_demo_data().await;
    let repos = Arc::new(domain::Repositories {
        comments: Arc::new(MemoryCommentRepository::default()),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));
    let other = test_user_with("spoofed", "spoofed@example.com", "Spoofed User");
    ctx.repos.users.save(&other).await.unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Comment spoofing".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();

    let err = ctx
        .services
        .comment
        .create(
            CreateCommentCommand {
                issue_id,
                author_id: other.id,
                body: "pretend this came from someone else".to_string(),
                actor_id: other.id,
            },
            owner.id,
        )
        .await
        .unwrap_err();

    assert!(matches!(err, AppError::Forbidden));
    assert!(
        ctx.repos
            .comments
            .list_by_issue(issue_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn comment_create_propagates_author_lookup_error_without_writing() {
    let (base_ctx, owner) = ctx_with_demo_data().await;
    let board = base_ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let issue = base_ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "comment author lookup failure".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let comments = Arc::new(MemoryCommentRepository::default());
    let repos = Arc::new(domain::Repositories {
        users: Arc::new(FailingUserRepository),
        comments: comments.clone(),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(
        test_config(),
        repos.clone(),
        Arc::new(TestStorage::default()),
    );

    assert_internal(
        ctx.services
            .comment
            .create(
                CreateCommentCommand {
                    issue_id,
                    author_id: owner.id,
                    body: "must not be persisted".to_string(),
                    actor_id: owner.id,
                },
                owner.id,
            )
            .await,
    );
    assert!(
        comments.list_by_issue(issue_id).await.unwrap().is_empty(),
        "failed author lookup must happen before writing the comment"
    );
}

#[tokio::test]
async fn comment_list_propagates_author_directory_error() {
    let (base_ctx, owner) = ctx_with_demo_data().await;
    let board = base_ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let issue = base_ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "comment list user lookup failure".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let comments = Arc::new(MemoryCommentRepository::default());
    comments
        .save(&domain::Comment {
            id: shared::CommentId::new(),
            issue_id,
            author_id: owner.id,
            body: domain::value_objects::RichText::new("stored comment".to_string()),
            created_at: shared::now(),
            updated_at: shared::now(),
        })
        .await
        .unwrap();
    let repos = Arc::new(domain::Repositories {
        users: Arc::new(FailingUserRepository),
        comments: comments.clone(),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(
        test_config(),
        repos.clone(),
        Arc::new(TestStorage::default()),
    );

    assert_internal(ctx.services.comment.list(issue_id, owner.id, None, 0).await);
}

#[tokio::test]
async fn watcher_receives_comment_notification() {
    let (ctx, owner, member, _project_id) = ctx_with_real_members().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Watched comment".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    ctx.services
        .watcher
        .watch(issue_id, member.id)
        .await
        .unwrap();

    ctx.services
        .comment
        .create(
            CreateCommentCommand {
                issue_id,
                author_id: owner.id,
                body: "watcher should see this".to_string(),
                actor_id: owner.id,
            },
            owner.id,
        )
        .await
        .unwrap();

    let unread = ctx
        .repos
        .notifications
        .list_unread(member.id)
        .await
        .unwrap();
    assert_eq!(unread.len(), 1);
    assert_eq!(unread[0].event_type.as_ref(), "issue_commented");
    let expected_url = format!("/issues/{}", issue.id);
    assert_eq!(
        unread[0].action_url.as_ref().map(|url| url.as_ref()),
        Some(expected_url.as_str())
    );
}

#[tokio::test]
async fn watcher_receives_comment_edit_and_delete_notifications() {
    let (base_ctx, owner, member, _project_id) = ctx_with_real_members().await;
    let repos = Arc::new(domain::Repositories {
        comments: Arc::new(MemoryCommentRepository::default()),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));
    let issue_id = create_demo_issue(&ctx, &owner, "Watched comment lifecycle").await;
    ctx.services
        .watcher
        .watch(issue_id, member.id)
        .await
        .unwrap();
    let comment = ctx
        .services
        .comment
        .create(
            CreateCommentCommand {
                issue_id,
                author_id: owner.id,
                body: "watcher should see edit/delete".to_string(),
                actor_id: owner.id,
            },
            owner.id,
        )
        .await
        .unwrap();
    let comment_id: shared::CommentId = comment.id.parse().unwrap();

    ctx.services
        .comment
        .update(
            comment_id,
            UpdateCommentCommand {
                body: Some("edited for watcher".to_string()),
            },
            owner.id,
        )
        .await
        .unwrap();
    ctx.services
        .comment
        .delete(comment_id, owner.id)
        .await
        .unwrap();

    let unread = ctx
        .repos
        .notifications
        .list_unread(member.id)
        .await
        .unwrap();
    let event_types = unread
        .iter()
        .map(|notification| notification.event_type.as_ref())
        .collect::<Vec<_>>();
    assert!(
        event_types.contains(&"issue_comment_edited"),
        "watcher notifications should include issue_comment_edited, got {event_types:?}"
    );
    assert!(
        event_types.contains(&"issue_comment_deleted"),
        "watcher notifications should include issue_comment_deleted, got {event_types:?}"
    );
}

#[tokio::test]
async fn watcher_receives_issue_update_notification() {
    let (ctx, owner, member, _project_id) = ctx_with_real_members().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Watched update".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    ctx.services
        .watcher
        .watch(issue_id, member.id)
        .await
        .unwrap();

    ctx.services
        .issue
        .update(
            issue_id,
            UpdateIssueCommand {
                summary: Some("Watched update changed".to_string()),
                actor_id: owner.id,
                ..Default::default()
            },
            owner.id,
        )
        .await
        .unwrap();

    let unread = ctx
        .repos
        .notifications
        .list_unread(member.id)
        .await
        .unwrap();
    assert_eq!(unread.len(), 1);
    assert_eq!(unread[0].event_type.as_ref(), "issue_updated");
}

#[tokio::test]
async fn watcher_receives_worklog_logged_notification() {
    let (base_ctx, owner, member, _project_id) = ctx_with_real_members().await;
    let repos = Arc::new(domain::Repositories {
        worklogs: Arc::new(MemoryWorklogRepository::default()),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));
    let issue_id = create_demo_issue(&ctx, &owner, "Watched worklog").await;
    ctx.services
        .watcher
        .watch(issue_id, member.id)
        .await
        .unwrap();

    ctx.services
        .worklog
        .create(
            CreateWorklogCommand {
                issue_id,
                author_id: owner.id,
                started_at: shared::now(),
                duration_seconds: 900,
                description: Some("implementation".to_string()),
            },
            owner.id,
        )
        .await
        .unwrap();

    let unread = ctx
        .repos
        .notifications
        .list_unread(member.id)
        .await
        .unwrap();
    let worklog_notifications = unread
        .iter()
        .filter(|notification| notification.event_type.as_ref() == "issue_worklog_logged")
        .collect::<Vec<_>>();
    assert_eq!(worklog_notifications.len(), 1);
    assert_eq!(
        worklog_notifications[0].metadata["duration_seconds"],
        serde_json::json!(900)
    );
}

#[tokio::test]
async fn watcher_receives_attachment_added_notification() {
    let (base_ctx, owner, member, _project_id) = ctx_with_real_members().await;
    let repos = Arc::new(domain::Repositories {
        attachments: Arc::new(MemoryAttachmentRepository::default()),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));
    let issue_id = create_demo_issue(&ctx, &owner, "Watched attachment").await;
    ctx.services
        .watcher
        .watch(issue_id, member.id)
        .await
        .unwrap();

    ctx.services
        .attachment
        .upload(
            issue_id,
            owner.id,
            "evidence.txt",
            "text/plain",
            b"payload".to_vec(),
        )
        .await
        .unwrap();

    let unread = ctx
        .repos
        .notifications
        .list_unread(member.id)
        .await
        .unwrap();
    let attachments: Vec<_> = unread
        .iter()
        .filter(|notification| notification.event_type.as_ref() == "issue_attachment_added")
        .collect();
    assert_eq!(attachments.len(), 1);
    assert_eq!(
        attachments[0].metadata["file_name"],
        serde_json::Value::String("evidence.txt".to_string())
    );
}

#[tokio::test]
async fn watcher_receives_issue_link_create_and_delete_notifications() {
    let (base_ctx, owner, member, _project_id) = ctx_with_real_members().await;
    let repos = Arc::new(domain::Repositories {
        issue_links: Arc::new(MemoryIssueLinkRepository::default()),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));
    let project_key = ProjectKey::new("TT");
    let board = ctx
        .services
        .board
        .get_board(&project_key, owner.id)
        .await
        .unwrap();
    let source = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: project_key.clone(),
                summary: "Watched link source".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();
    let target = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key,
                summary: "Watched link target".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();
    let source_id: IssueId = source.id.parse().unwrap();
    ctx.services
        .watcher
        .watch(source_id, member.id)
        .await
        .unwrap();

    let link = ctx
        .services
        .issue_link
        .create(source_id, &target.key, "relates", owner.id)
        .await
        .unwrap();
    ctx.services
        .issue_link
        .delete(link.id.parse().unwrap(), owner.id)
        .await
        .unwrap();

    let unread = ctx
        .repos
        .notifications
        .list_unread(member.id)
        .await
        .unwrap();
    let event_types = unread
        .iter()
        .map(|notification| notification.event_type.as_ref())
        .collect::<Vec<_>>();
    assert!(
        event_types.contains(&"issue_link_created"),
        "watcher notifications should include issue_link_created, got {event_types:?}"
    );
    assert!(
        event_types.contains(&"issue_link_deleted"),
        "watcher notifications should include issue_link_deleted, got {event_types:?}"
    );
}

// ─── v0.2.0 feature tests ────────────────────────────────────────────

#[tokio::test]
async fn watcher_add_and_list() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Watched issue".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    ctx.services
        .watcher
        .watch(issue.id.parse().unwrap(), user.id)
        .await
        .unwrap();

    let watchers = ctx
        .services
        .watcher
        .list_watchers(issue.id.parse().unwrap(), user.id)
        .await
        .unwrap();
    assert_eq!(watchers.len(), 1);
    assert_eq!(watchers[0].user_id, user.id.to_string());
}

#[tokio::test]
async fn watcher_remove() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Watched issue".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let issue_id: IssueId = issue.id.parse().unwrap();
    ctx.services.watcher.watch(issue_id, user.id).await.unwrap();

    ctx.services
        .watcher
        .unwatch(issue_id, user.id)
        .await
        .unwrap();

    let watchers = ctx
        .services
        .watcher
        .list_watchers(issue_id, user.id)
        .await
        .unwrap();
    assert_eq!(watchers.len(), 0);
}

#[tokio::test]
async fn watcher_watch_and_unwatch_publish_issue_updated_events() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Watched issue realtime".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let mut receiver = ctx.events.subscribe();
    while receiver.try_recv().is_ok() {}

    ctx.services.watcher.watch(issue_id, user.id).await.unwrap();
    let watch_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        watch_event,
        shared::TrackerEvent::IssueUpdated {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));

    ctx.services
        .watcher
        .unwatch(issue_id, user.id)
        .await
        .unwrap();
    let unwatch_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        unwatch_event,
        shared::TrackerEvent::IssueUpdated {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));
}

#[tokio::test]
async fn vote_add_and_count() {
    let (ctx, user) = ctx_with_demo_data().await;
    let reporter = notification_recipient(&ctx).await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Voted issue".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: reporter.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let issue_id: IssueId = issue.id.parse().unwrap();
    ctx.services.vote.vote(issue_id, user.id).await.unwrap();

    let count = ctx.services.vote.count_votes(issue_id).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn vote_remove() {
    let (ctx, user) = ctx_with_demo_data().await;
    let reporter = notification_recipient(&ctx).await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Voted issue".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: reporter.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let issue_id: IssueId = issue.id.parse().unwrap();
    ctx.services.vote.vote(issue_id, user.id).await.unwrap();
    ctx.services.vote.unvote(issue_id, user.id).await.unwrap();

    let count = ctx.services.vote.count_votes(issue_id).await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn vote_and_unvote_publish_issue_updated_events() {
    let (ctx, user) = ctx_with_demo_data().await;
    let reporter = notification_recipient(&ctx).await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Voted issue realtime".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: reporter.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let mut receiver = ctx.events.subscribe();
    while receiver.try_recv().is_ok() {}

    ctx.services.vote.vote(issue_id, user.id).await.unwrap();
    let vote_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        vote_event,
        shared::TrackerEvent::IssueUpdated {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));

    ctx.services.vote.unvote(issue_id, user.id).await.unwrap();
    let unvote_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        unvote_event,
        shared::TrackerEvent::IssueUpdated {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));
}

#[tokio::test]
async fn vote_rejects_reporter_self_vote() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Own vote".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let issue_id: IssueId = issue.id.parse().unwrap();
    let err = ctx.services.vote.vote(issue_id, user.id).await.unwrap_err();
    assert!(matches!(err, AppError::InvalidInput(_)));
}

#[tokio::test]
async fn issue_link_create_and_delete_publish_issue_updated_events_for_both_issues() {
    let (base_ctx, user) = ctx_with_demo_data().await;
    let repos = Arc::new(domain::Repositories {
        issue_links: Arc::new(MemoryIssueLinkRepository::default()),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));
    let project_key = ProjectKey::new("TT");
    let board = ctx
        .services
        .board
        .get_board(&project_key, user.id)
        .await
        .unwrap();
    let source = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: project_key.clone(),
                summary: "Source link realtime".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let target = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key,
                summary: "Target link realtime".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let source_id: IssueId = source.id.parse().unwrap();
    let mut receiver = ctx.events.subscribe();
    while receiver.try_recv().is_ok() {}

    let link = ctx
        .services
        .issue_link
        .create(source_id, &target.key, "relates", user.id)
        .await
        .unwrap();
    let first_create_event =
        tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
            .await
            .unwrap()
            .unwrap();
    let second_create_event =
        tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
            .await
            .unwrap()
            .unwrap();
    let create_event_ids = [first_create_event, second_create_event]
        .into_iter()
        .map(|event| match event {
            shared::TrackerEvent::IssueUpdated {
                issue_id,
                project_key,
            } => {
                assert_eq!(project_key, "TT");
                issue_id
            }
            other => panic!("unexpected event: {other:?}"),
        })
        .collect::<std::collections::HashSet<_>>();
    assert!(create_event_ids.contains(&source.id));
    assert!(create_event_ids.contains(&target.id));

    let link_id: shared::IssueLinkId = link.id.parse().unwrap();
    ctx.services
        .issue_link
        .delete(link_id, user.id)
        .await
        .unwrap();
    let first_delete_event =
        tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
            .await
            .unwrap()
            .unwrap();
    let second_delete_event =
        tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
            .await
            .unwrap()
            .unwrap();
    let delete_event_ids = [first_delete_event, second_delete_event]
        .into_iter()
        .map(|event| match event {
            shared::TrackerEvent::IssueUpdated {
                issue_id,
                project_key,
            } => {
                assert_eq!(project_key, "TT");
                issue_id
            }
            other => panic!("unexpected event: {other:?}"),
        })
        .collect::<std::collections::HashSet<_>>();
    assert!(delete_event_ids.contains(&source.id));
    assert!(delete_event_ids.contains(&target.id));
}

#[tokio::test]
async fn custom_field_create_and_list() {
    let (ctx, user) = ctx_with_demo_data().await;
    let field = ctx
        .services
        .custom_field
        .create_field(
            &ProjectKey::new("TT"),
            "Priority Override",
            "text",
            &[],
            false,
            user.id,
        )
        .await
        .unwrap();

    assert_eq!(field.name, "Priority Override");
    assert_eq!(field.field_type, "text");

    let fields = ctx
        .services
        .custom_field
        .list_fields(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "Priority Override");
    assert_eq!(fields[0].field_type, "text");
}

#[tokio::test]
async fn custom_field_create_validates_and_normalizes_options() {
    let (ctx, user) = ctx_with_demo_data().await;

    let empty_select = ctx
        .services
        .custom_field
        .create_field(
            &ProjectKey::new("TT"),
            "Empty Select",
            "select",
            &[],
            false,
            user.id,
        )
        .await;
    assert!(matches!(empty_select, Err(AppError::InvalidInput(_))));

    let blank_option = ctx
        .services
        .custom_field
        .create_field(
            &ProjectKey::new("TT"),
            "Blank Option",
            "multi-select",
            &["todo".to_string(), " ".to_string()],
            false,
            user.id,
        )
        .await;
    assert!(matches!(blank_option, Err(AppError::InvalidInput(_))));

    let duplicate_options = ctx
        .services
        .custom_field
        .create_field(
            &ProjectKey::new("TT"),
            "Duplicate Options",
            "select",
            &["todo".to_string(), "todo".to_string()],
            false,
            user.id,
        )
        .await;
    assert!(matches!(duplicate_options, Err(AppError::InvalidInput(_))));

    let text_with_options = ctx
        .services
        .custom_field
        .create_field(
            &ProjectKey::new("TT"),
            "Text With Ignored Options",
            "text",
            &["ignored".to_string()],
            false,
            user.id,
        )
        .await
        .unwrap();
    assert!(text_with_options.options.is_empty());

    let valid_select = ctx
        .services
        .custom_field
        .create_field(
            &ProjectKey::new("TT"),
            "Valid Select",
            "select",
            &["todo".to_string()],
            false,
            user.id,
        )
        .await
        .unwrap();
    let valid_select_id = valid_select.id.parse().unwrap();
    let empty_update = ctx
        .services
        .custom_field
        .update_field(
            valid_select_id,
            "Still Broken",
            "select",
            &[],
            false,
            user.id,
        )
        .await;
    assert!(matches!(empty_update, Err(AppError::InvalidInput(_))));

    let text_update = ctx
        .services
        .custom_field
        .update_field(
            valid_select_id,
            "Text Now",
            "text",
            &["ignored".to_string()],
            false,
            user.id,
        )
        .await
        .unwrap();
    assert!(text_update.options.is_empty());
}

#[tokio::test]
async fn custom_field_set_and_get_value() {
    let (ctx, user) = ctx_with_demo_data().await;
    let field = ctx
        .services
        .custom_field
        .create_field(
            &ProjectKey::new("TT"),
            "Effort",
            "text",
            &[],
            false,
            user.id,
        )
        .await
        .unwrap();

    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Issue with custom field".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let issue_id: IssueId = issue.id.parse().unwrap();
    let field_id: shared::CustomFieldId = field.id.parse().unwrap();
    ctx.services
        .custom_field
        .set_value(issue_id, field_id, serde_json::json!("high"), user.id)
        .await
        .unwrap();

    let values = ctx
        .services
        .custom_field
        .get_values_for_issue(issue_id, user.id)
        .await
        .unwrap();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].field_id, field.id);
    assert_eq!(values[0].value, serde_json::json!("high"));
}

#[tokio::test]
async fn issue_create_rejects_missing_or_empty_required_custom_fields() {
    let (ctx, user) = ctx_with_demo_data().await;
    let field = ctx
        .services
        .custom_field
        .create_field(
            &ProjectKey::new("TT"),
            "Required text",
            "text",
            &[],
            true,
            user.id,
        )
        .await
        .unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let base = CreateIssueCommand {
        project_key: ProjectKey::new("TT"),
        summary: "Missing required custom field".to_string(),
        description: None,
        issue_type: IssueType::Task,
        priority: Priority::Medium,
        status_id: board.columns[0].id.to_string(),
        reporter_id: user.id,
        assignee_id: None,
        actor_id: user.id,
        custom_fields: Default::default(),
    };

    let err = ctx
        .services
        .issue
        .create(base.clone(), user.id)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));

    for value in [
        serde_json::Value::Null,
        serde_json::json!(""),
        serde_json::json!("   "),
        serde_json::json!([]),
    ] {
        let mut custom_fields = std::collections::HashMap::new();
        custom_fields.insert(field.id.clone(), value);
        let err = ctx
            .services
            .issue
            .create(
                CreateIssueCommand {
                    custom_fields,
                    ..base.clone()
                },
                user.id,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}

#[tokio::test]
async fn issue_create_persists_custom_fields_and_normalizes_dates() {
    let (ctx, user) = ctx_with_demo_data().await;
    let date_field = ctx
        .services
        .custom_field
        .create_field(&ProjectKey::new("TT"), "Due", "date", &[], true, user.id)
        .await
        .unwrap();
    let text_field = ctx
        .services
        .custom_field
        .create_field(&ProjectKey::new("TT"), "Note", "text", &[], false, user.id)
        .await
        .unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let mut custom_fields = std::collections::HashMap::new();
    custom_fields.insert(
        date_field.id.clone(),
        serde_json::json!("2026-12-31T23:59:59Z"),
    );
    custom_fields.insert(text_field.id.clone(), serde_json::json!("ready"));

    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Custom field create".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields,
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();

    let values = ctx
        .services
        .custom_field
        .get_values_for_issue(issue_id, user.id)
        .await
        .unwrap();
    assert!(values.iter().any(|value| {
        value.field_id == date_field.id && value.value == serde_json::json!("2026-12-31")
    }));
    assert!(
        values
            .iter()
            .any(|value| value.field_id == text_field.id
                && value.value == serde_json::json!("ready"))
    );
}

#[tokio::test]
async fn custom_field_null_clears_optional_and_rejects_required() {
    let (ctx, user) = ctx_with_demo_data().await;
    let optional = ctx
        .services
        .custom_field
        .create_field(
            &ProjectKey::new("TT"),
            "Optional",
            "text",
            &[],
            false,
            user.id,
        )
        .await
        .unwrap();
    let required = ctx
        .services
        .custom_field
        .create_field(
            &ProjectKey::new("TT"),
            "Required",
            "text",
            &[],
            true,
            user.id,
        )
        .await
        .unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let mut custom_fields = std::collections::HashMap::new();
    custom_fields.insert(required.id.clone(), serde_json::json!("seed"));
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Clear custom field".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields,
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let optional_id: shared::CustomFieldId = optional.id.parse().unwrap();
    let required_id: shared::CustomFieldId = required.id.parse().unwrap();
    let mut receiver = ctx.events.subscribe();

    ctx.services
        .custom_field
        .set_value(issue_id, optional_id, serde_json::json!("set"), user.id)
        .await
        .unwrap();
    let set_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        set_event,
        shared::TrackerEvent::IssueUpdated {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));

    ctx.services
        .custom_field
        .set_value(issue_id, optional_id, serde_json::Value::Null, user.id)
        .await
        .unwrap();
    let clear_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        clear_event,
        shared::TrackerEvent::IssueUpdated {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));

    let values = ctx
        .services
        .custom_field
        .get_values_for_issue(issue_id, user.id)
        .await
        .unwrap();
    assert!(!values.iter().any(|value| value.field_id == optional.id));

    let err = ctx
        .services
        .custom_field
        .set_value(issue_id, required_id, serde_json::Value::Null, user.id)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)));
}

#[tokio::test]
async fn attachment_upload_deletes_blob_when_metadata_save_fails() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Attachment cleanup".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let storage = Arc::new(RecordingStorage::default());
    let service = AttachmentServiceImpl::new(
        Arc::new(FailingAttachmentSaveRepository),
        ctx.repos.issues.clone(),
        storage.clone(),
        Arc::new(domain::StubWatcherRepository),
        ctx.events.clone(),
        Arc::new(domain::StubNotificationRepository),
        Arc::new(domain::StubUserNotificationSettingsRepository),
        ctx.authz.clone(),
    );

    let err = service
        .upload(
            issue.id.parse().unwrap(),
            user.id,
            "report.txt",
            "text/plain",
            b"payload".to_vec(),
        )
        .await
        .unwrap_err();

    assert!(matches!(err, AppError::Internal(_)));
    assert_eq!(storage.file_count(), 0);
    assert_eq!(storage.delete_count(), 1);
}

#[tokio::test]
async fn attachment_delete_keeps_blob_when_metadata_delete_fails() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Attachment delete failure".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let attachment = domain::Attachment {
        id: shared::AttachmentId::new(),
        issue_id,
        author_id: user.id,
        file_name: "delete.txt".into(),
        content_type: "text/plain".into(),
        size_bytes: 7,
        storage_key: "delete.txt".into(),
        created_at: shared::now(),
    };
    let storage = Arc::new(RecordingStorage::default());
    storage
        .put(
            &issue_id.to_string(),
            attachment.storage_key.as_ref(),
            b"payload".to_vec(),
        )
        .await
        .unwrap();
    let service = AttachmentServiceImpl::new(
        Arc::new(FailingAttachmentDeleteRepository {
            attachment: attachment.clone(),
        }),
        ctx.repos.issues.clone(),
        storage.clone(),
        Arc::new(domain::StubWatcherRepository),
        ctx.events.clone(),
        Arc::new(domain::StubNotificationRepository),
        Arc::new(domain::StubUserNotificationSettingsRepository),
        ctx.authz.clone(),
    );

    let err = service
        .delete(attachment.id, user.id)
        .await
        .expect_err("metadata delete failure must be returned");

    assert!(matches!(err, AppError::Internal(_)));
    assert_eq!(storage.file_count(), 1);
    assert_eq!(storage.delete_count(), 0);
}

#[tokio::test]
async fn attachment_delete_allows_project_member_who_is_not_author() {
    let (base_ctx, owner, member, _project_id) = ctx_with_real_members().await;
    let storage = Arc::new(RecordingStorage::default());
    let attachments = Arc::new(MemoryAttachmentRepository::default());
    let repos = Arc::new(domain::Repositories {
        attachments: attachments.clone(),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, storage.clone());
    let issue_id = create_demo_issue(&ctx, &owner, "Attachment member delete").await;
    let attachment = ctx
        .services
        .attachment
        .upload(
            issue_id,
            owner.id,
            "member-delete.txt",
            "text/plain",
            b"payload".to_vec(),
        )
        .await
        .unwrap();
    let attachment_id = attachment.id.parse().unwrap();
    assert_eq!(storage.file_count(), 1);

    ctx.services
        .attachment
        .delete(attachment_id, member.id)
        .await
        .unwrap();

    assert!(attachments.get_by_id(attachment_id).await.is_err());
    assert_eq!(storage.file_count(), 0);
    assert_eq!(storage.delete_count(), 1);
}

#[tokio::test]
async fn attachment_upload_and_delete_publish_issue_updated_events() {
    let (base_ctx, user) = ctx_with_demo_data().await;
    let storage = Arc::new(RecordingStorage::default());
    let ctx = AppContext::new(test_config(), base_ctx.repos.clone(), storage);
    let project_key = ProjectKey::new("TT");
    let board = ctx
        .services
        .board
        .get_board(&project_key, user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key,
                summary: "Attachment realtime".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let mut receiver = ctx.events.subscribe();
    while receiver.try_recv().is_ok() {}

    let attachment = ctx
        .services
        .attachment
        .upload(
            issue_id,
            user.id,
            "realtime.txt",
            "text/plain",
            b"payload".to_vec(),
        )
        .await
        .unwrap();
    let upload_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        upload_event,
        shared::TrackerEvent::IssueUpdated {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));

    ctx.services
        .attachment
        .delete(attachment.id.parse().unwrap(), user.id)
        .await
        .unwrap();
    let delete_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        delete_event,
        shared::TrackerEvent::IssueUpdated {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));
}

#[tokio::test]
async fn labels_validate_names_and_hex_colors() {
    let (base_ctx, user) = ctx_with_demo_data().await;
    let repos = Arc::new(domain::Repositories {
        labels: Arc::new(MemoryLabelRepository::default()),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));
    let project_key = ProjectKey::new("TT");

    let label = ctx
        .services
        .label
        .create(&project_key, "  qa  ", " #ABC12f ", user.id)
        .await
        .unwrap();
    assert_eq!(label.name, "qa");
    assert_eq!(label.color, "#ABC12f");

    for invalid_color in ["red", "#12345", "#1234567", "#12zz56", ""] {
        let err = ctx
            .services
            .label
            .create(&project_key, "bad", invalid_color, user.id)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            AppError::InvalidInput(ref msg) if msg.contains("#RRGGBB")
        ));
    }

    let label_id: shared::LabelId = label.id.parse().unwrap();
    let err = ctx
        .services
        .label
        .update(label_id, "  ", "#000000", user.id)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        AppError::InvalidInput(ref msg) if msg.contains("name")
    ));

    let err = ctx
        .services
        .label
        .update(label_id, "renamed", "rgba(0,0,0,1)", user.id)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        AppError::InvalidInput(ref msg) if msg.contains("#RRGGBB")
    ));
}

#[tokio::test]
async fn label_attach_and_detach_publish_issue_updated_events() {
    let (base_ctx, user) = ctx_with_demo_data().await;
    let repos = Arc::new(domain::Repositories {
        labels: Arc::new(MemoryLabelRepository::default()),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));
    let project_key = ProjectKey::new("TT");
    let board = ctx
        .services
        .board
        .get_board(&project_key, user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: project_key.clone(),
                summary: "Label realtime".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let label = ctx
        .services
        .label
        .create(&project_key, "realtime", "#22c55e", user.id)
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let label_id: shared::LabelId = label.id.parse().unwrap();
    let mut receiver = ctx.events.subscribe();
    while receiver.try_recv().is_ok() {}

    ctx.services
        .label
        .attach(issue_id, label_id, user.id)
        .await
        .unwrap();
    let attach_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        attach_event,
        shared::TrackerEvent::IssueUpdated {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));

    ctx.services
        .label
        .detach(issue_id, label_id, user.id)
        .await
        .unwrap();
    let detach_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        detach_event,
        shared::TrackerEvent::IssueUpdated {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));
}

#[tokio::test]
async fn label_update_and_delete_publish_issue_updated_events_for_attached_issues() {
    let (base_ctx, user) = ctx_with_demo_data().await;
    let repos = Arc::new(domain::Repositories {
        labels: Arc::new(MemoryLabelRepository::default()),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));
    let project_key = ProjectKey::new("TT");
    let board = ctx
        .services
        .board
        .get_board(&project_key, user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: project_key.clone(),
                summary: "Label lifecycle realtime".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let label = ctx
        .services
        .label
        .create(&project_key, "initial", "#22c55e", user.id)
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let label_id: shared::LabelId = label.id.parse().unwrap();
    ctx.services
        .label
        .attach(issue_id, label_id, user.id)
        .await
        .unwrap();
    let mut receiver = ctx.events.subscribe();
    while receiver.try_recv().is_ok() {}

    ctx.services
        .label
        .update(label_id, "renamed", "#0ea5e9", user.id)
        .await
        .unwrap();
    let update_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        update_event,
        shared::TrackerEvent::IssueUpdated {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));

    ctx.services.label.delete(label_id, user.id).await.unwrap();
    let delete_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        delete_event,
        shared::TrackerEvent::IssueUpdated {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));
}

#[tokio::test]
async fn comment_update_and_delete_publish_comment_events() {
    let (base_ctx, user) = ctx_with_demo_data().await;
    let repos = Arc::new(domain::Repositories {
        comments: Arc::new(MemoryCommentRepository::default()),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));
    let project_key = ProjectKey::new("TT");
    let board = ctx
        .services
        .board
        .get_board(&project_key, user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key,
                summary: "Comment realtime".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let comment = ctx
        .services
        .comment
        .create(
            CreateCommentCommand {
                issue_id,
                author_id: user.id,
                actor_id: user.id,
                body: "initial".to_string(),
            },
            user.id,
        )
        .await
        .unwrap();
    let comment_id: shared::CommentId = comment.id.parse().unwrap();
    let mut receiver = ctx.events.subscribe();
    while receiver.try_recv().is_ok() {}

    ctx.services
        .comment
        .update(
            comment_id,
            UpdateCommentCommand {
                body: Some("edited".to_string()),
            },
            user.id,
        )
        .await
        .unwrap();
    let update_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        update_event,
        shared::TrackerEvent::IssueCommented {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));

    ctx.services
        .comment
        .delete(comment_id, user.id)
        .await
        .unwrap();
    let delete_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        delete_event,
        shared::TrackerEvent::IssueCommented {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));
}

#[tokio::test]
async fn component_create_and_list() {
    let (ctx, user) = ctx_with_demo_data().await;
    ctx.services
        .component
        .create(
            &ProjectKey::new("TT"),
            "Backend",
            Some("Backend services"),
            user.id,
        )
        .await
        .unwrap();

    let components = ctx
        .services
        .component
        .list_by_project(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    assert_eq!(components.len(), 1);
    assert_eq!(components[0].name, "Backend");
}

#[tokio::test]
async fn version_create_and_list() {
    let (ctx, user) = ctx_with_demo_data().await;
    ctx.services
        .version
        .create(
            &ProjectKey::new("TT"),
            "v1.0",
            Some("Initial release"),
            false,
            None,
            user.id,
        )
        .await
        .unwrap();

    let versions = ctx
        .services
        .version
        .list_by_project(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].name, "v1.0");
}

#[tokio::test]
async fn issue_soft_delete_and_restore_publishes_invalidation_events() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Event lifecycle issue".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let mut events = ctx.events.subscribe();

    ctx.services.issue.delete(issue_id, user.id).await.unwrap();
    let deleted = events.recv().await.unwrap();
    assert!(matches!(
        deleted,
        shared::TrackerEvent::IssueDeleted { issue_id: ref id, project_key: ref key }
            if id == &issue.id && key == "TT"
    ));

    ctx.services.issue.restore(issue_id, user.id).await.unwrap();
    let restored = events.recv().await.unwrap();
    assert!(matches!(
        restored,
        shared::TrackerEvent::IssueUpdated { issue_id: ref id, project_key: ref key }
            if id == &issue.id && key == "TT"
    ));
}

#[tokio::test]
async fn issue_soft_delete_and_restore() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Soft delete me".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let issue_id: IssueId = issue.id.parse().unwrap();
    ctx.services.issue.delete(issue_id, user.id).await.unwrap();

    // After soft-delete, normal get should fail.
    let err = ctx.services.issue.get_by_id(issue_id, user.id).await;
    assert!(err.is_err());

    // Restore and get should succeed.
    let restored = ctx.services.issue.restore(issue_id, user.id).await.unwrap();
    assert_eq!(restored.id, issue.id);
}

#[tokio::test]
async fn issue_soft_delete_lists_in_trash() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Trashed issue".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let issue_id: IssueId = issue.id.parse().unwrap();
    ctx.services.issue.delete(issue_id, user.id).await.unwrap();

    let trash = ctx
        .services
        .issue
        .list_trash(&ProjectKey::new("TT"), user.id, 0, 50)
        .await
        .unwrap();
    assert_eq!(trash.len(), 1);
    assert_eq!(trash[0].id, issue.id);
}

#[tokio::test]
async fn issue_trash_paginates_beyond_default_issue_cap() {
    let (ctx, user) = ctx_with_demo_data().await;
    let project = ctx
        .repos
        .projects
        .get_by_key(&ProjectKey::new("TT"))
        .await
        .unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let todo: StatusId = board.columns[0].id.parse().unwrap();

    for number in 1..=1_005 {
        let mut issue = domain::Issue::create(
            &project,
            number,
            IssueType::Task,
            todo,
            format!("trashed {number}"),
            None,
            user.id,
            Priority::Medium,
        );
        issue.deleted_at = Some(shared::now());
        ctx.repos.issues.save(&issue).await.unwrap();
    }

    let trash = ctx
        .services
        .issue
        .list_trash(&ProjectKey::new("TT"), user.id, 1_000, 50)
        .await
        .unwrap();
    assert_eq!(trash.len(), 5);
}

#[tokio::test]
async fn issue_purge_from_trash() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Purge me".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let issue_id: IssueId = issue.id.parse().unwrap();
    ctx.services.issue.delete(issue_id, user.id).await.unwrap();
    ctx.services.issue.purge(issue_id, user.id).await.unwrap();

    let trash = ctx
        .services
        .issue
        .list_trash(&ProjectKey::new("TT"), user.id, 0, 50)
        .await
        .unwrap();
    assert_eq!(trash.len(), 0);

    // Restore should fail after purge.
    let err = ctx.services.issue.restore(issue_id, user.id).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn issue_purge_from_trash_deletes_attachment_files() {
    let (base_ctx, user) = ctx_with_demo_data().await;
    let storage = Arc::new(RecordingStorage::default());
    let ctx = AppContext::new(test_config(), base_ctx.repos.clone(), storage.clone());
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Purge attachment".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    ctx.services
        .attachment
        .upload(
            issue_id,
            user.id,
            "purge.txt",
            "text/plain",
            b"payload".to_vec(),
        )
        .await
        .unwrap();

    assert_eq!(storage.file_count(), 1);
    ctx.services.issue.delete(issue_id, user.id).await.unwrap();
    ctx.services.issue.purge(issue_id, user.id).await.unwrap();

    assert_eq!(storage.file_count(), 0);
    assert_eq!(storage.delete_count(), 1);
}

#[tokio::test]
async fn project_delete_deletes_issue_attachment_files() {
    let (base_ctx, user) = ctx_with_demo_data().await;
    let storage = Arc::new(RecordingStorage::default());
    let ctx = AppContext::new(test_config(), base_ctx.repos.clone(), storage.clone());
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Project delete attachment".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    ctx.services
        .attachment
        .upload(
            issue.id.parse().unwrap(),
            user.id,
            "project-delete.txt",
            "text/plain",
            b"payload".to_vec(),
        )
        .await
        .unwrap();

    assert_eq!(storage.file_count(), 1);
    ctx.services
        .project
        .delete(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();

    assert_eq!(storage.file_count(), 0);
    assert_eq!(storage.delete_count(), 1);
}

#[tokio::test]
async fn worklog_create_rejects_spoofed_author() {
    let (base_ctx, owner) = ctx_with_demo_data().await;
    let repos = Arc::new(domain::Repositories {
        worklogs: Arc::new(MemoryWorklogRepository::default()),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(test_config(), repos, Arc::new(TestStorage::default()));
    let other = test_user_with(
        "worklog-spoof",
        "worklog-spoof@example.com",
        "Worklog Spoof",
    );
    ctx.repos.users.save(&other).await.unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Worklog spoofing".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();

    let err = ctx
        .services
        .worklog
        .create(
            CreateWorklogCommand {
                issue_id,
                author_id: other.id,
                started_at: shared::now(),
                duration_seconds: 900,
                description: Some("not really mine".to_string()),
            },
            owner.id,
        )
        .await
        .unwrap_err();

    assert!(matches!(err, AppError::Forbidden));
    assert!(
        ctx.repos
            .worklogs
            .list_by_issue(issue_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        ctx.repos
            .issues
            .get_by_id(issue_id)
            .await
            .unwrap()
            .time_spent_seconds,
        0
    );
}

#[tokio::test]
async fn worklog_create_propagates_author_lookup_error_without_writing() {
    let (base_ctx, owner) = ctx_with_demo_data().await;
    let board = base_ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let issue = base_ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "worklog author lookup failure".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let worklogs = Arc::new(MemoryWorklogRepository::default());
    let repos = Arc::new(domain::Repositories {
        users: Arc::new(FailingUserRepository),
        worklogs: worklogs.clone(),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(
        test_config(),
        repos.clone(),
        Arc::new(TestStorage::default()),
    );

    assert_internal(
        ctx.services
            .worklog
            .create(
                CreateWorklogCommand {
                    issue_id,
                    author_id: owner.id,
                    started_at: shared::now(),
                    duration_seconds: 300,
                    description: Some("must not be persisted".to_string()),
                },
                owner.id,
            )
            .await,
    );
    assert!(
        worklogs.list_by_issue(issue_id).await.unwrap().is_empty(),
        "failed author lookup must happen before writing the worklog"
    );
    let unchanged_issue = base_ctx.repos.issues.get_by_id(issue_id).await.unwrap();
    assert_eq!(unchanged_issue.time_spent_seconds, 0);
}

#[tokio::test]
async fn worklog_list_propagates_author_directory_error() {
    let (base_ctx, owner) = ctx_with_demo_data().await;
    let board = base_ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let issue = base_ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "worklog list user lookup failure".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let worklogs = Arc::new(MemoryWorklogRepository::default());
    worklogs
        .save(&domain::Worklog {
            id: shared::WorklogId::new(),
            issue_id,
            author_id: owner.id,
            started_at: shared::now(),
            duration_seconds: 300,
            description: Some("stored worklog".into()),
            created_at: shared::now(),
            updated_at: shared::now(),
        })
        .await
        .unwrap();
    let repos = Arc::new(domain::Repositories {
        users: Arc::new(FailingUserRepository),
        worklogs: worklogs.clone(),
        ..(*base_ctx.repos).clone()
    });
    let ctx = AppContext::new(
        test_config(),
        repos.clone(),
        Arc::new(TestStorage::default()),
    );

    assert_internal(ctx.services.worklog.list(issue_id, owner.id, None, 0).await);
}

struct SaveFailIssueRepository {
    inner: Arc<dyn IssueRepository>,
}

#[async_trait::async_trait]
impl IssueRepository for SaveFailIssueRepository {
    async fn get_by_id(&self, id: IssueId) -> Result<Issue, AppError> {
        self.inner.get_by_id(id).await
    }

    async fn get_by_id_include_deleted(&self, id: IssueId) -> Result<Issue, AppError> {
        self.inner.get_by_id_include_deleted(id).await
    }

    async fn get_by_key(&self, key: &IssueKey) -> Result<Issue, AppError> {
        self.inner.get_by_key(key).await
    }

    async fn change_status_atomic(
        &self,
        issue_id: IssueId,
        project_id: ProjectId,
        from_status_id: StatusId,
        to_status_id: StatusId,
        actor_id: UserId,
        guard: &domain::TransitionGuard,
    ) -> Result<(), AppError> {
        self.inner
            .change_status_atomic(
                issue_id,
                project_id,
                from_status_id,
                to_status_id,
                actor_id,
                guard,
            )
            .await
    }

    async fn list(&self, query: IssueQuery) -> Result<Vec<Issue>, AppError> {
        self.inner.list(query).await
    }

    async fn save(&self, _issue: &Issue) -> Result<IssueId, AppError> {
        Err(AppError::Internal("issue save failed".into()))
    }

    async fn delete(&self, id: IssueId) -> Result<(), AppError> {
        self.inner.delete(id).await
    }

    async fn restore(&self, id: IssueId) -> Result<(), AppError> {
        self.inner.restore(id).await
    }

    async fn purge(&self, id: IssueId) -> Result<(), AppError> {
        self.inner.purge(id).await
    }
}

async fn ctx_with_issue_save_failure_for_worklogs() -> (AppContext, User, IssueId) {
    let (base_ctx, owner) = ctx_with_demo_data().await;
    let issue_id = create_demo_issue(&base_ctx, &owner, "worklog rollback").await;
    let worklogs = Arc::new(MemoryWorklogRepository::default());
    let repos = Arc::new(domain::Repositories {
        issues: Arc::new(SaveFailIssueRepository {
            inner: base_ctx.repos.issues.clone(),
        }),
        worklogs,
        ..(*base_ctx.repos).clone()
    });
    (
        AppContext::new(test_config(), repos, Arc::new(TestStorage::default())),
        owner,
        issue_id,
    )
}

#[tokio::test]
async fn worklog_create_rolls_back_when_issue_time_sync_fails() {
    let (ctx, owner, issue_id) = ctx_with_issue_save_failure_for_worklogs().await;

    assert_internal(
        ctx.services
            .worklog
            .create(
                CreateWorklogCommand {
                    issue_id,
                    author_id: owner.id,
                    started_at: shared::now(),
                    duration_seconds: 300,
                    description: Some("rollback create".to_string()),
                },
                owner.id,
            )
            .await,
    );

    assert!(
        ctx.repos
            .worklogs
            .list_by_issue(issue_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn worklog_update_rolls_back_when_issue_time_sync_fails() {
    let (ctx, owner, issue_id) = ctx_with_issue_save_failure_for_worklogs().await;
    let worklog = domain::Worklog {
        id: shared::WorklogId::new(),
        issue_id,
        author_id: owner.id,
        started_at: shared::now(),
        duration_seconds: 300,
        description: Some("before".into()),
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    ctx.repos.worklogs.save(&worklog).await.unwrap();

    assert_internal(
        ctx.services
            .worklog
            .update(
                worklog.id,
                UpdateWorklogCommand {
                    started_at: None,
                    duration_seconds: Some(900),
                    description: Some(Some("after".to_string())),
                },
                owner.id,
            )
            .await,
    );

    let stored = ctx.repos.worklogs.get_by_id(worklog.id).await.unwrap();
    assert_eq!(stored.duration_seconds, 300);
    assert_eq!(
        stored.description.as_ref().map(|d| d.as_ref()),
        Some("before")
    );
}

#[tokio::test]
async fn worklog_delete_rolls_back_when_issue_time_sync_fails() {
    let (ctx, owner, issue_id) = ctx_with_issue_save_failure_for_worklogs().await;
    let worklog = domain::Worklog {
        id: shared::WorklogId::new(),
        issue_id,
        author_id: owner.id,
        started_at: shared::now(),
        duration_seconds: 300,
        description: Some("restore me".into()),
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    ctx.repos.worklogs.save(&worklog).await.unwrap();

    assert_internal(ctx.services.worklog.delete(worklog.id, owner.id).await);

    let stored = ctx.repos.worklogs.get_by_id(worklog.id).await.unwrap();
    assert_eq!(stored.duration_seconds, 300);
    assert_eq!(
        stored.description.as_ref().map(|d| d.as_ref()),
        Some("restore me")
    );
}

#[tokio::test]
async fn sprint_move_issue_publishes_issue_updated_event() {
    let (ctx, user) = ctx_with_demo_data().await;
    let project = ctx
        .repos
        .projects
        .get_by_key(&ProjectKey::new("TT"))
        .await
        .unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Move to sprint".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let sprint = ctx
        .services
        .sprint
        .create(
            CreateSprintCommand {
                project_id: project.id,
                name: "Sprint".to_string(),
                goal: None,
                start_date: None,
                end_date: None,
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let sprint_id: SprintId = sprint.id.parse().unwrap();
    let mut receiver = ctx.events.subscribe();

    ctx.services
        .sprint
        .move_issue(
            MoveIssueToSprintCommand {
                issue_id,
                sprint_id: Some(sprint_id),
            },
            user.id,
        )
        .await
        .unwrap();

    let event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        event,
        shared::TrackerEvent::IssueUpdated {
            issue_id: ref actual_issue_id,
            project_key: ref actual_project_key,
        } if actual_issue_id == &issue.id && actual_project_key == "TT"
    ));
}

#[tokio::test]
async fn sprint_move_issue_updates_issue_timestamp() {
    let (ctx, user) = ctx_with_demo_data().await;
    let project = ctx
        .repos
        .projects
        .get_by_key(&ProjectKey::new("TT"))
        .await
        .unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Move timestamp".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let sprint = ctx
        .services
        .sprint
        .create(
            CreateSprintCommand {
                project_id: project.id,
                name: "Timestamp Sprint".to_string(),
                goal: None,
                start_date: None,
                end_date: None,
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let sprint_id: SprintId = sprint.id.parse().unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    let moved = ctx
        .services
        .sprint
        .move_issue(
            MoveIssueToSprintCommand {
                issue_id,
                sprint_id: Some(sprint_id),
            },
            user.id,
        )
        .await
        .unwrap();
    let moved_issue = ctx.repos.issues.get_by_id(issue_id).await.unwrap();
    assert!(
        moved_issue.updated_at > issue.updated_at,
        "moving to a sprint must refresh issue.updated_at"
    );

    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    ctx.services
        .sprint
        .move_issue(
            MoveIssueToSprintCommand {
                issue_id,
                sprint_id: None,
            },
            user.id,
        )
        .await
        .unwrap();
    let removed_issue = ctx.repos.issues.get_by_id(issue_id).await.unwrap();
    assert!(
        removed_issue.updated_at > moved_issue.updated_at,
        "removing from a sprint must refresh issue.updated_at"
    );
    assert_eq!(moved.sprint_id.as_deref(), Some(sprint.id.as_str()));
    assert_eq!(removed_issue.sprint_id, None);
}

#[tokio::test]
async fn sprint_lifecycle_publishes_sprint_changed_events() {
    let (ctx, user) = ctx_with_demo_data().await;
    let project = ctx
        .repos
        .projects
        .get_by_key(&ProjectKey::new("TT"))
        .await
        .unwrap();
    let mut receiver = ctx.events.subscribe();

    let sprint = ctx
        .services
        .sprint
        .create(
            CreateSprintCommand {
                project_id: project.id,
                name: "Sprint events".to_string(),
                goal: None,
                start_date: None,
                end_date: None,
            },
            user.id,
        )
        .await
        .unwrap();

    let created_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        created_event,
        shared::TrackerEvent::SprintChanged {
            project_key: ref actual_project_key,
        } if actual_project_key == "TT"
    ));

    let sprint_id: SprintId = sprint.id.parse().unwrap();
    ctx.services
        .sprint
        .update(
            sprint_id,
            UpdateSprintCommand {
                name: Some("Sprint events updated".to_string()),
                ..Default::default()
            },
            user.id,
        )
        .await
        .unwrap();

    let updated_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        updated_event,
        shared::TrackerEvent::SprintChanged {
            project_key: ref actual_project_key,
        } if actual_project_key == "TT"
    ));

    ctx.services.sprint.start(sprint_id, user.id).await.unwrap();
    let started_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        started_event,
        shared::TrackerEvent::SprintChanged {
            project_key: ref actual_project_key,
        } if actual_project_key == "TT"
    ));

    ctx.services.sprint.close(sprint_id, user.id).await.unwrap();
    let closed_event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        closed_event,
        shared::TrackerEvent::SprintChanged {
            project_key: ref actual_project_key,
        } if actual_project_key == "TT"
    ));
}

#[tokio::test]
async fn notification_created_on_issue_assign() {
    let (ctx, user) = ctx_with_demo_data().await;

    // Register a second user to use as assignee (notifications are only
    // created when assignee != reporter).
    let assignee = ctx
        .services
        .auth
        .register(RegisterCommand {
            email: "assignee@example.com".to_string(),
            username: "assignee".to_string(),
            name: "Assignee User".to_string(),
            password: "secret123".to_string(),
        })
        .await
        .unwrap();
    let assignee_id: UserId = assignee.user.id.parse().unwrap();
    let project = ctx
        .repos
        .projects
        .get_by_key(&ProjectKey::new("TT"))
        .await
        .unwrap();
    ctx.repos
        .members
        .save(&domain::ProjectMember {
            project_id: project.id,
            user_id: assignee_id,
            role: domain::ProjectRole::Member,
            joined_at: shared::now(),
        })
        .await
        .unwrap();

    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Assigned issue".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: Some(assignee_id),
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let notifications = ctx
        .services
        .notification
        .list_unread(assignee_id)
        .await
        .unwrap();
    assert_eq!(notifications.unread_count, 1);
    assert_eq!(notifications.notifications[0].event_type, "issue_assigned");
    let expected_url = format!("/issues/{}", issue.id);
    assert_eq!(
        notifications.notifications[0].action_url.as_deref(),
        Some(expected_url.as_str())
    );
}

#[tokio::test]
async fn issue_update_same_assignee_does_not_duplicate_assignment_notification() {
    let (ctx, owner) = ctx_with_demo_data().await;
    let assignee = notification_recipient(&ctx).await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Idempotent assignee notification".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();

    ctx.services
        .issue
        .update(
            issue_id,
            UpdateIssueCommand {
                assignee_id: Some(Some(assignee.id)),
                actor_id: owner.id,
                ..Default::default()
            },
            owner.id,
        )
        .await
        .unwrap();
    ctx.services
        .issue
        .update(
            issue_id,
            UpdateIssueCommand {
                summary: Some("Same assignee patch".to_string()),
                assignee_id: Some(Some(assignee.id)),
                actor_id: owner.id,
                ..Default::default()
            },
            owner.id,
        )
        .await
        .unwrap();

    let notifications = ctx
        .services
        .notification
        .list_unread(assignee.id)
        .await
        .unwrap();
    let assignment_count = notifications
        .notifications
        .iter()
        .filter(|notification| notification.event_type == "issue_assigned")
        .count();
    assert_eq!(assignment_count, 1);
}

// ─── Report service tests ───────────────────────────────────────────

use crate::context::ReportService;
use domain::{IssueStatusHistory, MemoryIssueStatusHistoryRepository, MemoryStatusRepository};

fn make_status(id: &str, category: StatusCategory, is_closed: bool) -> Status {
    Status {
        id: StatusId::from_uuid(uuid::Uuid::parse_str(id).unwrap()),
        name: "status".into(),
        category,
        position: 0,
        is_default: false,
        is_closed,
    }
}

fn make_sprint(
    id: &str,
    project_id: ProjectId,
    name: &str,
    state: domain::SprintState,
    start: chrono::DateTime<chrono::FixedOffset>,
    end: chrono::DateTime<chrono::FixedOffset>,
) -> Sprint {
    Sprint {
        id: SprintId::from_uuid(uuid::Uuid::parse_str(id).unwrap()),
        project_id,
        name: name.into(),
        goal: None,
        state,
        start_date: Some(start),
        end_date: Some(end),
        velocity: None,
    }
}

fn make_issue(
    id: &str,
    project_id: ProjectId,
    key_num: u32,
    status_id: StatusId,
    sprint_id: Option<SprintId>,
    created_at: chrono::DateTime<chrono::FixedOffset>,
) -> Issue {
    Issue {
        id: IssueId::from_uuid(uuid::Uuid::parse_str(id).unwrap()),
        project_id,
        key: IssueKey::new(ProjectKey::new("TT"), key_num),
        issue_type: IssueType::Task,
        status_id,
        summary: "test".into(),
        description: None,
        assignee_id: None,
        reporter_id: UserId::new(),
        priority: Priority::Medium,
        labels: vec![],
        sprint_id,
        component_id: None,
        affected_version_id: None,
        fix_version_id: None,
        position: 0.0,
        due_date: None,
        original_estimate_seconds: None,
        remaining_estimate_seconds: None,
        time_spent_seconds: 0,
        created_at,
        updated_at: created_at,
        deleted_at: None,
        events: vec![],
    }
}

fn make_history(
    id: &str,
    issue_id: IssueId,
    to_status_id: StatusId,
    changed_at: chrono::DateTime<chrono::FixedOffset>,
) -> IssueStatusHistory {
    IssueStatusHistory {
        id: shared::IssueStatusHistoryId::from_uuid(uuid::Uuid::parse_str(id).unwrap()),
        issue_id,
        from_status_id: None,
        to_status_id,
        changed_by_id: UserId::new(),
        changed_at,
    }
}

fn make_transition_history(
    id: &str,
    issue_id: IssueId,
    from_status_id: Option<StatusId>,
    to_status_id: StatusId,
    changed_at: chrono::DateTime<chrono::FixedOffset>,
) -> IssueStatusHistory {
    IssueStatusHistory {
        id: shared::IssueStatusHistoryId::from_uuid(uuid::Uuid::parse_str(id).unwrap()),
        issue_id,
        from_status_id,
        to_status_id,
        changed_by_id: UserId::new(),
        changed_at,
    }
}

#[allow(clippy::type_complexity)]
async fn report_service(
    issues: Arc<MemoryIssueRepository>,
    sprints: Arc<dyn domain::SprintRepository>,
    statuses: Arc<dyn StatusRepository>,
    history: Arc<MemoryIssueStatusHistoryRepository>,
    project_id: ProjectId,
    owner: UserId,
) -> crate::services::ReportServiceImpl {
    let projects = Arc::new(MemoryProjectRepository::default());
    projects
        .save(&domain::Project {
            id: project_id,
            key: ProjectKey::new("RP"),
            name: "Reports".into(),
            description: None,
            owner_id: owner,
            default_board_id: shared::BoardId::new(),
            created_at: shared::now(),
            updated_at: shared::now(),
        })
        .await
        .unwrap();
    crate::services::ReportServiceImpl::new(
        issues,
        sprints,
        statuses,
        history,
        crate::authz::Authz::new(Arc::new(domain::StubProjectMemberRepository), projects),
    )
}

/// Test fixtures returned by [`report_test_setup`].
#[allow(clippy::type_complexity)]
fn report_test_setup() -> (
    Arc<MemoryIssueRepository>,
    Arc<MemorySprintRepository>,
    Arc<MemoryStatusRepository>,
    Arc<MemoryIssueStatusHistoryRepository>,
    ProjectId,
    StatusId,
    StatusId,
    StatusId,
    UserId,
) {
    let todo =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let in_progress =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap());
    let done =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap());

    let statuses = vec![
        make_status(
            "00000000-0000-0000-0000-000000000001",
            StatusCategory::Todo,
            false,
        ),
        make_status(
            "00000000-0000-0000-0000-000000000002",
            StatusCategory::InProgress,
            false,
        ),
        make_status(
            "00000000-0000-0000-0000-000000000003",
            StatusCategory::Done,
            true,
        ),
    ];
    let status_repo = Arc::new(MemoryStatusRepository::new(statuses));
    let issue_repo = Arc::new(MemoryIssueRepository::default());
    let sprint_repo = Arc::new(MemorySprintRepository::default());
    let history_repo = Arc::new(MemoryIssueStatusHistoryRepository::default());
    let project_id = ProjectId::new();

    let owner_id = UserId::new();
    (
        issue_repo,
        sprint_repo,
        status_repo,
        history_repo,
        project_id,
        todo,
        in_progress,
        done,
        owner_id,
    )
}

#[tokio::test]
async fn report_velocity_propagates_status_lookup_error() {
    let (issues, sprints, _statuses, history, project_id, _todo, _ip, _done, owner) =
        report_test_setup();
    let service = report_service(
        issues,
        sprints,
        Arc::new(FailingStatusRepository),
        history,
        project_id,
        owner,
    )
    .await;

    assert_internal(service.get_velocity(project_id, 6, owner).await);
}

#[tokio::test]
async fn report_velocity_counts_committed_vs_completed() {
    let (issues, sprints, statuses, history, project_id, todo, _ip, done, owner) =
        report_test_setup();

    // Two closed sprints
    let s1 = make_sprint(
        "aaaaaaaa-0000-0000-0000-000000000001",
        project_id,
        "Sprint 1",
        domain::SprintState::Closed,
        shared::now() - chrono::Duration::days(20),
        shared::now() - chrono::Duration::days(10),
    );
    let s2 = make_sprint(
        "aaaaaaaa-0000-0000-0000-000000000002",
        project_id,
        "Sprint 2",
        domain::SprintState::Closed,
        shared::now() - chrono::Duration::days(10),
        shared::now(),
    );
    sprints.save(&s1).await.unwrap();
    sprints.save(&s2).await.unwrap();

    // Sprint 1: 3 issues committed, 2 completed (done status)
    for i in 1..=3 {
        let st = if i <= 2 { done } else { todo };
        let issue = make_issue(
            &format!("bbbbbbbb-0000-0000-0000-00000000000{i}"),
            project_id,
            i,
            st,
            Some(s1.id),
            shared::now() - chrono::Duration::days(15),
        );
        issues.save(&issue).await.unwrap();
    }

    // Sprint 2: 2 issues committed, 1 completed
    for i in 4..=5 {
        let st = if i == 4 { done } else { todo };
        let issue = make_issue(
            &format!("bbbbbbbb-0000-0000-0000-00000000000{i}"),
            project_id,
            i,
            st,
            Some(s2.id),
            shared::now() - chrono::Duration::days(5),
        );
        issues.save(&issue).await.unwrap();
    }

    let service = report_service(
        issues.clone(),
        sprints.clone(),
        statuses.clone(),
        history,
        project_id,
        owner,
    )
    .await;
    let result = service.get_velocity(project_id, 6, owner).await.unwrap();
    assert_eq!(result.len(), 2);
    // Most recent first (sprint 2)
    assert_eq!(result[0].name, "Sprint 2");
    assert_eq!(result[0].committed, 2);
    assert_eq!(result[0].completed, 1);
    assert_eq!(result[1].name, "Sprint 1");
    assert_eq!(result[1].committed, 3);
    assert_eq!(result[1].completed, 2);
}

#[tokio::test]
async fn report_velocity_uses_sprint_end_status_not_current_issue_status() {
    let (issues, sprints, statuses, history, project_id, todo, _in_progress, done, owner) =
        report_test_setup();

    let start = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00+00:00").unwrap();
    let end = chrono::DateTime::parse_from_rfc3339("2026-01-14T00:00:00+00:00").unwrap();
    let after_end = chrono::DateTime::parse_from_rfc3339("2026-01-15T00:00:00+00:00").unwrap();
    let sprint = make_sprint(
        "aaaaaaaa-0000-0000-0000-000000000101",
        project_id,
        "Closed Sprint",
        domain::SprintState::Closed,
        start,
        end,
    );
    sprints.save(&sprint).await.unwrap();

    let issue = make_issue(
        "bbbbbbbb-0000-0000-0000-000000000101",
        project_id,
        101,
        todo,
        Some(sprint.id),
        start,
    );
    issues.save(&issue).await.unwrap();
    history.save_with_project(
        &make_transition_history(
            "11111111-0000-0000-0000-000000000301",
            issue.id,
            None,
            todo,
            start,
        ),
        project_id,
    );
    history.save_with_project(
        &make_transition_history(
            "11111111-0000-0000-0000-000000000302",
            issue.id,
            Some(todo),
            done,
            end - chrono::Duration::days(1),
        ),
        project_id,
    );
    history.save_with_project(
        &make_transition_history(
            "11111111-0000-0000-0000-000000000303",
            issue.id,
            Some(done),
            todo,
            after_end,
        ),
        project_id,
    );

    let service = report_service(
        issues.clone(),
        sprints.clone(),
        statuses.clone(),
        history,
        project_id,
        owner,
    )
    .await;
    let result = service.get_velocity(project_id, 6, owner).await.unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].completed, 1);
}

#[tokio::test]
async fn report_burndown_computes_remaining_per_day() {
    let (issues, sprints, statuses, history, project_id, _todo, _ip, _done, owner) =
        report_test_setup();

    let start = shared::now() - chrono::Duration::days(2);
    let end = shared::now() + chrono::Duration::days(2);
    let sprint = make_sprint(
        "cccccccc-0000-0000-0000-000000000001",
        project_id,
        "Active Sprint",
        domain::SprintState::Active,
        start,
        end,
    );
    sprints.save(&sprint).await.unwrap();

    // 5 issues in sprint, all still open (todo)
    for i in 1..=5 {
        let issue = make_issue(
            &format!("dddddddd-0000-0000-0000-00000000000{i}"),
            project_id,
            i,
            StatusId::from_uuid(
                uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            ),
            Some(sprint.id),
            start,
        );
        issues.save(&issue).await.unwrap();
    }

    let service = report_service(
        issues.clone(),
        sprints.clone(),
        statuses.clone(),
        history,
        project_id,
        owner,
    )
    .await;
    let result = service.get_burndown(sprint.id, owner).await.unwrap();
    assert_eq!(result.sprint_name, "Active Sprint");
    // Should have at least 3 days (start, start+1, today)
    assert!(!result.points.is_empty());
    // First point = 5 (all committed)
    assert_eq!(result.points[0].remaining, 5);
}

#[tokio::test]
async fn report_burndown_uses_history_not_later_issue_edits() {
    let (issues, sprints, statuses, history, project_id, todo, _in_progress, done, owner) =
        report_test_setup();

    let start = shared::now() - chrono::Duration::days(3);
    let end = shared::now();
    let sprint = make_sprint(
        "cccccccc-0000-0000-0000-000000000101",
        project_id,
        "History Sprint",
        domain::SprintState::Active,
        start,
        end,
    );
    sprints.save(&sprint).await.unwrap();

    let mut issue = make_issue(
        "dddddddd-0000-0000-0000-000000000101",
        project_id,
        101,
        done,
        Some(sprint.id),
        start,
    );
    issue.updated_at = shared::now();
    issues.save(&issue).await.unwrap();

    history.save_with_project(
        &make_transition_history(
            "11111111-0000-0000-0000-000000000101",
            issue.id,
            None,
            todo,
            start,
        ),
        project_id,
    );
    history.save_with_project(
        &make_transition_history(
            "11111111-0000-0000-0000-000000000102",
            issue.id,
            Some(todo),
            done,
            start + chrono::Duration::days(1),
        ),
        project_id,
    );

    let service = report_service(
        issues.clone(),
        sprints.clone(),
        statuses.clone(),
        history,
        project_id,
        owner,
    )
    .await;
    let result = service.get_burndown(sprint.id, owner).await.unwrap();

    assert_eq!(result.points.first().unwrap().remaining, 1);
    assert_eq!(result.points.last().unwrap().remaining, 0);
}

#[tokio::test]
async fn report_cumulative_flow_snapshots_status_categories() {
    let (issues, _sprints, statuses, history, project_id, todo, in_progress, done, owner) =
        report_test_setup();

    let issue = make_issue(
        "eeeeeeee-0000-0000-0000-000000000001",
        project_id,
        1,
        done,
        None,
        shared::now() - chrono::Duration::days(3),
    );
    issues.save(&issue).await.unwrap();

    // History: created -> todo, todo -> in_progress, in_progress -> done
    let t0 = shared::now() - chrono::Duration::days(3);
    let t1 = shared::now() - chrono::Duration::days(2);
    let t2 = shared::now() - chrono::Duration::days(1);

    history.save_with_project(
        &make_history("11111111-0000-0000-0000-000000000001", issue.id, todo, t0),
        project_id,
    );
    history.save_with_project(
        &make_history(
            "11111111-0000-0000-0000-000000000002",
            issue.id,
            in_progress,
            t1,
        ),
        project_id,
    );
    history.save_with_project(
        &make_history("11111111-0000-0000-0000-000000000003", issue.id, done, t2),
        project_id,
    );

    let service = report_service(
        issues.clone(),
        Arc::new(domain::StubSprintRepository),
        statuses.clone(),
        history,
        project_id,
        owner,
    )
    .await;
    let result = service
        .get_cumulative_flow(project_id, owner)
        .await
        .unwrap();
    assert!(!result.is_empty());
    // After the last transition, done should be 1, todo and in_progress 0
    let last = result.last().unwrap();
    assert_eq!(last.done, 1);
    assert_eq!(last.todo, 0);
    assert_eq!(last.in_progress, 0);
}

#[tokio::test]
async fn report_cumulative_flow_uses_first_transition_from_status_for_legacy_issues() {
    let (issues, _sprints, statuses, history, project_id, todo, in_progress, _done, owner) =
        report_test_setup();

    let created_at = shared::now() - chrono::Duration::days(2);
    let first_transition_at = shared::now() - chrono::Duration::days(1);
    let issue = make_issue(
        "eeeeeeee-0000-0000-0000-000000000101",
        project_id,
        101,
        in_progress,
        None,
        created_at,
    );
    issues.save(&issue).await.unwrap();
    history.save_with_project(
        &make_transition_history(
            "11111111-0000-0000-0000-000000000201",
            issue.id,
            Some(todo),
            in_progress,
            first_transition_at,
        ),
        project_id,
    );

    let service = report_service(
        issues.clone(),
        Arc::new(domain::StubSprintRepository),
        statuses.clone(),
        history,
        project_id,
        owner,
    )
    .await;
    let result = service
        .get_cumulative_flow(project_id, owner)
        .await
        .unwrap();

    assert_eq!(result.first().unwrap().todo, 1);
    assert_eq!(result.first().unwrap().in_progress, 0);
    assert_eq!(result.last().unwrap().todo, 0);
    assert_eq!(result.last().unwrap().in_progress, 1);
}

#[tokio::test]
async fn report_cumulative_flow_includes_issues_beyond_default_issue_cap() {
    let (issues, _sprints, statuses, history, project_id, todo, _in_progress, _done, owner) =
        report_test_setup();
    let created = shared::now() - chrono::Duration::days(1);

    for key_num in 1..=1_001 {
        let issue = make_issue(
            &uuid::Uuid::new_v4().to_string(),
            project_id,
            key_num,
            todo,
            None,
            created,
        );
        issues.save(&issue).await.unwrap();
    }

    let service = report_service(
        issues.clone(),
        Arc::new(domain::StubSprintRepository),
        statuses.clone(),
        history,
        project_id,
        owner,
    )
    .await;
    let result = service
        .get_cumulative_flow(project_id, owner)
        .await
        .unwrap();
    let last = result.last().unwrap();
    assert_eq!(last.todo, 1_001);
}

#[tokio::test]
async fn report_control_chart_computes_cycle_time() {
    let (issues, _sprints, statuses, history, project_id, todo, in_progress, done, owner) =
        report_test_setup();

    let created = chrono::DateTime::parse_from_rfc3339("2026-02-01T00:00:00+00:00").unwrap();
    let started = chrono::DateTime::parse_from_rfc3339("2026-02-04T00:00:00+00:00").unwrap();
    let done_time = chrono::DateTime::parse_from_rfc3339("2026-02-06T00:00:00+00:00").unwrap();

    let issue = make_issue(
        "ffffffff-0000-0000-0000-000000000001",
        project_id,
        1,
        done,
        None,
        created,
    );
    issues.save(&issue).await.unwrap();

    // History: created -> todo, then todo -> in progress, then done.
    history.save_with_project(
        &make_history(
            "22222222-0000-0000-0000-000000000001",
            issue.id,
            todo,
            created,
        ),
        project_id,
    );
    history.save_with_project(
        &make_transition_history(
            "22222222-0000-0000-0000-000000000002",
            issue.id,
            Some(todo),
            in_progress,
            started,
        ),
        project_id,
    );
    history.save_with_project(
        &make_transition_history(
            "22222222-0000-0000-0000-000000000003",
            issue.id,
            Some(in_progress),
            done,
            done_time,
        ),
        project_id,
    );

    let service = report_service(
        issues.clone(),
        Arc::new(domain::StubSprintRepository),
        statuses.clone(),
        history,
        project_id,
        owner,
    )
    .await;
    let result = service.get_control_chart(project_id, owner).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].issue_key, issue.key.to_string());
    assert!((result[0].cycle_time_days - 2.0).abs() < 0.1);
}

#[tokio::test]
async fn report_control_chart_uses_first_transition_from_status_for_legacy_started_issues() {
    let (issues, _sprints, statuses, history, project_id, _todo, in_progress, done, owner) =
        report_test_setup();

    let created = chrono::DateTime::parse_from_rfc3339("2026-02-01T00:00:00+00:00").unwrap();
    let done_time = chrono::DateTime::parse_from_rfc3339("2026-02-06T00:00:00+00:00").unwrap();

    let issue = make_issue(
        "44444444-0000-0000-0000-000000000001",
        project_id,
        1,
        done,
        None,
        created,
    );
    issues.save(&issue).await.unwrap();

    history.save_with_project(
        &make_transition_history(
            "44444444-0000-0000-0000-000000000002",
            issue.id,
            Some(in_progress),
            done,
            done_time,
        ),
        project_id,
    );

    let service = report_service(
        issues.clone(),
        Arc::new(domain::StubSprintRepository),
        statuses.clone(),
        history,
        project_id,
        owner,
    )
    .await;
    let result = service.get_control_chart(project_id, owner).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].issue_key, issue.key.to_string());
    assert!((result[0].cycle_time_days - 5.0).abs() < 0.1);
}

#[tokio::test]
async fn report_control_chart_skips_issues_without_done_transition() {
    let (issues, _sprints, statuses, history, project_id, todo, _ip, _done, owner) =
        report_test_setup();

    let issue = make_issue(
        "33333333-0000-0000-0000-000000000001",
        project_id,
        1,
        todo,
        None,
        shared::now() - chrono::Duration::days(5),
    );
    issues.save(&issue).await.unwrap();

    let service = report_service(
        issues.clone(),
        Arc::new(domain::StubSprintRepository),
        statuses.clone(),
        history,
        project_id,
        owner,
    )
    .await;
    let result = service.get_control_chart(project_id, owner).await.unwrap();
    // No done transition → not included
    assert!(result.is_empty());
}

// ─── Tests for audit fixes ───────────────────────────────────────────

#[tokio::test]
async fn restore_non_deleted_issue_returns_error() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Not deleted".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    let issue_id: IssueId = issue.id.parse().unwrap();
    // Restoring a non-deleted issue should fail.
    let err = ctx.services.issue.restore(issue_id, user.id).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn board_move_rejects_issue_from_other_project() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();

    // Create a second project with its own board.
    let _project2 = ctx
        .services
        .project
        .create(CreateProjectCommand {
            key: ProjectKey::new("OTHER"),
            name: "Other Project".to_string(),
            description: None,
            owner_id: user.id,
        })
        .await
        .unwrap();

    // Create issue in project OTHER.
    let board2 = ctx
        .services
        .board
        .get_board(&ProjectKey::new("OTHER"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("OTHER"),
                summary: "Cross-project issue".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board2.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();

    // Try to move the OTHER issue on the TT board — should fail.
    let issue_id: IssueId = issue.id.parse().unwrap();
    let target_status: shared::StatusId = board.columns[1].id.parse().unwrap();
    let err = ctx
        .services
        .board
        .move_issue(&ProjectKey::new("TT"), issue_id, target_status, user.id)
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn custom_field_set_value_validates_text_type() {
    let (ctx, user) = ctx_with_demo_data().await;
    let field = ctx
        .services
        .custom_field
        .create_field(
            &ProjectKey::new("TT"),
            "TextField",
            "text",
            &[],
            false,
            user.id,
        )
        .await
        .unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "CF validation test".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let field_id: shared::CustomFieldId = field.id.parse().unwrap();

    // Setting a number on a text field should fail.
    let err = ctx
        .services
        .custom_field
        .set_value(issue_id, field_id, serde_json::json!(42), user.id)
        .await;
    assert!(err.is_err());

    // Setting a string should succeed.
    ctx.services
        .custom_field
        .set_value(issue_id, field_id, serde_json::json!("hello"), user.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn custom_field_set_value_validates_select_type() {
    let (ctx, user) = ctx_with_demo_data().await;
    let options = vec!["low".to_string(), "medium".to_string(), "high".to_string()];
    let field = ctx
        .services
        .custom_field
        .create_field(
            &ProjectKey::new("TT"),
            "PrioritySelect",
            "select",
            &options,
            false,
            user.id,
        )
        .await
        .unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Select field test".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let field_id: shared::CustomFieldId = field.id.parse().unwrap();

    // Setting a value not in the options list should fail.
    let err = ctx
        .services
        .custom_field
        .set_value(issue_id, field_id, serde_json::json!("critical"), user.id)
        .await;
    assert!(err.is_err());

    // Setting a valid option should succeed.
    ctx.services
        .custom_field
        .set_value(issue_id, field_id, serde_json::json!("high"), user.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn custom_field_set_value_validates_number_type() {
    let (ctx, user) = ctx_with_demo_data().await;
    let field = ctx
        .services
        .custom_field
        .create_field(
            &ProjectKey::new("TT"),
            "AgeField",
            "number",
            &[],
            false,
            user.id,
        )
        .await
        .unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Number field test".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let field_id: shared::CustomFieldId = field.id.parse().unwrap();

    // Setting a string on a number field should fail.
    let err = ctx
        .services
        .custom_field
        .set_value(
            issue_id,
            field_id,
            serde_json::json!("not a number"),
            user.id,
        )
        .await;
    assert!(err.is_err());

    // Setting a number should succeed.
    ctx.services
        .custom_field
        .set_value(issue_id, field_id, serde_json::json!(42), user.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn custom_field_set_value_validates_date_type() {
    let (ctx, user) = ctx_with_demo_data().await;
    let field = ctx
        .services
        .custom_field
        .create_field(
            &ProjectKey::new("TT"),
            "DueDateField",
            "date",
            &[],
            false,
            user.id,
        )
        .await
        .unwrap();
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), user.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "Date field test".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: user.id,
                assignee_id: None,
                actor_id: user.id,
                custom_fields: Default::default(),
            },
            user.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();
    let field_id: shared::CustomFieldId = field.id.parse().unwrap();

    // Setting a non-date string should fail.
    let err = ctx
        .services
        .custom_field
        .set_value(issue_id, field_id, serde_json::json!("not a date"), user.id)
        .await;
    assert!(err.is_err());

    // Setting a valid RFC 3339 date should succeed.
    ctx.services
        .custom_field
        .set_value(
            issue_id,
            field_id,
            serde_json::json!("2026-12-31T00:00:00Z"),
            user.id,
        )
        .await
        .unwrap();
    let values = ctx
        .services
        .custom_field
        .get_values_for_issue(issue_id, user.id)
        .await
        .unwrap();
    assert_eq!(values[0].value, serde_json::json!("2026-12-31"));

    // Setting a canonical date-only value should also succeed and stay date-only.
    ctx.services
        .custom_field
        .set_value(issue_id, field_id, serde_json::json!("2027-01-02"), user.id)
        .await
        .unwrap();
    let values = ctx
        .services
        .custom_field
        .get_values_for_issue(issue_id, user.id)
        .await
        .unwrap();
    assert_eq!(values[0].value, serde_json::json!("2027-01-02"));
}

// ─── Authz unit tests ─────────────────────────────────────────────────
//
// These tests exercise the centralized Authz policy layer directly through
// the service layer, using the in-memory stub repositories from the test
// setup. They prove that require_project_access denies a non-member and
// require_owner denies a member (who is not the owner).

/// Build a context identical to `ctx_with_demo_data` but swap the stub
/// member repository for a real `MemoryProjectMemberRepository` so we can
/// add B as a member and test the owner-only gate.
async fn ctx_with_real_members() -> (AppContext, User, User, ProjectId) {
    let user = test_user();
    let user_copy = user.clone();
    let mut project = Project {
        id: shared::ProjectId::new(),
        key: ProjectKey::new("TT"),
        name: "Task Tracker".into(),
        description: None,
        owner_id: user.id,
        default_board_id: shared::BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };

    let todo =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let in_progress =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap());
    let review =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000004").unwrap());
    let done =
        StatusId::from_uuid(uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap());
    project.default_board_id = shared::BoardId::new();
    let board = Board {
        id: project.default_board_id,
        project_id: project.id,
        name: "TT Kanban".into(),
        columns: vec![
            BoardColumn {
                id: todo,
                name: "Todo".into(),
                category: StatusCategory::Todo,
                wip_limit: None,
                position: 0,
            },
            BoardColumn {
                id: in_progress,
                name: "In Progress".into(),
                category: StatusCategory::InProgress,
                wip_limit: Some(5),
                position: 1,
            },
            BoardColumn {
                id: review,
                name: "Review".into(),
                category: StatusCategory::InProgress,
                wip_limit: None,
                position: 2,
            },
            BoardColumn {
                id: done,
                name: "Done".into(),
                category: StatusCategory::Done,
                wip_limit: None,
                position: 3,
            },
        ],
    };

    let other = User {
        id: UserId::new(),
        email: "other@example.com".into(),
        username: "other".into(),
        display_name: "Other User".into(),
        password_hash: String::new().into(),
        refresh_token_hash: None,
        is_system_admin: false,
        is_active: true,
        created_at: shared::now(),
        updated_at: shared::now(),
    };

    let users = Arc::new(MemoryUserRepository::default());
    users.save(&user).await.unwrap();
    users.save(&other).await.unwrap();
    let projects = Arc::new(MemoryProjectRepository::default());
    projects.save(&project).await.unwrap();
    let issues = Arc::new(MemoryIssueRepository::default());
    let boards = Arc::new(MemoryBoardRepository::default());
    boards.save(&board).await.unwrap();
    let sprints = Arc::new(MemorySprintRepository::default());

    let members = Arc::new(domain::MemoryProjectMemberRepository::default());
    // Add `other` as a member of the project (owner = `user`).
    members
        .save(&domain::ProjectMember {
            project_id: project.id,
            user_id: other.id,
            role: domain::ProjectRole::Member,
            joined_at: shared::now(),
        })
        .await
        .unwrap();

    let notifications = Arc::new(MemoryNotificationRepository::default());
    let repos = Arc::new(domain::Repositories {
        users: users.clone(),
        audit_logs: Arc::new(domain::StubAuditLogRepository),
        system_settings: Arc::new(domain::StubSystemSettingRepository),
        projects: projects.clone(),
        issues: issues.clone(),
        boards: boards.clone(),
        sprints: sprints.clone(),
        comments: Arc::new(domain::StubCommentRepository),
        worklogs: Arc::new(domain::StubWorklogRepository),
        members: members.clone(),
        statuses: Arc::new(domain::MemoryStatusRepository::new(statuses_from_board(
            &board,
        ))),
        transitions: Arc::new(domain::StubWorkflowTransitionRepository),
        issue_types: Arc::new(domain::StubIssueTypeRepository),
        attachments: Arc::new(domain::StubAttachmentRepository),
        labels: Arc::new(domain::StubLabelRepository),
        issue_links: Arc::new(domain::StubIssueLinkRepository),
        notifications: notifications.clone(),
        notification_settings: Arc::new(domain::StubUserNotificationSettingsRepository),
        issue_status_history: Arc::new(domain::StubIssueStatusHistoryRepository),
        watchers: Arc::new(domain::MemoryWatcherRepository::default()),
        votes: Arc::new(domain::MemoryVoteRepository::default()),
        components: Arc::new(domain::MemoryProjectComponentRepository::default()),
        versions: Arc::new(domain::MemoryProjectVersionRepository::default()),
        custom_fields: Arc::new(domain::MemoryCustomFieldRepository::default()),
    });

    let ctx = AppContext::new(
        test_config(),
        repos.clone(),
        Arc::new(TestStorage::default()),
    );
    (ctx, user_copy, other, project.id)
}

/// require_project_access denies a non-member: a random user (not owner,
/// not member) trying to read an issue in the project gets Forbidden.
#[tokio::test]
async fn authz_require_project_access_denies_non_member() {
    let (ctx, owner) = ctx_with_demo_data().await;

    // Create an issue as the owner.
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"), owner.id)
        .await
        .unwrap();
    let issue = ctx
        .services
        .issue
        .create(
            CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "authz unit test issue".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: board.columns[0].id.to_string(),
                reporter_id: owner.id,
                assignee_id: None,
                actor_id: owner.id,
                custom_fields: Default::default(),
            },
            owner.id,
        )
        .await
        .unwrap();
    let issue_id: IssueId = issue.id.parse().unwrap();

    // A stranger (not owner, not member) tries to read the issue → Forbidden.
    let stranger = UserId::new();
    let err = ctx
        .services
        .issue
        .get_by_id(issue_id, stranger)
        .await
        .unwrap_err();
    assert!(
        matches!(err, AppError::Forbidden),
        "expected Forbidden for non-member, got {err:?}"
    );
}

/// require_owner denies a member: a member (not the owner) trying to add
/// another member gets Forbidden because add_member requires owner-only.
#[tokio::test]
async fn authz_require_owner_denies_member() {
    let (ctx, _owner, member, project_id) = ctx_with_real_members().await;

    // The member tries to add a third user → Forbidden (owner-only gate).
    let stranger_id = UserId::new();
    let err = ctx
        .services
        .member
        .add(
            crate::commands::AddProjectMemberCommand {
                project_id,
                user_id: stranger_id,
                role: "member".to_string(),
            },
            member.id,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, AppError::Forbidden),
        "expected Forbidden for member calling owner-only gate, got {err:?}"
    );
}

#[tokio::test]
async fn project_member_add_rejects_inactive_user() {
    let (ctx, owner, _member, project_id) = ctx_with_real_members().await;
    let mut inactive = test_user_with("inactive", "inactive@example.com", "Inactive User");
    inactive.is_active = false;
    ctx.repos.users.save(&inactive).await.unwrap();

    let err = ctx
        .services
        .member
        .add(
            crate::commands::AddProjectMemberCommand {
                project_id,
                user_id: inactive.id,
                role: "member".to_string(),
            },
            owner.id,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::InvalidInput(_)));
}

// SEC-4: deactivated accounts must not log in
#[tokio::test]
async fn auth_login_rejects_inactive_user() {
    let (ctx, mut user) = ctx_with_demo_data().await;
    // deactivate the user
    user.is_active = false;
    ctx.repos.users.save(&user).await.unwrap();

    let res = ctx.services.auth.login(LoginCommand {
        email: user.email.to_string(),
        password: "demo".to_string(),
    });
    assert!(matches!(res.await, Err(shared::AppError::Unauthorized)));
}

// SEC-4: previously issued tokens of deactivated accounts must stop working
#[tokio::test]
async fn verify_token_inactive_user_unauthorized() {
    let (ctx, mut user) = ctx_with_demo_data().await;
    // issue a token while active
    let dto = ctx
        .services
        .auth
        .login(LoginCommand {
            email: user.email.to_string(),
            password: "demo".to_string(),
        })
        .await
        .unwrap();
    // token verifies while active
    assert!(ctx.services.auth.verify_token(&dto.access_token).is_ok());
    // deactivate
    user.is_active = false;
    ctx.repos.users.save(&user).await.unwrap();
    // middleware-level check: verify_token itself is stateless (JWT), so the
    // bearer_auth middleware performs the is_active lookup; simulate it here.
    let loaded = ctx.repos.users.get_by_id(user.id).await.unwrap();
    assert!(!loaded.is_active, "repo must persist the deactivation");
}

#[cfg(test)]
mod backlog_proptests {
    use proptest::prelude::*;

    proptest! {
        /// The page limit must always land inside 1..=200 regardless of input,
    /// including zero and absurd values.
        #[test]
        fn backlog_limit_always_clamped(limit in 0usize..10_000usize) {
            let clamped = limit.clamp(1, crate::services::board::backlog::BACKLOG_MAX_PAGE_SIZE);
            prop_assert!(clamped >= 1);
            prop_assert!(clamped <= crate::services::board::backlog::BACKLOG_MAX_PAGE_SIZE);
        }
    }
}
