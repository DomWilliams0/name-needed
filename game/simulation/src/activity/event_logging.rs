use std::borrow::Cow;
use std::collections::VecDeque;
use std::convert::TryInto;

use crate::activity::HaulTarget;
use common::*;
use unit::world::WorldPoint;

use crate::ecs::*;
use crate::simulation::Tick;
use crate::WorldPosition;

struct RingBuffer<T>(VecDeque<T>, usize);

#[derive(Component, EcsComponent)]
#[storage(HashMapStorage)]
#[name("entity-logs")]
#[clone(disallow)]
pub struct EntityLoggingComponent {
    logs: RingBuffer<TimedLoggedEntityEvent>,
}

struct TimedLoggedEntityEvent(Tick, LoggedEntityEvent);

/// An event that relates to an entity and is displayed in the ui. All variants relate to THIS entity
#[cfg_attr(feature = "testing", derive(Eq, PartialEq))]
pub enum LoggedEntityEvent {
    /// Equipped the given item
    Equipped(Entity),
    /// Ate the given item
    Eaten(Entity),
    /// Picked up the given item
    PickedUp(Entity),
    /// Made a decision to do something
    AiDecision(LoggedEntityDecision),
}

#[cfg_attr(feature = "testing", derive(Eq, PartialEq))]
pub enum LoggedEntityDecision {
    GoPickup(Cow<'static, str>),
    GoEquip(Entity),
    EatHeldItem(Entity),
    Wander,
    Goto(WorldPoint),
    GoBreakBlock(WorldPosition),
    Follow(Entity),
    Haul { item: Entity, dest: HaulTarget },
}

impl<T> RingBuffer<T> {
    pub fn with_capacity(cap: usize) -> Self {
        Self(VecDeque::with_capacity(cap), cap)
    }

    pub fn push(&mut self, elem: T) {
        if self.0.len() == self.1 {
            let _ = self.0.pop_front();
        }

        self.0.push_back(elem);
    }

    #[cfg(test)]
    fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        self.0.iter()
    }
}

impl Default for EntityLoggingComponent {
    fn default() -> Self {
        let capacity = config::get().simulation.entity_logging_capacity;
        Self {
            logs: RingBuffer::with_capacity(capacity),
        }
    }
}

impl EntityLoggingComponent {
    /// Same as [log_event] but only fetches the current tick once
    pub fn log_events(&mut self, events: impl Iterator<Item = impl TryInto<LoggedEntityEvent>>) {
        let tick = Tick::fetch();
        for event in events {
            if let Ok(e) = event.try_into() {
                self.logs.push(TimedLoggedEntityEvent(tick, e));
            }
        }
    }

    pub fn log_event(&mut self, event: impl TryInto<LoggedEntityEvent>) {
        if let Ok(e) = event.try_into() {
            self.logs.push(TimedLoggedEntityEvent(Tick::fetch(), e));
        }
    }

    pub fn iter_logs(&self) -> impl Iterator<Item = &dyn Display> + DoubleEndedIterator + '_ {
        self.logs.0.iter().map(|e| e as &dyn Display)
    }
    pub fn iter_raw_logs(&self) -> impl Iterator<Item = &LoggedEntityEvent> + '_ {
        self.logs.0.iter().map(|e| &e.1)
    }
}

impl Display for LoggedEntityEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use LoggedEntityDecision::*;
        use LoggedEntityEvent::*;

        match self {
            Equipped(e) => write!(f, "equipped {}", e),
            Eaten(e) => write!(f, "ate {}", e),
            PickedUp(e) => write!(f, "picked up {}", e),

            AiDecision(decision) => {
                write!(f, "decided to ")?;
                match decision {
                    GoPickup(what) => write!(f, "pickup nearby {}", what),
                    GoEquip(e) => write!(f, "go pickup {}", *e),
                    EatHeldItem(e) => write!(f, "eat held {}", e),
                    Wander => write!(f, "wander around"),
                    Goto(target) => write!(f, "go to {}", target),
                    GoBreakBlock(pos) => write!(f, "break the block at {}", pos),
                    Follow(e) => write!(f, "follow {}", e),
                    Haul { item, dest } => write!(f, "haul {} to {}", item, dest),
                }
            }
        }
    }
}

impl Display for TimedLoggedEntityEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "T{:06}: {}", self.0.value(), self.1)
    }
}

#[cfg(test)]
mod tests {
    use common::Itertools;

    use super::*;

    #[test]
    fn ring_buffer_basic() {
        let mut ring = RingBuffer::<i32>::with_capacity(4);

        ring.push(1);
        ring.push(2);
        ring.push(3);
        ring.push(4);
        assert_eq!(ring.iter().copied().collect_vec(), vec![1, 2, 3, 4]);

        ring.push(5);
        assert_eq!(ring.iter().copied().collect_vec(), vec![2, 3, 4, 5]);

        ring.push(6);
        assert_eq!(ring.iter().copied().collect_vec(), vec![3, 4, 5, 6]);
    }
}
