use domain::{Issue, Project, Sprint, SprintState, User};
use shared::{BoardId, IssueType, Priority, ProjectId, ProjectKey, SprintId, StatusId, UserId};

use crate::dto::{IssueDto, ProjectDto, SprintDto, UserDto};

fn sample_user() -> User {
    User {
        id: UserId::new(),
        email: "a@b.com".into(),
        username: "ab".into(),
        display_name: "A B".into(),
        password_hash: "h".into(),
        refresh_token_hash: None,
        is_system_admin: false,
        is_active: true,
        created_at: shared::now(),
        updated_at: shared::now(),
    }
}

#[test]
fn user_dto_from_user() {
    let dto: UserDto = sample_user().into();
    assert_eq!(dto.display_name, "A B");
    assert_eq!(dto.email, "a@b.com");
}

#[test]
fn project_dto_from_project() {
    let p = Project {
        id: ProjectId::new(),
        key: ProjectKey::new("XX"),
        name: "X".into(),
        description: Some("desc".into()),
        owner_id: UserId::new(),
        default_board_id: BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    let dto = ProjectDto::from_project(p.clone(), "Owner Name".to_string(), 1, 2, 3);
    assert_eq!(dto.key, "XX");
    assert_eq!(dto.description, "desc");
    assert_eq!(dto.todo_count, 1);
}

#[test]
fn issue_dto_from_issue() {
    let reporter = sample_user();
    let project = Project {
        id: ProjectId::new(),
        key: ProjectKey::new("YY"),
        name: "Y".into(),
        description: None,
        owner_id: reporter.id,
        default_board_id: BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    let issue = Issue::create(
        &project,
        1,
        IssueType::Bug,
        StatusId::new(),
        "summary",
        None,
        reporter.id,
        Priority::High,
    );
    let dto = IssueDto::from_issue(
        issue.clone(),
        "Project Y".into(),
        "Todo".into(),
        None,
        Some("Reporter".into()),
    );
    assert_eq!(dto.summary, "summary");
    assert_eq!(dto.priority, "High");
    assert_eq!(dto.issue_type, "bug");
    assert_eq!(dto.reporter_name, Some("Reporter".into()));
}

#[test]
fn sprint_dto_from_sprint() {
    let project_id = ProjectId::new();
    let sprint = Sprint {
        id: SprintId::new(),
        project_id,
        name: "S1".into(),
        goal: Some("goal".into()),
        state: SprintState::Active,
        velocity: Some(42),
        start_date: None,
        end_date: None,
    };
    let dto = SprintDto::from_sprint(sprint, vec!["id-1".to_string()]);
    assert_eq!(dto.state, "active");
    assert_eq!(dto.velocity, 42);
    assert_eq!(dto.goal, "goal");
    assert_eq!(dto.project_id, project_id.to_string());
}

#[test]
fn board_projection_preserves_native_progress_without_promoting_legacy_done() {
    use crate::services::board::project_board_tasks;
    use domain::sdlc_lifecycle::{LifecycleStage, LifecycleStatus, SdlcBoardTask};
    let user = sample_user();
    let project = Project {
        id: ProjectId::new(),
        key: ProjectKey::new("BRD"),
        name: "Board".into(),
        description: None,
        owner_id: user.id,
        default_board_id: BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    let issues: Vec<_> = (1..=2)
        .map(|n| {
            IssueDto::from_issue(
                Issue::create(
                    &project,
                    n,
                    IssueType::Task,
                    StatusId::new(),
                    "Card",
                    None,
                    user.id,
                    Priority::Medium,
                ),
                "Board".into(),
                "Done".into(),
                None,
                None,
            )
        })
        .collect();
    let native = SdlcBoardTask {
        issue_id: issues[0].id.parse().unwrap(),
        enrolled: true,
        stage: LifecycleStage::Review,
        status: Some(LifecycleStatus::InProgress),
    };
    let projected = project_board_tasks(&issues, vec![native]).unwrap();
    assert_eq!(projected.len(), issues.len());
    assert!(projected[0].enrolled);
    assert!(!projected[1].enrolled);
    assert_eq!(projected[0].stage, LifecycleStage::Review);
    assert_eq!(projected[0].status, Some(LifecycleStatus::InProgress));
    assert_eq!(projected[1].stage, LifecycleStage::Draft);
    assert_eq!(projected[1].status, None);
    assert!(issues.iter().all(|issue| issue.status == "Done"));
    assert_eq!(LifecycleStage::BOARD_ORDER.len(), 9);
}
