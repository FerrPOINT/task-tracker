use infra::repos::SeaOrmRepositories;
use sea_orm::{DatabaseBackend, DbErr, MockDatabase, RuntimeErr};
use shared::{AppError, IssueId, ProjectId, ProjectKey, SprintId, UserId};
use uuid::Uuid;

fn mock_db_with_query_error() -> SeaOrmRepositories {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_errors([DbErr::Conn(RuntimeErr::Internal("mock".to_string()))])
        .into_connection();
    SeaOrmRepositories::new(db)
}

fn mock_db_with_exec_error() -> SeaOrmRepositories {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_exec_errors([DbErr::Conn(RuntimeErr::Internal("mock".to_string()))])
        .into_connection();
    SeaOrmRepositories::new(db)
}

fn assert_database_error(err: Result<impl std::fmt::Debug, AppError>) {
    match err {
        Err(AppError::Database(msg)) => {
            assert!(msg.contains("mock") || msg.contains("Query Error"))
        }
        other => panic!("expected AppError::Database, got {:?}", other),
    }
}

#[tokio::test]
async fn user_get_by_id_database_error() {
    let repos = mock_db_with_query_error();
    assert_database_error(repos.users.get_by_id(UserId::new()).await);
}

#[tokio::test]
async fn user_get_by_email_database_error() {
    let repos = mock_db_with_query_error();
    assert_database_error(repos.users.get_by_email("x@example.com").await);
}

#[tokio::test]
async fn project_get_by_id_database_error() {
    let repos = mock_db_with_query_error();
    assert_database_error(repos.projects.get_by_id(ProjectId::new()).await);
}

#[tokio::test]
async fn project_get_by_key_database_error() {
    let repos = mock_db_with_query_error();
    assert_database_error(repos.projects.get_by_key(&ProjectKey::new("TT")).await);
}

#[tokio::test]
async fn issue_get_by_id_database_error() {
    let repos = mock_db_with_query_error();
    assert_database_error(repos.issues.get_by_id(IssueId::new()).await);
}

#[tokio::test]
async fn board_get_by_id_database_error() {
    let repos = mock_db_with_query_error();
    assert_database_error(repos.boards.get_by_id(shared::BoardId::new()).await);
}

#[tokio::test]
async fn sprint_get_by_id_database_error() {
    let repos = mock_db_with_query_error();
    assert_database_error(repos.sprints.get_by_id(SprintId::new()).await);
}

#[tokio::test]
async fn user_save_database_error() {
    let repos = mock_db_with_exec_error();
    let user = domain::User {
        id: UserId::new(),
        username: "x".into(),
        email: "x@example.com".into(),
        display_name: "X".into(),
        password_hash: "h".into(),
        refresh_token_hash: None,
        is_system_admin: false,
        is_active: true,
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    assert_database_error(repos.users.save(&user).await);
}

#[tokio::test]
async fn project_save_database_error() {
    let repos = mock_db_with_exec_error();
    let project = domain::Project {
        id: ProjectId::new(),
        key: ProjectKey::new("TT"),
        name: "Test".into(),
        description: None,
        owner_id: UserId::new(),
        default_board_id: shared::BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    assert_database_error(repos.projects.save(&project).await);
}

#[tokio::test]
async fn issue_save_database_error() {
    let repos = mock_db_with_exec_error();
    let project = domain::Project {
        id: ProjectId::new(),
        key: ProjectKey::new("TT"),
        name: "Test".into(),
        description: None,
        owner_id: UserId::new(),
        default_board_id: shared::BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    let issue = domain::Issue::create(
        &project,
        1,
        shared::IssueType::Task,
        shared::StatusId::from_uuid(Uuid::nil()),
        "Summary".to_string(),
        None,
        UserId::new(),
        shared::Priority::Medium,
    );
    assert_database_error(repos.issues.save(&issue).await);
}

#[tokio::test]
async fn board_get_default_database_error() {
    let repos = mock_db_with_query_error();
    assert_database_error(
        repos
            .boards
            .get_default_by_project_key(&ProjectKey::new("TT"))
            .await,
    );
}

#[tokio::test]
async fn sprint_get_active_database_error() {
    let repos = mock_db_with_query_error();
    assert_database_error(repos.sprints.get_active_by_project(ProjectId::new()).await);
}

#[tokio::test]
async fn project_next_issue_number_database_error() {
    let repos = mock_db_with_query_error();
    assert_database_error(repos.projects.next_issue_number(ProjectId::new()).await);
}

#[tokio::test]
async fn issue_list_database_error() {
    let repos = mock_db_with_query_error();
    assert_database_error(repos.issues.list(domain::IssueQuery::default()).await);
}

#[tokio::test]
async fn project_list_database_error() {
    let repos = mock_db_with_query_error();
    assert_database_error(repos.projects.list(domain::ProjectQuery::default()).await);
}

#[tokio::test]
async fn audit_log_list_database_error() {
    let repos = mock_db_with_query_error();
    assert_database_error(repos.audit_logs.list(None, 10, 0).await);
}

#[tokio::test]
async fn system_setting_get_database_error() {
    let repos = mock_db_with_query_error();
    assert_database_error(repos.system_settings.get("auth.session_ttl").await);
}
