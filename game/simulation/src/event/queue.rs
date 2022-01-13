use std::collections::hash_map::Entry;
use std::collections::HashMap;

use common::{num_traits::FromPrimitive, *};

use crate::ecs::Entity;

use crate::event::prelude::*;

type BitsetInner = u32;

#[derive(Default)]
struct BitSet(BitsetInner);

// TODO event queue generic over event type
pub struct EntityEventQueue {
    events: Vec<EntityEvent>,

    /// subject -> interested subscriber and his subscriptions
    subscriptions: HashMap<Entity, SmallVec<[(Entity, BitSet); 2]>>,
    needs_cleanup: u32,
}

impl Default for EntityEventQueue {
    fn default() -> Self {
        Self {
            events: Vec::with_capacity(512),
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
        for (subject, subscriptions) in subscriptions.group_by(|sub| sub.subject).into_iter() {
            let subscriptions = subscriptions.map(|sub| sub.subscription);

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
        }
    }

    pub fn post(&mut self, event: EntityEvent) {
        debug!("posting event"; "event" => ?event);
        self.events.push(event);
    }

    pub fn post_multiple(&mut self, events: impl Iterator<Item = EntityEvent>) {
        let len_before = self.events.len();
        self.events.extend(events);

        let n = self.events.len() - len_before;
        debug!("posting {} events", n; "events" => ?&self.events[len_before..]);
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

        trace!(
            "unsubscribed {subscriber} from {count}",
            subscriber = subscriber,
            count = removals
        );
        self.needs_cleanup += removals;
    }

    pub fn unsubscribe(&mut self, subscriber: Entity, unsubscription: EntityEventSubscription) {
        if let Some(subs) = self.subscriptions.get_mut(&unsubscription.subject) {
            if let Some(idx) = subs.iter().position(|(e, _)| *e == subscriber) {
                let (_, bitset) = unsafe { subs.get_unchecked_mut(idx) };
                if bitset.remove(unsubscription.subscription) {
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

    /// A chance to view all events before consuming them
    pub fn events(&self) -> impl Iterator<Item = &EntityEvent> + '_ {
        self.events.iter()
    }

    /// Consumes all events posted since the last call.
    ///
    /// * f: called per subscribed entity, f(subscriber, event). If returns false, the subscriber
    ///  is erroneous and won't get any more events
    pub fn consume_events(&mut self, mut f: impl FnMut(Entity, EntityEvent) -> bool) {
        // move out of self
        let mut events = std::mem::take(&mut self.events);

        let grouped_events = events.drain(..).group_by(|evt| evt.subject);

        // shouldn't happen often if ever, so no need to cache this allocation in self
        let mut erroneous_subscribers = Vec::new();

        for (subject, events) in grouped_events.into_iter() {
            // find subscribers interested in this subject entity
            let subscribers = match self.subscriptions.get(&subject) {
                Some(subs) => subs,
                None => {
                    // no subscribers
                    let count = if logger().is_trace_enabled() {
                        events.count()
                    } else {
                        0
                    };
                    trace!("dropping {count} events because subject has no subscribers",
                        count = count; "subject" => subject
                    );
                    continue;
                }
            };

            // pass events to subscriptions
            for event in events {
                let event_type = EntityEventType::from(&event.payload);
                for subscriber in subscribers
                    .iter()
                    .filter_map(|(subscriber, sub)| sub.contains(event_type).as_some(subscriber))
                {
                    // need to clone event for each iteration, and we don't know how many iterations
                    // there will be up-front
                    let event = event.clone();

                    debug!("passing event"; "subscriber" => subscriber, "event" => ?&event);
                    let not_erroneous = f(*subscriber, event);

                    // events are passed asynchronously to handlers now, so we can't know at this
                    // point whether or not to unsubscribe

                    if !not_erroneous {
                        warn!("skipping remaining events for erroneous subscriber"; "subscriber" => subscriber);
                        erroneous_subscribers.push(*subscriber);
                        break;
                    }
                }
            }
        }

        drop(grouped_events);

        // unsubscribe erroneous subscribers from all
        // need to swap vec out from self to be able to access self mutably
        for subscriber in erroneous_subscribers {
            self.unsubscribe_all(subscriber)
        }

        // swap back allocation
        self.events.clear();
        let dummy = std::mem::replace(&mut self.events, events);
        debug_assert!(dummy.is_empty());
        std::mem::forget(dummy);

        self.maintain()
    }

    #[allow(dead_code)] // used for debugging
    pub fn log(&self) {
        if !logger().is_trace_enabled() {
            return;
        }

        trace!(">>> event subscribers >>>");
        for (subject, subs) in self.subscriptions.iter() {
            let count = subs.iter().filter(|(_, subs)| !subs.is_empty()).count();
            if count > 0 {
                trace!(
                    "subject has {count} subscribers",
                    count = count;
                    "subject" => subject,
                );
                for (subscriber, bitset) in subs {
                    if !bitset.is_empty() {
                        trace!("subscriptions for {subscriber}", subscriber = *subscriber; "subscriptions" => ?bitset);
                    }
                }
            }
        }
        trace!("<<< event subscribers <<<");
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
            EventSubscription::Specific(evt) => self.0 |= 1 << (evt as BitsetInner),
            EventSubscription::All => self.0 = BitsetInner::MAX,
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
        self.contains_type(ty.into() as BitsetInner)
    }

    fn contains_type(&self, ordinal: BitsetInner) -> bool {
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
                let bit = 1 << ty as BitsetInner;
                self.0 &= !bit;
                self.is_empty()
            }
        }
    }

    fn iter(&self) -> impl Iterator<Item = EntityEventType> + '_ {
        let bit_count = (std::mem::size_of::<BitsetInner>() * 8) as BitsetInner;
        (0..bit_count).filter_map(move |ord| {
            if self.contains_type(ord) {
                let ty = EntityEventType::from_u32(ord)
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
        if self.0 == BitsetInner::MAX {
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
    use common::once;

    use crate::ecs::*;
    use crate::ComponentWorld;

    use super::*;

    fn make_entities() -> (Entity, Entity) {
        {
            let w = EcsWorld::new();
            let a = w.create_entity().build();
            let b = w.create_entity().build();
            (a.into(), b.into())
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
        q.consume_events(|_, _| panic!("no subs"));

        // sub e2 to e1's dummy A only
        q.subscribe(
            e2,
            once(EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::Specific(EntityEventType::DummyA),
            }),
        );

        q.post(evt_1_dummy_a.clone());
        q.post(evt_1_dummy_b.clone());
        q.consume_events(|subscriber, e| {
            assert_eq!(subscriber, e2);
            assert_eq!(e.subject, e1);
            assert!(matches!(e.payload, EntityEventPayload::DummyA));
            false
        });

        // subscribe to e1 all
        q.subscribe(
            e2,
            once(EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::All,
            }),
        );
        q.post(evt_1_dummy_a);
        q.post(evt_1_dummy_b);
        q.post(evt_2_dummy_b);

        let mut dummy_a = 0;
        let mut dummy_b = 0;
        q.consume_events(|subscriber, e| {
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

            true
        });

        assert_eq!(dummy_a, 1);
        assert_eq!(dummy_b, 1);
    }

    fn count_events(q: &mut EntityEventQueue) -> usize {
        let mut count = 0;
        q.consume_events(|_, _| {
            count += 1;
            true
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
            once(EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::Specific(EntityEventType::DummyA),
            }),
        );
        q.subscribe(
            e2,
            once(EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::Specific(EntityEventType::DummyA),
            }),
        );
        q.subscribe(
            e2,
            once(EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::Specific(EntityEventType::DummyB),
            }),
        );
        q.subscribe(
            e2,
            once(EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::All,
            }),
        );
        q.subscribe(
            e2,
            once(EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::All,
            }),
        );

        q.post(evt_1_dummy_a);
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

            q.post(evt_1_dummy_a);
            q.post(evt_1_dummy_b);
            q.post(evt_2_dummy_a);
            q.post(evt_2_dummy_b);

            count_events(q)
        };

        // initially sub to all
        q.subscribe(
            e1,
            once(EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::All,
            }),
        );
        q.subscribe(
            e1,
            once(EntityEventSubscription {
                subject: e2,
                subscription: EventSubscription::All,
            }),
        );

        assert_eq!(count_events(&mut q), 4);

        // get rid of e1 subs manually
        q.unsubscribe(
            e1,
            EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::Specific(EntityEventType::DummyA),
            },
        );
        q.unsubscribe(
            e1,
            EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::Specific(EntityEventType::DummyB),
            },
        );

        // e2 has no subs, no effect
        q.unsubscribe(
            e2,
            EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::Specific(EntityEventType::DummyB),
            },
        );

        // repeated unsub, no effect
        q.unsubscribe(
            e1,
            EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::Specific(EntityEventType::DummyB),
            },
        );
        q.unsubscribe(
            e1,
            EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::Specific(EntityEventType::DummyB),
            },
        );

        // e2 events only
        assert_eq!(count_events(&mut q), 2);

        // resub manually
        q.subscribe(
            e1,
            once(EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::Specific(EntityEventType::DummyA),
            }),
        );

        // unsub from all e2
        q.unsubscribe(
            e1,
            EntityEventSubscription {
                subject: e2,
                subscription: EventSubscription::All,
            },
        );

        assert_eq!(count_events(&mut q), 1);

        // unsub from all
        q.subscribe(
            e1,
            once(EntityEventSubscription {
                subject: e2,
                subscription: EventSubscription::All,
            }),
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
        // logging::for_tests();

        let mut q = EntityEventQueue::default();
        let (e1, e2) = make_entities();

        // both subscribe to e1 dummy_b event
        q.subscribe(
            e1,
            once(EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::Specific(EntityEventType::DummyB),
            }),
        );
        q.subscribe(
            e2,
            once(EntityEventSubscription {
                subject: e1,
                subscription: EventSubscription::Specific(EntityEventType::DummyB),
            }),
        );

        q.log();

        q.post(EntityEvent {
            subject: e1,
            payload: EntityEventPayload::DummyB,
        });

        let mut subs = Vec::with_capacity(2);
        q.consume_events(|sub, _| {
            subs.push(sub);
            true
        });

        assert_eq!(subs.len(), 2);
    }
}
