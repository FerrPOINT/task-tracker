#[cfg(test)]
mod tests {
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
            created_at: shared::now(),
            updated_at: shared::now(),
        }
    }

    #[test]
    fn user_dto_from_user() {
        let dto: UserDto = sample_user().into();
        assert_eq!(dto.name, "A B");
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
        let dto = ProjectDto::from_project(p.clone(), 1, 2, 3);
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
        let sprint = Sprint {
            id: SprintId::new(),
            project_id: ProjectId::new(),
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
    }
}
