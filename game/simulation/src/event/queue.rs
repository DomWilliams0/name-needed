use crate::activity::EventUnsubscribeResult;
use crate::ecs::Entity;
use crate::event::subscription::{EntityEvent, EventSubscription};
use crate::event::{EntityEventSubscription, EntityEventType};
use common::{num_traits::FromPrimitive, *};
use std::collections::hash_map::Entry;
use std::collections::HashMap;

#[derive(Default)]
struct BitSet(usize);

// TODO event queue generic over event type
pub struct EntityEventQueue {
    events: Vec<EntityEvent>,
    unsubscribers: HashMap<Entity, Option<EntityEventSubscription>>,

    /// subject -> interested subscriber and his subscriptions
    subscriptions: HashMap<Entity, SmallVec<[(Entity, BitSet); 2]>>,
    needs_cleanup: u32,
}

impl Default for EntityEventQueue {
    fn default() -> Self {
        Self {
            events: Vec::with_capacity(512),
            unsubscribers: HashMap::with_capacity(64),
            subscriptions: HashMap::with_capacity(64),
            needs_cleanup: 0,
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
                let subscriptions = subscriptions.map(|EntityEventSubscription(_, sub)| sub);

                match self.subscriptions.entry(subject) {
                    Entry::Occupied(mut e) => {
                        let subs = e.get_mut();
                        let existing = subs.iter_mut().find(|(sub, _)| *sub == subscriber);

                        if let Some((_, bitset)) = existing {
                            bitset.add_all(subscriptions);
                        } else {
                            let bitset = BitSet::with(subscriptions);
                            subs.push((subscriber, bitset));
                        }
                    }
                    Entry::Vacant(e) => {
                        let bitset = BitSet::with(subscriptions);
                        e.insert(smallvec![(subscriber, bitset)]);
                    }
                };
            });
    }

    pub fn post(&mut self, event: EntityEvent) {
        common::debug!("posting event: {:?}", event);
        self.events.push(event);
    }

    pub fn unsubscribe_all(&mut self, subscriber: Entity) {
        let mut removals = 0;
        self.subscriptions
            .values_mut()
            .flat_map(|subs| subs.iter_mut())
            .filter(|(interested, _)| *interested == subscriber)
            .for_each(|(_, bitset)| {
                bitset.clear();
                removals += 1;
            });

        self.needs_cleanup += removals;
    }

    pub fn unsubscribe(&mut self, subscriber: Entity, unsubscription: EntityEventSubscription) {
        let EntityEventSubscription(subject, sub) = unsubscription;
        if let Some(subs) = self.subscriptions.get_mut(&subject) {
            if let Some(idx) = subs.iter().position(|(e, _)| *e == subscriber) {
                let (_, bitset) = unsafe { subs.get_unchecked_mut(idx) };
                if bitset.remove(sub) {
                    self.needs_cleanup += 1;
                }
            }
        }
    }

    fn maintain(&mut self) {
        // TODO track by game tick instead of just number of ops
        if self.needs_cleanup > 500 {
            self.needs_cleanup = 0;
            self.subscriptions.retain(|_, subs| {
                subs.retain(|(_, bitset)| !bitset.is_empty());
                !subs.is_empty()
            });
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
                    if sub.contains(&event.payload) {
                        Some(subscriber)
                    } else {
                        None
                    }
                }) {
                    let unsubscribed_already = self
                        .unsubscribers
                        .get(subscriber)
                        .map(|unsub| match unsub {
                            None => true, // unsub from all
                            Some(sub) => sub.matches(event),
                        })
                        .unwrap_or(false);

                    if unsubscribed_already {
                        // already unsubscribed, no more events pls
                        continue;
                    }

                    let result = f(*subscriber, event);
                    let unsubscription = match result {
                        EventUnsubscribeResult::UnsubscribeAll => None,
                        EventUnsubscribeResult::Unsubscribe(subs) => Some(subs),
                        EventUnsubscribeResult::StaySubscribed => continue,
                    };

                    self.unsubscribers.insert(*subscriber, unsubscription);
                }
            }
        }

        // handle unsubscriptions
        // need to swap vec out from self to be able to access self mutably
        let mut unsubs = std::mem::take(&mut self.unsubscribers);
        for (unsubscriber, unsubs) in unsubs.drain() {
            match unsubs {
                None => {
                    debug!(
                        "unsubscribing {} from all subscriptions",
                        crate::entity_pretty!(unsubscriber)
                    );
                    self.unsubscribe_all(unsubscriber)
                }
                Some(unsub) => {
                    debug!(
                        "unsubscribing {} from {:?}",
                        crate::entity_pretty!(unsubscriber),
                        unsub
                    );
                    self.unsubscribe(unsubscriber, unsub)
                }
            }
        }
        self.unsubscribers = unsubs;
        self.events.clear();

        self.maintain()
    }

    pub fn log(&self) {
        for (subject, subs) in self.subscriptions.iter() {
            let count = subs.iter().filter(|(_, subs)| !subs.is_empty()).count();
            if count > 0 {
                debug!(
                    "subject {} has {} subscribers",
                    crate::entity_pretty!(subject),
                    count,
                );
                for (subscriber, bitset) in subs {
                    if !bitset.is_empty() {
                        debug!(" - {} -> {:?}", crate::entity_pretty!(subscriber), bitset);
                    }
                }
            }
        }
    }
}

impl BitSet {
    pub fn with(subscriptions: impl Iterator<Item = EventSubscription>) -> Self {
        let mut bits = Self::default();
        bits.add_all(subscriptions);
        bits
    }

    pub fn add(&mut self, subscription: EventSubscription) {
        match subscription {
            EventSubscription::Specific(evt) => self.0 |= 1 << (evt as usize),
            EventSubscription::All => self.0 = usize::MAX,
        }
    }

    pub fn add_all(&mut self, subscriptions: impl Iterator<Item = EventSubscription>) {
        for sub in subscriptions {
            let is_all = matches!(sub, EventSubscription::All);

            self.add(sub);

            if is_all {
                // all further subs are a nop
                break;
            }
        }
    }

    pub fn contains<E: Into<EntityEventType>>(&self, ty: E) -> bool {
        self.contains_type(ty.into() as usize)
    }

    fn contains_type(&self, ordinal: usize) -> bool {
        let bit = 1 << ordinal;
        self.0 & bit != 0
    }

    pub fn clear(&mut self) {
        self.0 = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Returns true if now is empty
    pub fn remove(&mut self, unsubscription: EventSubscription) -> bool {
        match unsubscription {
            EventSubscription::All => {
                self.clear();
                true
            }
            EventSubscription::Specific(ty) => {
                let bit = 1usize << ty as usize;
                self.0 &= !bit;
                self.is_empty()
            }
        }
    }

    fn iter(&self) -> impl Iterator<Item = EntityEventType> + '_ {
        let bit_count = std::mem::size_of::<usize>() * 8;
        (0..bit_count).filter_map(move |ord| {
            if self.contains_type(ord) {
                let ty = EntityEventType::from_usize(ord)
                    .unwrap_or_else(|| panic!("invalid event type bit set {}", ord));
                Some(ty)
            } else {
                None
            }
        })
    }
}

impl Debug for BitSet {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.0 == usize::MAX {
            write!(f, "Bitset(ALL)")
        } else {
            write!(f, "Bitset(")?;
            let mut list = f.debug_set();
            list.entries(self.iter());
            list.finish()?;
            write!(f, ")")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::WorldExt;
    use crate::event::{EntityEventPayload, EntityEventType};
    use common::once;
    use specs::Builder;

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

        let evt_1_dummy_a = EntityEvent {
            subject: e1,
            payload: EntityEventPayload::DummyA,
        };

        let evt_1_dummy_b = EntityEvent {
            subject: e1,
            payload: EntityEventPayload::DummyB,
        };
        let evt_2_dummy_b = EntityEvent {
            subject: e2,
            payload: EntityEventPayload::DummyB,
        };

        // no subs yet
        q.post(evt_1_dummy_a.clone());
        q.post(evt_1_dummy_b.clone());
        q.handle_events(|_, _| panic!("no subs"));

        // sub e2 to e1's dummy A only
        q.subscribe(
            e2,
            once(EntityEventSubscription(
                e1,
                EventSubscription::Specific(EntityEventType::DummyA),
            )),
        );

        q.post(evt_1_dummy_a.clone());
        q.post(evt_1_dummy_b.clone());
        q.handle_events(|subscriber, e| {
            assert_eq!(subscriber, e2);
            assert_eq!(e.subject, e1);
            assert!(matches!(e.payload, EntityEventPayload::DummyA));
            EventUnsubscribeResult::UnsubscribeAll
        });

        // subscribe to e1 all
        q.subscribe(
            e2,
            once(EntityEventSubscription(e1, EventSubscription::All)),
        );
        q.post(evt_1_dummy_a.clone());
        q.post(evt_1_dummy_b.clone());
        q.post(evt_2_dummy_b.clone());

        let mut dummy_a = 0;
        let mut dummy_b = 0;
        q.handle_events(|subscriber, e| {
            assert_eq!(subscriber, e2);
            assert_eq!(e.subject, e1);

            match &e.payload {
                EntityEventPayload::DummyA => {
                    dummy_a += 1;
                }
                EntityEventPayload::DummyB => {
                    dummy_b += 1;
                }
                _ => unreachable!(),
            }

            EventUnsubscribeResult::StaySubscribed
        });

        assert_eq!(dummy_a, 1);
        assert_eq!(dummy_b, 1);
    }

    fn count_events(q: &mut EntityEventQueue) -> usize {
        let mut count = 0;
        q.handle_events(|_, _| {
            count += 1;
            EventUnsubscribeResult::StaySubscribed
        });

        count
    }

    #[test]
    fn repeated_subscriptions() {
        let mut q = EntityEventQueue::default();
        let (e1, e2) = make_entities();

        let evt_1_dummy_a = EntityEvent {
            subject: e1,
            payload: EntityEventPayload::DummyB,
        };

        q.subscribe(
            e2,
            once(EntityEventSubscription(
                e1,
                EventSubscription::Specific(EntityEventType::DummyA),
            )),
        );
        q.subscribe(
            e2,
            once(EntityEventSubscription(
                e1,
                EventSubscription::Specific(EntityEventType::DummyA),
            )),
        );
        q.subscribe(
            e2,
            once(EntityEventSubscription(
                e1,
                EventSubscription::Specific(EntityEventType::DummyB),
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

        q.post(evt_1_dummy_a.clone());
        assert_eq!(count_events(&mut q), 1);
    }

    #[test]
    fn unsubscribe() {
        let mut q = EntityEventQueue::default();
        let (e1, e2) = make_entities();

        let count_events = |q: &mut EntityEventQueue| {
            let evt_1_dummy_a = EntityEvent {
                subject: e1,
                payload: EntityEventPayload::DummyA,
            };
            let evt_1_dummy_b = EntityEvent {
                subject: e1,
                payload: EntityEventPayload::DummyB,
            };
            let evt_2_dummy_a = EntityEvent {
                subject: e2,
                payload: EntityEventPayload::DummyA,
            };
            let evt_2_dummy_b = EntityEvent {
                subject: e2,
                payload: EntityEventPayload::DummyB,
            };

            q.post(evt_1_dummy_a.clone());
            q.post(evt_1_dummy_b.clone());
            q.post(evt_2_dummy_a.clone());
            q.post(evt_2_dummy_b.clone());

            count_events(q)
        };

        // initially sub to all
        q.subscribe(
            e1,
            once(EntityEventSubscription(e1, EventSubscription::All)),
        );
        q.subscribe(
            e1,
            once(EntityEventSubscription(e2, EventSubscription::All)),
        );

        assert_eq!(count_events(&mut q), 4);

        // get rid of e1 subs manually
        q.unsubscribe(
            e1,
            EntityEventSubscription(e1, EventSubscription::Specific(EntityEventType::DummyA)),
        );
        q.unsubscribe(
            e1,
            EntityEventSubscription(e1, EventSubscription::Specific(EntityEventType::DummyB)),
        );

        // e2 has no subs, no effect
        q.unsubscribe(
            e2,
            EntityEventSubscription(e1, EventSubscription::Specific(EntityEventType::DummyB)),
        );

        // repeated unsub, no effect
        q.unsubscribe(
            e1,
            EntityEventSubscription(e1, EventSubscription::Specific(EntityEventType::DummyB)),
        );
        q.unsubscribe(
            e1,
            EntityEventSubscription(e1, EventSubscription::Specific(EntityEventType::DummyB)),
        );

        // e2 events only
        assert_eq!(count_events(&mut q), 2);

        // resub manually
        q.subscribe(
            e1,
            once(EntityEventSubscription(
                e1,
                EventSubscription::Specific(EntityEventType::DummyA),
            )),
        );

        // unsub from all e2
        q.unsubscribe(e1, EntityEventSubscription(e2, EventSubscription::All));

        assert_eq!(count_events(&mut q), 1);

        // unsub from all
        q.subscribe(
            e1,
            once(EntityEventSubscription(e2, EventSubscription::All)),
        );

        q.unsubscribe_all(e1);
        assert_eq!(count_events(&mut q), 0);
    }

    #[test]
    fn is_subscription_actually_what_we_want() {
        let mut bitset = BitSet::default();
        bitset.add(EventSubscription::Specific(EntityEventType::DummyB));

        assert!(bitset.contains(EntityEventType::DummyB));

        let subs = bitset.iter().collect_vec();
        assert_eq!(subs, vec![EntityEventType::DummyB]);
    }

    #[test]
    fn multiple_subscribers() {
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Trace)
            .is_test(true)
            .try_init();

        let mut q = EntityEventQueue::default();
        let (e1, e2) = make_entities();

        // both subscribe to e1 dummy_b event
        q.subscribe(
            e1,
            once(EntityEventSubscription(
                e1,
                EventSubscription::Specific(EntityEventType::DummyB),
            )),
        );
        q.subscribe(
            e2,
            once(EntityEventSubscription(
                e1,
                EventSubscription::Specific(EntityEventType::DummyB),
            )),
        );

        q.log();

        q.post(EntityEvent {
            subject: e1,
            payload: EntityEventPayload::DummyB,
        });

        let mut subs = Vec::with_capacity(2);
        q.handle_events(|sub, _| {
            subs.push(sub);
            EventUnsubscribeResult::StaySubscribed
        });

        assert_eq!(subs.len(), 2);
    }
}
