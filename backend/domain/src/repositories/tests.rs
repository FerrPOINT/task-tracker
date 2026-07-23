#[cfg(test)]
mod tests {
    use crate::{
        BoardRepository, EventBus, IssueRepository, ProjectQuery, ProjectRepository, Repositories,
        SprintRepository, StubBoardRepository, StubEventBus, StubIssueRepository,
        StubProjectRepository, StubSprintRepository, StubUnitOfWork, StubUserRepository,
        UnitOfWork, UserRepository,
    };
    use shared::{AppError, BoardId, IssueId, ProjectId, ProjectKey, SprintId, UserId};

    #[tokio::test]
    async fn stub_user_repository() {
        let r = StubUserRepository;
        let id = UserId::new();
        assert!(matches!(
            r.get_by_id(id).await,
            Err(AppError::NotFound { .. })
        ));
        assert!(matches!(
            r.get_by_email("a@b.com").await,
            Err(AppError::NotFound { .. })
        ));
        assert!(
            r.save(&crate::User {
                id,
                email: "a@b.com".into(),
                username: "ab".into(),
                display_name: "A B".into(),
                password_hash: "h".into(),
                created_at: shared::now(),
                updated_at: shared::now(),
            })
            .await
            .is_ok()
        );
    }

    #[tokio::test]
    async fn stub_project_repository() {
        let r = StubProjectRepository;
        assert!(matches!(
            r.get_by_id(ProjectId::new()).await,
            Err(AppError::NotFound { .. })
        ));
        assert!(matches!(
            r.get_by_key(&ProjectKey::new("XX")).await,
            Err(AppError::NotFound { .. })
        ));
        assert_eq!(r.list(ProjectQuery::default()).await.unwrap().len(), 0);
        assert!(r.next_issue_number(ProjectId::new()).await.is_ok());
    }

    #[tokio::test]
    async fn stub_issue_repository() {
        let r = StubIssueRepository;
        assert!(matches!(
            r.get_by_id(IssueId::new()).await,
            Err(AppError::NotFound { .. })
        ));
        assert_eq!(r.list(crate::IssueQuery::default()).await.unwrap().len(), 0);
        assert!(r.delete(IssueId::new()).await.is_ok());
    }

    #[tokio::test]
    async fn stub_board_repository() {
        let r = StubBoardRepository;
        assert!(matches!(
            r.get_by_id(BoardId::new()).await,
            Err(AppError::NotFound { .. })
        ));
        assert!(matches!(
            r.get_default_by_project(ProjectId::new()).await,
            Err(AppError::NotFound { .. })
        ));
        assert!(matches!(
            r.get_default_by_project_key(&ProjectKey::new("XX")).await,
            Err(AppError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn stub_sprint_repository() {
        let r = StubSprintRepository;
        assert!(matches!(
            r.get_active_by_project(ProjectId::new()).await.unwrap(),
            None
        ));
        assert!(matches!(
            r.get_by_id(SprintId::new()).await,
            Err(AppError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn repositories_default_uses_stubs() {
        let repos = Repositories::default();
        assert!(repos.users.get_by_id(UserId::new()).await.is_err());
        assert!(
            repos
                .projects
                .get_by_key(&ProjectKey::new("XX"))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn stub_unit_of_work_runs_closure() {
        let uow = StubUnitOfWork;
        let result = uow
            .with_transaction(|repos| {
                Box::pin(async move { repos.projects.list(ProjectQuery::default()).await })
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn stub_event_bus() {
        let bus = StubEventBus;
        assert!(
            bus.publish(crate::ProjectEvent::Created {
                project_id: ProjectId::new(),
                owner_id: UserId::new(),
            })
            .await
            .is_ok()
        );
    }
}
