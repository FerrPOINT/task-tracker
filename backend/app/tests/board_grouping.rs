use app::{
    context::{AppContext, BoardService},
    services::BoardServiceImpl,
};
use domain::{BoardRepository, IssueRepository, ProjectRepository};
use std::sync::Arc;
#[tokio::test]
async fn board_preserves_issue_order_empty_and_repeated_columns() {
    let user = shared::UserId::new();
    let a = shared::StatusId::new();
    let b = shared::StatusId::new();
    let empty = shared::StatusId::new();
    let project = domain::Project {
        id: shared::ProjectId::new(),
        key: shared::ProjectKey::new("BOARD"),
        name: "Board fixture".into(),
        description: None,
        owner_id: user,
        default_board_id: shared::BoardId::new(),
        created_at: shared::now(),
        updated_at: shared::now(),
    };
    let projects = Arc::new(domain::MemoryProjectRepository::default());
    projects.save(&project).await.unwrap();
    let boards = Arc::new(domain::MemoryBoardRepository::default());
    let issues = Arc::new(domain::MemoryIssueRepository::default());
    let columns = vec![(a, "A"), (empty, "Empty"), (b, "B"), (a, "Repeated")]
        .into_iter()
        .enumerate()
        .map(|(position, (id, name))| domain::BoardColumn {
            id,
            name: name.into(),
            category: domain::StatusCategory::Todo,
            wip_limit: Some(17),
            position: position as i32,
        })
        .collect();
    boards
        .save(&domain::Board {
            id: project.default_board_id,
            project_id: project.id,
            name: "Board".into(),
            columns,
        })
        .await
        .unwrap();
    for (index, status) in [a, b, a, b, a].into_iter().enumerate() {
        let issue = domain::Issue::create(
            &project,
            index as u32 + 1,
            shared::IssueType::Task,
            status,
            "Ordered",
            None,
            user,
            shared::Priority::Medium,
        );
        issues.save(&issue).await.unwrap();
    }
    let repos = Arc::new(domain::Repositories {
        projects,
        boards,
        issues,
        ..Default::default()
    });
    let ctx = AppContext::new(
        Arc::new(shared::AppConfig::default()),
        repos.clone(),
        Arc::new(domain::InMemoryStorage::default()),
    );
    let service = BoardServiceImpl::new(
        repos.boards.clone(),
        repos.issues.clone(),
        repos.sprints.clone(),
        repos.users.clone(),
        repos.labels.clone(),
        repos.statuses.clone(),
        repos.transitions.clone(),
        repos.projects.clone(),
        ctx.events,
        ctx.authz,
    );
    let selected = repos
        .issues
        .list_unbounded(domain::IssueQuery {
            project_id: Some(project.id),
            ..Default::default()
        })
        .await
        .unwrap();
    let dto = service.get_board(&project.key, user).await.unwrap();
    for (column, status) in dto.columns.iter().zip([a, empty, b, a]) {
        let expected: Vec<_> = selected
            .iter()
            .filter(|i| i.status_id == status)
            .map(|i| i.id.to_string())
            .collect();
        assert_eq!(column.issue_ids, expected);
        assert_eq!(column.wip_limit, Some(17));
    }
    assert_eq!(dto.issues.len(), 5);
    assert!(dto.columns[1].issue_ids.is_empty());
    assert_eq!(dto.columns[0].issue_ids, dto.columns[3].issue_ids);
}
