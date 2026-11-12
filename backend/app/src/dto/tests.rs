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
fn unified_board_preserves_legacy_cards_without_claiming_sdlc_progress() {
    use super::BoardDto;
    use domain::sdlc_lifecycle::{LifecycleStage, LifecycleStatus, SdlcBoardTask};

    // A custom historical name, Done and a real SDLC task must not select
    // different board modes or turn old statuses into deployment evidence.
    for project_key in ["PDLC", "OTHER"] {
        let project_id = ProjectId::new();
        let owner = UserId::new();
        let project = Project {
            id: project_id,
            key: ProjectKey::new(project_key),
            name: "Board fixture".into(),
            description: None,
            owner_id: owner,
            default_board_id: BoardId::new(),
            created_at: shared::now(),
            updated_at: shared::now(),
        };
        let issues: Vec<_> = ["Done", "Custom review", "To Do"]
            .into_iter()
            .enumerate()
            .map(|(index, status)| {
                let issue = Issue::create(
                    &project,
                    index as u32 + 1,
                    IssueType::Task,
                    StatusId::new(),
                    "Existing card",
                    None,
                    owner,
                    Priority::High,
                );
                IssueDto::from_issue(issue, project.name.to_string(), status.into(), None, None)
            })
            .collect();
        let original = serde_json::to_value(&issues).unwrap();
        let ids: Vec<_> = issues.iter().map(|issue| issue.id.clone()).collect();
        let board = BoardDto {
            project_id: project_id.to_string(),
            project_key: project_key.into(),
            issues,
            sprint: SprintDto {
                id: "historical-sprint".into(),
                project_id: project_id.to_string(),
                name: "Sprint".into(),
                goal: String::new(),
                state: "active".into(),
                velocity: 0,
                remaining_days: None,
                issue_ids: ids.clone(),
                start_date: None,
                end_date: None,
            },
        };
        let board = board
            .with_sdlc_tasks(vec![SdlcBoardTask {
                issue_id: ids[2].parse().unwrap(),
                stage: LifecycleStage::Testing,
                status: Some(LifecycleStatus::InProgress),
            }])
            .unwrap();
        assert_eq!(board.cards.len(), 3);
        assert_eq!(board.sprint.issue_ids, ids);
        assert_eq!(
            serde_json::to_value(
                board
                    .cards
                    .iter()
                    .map(|(issue, _)| issue)
                    .collect::<Vec<_>>()
            )
            .unwrap(),
            original
        );
        for (_, task) in &board.cards[..2] {
            assert_eq!(task.stage, LifecycleStage::Draft);
            assert_eq!(task.status, None);
        }
        assert_eq!(board.cards[2].1.stage, LifecycleStage::Testing);
        assert_eq!(board.cards[2].1.status, Some(LifecycleStatus::InProgress));
    }
}
