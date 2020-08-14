use crate::activity::EventUnsubscribeResult;
use crate::ecs::Entity;
use crate::event::component::EntityEvent;
use crate::event::EntityEventSubscription;
use common::{Itertools, SmallVec};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

// TODO event queue generic over event type
pub struct EntityEventQueue {
    events: Vec<EntityEvent>,
    unsubscribers: HashSet<Entity>,

    /// subject -> interested subscriber and his subscriptions
    subscriptions: HashMap<Entity, SmallVec<[(Entity, EntityEventSubscription); 2]>>,
    needs_cleanup: bool,
}

impl Default for EntityEventQueue {
    fn default() -> Self {
        Self {
            events: Vec::with_capacity(512),
            unsubscribers: HashSet::with_capacity(16),
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

        for (subject, sub) in self.subscriptions.iter() {
            common::info!("SUBJECT {} => {:#?}", crate::entity_pretty!(subject), sub);
        }
    }

    pub fn post(&mut self, event: EntityEvent) {
        common::debug!("posting event: {:?}", event);
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
            // TODO only tidy up occasionally rather than every time e.g. when threshold is reached
            self.subscriptions.retain(|_, subs| !subs.is_empty());
        }
    }

    pub fn handle_events(
        &mut self,
        mut f: impl FnMut(Entity, &EntityEvent) -> EventUnsubscribeResult,
    ) {
        let grouped_events = self.events.iter().group_by(|evt| evt.subject);

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
                for subscriber in subscribers.iter().filter_map(|(subscriber, sub)| {
                    if sub.1.matches(&event.payload) {
                        Some(subscriber)
                    } else {
                        None
                    }
                }) {
                    // already subscribed, no more events pls
                    if !self.unsubscribers.contains(subscriber) {
                        if let EventUnsubscribeResult::UnsubscribeAll = f(*subscriber, event) {
                            self.unsubscribers.insert(*subscriber);
                        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::WorldExt;
    use crate::event::{EntityEventPayload, EntityEventType, EventSubscription};
    use common::once;
    use specs::Builder;
    use unit::world::WorldPoint;

    fn make_entities() -> (Entity, Entity) {
        {
            let mut w = crate::ecs::EcsWorld::new();
            let a = w.create_entity().build();
            let b = w.create_entity().build();
            (a, b)
        }
    }

    #[test]
    fn subscription() {
        let mut q = EntityEventQueue::default();
        let (e1, e2) = make_entities();

        let evt_1_arrived = EntityEvent {
            subject: e1,
            payload: EntityEventPayload::Arrived(WorldPoint::default()),
        };

        let evt_1_dummy = EntityEvent {
            subject: e1,
            payload: EntityEventPayload::Dummy,
        };
        let evt_2_dummy = EntityEvent {
            subject: e2,
            payload: EntityEventPayload::Dummy,
        };

        // no subs yet
        q.post(evt_1_arrived.clone());
        q.post(evt_1_dummy.clone());
        q.handle_events(|_, _| panic!("no subs"));

        // sub e2 to e1's arrival only
        q.subscribe(
            e2,
            once(EntityEventSubscription(
                e1,
                EventSubscription::Specific(EntityEventType::Arrived),
            )),
        );

        q.post(evt_1_arrived.clone());
        q.post(evt_1_dummy.clone());
        q.handle_events(|subscriber, e| {
            assert_eq!(subscriber, e2);
            assert_eq!(e.subject, e1);
            assert!(matches!(e.payload, EntityEventPayload::Arrived(_)));
            EventUnsubscribeResult::UnsubscribeAll
        });

        // subscribe to e1 all
        q.subscribe(
            e2,
            once(EntityEventSubscription(e1, EventSubscription::All)),
        );
        q.post(evt_1_arrived.clone());
        q.post(evt_1_dummy.clone());
        q.post(evt_2_dummy.clone());
        let mut arrival_done = false;
        q.handle_events(|subscriber, e| {
            assert_eq!(subscriber, e2);
            assert_eq!(e.subject, e1);

            match &e.payload {
                EntityEventPayload::Arrived(_) => {
                    assert!(!arrival_done);
                    arrival_done = true;
                }
                EntityEventPayload::Dummy => {
                    assert!(arrival_done);
                }
                _ => unreachable!(),
            }

            EventUnsubscribeResult::UnsubscribeAll
        });
    }

    #[test]
    fn repeated_subscriptions() {
        let mut q = EntityEventQueue::default();
        let (e1, e2) = make_entities();

        let evt_1_arrived = EntityEvent {
            subject: e1,
            payload: EntityEventPayload::Arrived(WorldPoint::default()),
        };

        q.subscribe(
            e2,
            once(EntityEventSubscription(
                e1,
                EventSubscription::Specific(EntityEventType::Arrived),
            )),
        );
        q.subscribe(
            e2,
            once(EntityEventSubscription(
                e1,
                EventSubscription::Specific(EntityEventType::Arrived),
            )),
        );
        q.subscribe(
            e2,
            once(EntityEventSubscription(
                e1,
                EventSubscription::Specific(EntityEventType::Dummy),
            )),
        );
        q.subscribe(
            e2,
            once(EntityEventSubscription(e1, EventSubscription::All)),
        );
        q.subscribe(
            e2,
            once(EntityEventSubscription(e1, EventSubscription::All)),
        );

        q.post(evt_1_arrived.clone());
        let mut count = 0;
        q.handle_events(|_, _| {
            count += 1;
            EventUnsubscribeResult::StaySubscribed
        });

        assert_eq!(count, 1);
    }
}
