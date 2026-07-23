#[cfg(test)]
mod tests {
    use crate::{AppCache, DomainEvent, EventBus, build_event_bus};

    #[tokio::test]
    async fn cache_get_set_roundtrip() {
        let cache = AppCache::new();
        cache.set("k".to_string(), "v".to_string()).await;
        assert_eq!(cache.get("k").await, Some("v".to_string()));
        assert_eq!(cache.get("missing").await, None);
    }

    #[test]
    fn event_bus_publish_and_subscribe() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        bus.publish(DomainEvent::Project(domain::ProjectEvent::Created {
            project_id: shared::ProjectId::new(),
            owner_id: shared::UserId::new(),
        }));
        let event = rx.try_recv().expect("should receive event");
        assert!(matches!(event, DomainEvent::Project(_)));
    }

    #[test]
    fn build_event_bus_returns_arc() {
        let bus = build_event_bus();
        let mut rx = bus.subscribe();
        bus.publish(DomainEvent::Issue(domain::IssueEvent::Created {
            issue_id: shared::IssueId::new(),
            reporter_id: shared::UserId::new(),
        }));
        assert!(rx.try_recv().is_ok());
    }
}
