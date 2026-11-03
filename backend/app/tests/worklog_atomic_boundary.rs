//! Fast public service boundary; the PostgreSQL gate independently proves transaction rollback.
use app::{
    AppContext,
    commands::{CreateWorklogCommand, UpdateWorklogCommand},
};
use domain::{IssueRepository, ProjectRepository, UserRepository};
use std::sync::Arc;

#[tokio::test]
async fn worklog_service_uses_the_shared_owner_without_saving_a_stale_issue() {
    let now = shared::now();
    let user = shared::UserId::new();
    let project = domain::Project {
        id: shared::ProjectId::new(),
        key: shared::ProjectKey::new("TIME"),
        name: "Time".into(),
        description: None,
        owner_id: user,
        default_board_id: shared::BoardId::new(),
        created_at: now,
        updated_at: now,
    };
    let projects = Arc::new(domain::MemoryProjectRepository::default());
    projects.save(&project).await.unwrap();
    let issues = Arc::new(domain::MemoryIssueRepository::default());
    let mut issue = domain::Issue::create(
        &project,
        1,
        shared::IssueType::Task,
        shared::StatusId::new(),
        "Original",
        None,
        user,
        shared::Priority::Medium,
    );
    issue.original_estimate_seconds = Some(600);
    issue.remaining_estimate_seconds = Some(600);
    issues.save(&issue).await.unwrap();
    let users = Arc::new(domain::MemoryUserRepository::default());
    users
        .save(&domain::User {
            id: user,
            email: "time@example.test".into(),
            username: "time".into(),
            display_name: "Time".into(),
            password_hash: "!".into(),
            refresh_token_hash: None,
            is_system_admin: false,
            is_active: true,
            created_at: now,
            updated_at: now,
        })
        .await
        .unwrap();
    let repos = Arc::new(domain::Repositories {
        users,
        projects,
        issues: issues.clone(),
        worklogs: Arc::new(issues.worklog_repository()),
        ..Default::default()
    });
    let ctx = AppContext::new(
        Arc::new(shared::AppConfig::default()),
        repos,
        Arc::new(domain::InMemoryStorage::default()),
    );
    let value = ctx
        .services
        .worklog
        .create(
            CreateWorklogCommand {
                issue_id: issue.id,
                author_id: user,
                started_at: now,
                duration_seconds: 120,
                description: Some("work".into()),
            },
            user,
        )
        .await
        .unwrap();
    let current = issues.get_by_id(issue.id).await.unwrap();
    assert_eq!(
        (
            current.time_spent_seconds,
            current.remaining_estimate_seconds
        ),
        (120, Some(480))
    );
    // A normal content save starts with an older snapshot; aggregate ownership is unchanged.
    issue.summary = "Renamed".into();
    issues.save(&issue).await.unwrap();
    let id = value.id.parse().unwrap();
    ctx.services
        .worklog
        .update(
            id,
            UpdateWorklogCommand {
                started_at: None,
                duration_seconds: Some(300),
                description: None,
            },
            user,
        )
        .await
        .unwrap();
    let current = issues.get_by_id(issue.id).await.unwrap();
    assert_eq!(
        (
            current.time_spent_seconds,
            current.remaining_estimate_seconds
        ),
        (300, Some(300))
    );
    assert_eq!(current.summary.as_ref(), "Renamed");
    ctx.services.worklog.delete(id, user).await.unwrap();
    let current = issues.get_by_id(issue.id).await.unwrap();
    assert_eq!(
        (
            current.time_spent_seconds,
            current.remaining_estimate_seconds
        ),
        (0, Some(600))
    );
    assert_eq!(current.summary.as_ref(), "Renamed");
}
