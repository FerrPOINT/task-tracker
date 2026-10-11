use domain::{
    Board, BoardColumn, Issue, Notification, Project, ProjectQuery, Sprint, SprintState,
    StatusCategory, User,
};
use infra::repos::{SeaOrmRepositories, to_domain_repositories};
use migration::MigratorTrait;
use sea_orm::{ConnectionTrait, Database};
use shared::{
    BoardId, IssueType, Priority, ProjectId, ProjectKey, SprintId, StatusId, UserId, now,
};
use uuid::Uuid;

fn base_db_url() -> String {
    std::env::var("TT_TEST_DATABASE_URL").unwrap_or_else(|_| {
        let path = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".tt_db_url");

        std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
            .trim()
            .to_string()
    })
}

fn infra_test_db_url() -> String {
    let base_url = base_db_url();
    format!(
        "{}/tasktracker_infra_test",
        base_url
            .rsplit_once('/')
            .map(|(host, _)| host)
            .unwrap_or(&base_url)
    )
}

async fn setup() -> domain::Repositories {
    let db = Database::connect(infra_test_db_url())
        .await
        .expect("connect to test db");
    migration::Migrator::up(&db, None)
        .await
        .expect("run migrations");
    let _ = db
        .execute_unprepared("TRUNCATE TABLE users, projects, issues, boards, sprints, labels, project_members, comments, worklogs CASCADE")
        .await;
    to_domain_repositories(SeaOrmRepositories::new(db))
}

fn test_user() -> User {
    let suffix = Uuid::new_v4().to_string();
    User {
        id: UserId::new(),
        email: format!("repo-test-{suffix}@example.com").into(),
        username: format!("repotest-{}", &suffix[..8]).into(),
        display_name: "Repo Test".into(),
        password_hash: "$argon2id$v=19$m=65536,t=3,p=4$stN/enhZ9yOvgWC9E8Y6BA$IL9I0WONb/I6zoT4rdmdkrPcIFADFxsLCjrO0ySSl0Y".into(),
        refresh_token_hash: None,
        is_system_admin: false,
        is_active: true,
        created_at: now(),
        updated_at: now(),
    }
}

fn test_project(owner_id: UserId) -> Project {
    let suffix = Uuid::new_v4().to_string();
    Project {
        id: ProjectId::new(),
        key: ProjectKey::new(format!("REPO{}", suffix[..6].to_uppercase()).as_str()),
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
            category: StatusCategory::Todo,
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
#[ignore = "requires isolated PostgreSQL test database"]
async fn deleted_issue_key_can_be_resolved_for_restore_only() {
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
        "Restore fixture",
        None,
        user.id,
        Priority::Medium,
    );
    repos.issues.save(&issue).await.unwrap();
    assert_eq!(
        repos
            .issues
            .get_by_key_include_deleted(&issue.key)
            .await
            .unwrap()
            .id,
        issue.id
    );
    repos.issues.delete(issue.id).await.unwrap();
    assert!(repos.issues.get_by_key(&issue.key).await.is_err());
    assert_eq!(
        repos
            .issues
            .get_by_key_include_deleted(&issue.key)
            .await
            .unwrap()
            .id,
        issue.id
    );
    repos.issues.restore(issue.id).await.unwrap();
    assert_eq!(
        repos.issues.get_by_key(&issue.key).await.unwrap().id,
        issue.id
    );
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
async fn project_delete_cascades_project_issues_and_notifications() {
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
        "project delete cascade",
        None,
        user.id,
        Priority::Medium,
    );
    repos.issues.save(&issue).await.unwrap();
    repos
        .notifications
        .save(&Notification {
            id: shared::NotificationId::new(),
            recipient_id: user.id,
            event_type: "issue_updated".into(),
            entity_type: "issue".into(),
            entity_id: Some(issue.id.as_uuid()),
            actor_id: Some(user.id),
            title: "Issue updated".into(),
            body: None,
            is_read: false,
            read_at: None,
            action_url: Some(format!("/issues/{}", issue.id).into()),
            metadata: serde_json::json!({}),
            created_at: now(),
        })
        .await
        .unwrap();

    repos.projects.delete(project.id).await.unwrap();

    assert!(repos.projects.get_by_id(project.id).await.is_err());
    assert!(
        repos
            .issues
            .get_by_id_include_deleted(issue.id)
            .await
            .is_err()
    );
    assert!(
        repos
            .notifications
            .list_unread(user.id)
            .await
            .unwrap()
            .is_empty()
    );
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
            accessible_project_ids: None,
            status_id: Some(status),
            assignee_id: None,
            sprint_id: None,
            search_text: None,
            priority: None,
            sort_by: None,
            sort_order: None,
            limit: 10,
            offset: 0,
            jql: None,
            jql_user_id: None,
            deleted_only: false,
            include_deleted: false,
        })
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn issue_purge_deletes_issue_notifications() {
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
        "purge notifications",
        None,
        user.id,
        Priority::Medium,
    );
    repos.issues.save(&issue).await.unwrap();
    repos
        .notifications
        .save(&Notification {
            id: shared::NotificationId::new(),
            recipient_id: user.id,
            event_type: "issue_updated".into(),
            entity_type: "issue".into(),
            entity_id: Some(issue.id.as_uuid()),
            actor_id: Some(user.id),
            title: "Issue updated".into(),
            body: None,
            is_read: false,
            read_at: None,
            action_url: Some(format!("/issues/{}", issue.id).into()),
            metadata: serde_json::json!({}),
            created_at: now(),
        })
        .await
        .unwrap();

    repos.issues.delete(issue.id).await.unwrap();
    repos.issues.purge(issue.id).await.unwrap();

    assert!(
        repos
            .issues
            .get_by_id_include_deleted(issue.id)
            .await
            .is_err()
    );
    assert!(
        repos
            .notifications
            .list_unread(user.id)
            .await
            .unwrap()
            .is_empty()
    );
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
async fn user_repo_rejects_duplicate_email() {
    let repos = setup().await;
    let user = test_user();
    repos.users.save(&user).await.unwrap();

    let mut dup = test_user();
    dup.id = UserId::new();
    dup.email = user.email.clone();
    let err = repos.users.save(&dup).await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn project_repo_rejects_duplicate_key() {
    let repos = setup().await;
    let user = test_user();
    repos.users.save(&user).await.unwrap();
    let project = test_project(user.id);
    repos.projects.save(&project).await.unwrap();

    let mut dup = test_project(user.id);
    dup.id = ProjectId::new();
    dup.key = project.key.clone();
    let err = repos.projects.save(&dup).await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn project_next_issue_number_uses_numeric_suffix_ordering() {
    let repos = setup().await;
    let user = test_user();
    repos.users.save(&user).await.unwrap();
    let project = test_project(user.id);
    repos.projects.save(&project).await.unwrap();

    let status =
        StatusId::from_uuid(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let issue_9 = Issue::create(
        &project,
        9,
        IssueType::Task,
        status,
        "ninth",
        None,
        user.id,
        Priority::Medium,
    );
    let issue_10 = Issue::create(
        &project,
        10,
        IssueType::Task,
        status,
        "tenth",
        None,
        user.id,
        Priority::Medium,
    );
    repos.issues.save(&issue_9).await.unwrap();
    repos.issues.save(&issue_10).await.unwrap();

    let next = repos.projects.next_issue_number(project.id).await.unwrap();
    assert_eq!(next, 11);

    // A later save with a lower key must not decrease the high-water mark.
    let issue_8 = Issue::create(
        &project,
        8,
        IssueType::Task,
        status,
        "eighth",
        None,
        user.id,
        Priority::Medium,
    );
    repos.issues.save(&issue_8).await.unwrap();
    for issue in [&issue_8, &issue_9, &issue_10] {
        repos.issues.delete(issue.id).await.unwrap();
        repos.issues.purge(issue.id).await.unwrap();
    }
    assert_eq!(
        repos.projects.next_issue_number(project.id).await.unwrap(),
        12
    );
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn issue_creation_counter_rolls_back_with_failed_insert() {
    let repos = setup().await;
    let user = test_user();
    repos.users.save(&user).await.unwrap();
    let project = test_project(user.id);
    repos.projects.save(&project).await.unwrap();
    let issue = Issue::create(
        &project,
        42,
        IssueType::Task,
        StatusId::from_uuid(Uuid::new_v4()),
        "invalid status",
        None,
        user.id,
        Priority::Medium,
    );
    assert!(repos.issues.save(&issue).await.is_err());
    assert_eq!(
        repos.projects.next_issue_number(project.id).await.unwrap(),
        1
    );

    let history = domain::IssueStatusHistory {
        id: shared::IssueStatusHistoryId::new(),
        issue_id: issue.id,
        from_status_id: None,
        to_status_id: issue.status_id,
        changed_by_id: user.id,
        changed_at: now(),
    };
    assert!(
        repos
            .issues
            .create_with_initial_data(&issue, &history, &[])
            .await
            .is_err()
    );
    assert_eq!(
        repos.projects.next_issue_number(project.id).await.unwrap(),
        2
    );
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn issue_creation_with_initial_data_advances_counter_after_purge() {
    let repos = setup().await;
    let user = test_user();
    repos.users.save(&user).await.unwrap();
    let project = test_project(user.id);
    repos.projects.save(&project).await.unwrap();
    let status =
        StatusId::from_uuid(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let issue = Issue::create(
        &project,
        42,
        IssueType::Task,
        status,
        "direct creation",
        None,
        user.id,
        Priority::Medium,
    );
    let history = domain::IssueStatusHistory {
        id: shared::IssueStatusHistoryId::new(),
        issue_id: issue.id,
        from_status_id: None,
        to_status_id: status,
        changed_by_id: user.id,
        changed_at: now(),
    };
    repos
        .issues
        .create_with_initial_data(&issue, &history, &[])
        .await
        .unwrap();
    repos.issues.delete(issue.id).await.unwrap();
    repos.issues.purge(issue.id).await.unwrap();
    assert_eq!(
        repos.projects.next_issue_number(project.id).await.unwrap(),
        43
    );
}

#[tokio::test]
#[ignore = "requires docker test stack"]
#[serial_test::serial]
async fn archived_namespace_context_is_not_drained_while_assignment_exists() {
    use shared::resource_context::{NamespaceRef, OwnerCommand, ResourceKind, ResourceRef};

    let repos = setup().await;
    let user = test_user();
    repos.users.save(&user).await.unwrap();
    let project = test_project(user.id);
    repos.projects.save(&project).await.unwrap();
    let registry =
        Uuid::parse_str(&std::env::var("TT_NAMESPACE__REGISTRY_INSTANCE_ID").unwrap()).unwrap();
    let instance = Uuid::parse_str(&std::env::var("TT_NAMESPACE__INSTANCE_ID").unwrap()).unwrap();
    let namespace_id = Uuid::new_v4();
    let command = OwnerCommand {
        schema_version: 1,
        namespace: NamespaceRef {
            registry_instance_id: registry,
            namespace_id,
        },
        resource: ResourceRef {
            kind: ResourceKind::TrackerProject,
            instance_id: instance,
            resource_id: project.id.as_uuid(),
        },
        operation_id: Uuid::new_v4(),
        generation: 2,
        state: "archived".into(),
        create_spec: None,
    };
    let db = Database::connect(infra_test_db_url()).await.unwrap();
    db.execute_unprepared("SELECT 1").await.unwrap();
    let task = Issue::create(
        &project,
        1,
        IssueType::Task,
        StatusId::from_uuid(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
        "active assignment",
        None,
        user.id,
        Priority::Medium,
    );
    repos.issues.save(&task).await.unwrap();
    db.execute(sea_orm::Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Postgres,
        "INSERT INTO sdlc_instance(instance_id) VALUES ('tracker-test') ON CONFLICT DO NOTHING",
        vec![],
    ))
    .await
    .unwrap();
    db.execute(sea_orm::Statement::from_sql_and_values(sea_orm::DatabaseBackend::Postgres,
        "INSERT INTO sdlc_tasks(task_id,tracker_instance_id,project_id,root_task_id,owner_subject,state) VALUES ($1,'tracker-test',$2,$1,'owner',jsonb_build_object('task_id',$1::text,'root_task_id',$1::text,'project_id',$2::text,'tracker_instance_id','tracker-test','owner_subject','owner','stage','Backlog'))",
        vec![task.id.as_uuid().into(), project.id.as_uuid().into()])).await.unwrap();
    db.execute(sea_orm::Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Postgres,
        "INSERT INTO sdlc_assignments(task_id,version,payload) VALUES ($1,1,'{}'::jsonb)",
        vec![task.id.as_uuid().into()],
    ))
    .await
    .unwrap();
    db.execute(sea_orm::Statement::from_sql_and_values(sea_orm::DatabaseBackend::Postgres,
        "INSERT INTO tracker_namespace_bindings(resource_id,registry_instance_id,namespace_id,generation,state,command) VALUES ($1,$2,$3,2,'archived',$4)",
        vec![project.id.as_uuid().into(), registry.into(), namespace_id.into(), serde_json::to_value(&command).unwrap().into()])).await.unwrap();

    let listed = repos
        .projects
        .namespace_contexts(None, 10, 0)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert!(!listed[0].binding.drained);
    assert!(
        serde_json::to_value(
            repos
                .projects
                .namespace_binding(project.id)
                .await
                .unwrap()
                .unwrap()
        )
        .unwrap()["drained"]
            == false
    );
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn issue_creation_ticket_advances_after_legacy_number_collision() {
    let repos = setup().await;
    let user = test_user();
    repos.users.save(&user).await.unwrap();
    let project = test_project(user.id);
    repos.projects.save(&project).await.unwrap();
    let operation = Uuid::new_v4();
    let payload = serde_json::json!({"summary":"same idempotent request"});
    let reserved = repos
        .projects
        .issue_creation_ticket(project.id, user.id, operation, &payload, true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reserved.number, 1);

    // Model an older client winning the unique issue key after the durable
    // receipt reserved it but before its own insert commits.
    let legacy = Issue::create(
        &project,
        reserved.number,
        IssueType::Task,
        StatusId::from_uuid(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()),
        "legacy writer",
        None,
        user.id,
        Priority::Medium,
    );
    repos.issues.save(&legacy).await.unwrap();

    let advanced = repos
        .projects
        .advance_issue_creation_ticket(
            project.id,
            user.id,
            operation,
            &payload,
            reserved.issue_id,
            reserved.number,
        )
        .await
        .unwrap();
    assert_eq!(advanced.issue_id, reserved.issue_id);
    assert_eq!(advanced.number, 2);
    assert!(!advanced.completed);
    let replay = repos
        .projects
        .issue_creation_ticket(project.id, user.id, operation, &payload, false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(replay.issue_id, reserved.issue_id);
    assert_eq!(replay.number, advanced.number);
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn issue_repo_save_updates_existing_issue() {
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
        "before",
        None,
        user.id,
        Priority::Medium,
    );
    repos.issues.save(&issue).await.unwrap();

    let mut updated = issue.clone();
    updated.summary = "after".into();
    repos.issues.save(&updated).await.unwrap();

    let found = repos.issues.get_by_id(issue.id).await.unwrap();
    assert_eq!(found.summary, "after".into());
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

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn issue_search_escapes_wildcards_and_folds_unicode_case() {
    let repos = setup().await;
    let user = test_user();
    repos.users.save(&user).await.unwrap();
    let project = test_project(user.id);
    repos.projects.save(&project).await.unwrap();

    let status =
        StatusId::from_uuid(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let mk = |n: u32, summary: &str| {
        let mut i = Issue::create(
            &project,
            n,
            IssueType::Task,
            status,
            summary,
            None,
            user.id,
            Priority::Medium,
        );
        i.summary = summary.into();
        i
    };
    let a = mk(1, "wild%card probe");
    let b = mk(2, "Проверка поиска");
    repos.issues.save(&a).await.unwrap();
    repos.issues.save(&b).await.unwrap();

    let q = |text: &str| {
        repos.issues.list(domain::IssueQuery {
            project_id: Some(project.id),
            accessible_project_ids: None,
            status_id: None,
            assignee_id: None,
            sprint_id: None,
            search_text: Some(text.to_string()),
            priority: None,
            sort_by: None,
            sort_order: None,
            limit: 50,
            offset: 0,
            jql: None,
            jql_user_id: None,
            deleted_only: false,
            include_deleted: false,
        })
    };

    // Literal % must not act as a wildcard: only the issue containing the literal text matches.
    let res = q("wild%card").await.unwrap();
    assert_eq!(res.len(), 1, "literal % must match exactly one issue");
    // Cyrillic lowercase must match Title-case storage.
    let res = q("проверка").await.unwrap();
    assert_eq!(res.len(), 1, "unicode case folding must work");
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn jql_contains_treats_percent_as_literal() {
    let repos = setup().await;
    let user = test_user();
    repos.users.save(&user).await.unwrap();
    let project = test_project(user.id);
    repos.projects.save(&project).await.unwrap();

    let status =
        StatusId::from_uuid(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let pct = Issue::create(
        &project,
        1,
        IssueType::Task,
        status,
        "progress hit 100% done",
        None,
        user.id,
        Priority::Medium,
    );
    let other = Issue::create(
        &project,
        2,
        IssueType::Task,
        status,
        "no metacharacters here",
        None,
        user.id,
        Priority::Medium,
    );
    repos.issues.save(&pct).await.unwrap();
    repos.issues.save(&other).await.unwrap();

    let list = repos
        .issues
        .list(domain::IssueQuery {
            project_id: Some(project.id),
            jql: Some(domain::jql::parse("summary ~ \"100%\"").expect("valid JQL")),
            jql_user_id: Some(user.id),
            limit: 50,
            offset: 0,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(list.len(), 1, "literal % must match only the 100% issue");
    assert!(list[0].summary.as_ref().contains("100%"));
}

#[tokio::test]
#[ignore = "requires docker test stack"]
async fn jql_query_combines_with_project_and_text_filters() {
    let repos = setup().await;
    let user = test_user();
    repos.users.save(&user).await.unwrap();
    let project_a = test_project(user.id);
    let project_b = test_project(user.id);
    repos.projects.save(&project_a).await.unwrap();
    repos.projects.save(&project_b).await.unwrap();

    let status =
        StatusId::from_uuid(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let matching = Issue::create(
        &project_a,
        1,
        IssueType::Task,
        status,
        "scoped needle",
        None,
        user.id,
        Priority::High,
    );
    let wrong_text = Issue::create(
        &project_a,
        2,
        IssueType::Task,
        status,
        "scoped haystack",
        None,
        user.id,
        Priority::High,
    );
    let wrong_project = Issue::create(
        &project_b,
        1,
        IssueType::Task,
        status,
        "scoped needle",
        None,
        user.id,
        Priority::High,
    );
    repos.issues.save(&matching).await.unwrap();
    repos.issues.save(&wrong_text).await.unwrap();
    repos.issues.save(&wrong_project).await.unwrap();

    let list = repos
        .issues
        .list(domain::IssueQuery {
            project_id: Some(project_a.id),
            search_text: Some("needle".to_string()),
            jql: Some(domain::jql::parse("priority = high").expect("valid JQL")),
            jql_user_id: Some(user.id),
            sort_by: Some("created".to_string()),
            sort_order: Some("desc".to_string()),
            limit: 50,
            offset: 0,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, matching.id);
}
