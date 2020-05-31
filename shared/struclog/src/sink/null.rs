use crate::sink::EventSink;
use crate::{Event, Span};

#[derive(Default)]
pub struct NullSink;

impl EventSink for NullSink {
    fn on_attach(&mut self) {}

    fn on_detach(&mut self) {}

    fn enter_span(&mut self, _: Span) {}

    fn pop_span(&mut self) {}

    fn post(&mut self, _: Event) {}
}
