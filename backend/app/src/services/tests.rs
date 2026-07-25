#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use domain::{
        Board, BoardColumn, BoardRepository, ColumnCategory, Issue, IssueQuery, IssueRepository,
        MemoryBoardRepository, MemoryIssueRepository, MemoryProjectRepository,
        MemorySprintRepository, MemoryUserRepository, Project, ProjectQuery, ProjectRepository,
        Sprint, SprintRepository, User, UserRepository,
    };
    use shared::{
        AppConfig, AppError, AuthConfig, DatabaseConfig, IssueId, IssueKey, IssueType, Priority,
        ProjectId, ProjectKey, ServerConfig, SprintId, StatusId, UserId,
    };

    use crate::commands::{
        CreateIssueCommand, CreateProjectCommand, LoginCommand, RegisterCommand, UpdateIssueCommand,
    };
    use crate::context::AppContext;

    fn test_user() -> User {
        User {
            id: UserId::new(),
            email: "demo@example.com".into(),
            username: "demo".into(),
            display_name: "Demo User".into(),
            password_hash: "$argon2id$v=19$m=65536,t=3,p=4$stN/enhZ9yOvgWC9E8Y6BA$IL9I0WONb/I6zoT4rdmdkrPcIFADFxsLCjrO0ySSl0Y".into(),
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
            },
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

        let todo = StatusId::from_uuid(
            uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
        );
        let in_progress = StatusId::from_uuid(
            uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
        );
        let review = StatusId::from_uuid(
            uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000004").unwrap(),
        );
        let done = StatusId::from_uuid(
            uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap(),
        );
        project.default_board_id = shared::BoardId::new();
        let board = Board {
            id: project.default_board_id,
            project_id: project.id,
            name: "TT Kanban".into(),
            columns: vec![
                BoardColumn {
                    id: todo,
                    name: "Todo".into(),
                    category: ColumnCategory::Todo,
                    wip_limit: None,
                    position: 0,
                },
                BoardColumn {
                    id: in_progress,
                    name: "In Progress".into(),
                    category: ColumnCategory::InProgress,
                    wip_limit: Some(5),
                    position: 1,
                },
                BoardColumn {
                    id: review,
                    name: "Review".into(),
                    category: ColumnCategory::InProgress,
                    wip_limit: None,
                    position: 2,
                },
                BoardColumn {
                    id: done,
                    name: "Done".into(),
                    category: ColumnCategory::Done,
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
        });
        AppContext::new(test_config(), repos.clone());
        (AppContext::new(test_config(), repos.clone()), user_copy)
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

        assert!(!dto.token.is_empty());
        let claims = ctx.services.auth.verify_token(&dto.token).unwrap();
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

        let results = ctx.services.search.search("keyword").await.unwrap();
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

        let err = ctx.services.search.search("orphan keyword").await;
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
            async fn save(&self, _user: &User) -> Result<UserId, AppError> {
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
            async fn get_default_by_project(
                &self,
                _project_id: ProjectId,
            ) -> Result<Board, AppError> {
                Err(AppError::Internal("x".into()))
            }
            async fn get_default_by_project_key(
                &self,
                _key: &ProjectKey,
            ) -> Result<Board, AppError> {
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
        }

        let repos = Arc::new(domain::Repositories {
            users: Arc::new(FailingUserRepository),
            projects: Arc::new(FailingProjectRepository),
            issues: Arc::new(FailingIssueRepository),
            boards: Arc::new(FailingBoardRepository),
            sprints: Arc::new(FailingSprintRepository),
        });
        AppContext::new(test_config(), repos)
    }

    #[tokio::test]
    async fn project_create_propagates_repo_error() {
        let ctx = failing_context();
        let err = ctx
            .services
            .project
            .create(CreateProjectCommand {
                key: ProjectKey::new("NP"),
                name: "New".to_string(),
                description: None,
                owner_id: UserId::new(),
            })
            .await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn issue_create_propagates_repo_error() {
        let ctx = failing_context();
        let err = ctx
            .services
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
            .await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn board_get_propagates_repo_error() {
        let ctx = failing_context();
        let err = ctx.services.board.get_board(&ProjectKey::new("TT")).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn dashboard_get_propagates_repo_error() {
        let ctx = failing_context();
        let err = ctx.services.dashboard.get_dashboard(UserId::new()).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn search_propagates_repo_error() {
        let ctx = failing_context();
        let err = ctx.services.search.search("q").await;
        assert!(err.is_err());
    }
}
