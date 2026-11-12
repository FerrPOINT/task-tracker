//! Request-local report indexing preserves the existing temporal and project semantics.
use domain::{IssueRepository, IssueStatusHistoryRepository, ProjectRepository, SprintRepository};
use std::sync::Arc;
#[tokio::test]
async fn report_index_preserves_creation_boundaries_transitions_and_project_isolation() {
    let start = shared::now() - chrono::Duration::days(4);
    let owner = shared::UserId::new();
    let project = domain::Project {
        id: shared::ProjectId::new(),
        key: shared::ProjectKey::new("REPORT"),
        name: "Reports".into(),
        description: None,
        owner_id: owner,
        default_board_id: shared::BoardId::new(),
        created_at: start,
        updated_at: start,
    };
    let projects = Arc::new(domain::MemoryProjectRepository::default());
    projects.save(&project).await.unwrap();
    let issues = Arc::new(domain::MemoryIssueRepository::default());
    let (todo, working, done) = (
        shared::StatusId::new(),
        shared::StatusId::new(),
        shared::StatusId::new(),
    );
    let statuses = Arc::new(domain::MemoryStatusRepository::new(
        [
            (todo, domain::StatusCategory::Todo, false),
            (working, domain::StatusCategory::InProgress, false),
            (done, domain::StatusCategory::Done, true),
        ]
        .into_iter()
        .map(|(id, category, is_closed)| domain::Status {
            id,
            name: "Fixture".into(),
            category,
            position: 0,
            is_default: false,
            is_closed,
        })
        .collect(),
    ));
    let sprint = domain::Sprint {
        id: shared::SprintId::new(),
        project_id: project.id,
        name: "Historical".into(),
        goal: None,
        state: domain::SprintState::Closed,
        start_date: Some(start),
        end_date: Some(start + chrono::Duration::days(3)),
        velocity: None,
    };
    let sprints = Arc::new(domain::MemorySprintRepository::default());
    sprints.save(&sprint).await.unwrap();
    let mut first = domain::Issue::create(
        &project,
        1,
        shared::IssueType::Task,
        done,
        "First",
        None,
        owner,
        shared::Priority::Medium,
    );
    first.created_at = start;
    first.sprint_id = Some(sprint.id);
    issues.save(&first).await.unwrap();
    for (number, created_at) in [(2, start), (3, start + chrono::Duration::hours(60))] {
        let mut issue = domain::Issue::create(
            &project,
            number,
            shared::IssueType::Task,
            todo,
            "Other",
            None,
            owner,
            shared::Priority::Medium,
        );
        issue.created_at = created_at;
        issue.sprint_id = Some(sprint.id);
        issues.save(&issue).await.unwrap();
    }
    let history = Arc::new(domain::MemoryIssueStatusHistoryRepository::default());
    for (from, to, day) in [(todo, working, 1), (working, done, 2)] {
        history
            .save_for_project(
                &domain::IssueStatusHistory {
                    id: shared::IssueStatusHistoryId::new(),
                    issue_id: first.id,
                    from_status_id: Some(from),
                    to_status_id: to,
                    changed_by_id: owner,
                    changed_at: start + chrono::Duration::days(day),
                },
                project.id,
            )
            .await
            .unwrap();
    }
    history
        .save_for_project(
            &domain::IssueStatusHistory {
                id: shared::IssueStatusHistoryId::new(),
                issue_id: shared::IssueId::new(),
                from_status_id: Some(todo),
                to_status_id: done,
                changed_by_id: owner,
                changed_at: start + chrono::Duration::hours(3),
            },
            shared::ProjectId::new(),
        )
        .await
        .unwrap();
    let repos = Arc::new(domain::Repositories {
        projects,
        issues,
        statuses,
        sprints,
        issue_status_history: history,
        ..Default::default()
    });
    let ctx = app::AppContext::new(
        Arc::new(shared::AppConfig::default()),
        repos,
        Arc::new(domain::InMemoryStorage::default()),
    );
    let velocity = ctx
        .services
        .report
        .get_velocity(project.id, 6, owner)
        .await
        .unwrap();
    assert_eq!(velocity.len(), 1);
    assert_eq!((velocity[0].committed, velocity[0].completed), (3, 1));
    let burndown = ctx
        .services
        .report
        .get_burndown(sprint.id, owner)
        .await
        .unwrap();
    assert_eq!(
        burndown
            .points
            .iter()
            .map(|p| p.remaining)
            .collect::<Vec<_>>(),
        vec![2, 2, 1, 2]
    );
    let cumulative = ctx
        .services
        .report
        .get_cumulative_flow(project.id, owner)
        .await
        .unwrap();
    assert_eq!(
        cumulative
            .iter()
            .map(|p| (p.todo, p.in_progress, p.done))
            .collect::<Vec<_>>(),
        vec![(2, 0, 0), (1, 1, 0), (1, 0, 1), (2, 0, 1)]
    );
    let control = ctx
        .services
        .report
        .get_control_chart(project.id, owner)
        .await
        .unwrap();
    assert_eq!(control.len(), 1);
    assert_eq!(control[0].issue_key, first.key.to_string());
    assert_eq!(control[0].cycle_time_days, 1.0);
}
