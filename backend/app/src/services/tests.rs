#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use domain::{
        Board, BoardColumn, BoardRepository, ColumnCategory, IssueRepository,
        MemoryBoardRepository, MemoryIssueRepository, MemoryProjectRepository,
        MemorySprintRepository, MemoryUserRepository, Project, ProjectRepository, SprintRepository,
        User, UserRepository,
    };
    use shared::{
        AppConfig, AuthConfig, DatabaseConfig, IssueType, Priority, ProjectKey, ServerConfig,
        StatusId, UserId,
    };

    use crate::commands::{CreateIssueCommand, LoginCommand, RegisterCommand, UpdateIssueCommand};
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
    async fn auth_duplicate_registration_fails() {
        let (ctx, _user) = ctx_with_demo_data().await;
        let first = ctx
            .services
            .auth
            .register(RegisterCommand {
                email: "dup@example.com".to_string(),
                username: "dup".to_string(),
                name: "Dup".to_string(),
                password: "secret123".to_string(),
            })
            .await;
        assert!(first.is_ok());
        let second = ctx
            .services
            .auth
            .register(RegisterCommand {
                email: "dup@example.com".to_string(),
                username: "dup2".to_string(),
                name: "Dup2".to_string(),
                password: "secret123".to_string(),
            })
            .await;
        assert!(second.is_err());
    }
}
