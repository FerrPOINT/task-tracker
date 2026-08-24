use std::sync::Arc;

type TestStorage = domain::InMemoryStorage;
use domain::{
    Board, BoardColumn, BoardRepository, Issue, IssueQuery, IssueRepository, MemoryBoardRepository,
    MemoryIssueRepository, MemoryNotificationRepository, MemoryProjectRepository,
    MemorySprintRepository, MemoryUserRepository, Notification, NotificationRepository, Project,
    ProjectQuery, ProjectRepository, Sprint, SprintRepository, StatusCategory, User,
    UserNotificationSettingsRepository, UserRepository,
};
use shared::{
    AppConfig, AppError, AuthConfig, DatabaseConfig, IssueId, IssueKey, IssueType, NotificationId,
    Priority, ProjectId, ProjectKey, ServerConfig, SprintId, StatusId, UserId,
};

use crate::commands::{
    CreateIssueCommand, CreateProjectCommand, LoginCommand, RegisterCommand, UpdateIssueCommand,
    UpdateNotificationSettingsCommand,
};
use crate::context::{AppContext, NotificationService};
use crate::services::NotificationServiceImpl;

fn test_user() -> User {
    User {
        id: UserId::new(),
        email: "demo@example.com".into(),
        username: "demo".into(),
        display_name: "Demo User".into(),
        password_hash: "$argon2id$v=19$m=65536,t=3,p=4$stN/enhZ9yOvgWC9E8Y6BA$IL9I0WONb/I6zoT4rdmdkrPcIFADFxsLCjrO0ySSl0Y".into(),
        refresh_token_hash: None,
        created_at: shared::now(),
        updated_at: shared::now(),
    }
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
    })
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
    let issues = Arc::new(MemoryIssueRepository::default());
    let boards = Arc::new(MemoryBoardRepository::default());
    boards.save(&board).await.unwrap();
    let sprints = Arc::new(MemorySprintRepository::default());

    let repos = Arc::new(domain::Repositories {
        users: users.clone(),
        projects: projects.clone(),
        issues: issues.clone(),
        boards: boards.clone(),
        sprints: sprints.clone(),
        comments: Arc::new(domain::StubCommentRepository),
        worklogs: Arc::new(domain::StubWorklogRepository),
        members: Arc::new(domain::StubProjectMemberRepository),
        statuses: Arc::new(domain::StubStatusRepository),
        transitions: Arc::new(domain::StubWorkflowTransitionRepository),
        issue_types: Arc::new(domain::StubIssueTypeRepository),
        attachments: Arc::new(domain::StubAttachmentRepository),
        labels: Arc::new(domain::StubLabelRepository),
        issue_links: Arc::new(domain::StubIssueLinkRepository),
        saved_filters: Arc::new(domain::StubSavedFilterRepository),
        notifications: Arc::new(domain::StubNotificationRepository),
        notification_settings: Arc::new(domain::StubUserNotificationSettingsRepository),
        issue_status_history: Arc::new(domain::StubIssueStatusHistoryRepository),
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
async fn auth_expired_token_fails_verification() {
    let (ctx, _user) = ctx_with_demo_data().await;
    let expired = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &crate::auth::UserClaims {
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
        .get_board(&ProjectKey::new("TT"))
        .await
        .unwrap();
    let status_id = board.columns[0].id.to_string();

    let issue = ctx
        .services
        .issue
        .create(CreateIssueCommand {
            project_key: ProjectKey::new("TT"),
            summary: "Test issue".to_string(),
            description: None,
            issue_type: IssueType::Task,
            priority: Priority::Medium,
            status_id,
            reporter_id: user.id,
            assignee_id: None,
        })
        .await
        .unwrap();

    assert_eq!(issue.project_key, "TT");
    assert_eq!(issue.summary, "Test issue");
    assert!(!issue.key.is_empty());
}

#[tokio::test]
async fn issue_service_update_and_move() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"))
        .await
        .unwrap();
    let todo_id = board.columns[0].id.to_string();
    let in_progress_id = board.columns[1].id.to_string();
    let project_key = ProjectKey::new("TT");

    let created = ctx
        .services
        .issue
        .create(CreateIssueCommand {
            project_key: project_key.clone(),
            summary: "Move me".to_string(),
            description: None,
            issue_type: IssueType::Task,
            priority: Priority::Low,
            status_id: todo_id,
            reporter_id: user.id,
            assignee_id: None,
        })
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
            },
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
async fn dashboard_lists_assigned_issues() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"))
        .await
        .unwrap();
    let status_id = board.columns[0].id.to_string();
    ctx.services
        .issue
        .create(CreateIssueCommand {
            project_key: ProjectKey::new("TT"),
            summary: "Assigned task".to_string(),
            description: None,
            issue_type: IssueType::Task,
            priority: Priority::Medium,
            status_id,
            reporter_id: user.id,
            assignee_id: Some(user.id),
        })
        .await
        .unwrap();

    let dashboard = ctx.services.dashboard.get_dashboard(user.id).await.unwrap();
    assert_eq!(dashboard.assigned_issues.len(), 1);
}

#[tokio::test]
async fn search_finds_issue() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"))
        .await
        .unwrap();
    let status_id = board.columns[0].id.to_string();
    ctx.services
        .issue
        .create(CreateIssueCommand {
            project_key: ProjectKey::new("TT"),
            summary: "Searchable keyword".to_string(),
            description: None,
            issue_type: IssueType::Task,
            priority: Priority::Medium,
            status_id,
            reporter_id: user.id,
            assignee_id: None,
        })
        .await
        .unwrap();

    let results = ctx
        .services
        .search
        .search(Default::default())
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
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
        .list(crate::commands::ProjectQueryDto::default())
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
    let by_key = ctx
        .services
        .project
        .get_by_key(&ProjectKey::new("NP"))
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
        .list(crate::commands::ProjectQueryDto::default())
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].key, "TT");
    let by_key = ctx
        .services
        .project
        .get_by_key(&ProjectKey::new("TT"))
        .await
        .unwrap();
    assert_eq!(by_key.key, "TT");
}

#[tokio::test]
async fn board_service_backlog() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"))
        .await
        .unwrap();
    let status_id = board.columns[0].id.to_string();
    ctx.services
        .issue
        .create(CreateIssueCommand {
            project_key: ProjectKey::new("TT"),
            summary: "Backlog item".to_string(),
            description: None,
            issue_type: IssueType::Task,
            priority: Priority::Medium,
            status_id,
            reporter_id: user.id,
            assignee_id: None,
        })
        .await
        .unwrap();
    let backlog = ctx
        .services
        .board
        .get_backlog(&ProjectKey::new("TT"))
        .await
        .unwrap();
    assert_eq!(backlog.backlog_issues.len(), 1);
    assert_eq!(backlog.backlog_issues[0].summary, "Backlog item");
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
        .create(CreateIssueCommand {
            project_key: ProjectKey::new("ZZ"),
            summary: "orphan".to_string(),
            description: None,
            issue_type: IssueType::Task,
            priority: Priority::Medium,
            status_id: "00000000-0000-0000-0000-000000000001".to_string(),
            reporter_id: user.id,
            assignee_id: None,
        })
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn issue_service_create_fails_for_invalid_status_id() {
    let (ctx, user) = ctx_with_demo_data().await;
    let err = ctx
        .services
        .issue
        .create(CreateIssueCommand {
            project_key: ProjectKey::new("TT"),
            summary: "bad status".to_string(),
            description: None,
            issue_type: IssueType::Task,
            priority: Priority::Medium,
            status_id: "not-a-uuid".to_string(),
            reporter_id: user.id,
            assignee_id: None,
        })
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn issue_service_update_fails_for_invalid_status_id() {
    let (ctx, user) = ctx_with_demo_data().await;
    let board = ctx
        .services
        .board
        .get_board(&ProjectKey::new("TT"))
        .await
        .unwrap();
    let created = ctx
        .services
        .issue
        .create(CreateIssueCommand {
            project_key: ProjectKey::new("TT"),
            summary: "Update me".to_string(),
            description: None,
            issue_type: IssueType::Task,
            priority: Priority::Low,
            status_id: board.columns[0].id.to_string(),
            reporter_id: user.id,
            assignee_id: None,
        })
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
            },
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
            },
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
    let err = ctx.services.issue.get_by_id(shared::IssueId::new()).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn dashboard_get_fails_when_project_missing() {
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

    let err = ctx.services.dashboard.get_dashboard(user.id).await;
    assert!(err.is_err());
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

    let err = ctx.services.search.search(Default::default()).await;
    assert!(err.is_err());
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

    let err = ctx.services.issue.get_by_id(issue.id).await;
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
        async fn get_by_id(&self, _id: IssueId) -> Result<Issue, AppError> {
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
    }

    #[derive(Default)]
    struct FailingUserRepository;
    #[async_trait::async_trait]
    impl UserRepository for FailingUserRepository {
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
        saved_filters: Arc::new(domain::StubSavedFilterRepository),
        notifications: Arc::new(domain::StubNotificationRepository),
        notification_settings: Arc::new(domain::StubUserNotificationSettingsRepository),
        issue_status_history: Arc::new(domain::StubIssueStatusHistoryRepository),
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
            .create(CreateIssueCommand {
                project_key: ProjectKey::new("TT"),
                summary: "x".to_string(),
                description: None,
                issue_type: IssueType::Task,
                priority: Priority::Medium,
                status_id: "00000000-0000-0000-0000-000000000001".to_string(),
                reporter_id: UserId::new(),
                assignee_id: None,
            })
            .await,
    );
}

#[tokio::test]
async fn board_get_propagates_repo_error() {
    let ctx = failing_context();
    assert_internal(ctx.services.board.get_board(&ProjectKey::new("TT")).await);
}

#[tokio::test]
async fn dashboard_get_propagates_repo_error() {
    let ctx = failing_context();
    assert_internal(ctx.services.dashboard.get_dashboard(UserId::new()).await);
}

#[tokio::test]
async fn search_propagates_repo_error() {
    let ctx = failing_context();
    assert_internal(ctx.services.search.search(Default::default()).await);
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
// ─── Report service tests ───────────────────────────────────────────

use crate::context::ReportService;
use crate::services::ReportServiceImpl;
use domain::{
    IssueStatusHistory, MemoryIssueStatusHistoryRepository, MemoryStatusRepository, Status,
};

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
        position: 0.0,
        due_date: None,
        original_estimate_seconds: None,
        remaining_estimate_seconds: None,
        time_spent_seconds: 0,
        created_at,
        updated_at: created_at,
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

    (
        issue_repo,
        sprint_repo,
        status_repo,
        history_repo,
        project_id,
        todo,
        in_progress,
        done,
    )
}

#[tokio::test]
async fn report_velocity_counts_committed_vs_completed() {
    let (issues, sprints, statuses, _history, project_id, todo, _ip, done) = report_test_setup();

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

    let service = ReportServiceImpl::new(
        issues.clone(),
        sprints.clone(),
        statuses.clone(),
        Arc::new(domain::StubIssueStatusHistoryRepository),
    );
    let result = service.get_velocity(project_id, 6).await.unwrap();
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
async fn report_burndown_computes_remaining_per_day() {
    let (issues, sprints, _statuses, _history, project_id, _todo, _ip, _done) = report_test_setup();

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

    let service = ReportServiceImpl::new(
        issues.clone(),
        sprints.clone(),
        Arc::new(domain::StubStatusRepository),
        Arc::new(domain::StubIssueStatusHistoryRepository),
    );
    let result = service.get_burndown(sprint.id).await.unwrap();
    assert_eq!(result.sprint_name, "Active Sprint");
    // Should have at least 3 days (start, start+1, today)
    assert!(!result.points.is_empty());
    // First point = 5 (all committed)
    assert_eq!(result.points[0].remaining, 5);
}

#[tokio::test]
async fn report_cumulative_flow_snapshots_status_categories() {
    let (issues, _sprints, statuses, history, project_id, todo, in_progress, done) =
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

    let service = ReportServiceImpl::new(
        issues.clone(),
        Arc::new(domain::StubSprintRepository),
        statuses.clone(),
        history.clone(),
    );
    let result = service.get_cumulative_flow(project_id).await.unwrap();
    assert!(!result.is_empty());
    // After the last transition, done should be 1, todo and in_progress 0
    let last = result.last().unwrap();
    assert_eq!(last.done, 1);
    assert_eq!(last.todo, 0);
    assert_eq!(last.in_progress, 0);
}

#[tokio::test]
async fn report_control_chart_computes_cycle_time() {
    let (issues, _sprints, statuses, history, project_id, todo, _ip, done) = report_test_setup();

    let created = shared::now() - chrono::Duration::days(5);
    let done_time = shared::now() - chrono::Duration::days(1);

    let issue = make_issue(
        "ffffffff-0000-0000-0000-000000000001",
        project_id,
        1,
        done,
        None,
        created,
    );
    issues.save(&issue).await.unwrap();

    // History: created -> todo, then todo -> done after 4 days
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
        &make_history(
            "22222222-0000-0000-0000-000000000002",
            issue.id,
            done,
            done_time,
        ),
        project_id,
    );

    let service = ReportServiceImpl::new(
        issues.clone(),
        Arc::new(domain::StubSprintRepository),
        statuses.clone(),
        history.clone(),
    );
    let result = service.get_control_chart(project_id).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].issue_key, issue.key.to_string());
    // 4 days cycle time (5 - 1 = 4)
    assert!((result[0].cycle_time_days - 4.0).abs() < 0.1);
}

#[tokio::test]
async fn report_control_chart_skips_issues_without_done_transition() {
    let (issues, _sprints, statuses, history, project_id, todo, _ip, _done) = report_test_setup();

    let issue = make_issue(
        "33333333-0000-0000-0000-000000000001",
        project_id,
        1,
        todo,
        None,
        shared::now() - chrono::Duration::days(5),
    );
    issues.save(&issue).await.unwrap();

    let service = ReportServiceImpl::new(
        issues.clone(),
        Arc::new(domain::StubSprintRepository),
        statuses.clone(),
        history.clone(),
    );
    let result = service.get_control_chart(project_id).await.unwrap();
    // No done transition → not included
    assert!(result.is_empty());
}
