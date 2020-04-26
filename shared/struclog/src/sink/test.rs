use crate::sink::EventSink;
use crate::{EntityEvent, EntityId, Event, Span};

#[derive(Default)]
pub struct TestSink {
    pub events: Vec<(Vec<Span>, Event)>,
    pub spans: Vec<Span>,
}

impl EventSink for TestSink {
    fn on_attach(&mut self) {}

    fn on_detach(&mut self) {}

    fn enter_span(&mut self, s: Span) {
        self.spans.push(s);
    }

    fn pop_span(&mut self) {
        self.spans.pop();
    }

    fn post(&mut self, e: Event) {
        self.events.push((self.spans.clone(), e));
    }
}

impl TestSink {
    pub fn filter_entity(
        &self,
        entity_id: EntityId,
    ) -> impl Iterator<Item = (&Vec<Span>, &EntityEvent)> {
        self.events
            .iter()
            .filter_map(|(spans, event)| match event {
                Event::Entity(e) => Some((spans, e)),
                _ => None,
            })
            .filter(move |(_, e)| e.entity_id() == entity_id)
    }
}
