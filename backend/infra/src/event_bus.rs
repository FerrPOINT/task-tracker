use std::sync::Arc;

use domain::{IssueEvent, ProjectEvent};
use tokio::sync::broadcast;

#[cfg(test)]
#[path = "event_bus/tests.rs"]
mod tests;

#[derive(Clone, Debug)]
pub enum DomainEvent {
    Issue(IssueEvent),
    Project(ProjectEvent),
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<DomainEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.tx.subscribe()
    }

    pub fn publish(&self, event: DomainEvent) {
        let _ = self.tx.send(event);
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Publishable {
    fn issue(event: IssueEvent) -> DomainEvent {
        DomainEvent::Issue(event)
    }

    fn project(event: ProjectEvent) -> DomainEvent {
        DomainEvent::Project(event)
    }
}

impl Publishable for EventBus {}

pub fn build_event_bus() -> Arc<EventBus> {
    Arc::new(EventBus::new())
}
