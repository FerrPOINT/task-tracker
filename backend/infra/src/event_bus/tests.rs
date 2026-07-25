use super::*;
use domain::{IssueEvent, ProjectEvent};

#[tokio::test]
async fn event_bus_publish_and_subscribe_issue() {
    let bus = EventBus::new();
    let mut rx = bus.subscribe();
    let event = DomainEvent::Issue(IssueEvent::Created {
        issue_id: shared::IssueId::new(),
        reporter_id: shared::UserId::new(),
    });
    bus.publish(event);
    let received = rx.recv().await.unwrap();
    assert!(matches!(
        received,
        DomainEvent::Issue(IssueEvent::Created { .. })
    ));
}

#[tokio::test]
async fn event_bus_publishable_helpers() {
    let bus = EventBus::new();
    let issue_event = EventBus::issue(IssueEvent::Created {
        issue_id: shared::IssueId::new(),
        reporter_id: shared::UserId::new(),
    });
    let project_event = EventBus::project(ProjectEvent::Created {
        project_id: shared::ProjectId::new(),
        owner_id: shared::UserId::new(),
    });
    assert!(matches!(
        issue_event,
        DomainEvent::Issue(IssueEvent::Created { .. })
    ));
    assert!(matches!(
        project_event,
        DomainEvent::Project(ProjectEvent::Created { .. })
    ));
    let _ = bus.subscribe();
    bus.publish(issue_event);
    bus.publish(project_event);
}

#[tokio::test]
async fn event_bus_build_returns_arc() {
    let bus = build_event_bus();
    bus.publish(DomainEvent::Issue(IssueEvent::Created {
        issue_id: shared::IssueId::new(),
        reporter_id: shared::UserId::new(),
    }));
    assert_eq!(bus.subscribe().len(), 0);
}
