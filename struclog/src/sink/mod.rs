use crate::event::{Event, Verbosity};
use crate::span::Span;
use lazy_static::lazy_static;
use parking_lot::Mutex;

#[cfg(test)]
pub mod test;

#[cfg(feature = "ipc")]
pub mod ipc;

#[derive(Copy, Clone)]
pub struct Record(pub Event, pub Verbosity);

pub trait EventSink: Send {
    fn on_attach(&mut self);
    fn on_detach(&mut self);

    fn enter_span(&mut self, s: Span);
    fn pop_span(&mut self);

    fn post(&mut self, e: Event);
}

lazy_static! {
    static ref GLOBAL_SINK: Mutex<Option<Box<dyn EventSink>>> = Mutex::new(None);
}

pub fn do_with<F: FnMut(&mut Box<dyn EventSink>) -> ()>(mut f: F) {
    let global: &mut Option<Box<dyn EventSink>> = &mut GLOBAL_SINK.lock();

    if let Some(sink) = global.as_mut() {
        f(sink);
    }
}

pub fn init(sink: Option<Box<dyn EventSink>>) {
    let global: &mut Option<Box<dyn EventSink>> = &mut GLOBAL_SINK.lock();

    if let Some(mut old) = global.take() {
        old.on_detach();
    }

    if let Some(sink) = sink {
        *global = Some(sink);
        global.as_mut().unwrap().on_attach();
    }
}

pub fn deinit() {
    init(None)
}

pub fn enter_span(s: Span) {
    do_with(|sink| sink.enter_span(s));
}

pub fn pop_span() {
    do_with(|sink| sink.pop_span());
}

pub fn post(r: Record) {
    if r.1.should_log_static() {
        do_with(|sink| sink.post(r.0));
    }
}

#[cfg(test)]
mod tests {
    use crate::event::{EntityEvent, Event, Verbosity};
    use crate::sink::test::TestSink;
    use crate::sink::{EventSink, Record};
    use crate::span::Span;

    #[test]
    fn sink() {
        let mut sink = TestSink::default();

        sink.enter_span(Span::Setup);
        sink.post(Event::Entity(EntityEvent::Create(10)));

        sink.enter_span(Span::Tick);
        sink.post(Event::Entity(EntityEvent::Create(20)));
        sink.pop_span();

        sink.post(Event::Entity(EntityEvent::Create(30)));
        sink.pop_span();

        assert_eq!(sink.spans, vec![]);
        assert_eq!(
            sink.events,
            vec![
                (vec![Span::Setup], Event::Entity(EntityEvent::Create(10))),
                (
                    vec![Span::Setup, Span::Tick],
                    Event::Entity(EntityEvent::Create(20)),
                ),
                (vec![Span::Setup], Event::Entity(EntityEvent::Create(30))),
            ]
        );

        assert_eq!(
            sink.filter_entity(20).collect::<Vec<_>>(),
            vec![(&vec![Span::Setup, Span::Tick], &EntityEvent::Create(20))]
        );

        assert!(sink.filter_entity(100).next().is_none());
    }
}
