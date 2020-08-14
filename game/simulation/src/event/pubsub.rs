use std::collections::HashMap;
use std::rc::Rc;

use common::derive_more::From;
use common::*;

use crate::event::component::{EntityEventPayload, EntityEventType};

// TODO derive perfect hash for event types

// TODO subscribe with event handler typeid to disallow dupes?
// TODO weak reference to subscribers
pub type EventHandler = Rc<dyn EventSubscriber>;

#[derive(From)]
struct EventHandlerWrapper(EventHandler);

pub struct EventDispatcher {
    specific_subs: HashMap<EntityEventType, SmallVec<[EventHandlerWrapper; 2]>>,
    all_subs: Vec<EventHandlerWrapper>,
}

// TODO use a bitmask for event subscription instead of a special case for all
#[derive(Clone, Debug)]
pub enum EventSubscription {
    All,
    Specific(EntityEventType),
}

pub trait EventSubscriber {
    fn handle(&self, event: &EntityEventPayload);
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self {
            specific_subs: HashMap::new(),
            all_subs: Vec::new(),
        }
    }

    pub fn subscribe(&mut self, subscription: EventSubscription, handler: EventHandler) {
        // TODO ensure handler is not already subscribed
        match subscription {
            EventSubscription::All => {
                self.all_subs.push(handler.into());
            }
            EventSubscription::Specific(ty) => self
                .specific_subs
                .entry(ty)
                .or_insert_with(|| SmallVec::new())
                .push(handler.into()),
        }
    }

    // TODO ideally we should be able to pass a reference here rather than a rc clone
    pub fn unsubscribe(&mut self, subscription: EventSubscription, handler: EventHandler) {
        match subscription {
            EventSubscription::All => {
                if let Some(idx) = self.all_subs.iter().position(|h| h == handler) {
                    self.all_subs.swap_remove(idx); // order doesn't matter
                }
            }
            EventSubscription::Specific(ty) => {
                self.specific_subs.entry(ty).and_modify(|handlers| {
                    if let Some(idx) = handlers.iter().position(|h| h == handler) {
                        handlers.swap_remove(idx); // order doesn't matter
                    }
                });
            }
        }
        // TODO intelligently shrink subscriber lists at some point to avoid monotonic increase in mem usage
    }

    pub fn publish(&self, event: &EntityEventPayload) {
        let event_type = event.into();

        let handlers = {
            let specific_handlers = self
                .specific_subs
                .get(&event_type)
                .map(|handlers| handlers.iter())
                .into_iter()
                .flatten();

            let any_handlers = self.all_subs.iter();

            any_handlers.chain(specific_handlers)
        };

        for handler in handlers {
            handler.0.handle(event);
        }
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq<EventHandler> for &EventHandlerWrapper {
    fn eq(&self, other: &EventHandler) -> bool {
        Rc::ptr_eq(&self.0, other)
    }
}

impl PartialEq<EventHandler> for EventHandlerWrapper {
    fn eq(&self, other: &EventHandler) -> bool {
        Rc::ptr_eq(&self.0, other)
    }
}

// impl PartialEq<EventHandlerWrapper> for EventHandler{
//     fn eq(&self, other: &EventHandlerWrapper) -> bool {
//         Rc::ptr_eq(self, &other.0)
//     }
// }
//
impl PartialEq<EventHandlerWrapper> for EventHandlerWrapper {
    fn eq(&self, other: &EventHandlerWrapper) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for EventHandlerWrapper {}

impl EventSubscription {
    pub fn matches(&self, event: &EntityEventPayload) -> bool {
        match self {
            EventSubscription::All => true,
            EventSubscription::Specific(ty) => *ty == EntityEventType::from(event),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn pub_sub() {
        let mut dispatcher = EventDispatcher::new();

        #[derive(PartialEq, Eq)]
        struct SpecificSub;
        impl EventSubscriber for SpecificSub {
            fn handle(&self, event: &EntityEventPayload) {
                assert!(matches!(event, &EntityEventPayload::DummyB));
            }
        }

        #[derive(PartialEq, Eq)]
        struct AnySub(Cell<usize>);
        impl EventSubscriber for AnySub {
            fn handle(&self, _: &EntityEventPayload) {
                self.0.set(self.0.get() + 1)
            }
        }

        let specific = Rc::new(SpecificSub);
        let any = Rc::new(AnySub(Cell::new(0)));

        // TODO try with no subs
        dispatcher.subscribe(
            EventSubscription::Specific(EntityEventType::Arrived),
            specific.clone(),
        );
        dispatcher.subscribe(EventSubscription::All, any.clone());

        dispatcher.publish(&EntityEventPayload::DummyB);
        dispatcher.publish(&EntityEventPayload::DummyA);

        dispatcher.unsubscribe(EventSubscription::All, any.clone());
        dispatcher.publish(&EntityEventPayload::DummyA);

        assert_eq!(any.0.get(), 2);
    }
}
