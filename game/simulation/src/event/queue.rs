use crate::activity::EventUnsubscribeResult;
use crate::ecs::Entity;
use crate::event::component::EntityEvent;
use crate::event::EntityEventSubscription;
use common::{Itertools, SmallVec};
use std::collections::hash_map::Entry;
use std::collections::HashMap;

// TODO event queue generic over event type
pub struct EntityEventQueue {
    events: Vec<EntityEvent>,
    unsubscribers: Vec<Entity>,

    /// subject -> interested subscriber and his subscriptions
    subscriptions: HashMap<Entity, SmallVec<[(Entity, EntityEventSubscription); 2]>>,
    needs_cleanup: bool,
}

impl Default for EntityEventQueue {
    fn default() -> Self {
        Self {
            events: Vec::with_capacity(512),
            unsubscribers: Vec::with_capacity(16),
            subscriptions: HashMap::with_capacity(64),
            needs_cleanup: false,
        }
    }
}

impl EntityEventQueue {
    pub fn subscribe(
        &mut self,
        subscriber: Entity,
        subscriptions: impl Iterator<Item = EntityEventSubscription>,
    ) {
        subscriptions
            .group_by(|EntityEventSubscription(subject, _)| *subject)
            .into_iter()
            .for_each(|(subject, subscriptions)| {
                let subscriptions = subscriptions.map(|sub| (subscriber, sub));

                match self.subscriptions.entry(subject) {
                    Entry::Occupied(mut e) => {
                        let subs = e.get_mut();
                        subs.extend(subscriptions);
                    }
                    Entry::Vacant(e) => {
                        let subs = e.insert(SmallVec::new());
                        subs.extend(subscriptions);
                    }
                };
            });
    }

    pub fn post(&mut self, event: EntityEvent) {
        self.events.push(event);
    }

    pub fn unsubscribe_all(&mut self, subscriber: Entity) {
        self.subscriptions
            .values_mut()
            .for_each(|subs| subs.retain(|(interested, _)| *interested == subscriber));
        self.needs_cleanup = true;
    }

    fn tidy_up(&mut self) {
        if std::mem::take(&mut self.needs_cleanup) {
            self.subscriptions.retain(|_, subs| !subs.is_empty());
        }
    }

    pub fn handle_events(
        &mut self,
        mut f: impl FnMut(Entity, &EntityEvent) -> EventUnsubscribeResult,
    ) {
        let grouped_events = self
            .events
            .iter()
            .group_by(|EntityEvent(subject, _)| *subject);

        for (subject, events) in grouped_events.into_iter() {
            // find subscribers interested in this subject entity
            let subscribers = match self.subscriptions.get(&subject) {
                Some(subs) => subs,
                None => {
                    // no subscribers
                    continue;
                }
            };

            for event in events {
                let payload = &event.1;

                for subscriber in subscribers.iter().filter_map(|(subscriber, sub)| {
                    if sub.1.matches(payload) {
                        Some(subscriber)
                    } else {
                        None
                    }
                }) {
                    if let EventUnsubscribeResult::UnsubscribeAll = f(*subscriber, event) {
                        self.unsubscribers.push(*subscriber);
                    }
                }
            }
        }

        // handle unsubscriptions
        // need to swap vec out from self to be able to access self mutably
        let unsubs = std::mem::take(&mut self.unsubscribers);
        for unsubscriber in unsubs.iter().copied() {
            self.unsubscribe_all(unsubscriber);
        }
        self.unsubscribers = unsubs;

        self.unsubscribers.clear();
        self.events.clear();

        // cleanup from unsubscribing
        self.tidy_up()
    }
}
