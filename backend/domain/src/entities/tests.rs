#[cfg(test)]
mod tests {
    use crate::value_objects::ArcStr;
    use crate::{Board, BoardColumn, ColumnCategory, Issue, Project, Sprint, SprintState, User};
    use shared::{
        BoardId, IssueId, IssueKey, IssueType, Priority, ProjectId, ProjectKey, SprintId, StatusId,
        UserId,
    };
    use std::str::FromStr;

    fn demo_project() -> Project {
        Project {
            id: ProjectId::new(),
            key: ProjectKey::new("TT"),
            name: "Task Tracker".into(),
            description: None,
            owner_id: UserId::new(),
            default_board_id: BoardId::new(),
            created_at: shared::now(),
            updated_at: shared::now(),
        }
    }

    fn demo_user() -> User {
        User {
            id: UserId::new(),
            email: "a@b.com".into(),
            username: "demo".into(),
            display_name: "Demo".into(),
            password_hash: "x".into(),
            created_at: shared::now(),
            updated_at: shared::now(),
        }
    }

    #[test]
    fn issue_create_emits_event() {
        let project = demo_project();
        let reporter = demo_user();
        let status = StatusId::new();
        let issue = Issue::create(
            &project,
            1,
            IssueType::Task,
            status,
            "summary",
            None,
            reporter.id,
            Priority::Medium,
        );
        assert_eq!(issue.key, IssueKey::new(ProjectKey::new("TT"), 1));
        assert_eq!(issue.project_id, project.id);
        assert!(matches!(issue.events[0], crate::IssueEvent::Created { .. }));
    }

    #[test]
    fn issue_assign_and_change_status_noop_when_same() {
        let project = demo_project();
        let reporter = demo_user();
        let status = StatusId::new();
        let mut issue = Issue::create(
            &project,
            1,
            IssueType::Task,
            status,
            "summary",
            None,
            reporter.id,
            Priority::Medium,
        );
        let updated_before = issue.updated_at;
        issue.assign(issue.assignee_id);
        issue.change_status(issue.status_id);
        assert_eq!(issue.updated_at, updated_before);
        assert_eq!(issue.events.len(), 1);
    }

    #[test]
    fn issue_assign_unassign_emits_events() {
        let project = demo_project();
        let reporter = demo_user();
        let status = StatusId::new();
        let mut issue = Issue::create(
            &project,
            1,
            IssueType::Task,
            status,
            "summary",
            None,
            reporter.id,
            Priority::Medium,
        );
        issue.assign(Some(reporter.id));
        assert!(matches!(
            issue.events[1],
            crate::IssueEvent::Assigned {
                assignee_id: Some(_),
                ..
            }
        ));
        issue.assign(None);
        assert!(matches!(
            issue.events[2],
            crate::IssueEvent::Assigned {
                assignee_id: None,
                ..
            }
        ));
    }

    #[test]
    fn issue_change_status_and_position() {
        let project = demo_project();
        let reporter = demo_user();
        let status = StatusId::new();
        let mut issue = Issue::create(
            &project,
            1,
            IssueType::Task,
            status,
            "summary",
            None,
            reporter.id,
            Priority::Medium,
        );
        let new_status = StatusId::new();
        issue.change_status(new_status);
        assert_eq!(issue.status_id, new_status);
        assert!(matches!(
            issue.events[1],
            crate::IssueEvent::StatusChanged { .. }
        ));

        issue.set_position(1.5);
        assert!((issue.position - 1.5).abs() < f64::EPSILON);
        issue.set_position(1.5);
        assert_eq!(issue.events.len(), 2);
    }

    #[test]
    fn issue_take_events_clears() {
        let project = demo_project();
        let reporter = demo_user();
        let status = StatusId::new();
        let mut issue = Issue::create(
            &project,
            1,
            IssueType::Task,
            status,
            "summary",
            None,
            reporter.id,
            Priority::Medium,
        );
        let events = issue.take_events();
        assert!(!events.is_empty());
        assert!(issue.events.is_empty());
    }

    #[test]
    fn sprint_state_from_str() {
        assert_eq!(
            SprintState::from_str("future").unwrap(),
            SprintState::Future
        );
        assert_eq!(
            SprintState::from_str("ACTIVE").unwrap(),
            SprintState::Active
        );
        assert_eq!(
            SprintState::from_str("Closed").unwrap(),
            SprintState::Closed
        );
        assert!(SprintState::from_str("unknown").is_err());
        assert_eq!(SprintState::default(), SprintState::Future);
    }

    #[test]
    fn board_default_has_columns() {
        let board = Board::default();
        assert_eq!(board.columns.len(), 3);
        assert!(matches!(board.columns[0].category, ColumnCategory::Todo));
        assert!(matches!(board.columns[2].category, ColumnCategory::Done));
    }

    #[test]
    fn column_category_default() {
        assert_eq!(ColumnCategory::default(), ColumnCategory::Todo);
    }
}
