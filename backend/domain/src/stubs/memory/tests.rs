#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::{
        Issue, IssueQuery, Project, ProjectEvent, ProjectQuery, Repositories, Sprint, SprintState,
        User,
    };
    use shared::{IssueType, Priority, StatusId};
    use std::str::FromStr;

    fn sample_user() -> User {
        User {
            id: UserId::new(),
            email: "u@example.com".into(),
            username: "u".into(),
            display_name: "User".into(),
            password_hash: "hash".into(),
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
        assert_eq!(repo.next_issue_number(project.id).await.unwrap(), 2);
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

        repo.delete(issue.id).await.unwrap();
        assert!(repo.get_by_id(issue.id).await.is_err());
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
}
