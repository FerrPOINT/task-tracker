use infra::repos::SeaOrmRepositories;
use sea_orm::{DatabaseBackend, DbErr, MockDatabase, RuntimeErr};
use shared::{IssueId, ProjectId, ProjectKey, SprintId, UserId};
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

#[tokio::test]
async fn user_get_by_id_database_error() {
    let repos = mock_db_with_query_error();
    let err = repos.users.get_by_id(UserId::new()).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn user_get_by_email_database_error() {
    let repos = mock_db_with_query_error();
    let err = repos.users.get_by_email("x@example.com").await;
    assert!(err.is_err());
}

#[tokio::test]
async fn project_get_by_id_database_error() {
    let repos = mock_db_with_query_error();
    let err = repos.projects.get_by_id(ProjectId::new()).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn project_get_by_key_database_error() {
    let repos = mock_db_with_query_error();
    let err = repos.projects.get_by_key(&ProjectKey::new("TT")).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn issue_get_by_id_database_error() {
    let repos = mock_db_with_query_error();
    let err = repos.issues.get_by_id(IssueId::new()).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn board_get_by_id_database_error() {
    let repos = mock_db_with_query_error();
    let err = repos.boards.get_by_id(shared::BoardId::new()).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn sprint_get_by_id_database_error() {
    let repos = mock_db_with_query_error();
    let err = repos.sprints.get_by_id(SprintId::new()).await;
    assert!(err.is_err());
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
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    let err = repos.users.save(&user).await;
    assert!(err.is_err());
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
    let err = repos.projects.save(&project).await;
    assert!(err.is_err());
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
    let err = repos.issues.save(&issue).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn board_get_default_database_error() {
    let repos = mock_db_with_query_error();
    let err = repos
        .boards
        .get_default_by_project_key(&ProjectKey::new("TT"))
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn sprint_get_active_database_error() {
    let repos = mock_db_with_query_error();
    let err = repos.sprints.get_active_by_project(ProjectId::new()).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn project_next_issue_number_database_error() {
    let repos = mock_db_with_query_error();
    let err = repos.projects.next_issue_number(ProjectId::new()).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn issue_list_database_error() {
    let repos = mock_db_with_query_error();
    let err = repos.issues.list(domain::IssueQuery::default()).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn project_list_database_error() {
    let repos = mock_db_with_query_error();
    let err = repos.projects.list(domain::ProjectQuery::default()).await;
    assert!(err.is_err());
}
