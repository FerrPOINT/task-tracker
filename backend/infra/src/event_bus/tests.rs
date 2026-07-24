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
async fn event_bus_publish_and_subscribe_project() {
    let bus = EventBus::default();
    let mut rx = bus.subscribe();
    let event = DomainEvent::Project(ProjectEvent::Created {
        project_id: shared::ProjectId::new(),
        owner_id: shared::UserId::new(),
    });
    bus.publish(event);
    let received = rx.recv().await.unwrap();
    assert!(matches!(
        received,
        DomainEvent::Project(ProjectEvent::Created { .. })
    ));
}
