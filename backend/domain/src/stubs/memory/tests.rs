use crate::memory::{
    MemoryAuditLogRepository, MemoryBoardRepository, MemoryEventBus, MemoryIssueRepository,
    MemoryNotificationRepository, MemoryProjectRepository, MemorySprintRepository,
    MemorySystemSettingRepository, MemoryUnitOfWork, MemoryUserRepository,
};
use crate::{
    AuditLog, AuditLogRepository, Board, BoardRepository, EventBus, Issue, IssueQuery,
    IssueRepository, Notification, NotificationRepository, NotificationUserSettings, Project,
    ProjectEvent, ProjectQuery, ProjectRepository, Repositories, Sprint, SprintRepository,
    SprintState, SystemSetting, SystemSettingRepository, UnitOfWork, User,
    UserNotificationSettingsRepository, UserRepository,
};
use shared::{
    BoardId, IssueId, IssueType, NotificationId, Priority, ProjectId, ProjectKey, SprintId,
    StatusId, UserId,
};
use std::str::FromStr;

fn sample_user() -> User {
    User {
        id: UserId::new(),
        email: "u@example.com".into(),
        username: "u".into(),
        display_name: "User".into(),
        password_hash: "hash".into(),
        refresh_token_hash: None,
        is_system_admin: false,
        is_active: true,
        created_at: shared::now(),
        updated_at: shared::now(),
    }
}

fn sample_project(owner_id: UserId) -> Project {
    Project {
        id: ProjectId::new(),
        key: ProjectKey::new("TEST"),
        name: "Test".into(),
        description: Some("desc".into()),
        owner_id,
        default_board_id: BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    }
}

#[tokio::test]
async fn memory_user_repository_lifecycle() {
    let repo = MemoryUserRepository::default();
    let user = sample_user();

    assert!(repo.get_by_id(user.id).await.is_err());
    repo.save(&user).await.unwrap();
    assert_eq!(
        repo.get_by_id(user.id).await.unwrap().email.as_ref(),
        "u@example.com"
    );
    assert_eq!(
        repo.get_by_email("u@example.com").await.unwrap().id,
        user.id
    );

    let mut updated = user.clone();
    updated.display_name = "Updated".into();
    repo.save(&updated).await.unwrap();
    assert_eq!(
        repo.get_by_id(user.id).await.unwrap().display_name.as_ref(),
        "Updated"
    );
}

#[tokio::test]
async fn memory_project_repository_lifecycle() {
    let repo = MemoryProjectRepository::default();
    let owner = UserId::new();
    let project = sample_project(owner);

    assert!(repo.get_by_id(project.id).await.is_err());
    repo.save(&project).await.unwrap();
    assert_eq!(
        repo.get_by_id(project.id).await.unwrap().name.as_ref(),
        "Test"
    );
    assert_eq!(repo.get_by_key(&project.key).await.unwrap().id, project.id);
    assert_eq!(repo.list(ProjectQuery::default()).await.unwrap().len(), 1);
    // Monotonic per-project counter: first allocation is 1, next is 2.
    assert_eq!(repo.next_issue_number(project.id).await.unwrap(), 1);
    assert_eq!(repo.next_issue_number(project.id).await.unwrap(), 2);
    repo.save(&project).await.unwrap();
    let mut renamed = project.clone();
    renamed.name = "Renamed".into();
    repo.save(&renamed).await.unwrap();
    assert_eq!(
        repo.get_by_id(project.id).await.unwrap().name.as_ref(),
        "Renamed"
    );
}

#[tokio::test]
async fn memory_issue_repository_filters_and_search() {
    let repo = MemoryIssueRepository::default();
    let project_id = ProjectId::new();
    let user_id = UserId::new();
    let status = StatusId::from_uuid(uuid::Uuid::nil());
    let project = Project {
        id: project_id,
        key: ProjectKey::new("TEST"),
        name: "Test".into(),
        description: None,
        owner_id: UserId::new(),
        default_board_id: BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    let mut issue = Issue::create(
        &project,
        1,
        IssueType::Task,
        status,
        "searchable summary",
        None,
        user_id,
        Priority::Medium,
    );
    issue.assign(Some(user_id));
    repo.save(&issue).await.unwrap();

    let found = repo.get_by_id(issue.id).await.unwrap();
    assert_eq!(found.summary.as_ref(), "searchable summary");

    let by_key = repo.get_by_key(&issue.key).await.unwrap();
    assert_eq!(by_key.id, issue.id);

    let filtered = repo
        .list(IssueQuery {
            project_id: Some(project_id),
            assignee_id: Some(user_id),
            search_text: Some("summary".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(filtered.len(), 1);

    let empty = repo
        .list(IssueQuery {
            status_id: Some(StatusId::new()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(empty.is_empty());

    repo.save(&issue).await.unwrap();
    let mut updated = issue.clone();
    updated.summary = "updated".into();
    repo.save(&updated).await.unwrap();
    assert_eq!(
        repo.get_by_id(issue.id).await.unwrap().summary.as_ref(),
        "updated"
    );
}

#[tokio::test]
async fn memory_board_and_sprint_repositories() {
    let boards = MemoryBoardRepository::default();
    let sprints = MemorySprintRepository::default();
    let project_id = ProjectId::new();
    let board = Board {
        id: BoardId::new(),
        project_id,
        name: "Main".into(),
        columns: vec![],
    };
    boards.save(&board).await.unwrap();
    assert_eq!(boards.get_by_id(board.id).await.unwrap().id, board.id);
    assert_eq!(
        boards.get_default_by_project(project_id).await.unwrap().id,
        board.id
    );
    assert!(
        boards
            .get_default_by_project_key(&ProjectKey::from_str("NONE").unwrap())
            .await
            .is_err()
    );

    let sprint = Sprint {
        id: SprintId::new(),
        project_id,
        name: "S1".into(),
        goal: None,
        state: SprintState::Active,
        start_date: None,
        end_date: None,
        velocity: None,
    };
    sprints.save(&sprint).await.unwrap();
    assert_eq!(sprints.get_by_id(sprint.id).await.unwrap().id, sprint.id);
    assert!(
        sprints
            .get_active_by_project(project_id)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        sprints
            .get_active_by_project(ProjectId::new())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn memory_unit_of_work_and_event_bus() {
    let repos = Repositories::default();
    let uow = MemoryUnitOfWork::new(repos.clone());
    let result = uow
        .with_transaction(|_| Box::pin(async move { Ok(42) }))
        .await
        .unwrap();
    assert_eq!(result, 42);

    let bus = MemoryEventBus::default();
    bus.publish(ProjectEvent::Created {
        project_id: ProjectId::new(),
        owner_id: UserId::new(),
    })
    .await
    .unwrap();
    assert_eq!(bus.drained().len(), 1);
}

#[tokio::test]
async fn memory_notification_repository_lists_unread_and_marks_recipient_notifications_read() {
    let repo = MemoryNotificationRepository::default();
    let recipient = UserId::new();
    let other_recipient = UserId::new();
    let now = shared::now();
    let unread = Notification {
        id: NotificationId::new(),
        recipient_id: recipient,
        event_type: "issue_assigned".into(),
        entity_type: "issue".into(),
        entity_id: Some(IssueId::new().as_uuid()),
        actor_id: Some(UserId::new()),
        title: "Assigned to you".into(),
        body: Some("Review the issue".into()),
        is_read: false,
        read_at: None,
        action_url: Some("/issues/TT-1".into()),
        metadata: serde_json::json!({"priority": "high"}),
        created_at: now,
    };
    let already_read = Notification {
        id: NotificationId::new(),
        recipient_id: recipient,
        event_type: "issue_commented".into(),
        entity_type: "issue".into(),
        entity_id: Some(IssueId::new().as_uuid()),
        actor_id: None,
        title: "New comment".into(),
        body: None,
        is_read: true,
        read_at: Some(now),
        action_url: None,
        metadata: serde_json::Value::Null,
        created_at: now,
    };
    let someone_elses = Notification {
        id: NotificationId::new(),
        recipient_id: other_recipient,
        ..unread.clone()
    };

    repo.save(&unread).await.unwrap();
    repo.save(&already_read).await.unwrap();
    repo.save(&someone_elses).await.unwrap();

    assert_eq!(
        repo.list_unread(recipient).await.unwrap(),
        vec![unread.clone()]
    );

    repo.mark_read(unread.id, recipient).await.unwrap();
    assert!(repo.list_unread(recipient).await.unwrap().is_empty());
    assert!(repo.mark_read(unread.id, other_recipient).await.is_err());

    let second_unread = Notification {
        id: NotificationId::new(),
        ..unread.clone()
    };
    repo.save(&second_unread).await.unwrap();
    repo.mark_all_read(recipient).await.unwrap();
    assert!(repo.list_unread(recipient).await.unwrap().is_empty());
    assert_eq!(
        repo.list_unread(other_recipient).await.unwrap(),
        vec![someone_elses]
    );
}

#[tokio::test]
async fn memory_notification_settings_repository_round_trips_user_preferences() {
    let repo = MemoryNotificationRepository::default();
    let user_id = UserId::new();
    let settings = NotificationUserSettings {
        user_id,
        email_frequency: "daily".into(),
        disabled_event_types: vec!["issue_updated".into(), "issue_commented".into()],
        notify_own_changes: false,
    };

    assert!(repo.get_settings(user_id).await.is_err());
    repo.save_settings(&settings).await.unwrap();
    assert_eq!(repo.get_settings(user_id).await.unwrap(), settings);

    let updated = NotificationUserSettings {
        email_frequency: "immediate".into(),
        ..settings.clone()
    };
    repo.save_settings(&updated).await.unwrap();
    assert_eq!(repo.get_settings(user_id).await.unwrap(), updated);
}

#[tokio::test]
async fn memory_audit_log_repository_filters_by_actor_and_limits_newest_entries() {
    let repo = MemoryAuditLogRepository::default();
    let actor = UserId::new();
    let other_actor = UserId::new();
    let older = AuditLog {
        id: shared::AuditLogId::new(),
        actor_id: actor,
        action: "issue.created".into(),
        entity_type: "issue".into(),
        entity_id: None,
        metadata: serde_json::Value::Null,
        created_at: shared::now() - chrono::Duration::seconds(1),
    };
    let newer = AuditLog {
        id: shared::AuditLogId::new(),
        action: "issue.updated".into(),
        created_at: shared::now(),
        ..older.clone()
    };
    let unrelated = AuditLog {
        id: shared::AuditLogId::new(),
        actor_id: other_actor,
        ..newer.clone()
    };

    repo.save(&older).await.unwrap();
    repo.save(&newer).await.unwrap();
    repo.save(&unrelated).await.unwrap();

    assert_eq!(repo.list(Some(actor), 1, 0).await.unwrap(), vec![newer]);
    assert_eq!(
        repo.list(Some(other_actor), 10, 0).await.unwrap(),
        vec![unrelated]
    );
}

#[tokio::test]
async fn memory_system_setting_repository_replaces_existing_keys() {
    let repo = MemorySystemSettingRepository::default();
    let setting = SystemSetting {
        key: "auth.session_ttl".into(),
        value: serde_json::json!(3600),
        updated_at: shared::now(),
    };
    repo.save(&setting).await.unwrap();
    assert_eq!(repo.get("auth.session_ttl").await.unwrap(), setting);

    let updated = SystemSetting {
        value: serde_json::json!(7200),
        updated_at: shared::now(),
        ..setting.clone()
    };
    repo.save(&updated).await.unwrap();
    assert_eq!(repo.list().await.unwrap(), vec![updated]);
    assert!(repo.get("unknown").await.is_err());
}
