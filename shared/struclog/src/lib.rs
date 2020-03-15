pub use sink::{deinit, init};
pub use span::Span;

pub use crate::event::Verbosity;
pub use crate::event::{EntityEvent, EntityId, Event};
use crate::sink::Record;
use crate::span::SpanGuard;

mod event;
pub mod sink;
mod span;

pub fn event_info(e: Event) {
    event(Verbosity::Info, e);
}

pub fn event_verbose(e: Event) {
    event(Verbosity::Verbose, e);
}

pub fn event_error(e: Event) {
    event(Verbosity::Error, e);
}

pub fn event_trace(e: Event) {
    event(Verbosity::Trace, e);
}

#[inline(always)]
fn event(verbosity: Verbosity, e: Event) {
    if verbosity.should_log_static() {
        sink::post(Record(e, verbosity));
    }
}

pub fn enter_span(s: Span) -> SpanGuard {
    sink::enter_span(s);
    SpanGuard
}
