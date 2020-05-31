use crate::event::Event;
use crate::Span;
use lazy_static::lazy_static;
use parking_lot::Mutex;

#[cfg(test)]
pub mod test;

#[cfg(feature = "json")]
pub mod json;

#[cfg(feature = "null")]
pub mod null;

pub trait EventSink: Send {
    // fn new() -> Box<Self> where Self: Sized;

    fn on_attach(&mut self);
    fn on_detach(&mut self);

    fn enter_span(&mut self, span: Span);
    fn pop_span(&mut self);

    fn post(&mut self, event: Event);
    fn post_batch(&mut self, events: Vec<Event>) {
        for e in events {
            self.post(e)
        }
    }
}

lazy_static! {
    static ref GLOBAL_SINK: Mutex<Option<Box<dyn EventSink>>> = Mutex::new(None);
}

pub fn do_with<F: FnOnce(&mut Box<dyn EventSink>) -> ()>(f: F) {
    let global: &mut Option<Box<dyn EventSink>> = &mut GLOBAL_SINK.lock();

    if let Some(sink) = global.as_mut() {
        f(sink);
    }
}

pub fn init() {
    #[allow(clippy::match_single_binding)]
    let sink = Box::new(match () {
        #[cfg(feature = "json")]
        _ => json::JsonPipeSink::default(),
        #[cfg(feature = "null")]
        _ => null::NullSink::default(),
        #[cfg(not(any(feature = "null", feature = "json")))]
        _ => compile_error!("no sink chosen with features"),
    }) as Box<dyn EventSink>;
    do_init(Some(sink))
}

pub fn deinit() {
    do_init(None)
}

fn do_init(sink: Option<Box<dyn EventSink>>) {
    let global: &mut Option<Box<dyn EventSink>> = &mut GLOBAL_SINK.lock();

    if let Some(mut old) = global.take() {
        old.on_detach();
    }

    if let Some(sink) = sink {
        *global = Some(sink);
        global.as_mut().unwrap().on_attach();
    }
}

pub(crate) fn enter_span(s: Span) {
    do_with(|sink| sink.enter_span(s));
}

pub(crate) fn pop_span() {
    do_with(|sink| sink.pop_span());
}

pub fn post<E: Into<Event>>(event: E) {
    let event = event.into();
    do_with(move |sink| sink.post(event));
}

pub fn post_batch(events: Vec<Event>) {
    do_with(move |sink| sink.post_batch(events));
}

#[cfg(test)]
mod tests {
    /*
    use crate::event::{EntityEvent, Event};
    use crate::sink::test::TestSink;
    use crate::sink::EventSink;
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

        *//*
        assert_eq!(
            sink.filter_entity(20).collect::<Vec<_>>(),
            vec![(&vec![Span::Setup, Span::Tick], &EntityEvent::Create(20))]
        );

        assert!(sink.filter_entity(100).next().is_none());
        *//*
    }
    */
}
