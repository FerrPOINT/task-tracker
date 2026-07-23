#[cfg(test)]
mod tests {
    use domain::{IssueEvent, ProjectEvent};
    use shared::{IssueId, ProjectId, UserId};

    use crate::{DomainEvent, EventBus, build_event_bus};

    #[test]
    fn event_bus_publish_and_subscribe() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        bus.publish(DomainEvent::Project(ProjectEvent::Created {
            project_id: ProjectId::new(),
            owner_id: UserId::new(),
        }));
        let event = rx.try_recv().expect("should receive event");
        assert!(matches!(event, DomainEvent::Project(_)));
    }

    #[test]
    fn build_event_bus_returns_arc() {
        let bus = build_event_bus();
        let mut rx = bus.subscribe();
        bus.publish(DomainEvent::Issue(IssueEvent::Created {
            issue_id: IssueId::new(),
            reporter_id: UserId::new(),
        }));
        assert!(rx.try_recv().is_ok());
    }
}
