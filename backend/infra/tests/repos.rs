use domain::{
    Board, BoardColumn, ColumnCategory, Issue, Project, ProjectQuery, Sprint, SprintState, User,
};
use infra::repos::{SeaOrmRepositories, to_domain_repositories};
use migration::MigratorTrait;
use sea_orm::{ConnectionTrait, Database};
use shared::{
    BoardId, IssueType, Priority, ProjectId, ProjectKey, SprintId, StatusId, UserId, now,
};
use uuid::Uuid;

async fn setup() -> domain::Repositories {
    let base_url = std::fs::read_to_string("/root/.tt_db_url")
        .expect("read /root/.tt_db_url")
        .trim()
        .to_string();
    // Replace database name with isolated infra test DB
    let db_url = format!(
        "{}/{}",
        base_url
            .rsplit_once('/')
            .map(|(h, _)| h)
            .unwrap_or(&base_url),
        "tasktracker_infra_test"
    );
    let db = Database::connect(&db_url)
        .await
        .expect("connect to test db");
    migration::Migrator::up(&db, None)
        .await
        .expect("run migrations");
    let _ = db
        .execute_unprepared("TRUNCATE TABLE users, projects, issues, boards, sprints, labels, project_members CASCADE")
        .await;
    to_domain_repositories(SeaOrmRepositories::new(db))
}

fn test_user() -> User {
    let suffix = Uuid::new_v4().to_string();
    User {
        id: UserId::new(),
        email: format!("repo-test-{}@example.com", suffix).into(),
        username: format!("repotest-{}", &suffix[..8]).into(),
        display_name: "Repo Test".into(),
        password_hash: "$argon2id$v=19$m=65536,t=3,p=4$stN/enhZ9yOvgWC9E8Y6BA$IL9I0WONb/I6zoT4rdmdkrPcIFADFxsLCjrO0ySSl0Y".into(),
        created_at: now(),
        updated_at: now(),
    }
}

fn test_project(owner_id: UserId) -> Project {
    let suffix = Uuid::new_v4().to_string();
    Project {
        id: ProjectId::new(),
        key: ProjectKey::new(format!("REPO{}", &suffix[..6].to_uppercase()).as_str()),
        name: "Repo Test Project".into(),
        description: Some("for infra tests".into()),
        owner_id,
        default_board_id: BoardId::new(),
        created_at: now(),
        updated_at: now(),
    }
}

fn test_board(project_id: ProjectId, board_id: BoardId) -> Board {
    let todo =
        StatusId::from_uuid(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    Board {
        id: board_id,
        project_id,
        name: "Test Board".into(),
        columns: vec![BoardColumn {
            id: todo,
            name: "Todo".into(),
            category: ColumnCategory::Todo,
            wip_limit: None,
            position: 0,
        }],
    }
}

fn test_sprint(project_id: ProjectId) -> Sprint {
    Sprint {
        id: SprintId::new(),
        project_id,
        name: "Sprint 1".into(),
        goal: Some("test goal".into()),
        state: SprintState::Active,
        start_date: Some(now()),
        end_date: Some(now()),
        velocity: Some(10),
    }
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn user_repo_crud() {
    let repos = setup().await;

    let user = test_user();
    repos.users.save(&user).await.expect("save user");

    let found = repos.users.get_by_id(user.id).await.expect("get by id");
    assert_eq!(found.email, user.email);

    let found_email = repos
        .users
        .get_by_email(user.email.as_ref())
        .await
        .expect("get by email");
    assert_eq!(found_email.id, user.id);

    let missing = repos.users.get_by_id(UserId::new()).await;
    assert!(missing.is_err());
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn project_repo_queries() {
    let repos = setup().await;
    let user = test_user();
    repos.users.save(&user).await.unwrap();
    let project = test_project(user.id);
    repos.projects.save(&project).await.unwrap();

    let found = repos.projects.get_by_id(project.id).await.unwrap();
    assert!(found.key.as_str().starts_with("REPO"));

    let found_key = repos.projects.get_by_key(&project.key).await.unwrap();
    assert!(found_key.key.as_str().starts_with("REPO"));

    let list = repos.projects.list(ProjectQuery::default()).await.unwrap();
    assert!(!list.is_empty());

    let updated = repos.projects.get_by_id(project.id).await.unwrap();
    assert_eq!(updated.name, project.name);

    let next = repos.projects.next_issue_number(project.id).await.unwrap();
    assert_eq!(next, 1);
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn issue_repo_crud_and_query() {
    let repos = setup().await;
    let user = test_user();
    repos.users.save(&user).await.unwrap();
    let project = test_project(user.id);
    repos.projects.save(&project).await.unwrap();

    let status =
        StatusId::from_uuid(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let issue = Issue::create(
        &project,
        1,
        IssueType::Task,
        status,
        "test issue",
        None,
        user.id,
        Priority::Medium,
    );

    repos.issues.save(&issue).await.unwrap();

    let found = repos.issues.get_by_id(issue.id).await.unwrap();
    assert_eq!(found.summary, issue.summary);

    let found_key = repos.issues.get_by_key(&issue.key).await.unwrap();
    assert_eq!(found_key.id, issue.id);

    let list = repos
        .issues
        .list(domain::IssueQuery {
            project_id: Some(project.id),
            status_id: Some(status),
            assignee_id: None,
            sprint_id: None,
            search_text: None,
            limit: 10,
            offset: 0,
        })
        .await
        .unwrap();
    assert_eq!(list.len(), 1);

    repos.issues.delete(issue.id).await.unwrap();
    let missing = repos.issues.get_by_id(issue.id).await;
    assert!(missing.is_err());
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn board_repo_queries() {
    let repos = setup().await;
    let user = test_user();
    repos.users.save(&user).await.unwrap();
    let project = test_project(user.id);
    repos.projects.save(&project).await.unwrap();
    let board = test_board(project.id, project.default_board_id);
    repos.boards.save(&board).await.unwrap();

    let found = repos.boards.get_by_id(board.id).await.unwrap();
    assert_eq!(found.name, "Test Board".into());

    let found_project = repos
        .boards
        .get_default_by_project(project.id)
        .await
        .unwrap();
    assert_eq!(found_project.id, board.id);

    let found_key = repos
        .boards
        .get_default_by_project_key(&project.key)
        .await
        .unwrap();
    assert_eq!(found_key.id, board.id);
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn sprint_repo_queries() {
    let repos = setup().await;
    let user = test_user();
    repos.users.save(&user).await.unwrap();
    let project = test_project(user.id);
    repos.projects.save(&project).await.unwrap();
    let sprint = test_sprint(project.id);
    repos.sprints.save(&sprint).await.unwrap();

    let active = repos
        .sprints
        .get_active_by_project(project.id)
        .await
        .unwrap();
    assert!(active.is_some());

    let found = repos.sprints.get_by_id(sprint.id).await.unwrap();
    assert_eq!(found.name, "Sprint 1".into());
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn repo_missing_entities_return_not_found() {
    let repos = setup().await;
    let missing_user = repos.users.get_by_id(UserId::new()).await;
    assert!(missing_user.is_err());
    let missing_project = repos.projects.get_by_id(ProjectId::new()).await;
    assert!(missing_project.is_err());
    let missing_issue = repos.issues.get_by_id(shared::IssueId::new()).await;
    assert!(missing_issue.is_err());
}
