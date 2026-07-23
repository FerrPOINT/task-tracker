#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::stubs::memory::{
        MemoryBoardRepository, MemoryEventBus, MemoryIssueRepository, MemoryProjectRepository,
        MemorySprintRepository, MemoryUnitOfWork, MemoryUserRepository,
    };
    use crate::{
        Board, BoardRepository, EventBus, Issue, IssueQuery, IssueRepository, Project,
        ProjectQuery, ProjectRepository, Sprint, SprintRepository, SprintState, UnitOfWork, User,
        UserRepository,
    };
    use shared::{
        AppError, BoardId, IssueId, IssueKey, IssueType, Priority, ProjectId, ProjectKey, SprintId,
        StatusId, UserId,
    };

    fn user(id: UserId, email: &str) -> User {
        User {
            id,
            email: email.into(),
            username: email.split('@').next().unwrap().into(),
            display_name: "U".into(),
            password_hash: "h".into(),
            created_at: shared::now(),
            updated_at: shared::now(),
        }
    }

    fn project(id: ProjectId, key: &str) -> Project {
        Project {
            id,
            key: ProjectKey::new(key),
            name: key.into(),
            description: None,
            owner_id: UserId::new(),
            default_board_id: BoardId::new(),
            created_at: shared::now(),
            updated_at: shared::now(),
        }
    }

    fn issue(id: IssueId, project_id: ProjectId, number: u32) -> Issue {
        Issue::create(
            &project(project_id, "TT"),
            number,
            IssueType::Task,
            StatusId::new(),
            format!("Issue {}", number),
            None,
            UserId::new(),
            Priority::Medium,
        )
    }

    fn sprint(id: SprintId, project_id: ProjectId) -> Sprint {
        Sprint {
            id,
            project_id,
            name: "S1".into(),
            goal: None,
            state: SprintState::Active,
            start_date: None,
            end_date: None,
            velocity: None,
        }
    }

    #[tokio::test]
    async fn memory_user_repository() {
        let r = MemoryUserRepository::default();
        let id = UserId::new();
        let u = user(id, "a@b.com");
        assert!(r.save(&u).await.is_ok());
        assert_eq!(r.get_by_id(id).await.unwrap().id, id);
        assert_eq!(r.get_by_email("a@b.com").await.unwrap().id, id);
        assert!(matches!(
            r.get_by_id(UserId::new()).await,
            Err(AppError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn memory_project_repository() {
        let r = MemoryProjectRepository::default();
        let p = project(ProjectId::new(), "TT");
        assert!(r.save(&p).await.is_ok());
        assert_eq!(r.get_by_id(p.id).await.unwrap().id, p.id);
        assert_eq!(r.get_by_key(&p.key).await.unwrap().id, p.id);
        assert!(r.next_issue_number(p.id).await.is_ok());
        assert!(!r.list(ProjectQuery::default()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn memory_issue_repository() {
        let r = MemoryIssueRepository::default();
        let pid = ProjectId::new();
        let i = issue(IssueId::new(), pid, 1);
        assert!(r.save(&i).await.is_ok());
        assert_eq!(r.get_by_id(i.id).await.unwrap().id, i.id);
        assert_eq!(r.get_by_key(&i.key).await.unwrap().id, i.id);
        assert!(
            !r.list(IssueQuery {
                project_id: Some(pid),
                ..Default::default()
            })
            .await
            .unwrap()
            .is_empty()
        );
        assert!(r.delete(i.id).await.is_ok());
        assert!(matches!(
            r.get_by_id(i.id).await,
            Err(AppError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn memory_board_repository() {
        let r = MemoryBoardRepository::default();
        let b = Board {
            id: BoardId::new(),
            project_id: ProjectId::new(),
            name: "Default".into(),
            columns: vec![],
        };
        assert!(r.save(&b).await.is_ok());
        assert_eq!(r.get_by_id(b.id).await.unwrap().id, b.id);
        assert_eq!(
            r.get_default_by_project(b.project_id).await.unwrap().id,
            b.id
        );
    }

    #[tokio::test]
    async fn memory_sprint_repository() {
        let r = MemorySprintRepository::default();
        let pid = ProjectId::new();
        let s = sprint(SprintId::new(), pid);
        assert!(r.save(&s).await.is_ok());
        assert!(r.get_active_by_project(pid).await.unwrap().is_some());
        assert_eq!(r.get_by_id(s.id).await.unwrap().id, s.id);
    }

    #[tokio::test]
    async fn memory_unit_of_work_runs() {
        let repos = crate::Repositories {
            users: Arc::new(MemoryUserRepository::default()),
            projects: Arc::new(MemoryProjectRepository::default()),
            issues: Arc::new(MemoryIssueRepository::default()),
            boards: Arc::new(MemoryBoardRepository::default()),
            sprints: Arc::new(MemorySprintRepository::default()),
        };
        let uow = MemoryUnitOfWork::new(repos);
        let result = uow
            .with_transaction(|repos| {
                Box::pin(async move { repos.projects.list(ProjectQuery::default()).await })
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn memory_event_bus() {
        let bus = MemoryEventBus::default();
        assert!(
            bus.publish(crate::ProjectEvent::Created {
                project_id: ProjectId::new(),
                owner_id: UserId::new(),
            })
            .await
            .is_ok()
        );
        assert_eq!(bus.drained().len(), 1);
    }
}
