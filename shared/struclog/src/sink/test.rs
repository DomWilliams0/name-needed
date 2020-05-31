use crate::sink::EventSink;
use crate::{Event, Span};

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
