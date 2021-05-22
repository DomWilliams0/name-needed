use crate::activity::HaulTarget;

use crate::ecs::*;

use crate::simulation::Tick;
use crate::WorldPosition;
use common::*;
use std::borrow::Cow;
use std::collections::VecDeque;
use std::convert::TryInto;
use unit::world::WorldPoint;

struct RingBuffer<T>(VecDeque<T>, usize);

#[derive(Component, EcsComponent)]
#[storage(HashMapStorage)]
#[name("entity-logs")]
pub struct EntityLoggingComponent {
    logs: RingBuffer<TimedLoggedEntityEvent>,
}

struct TimedLoggedEntityEvent(Tick, LoggedEntityEvent);

/// An event that relates to an entity and is displayed in the ui. All variants relate to THIS entity
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

pub enum LoggedEntityDecision {
    GoPickup(Cow<'static, str>),
    EatHeldItem(Entity),
    Wander,
    Goto {
        target: WorldPoint,
        reason: &'static str,
    },
    GoBreakBlock(WorldPosition),
    Follow(Entity),
    Haul {
        item: Entity,
        dest: HaulTarget,
    },
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

    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        self.0.iter()
    }
}

impl Default for EntityLoggingComponent {
    fn default() -> Self {
        Self {
            // TODO get initial size from config
            logs: RingBuffer::with_capacity(64),
        }
    }
}

impl EntityLoggingComponent {
    pub fn log_event(&mut self, event: impl TryInto<LoggedEntityEvent>) {
        // TODO dont allocate string here
        // TODO pass in an impl LogEvent instead
        // TODO optimise for the multiple case
        if let Ok(e) = event.try_into() {
            self.logs.push(TimedLoggedEntityEvent(Tick::fetch(), e));
        }
    }

    pub fn iter_logs(&self) -> impl Iterator<Item = &dyn Display> + '_ {
        self.logs.iter().map(|e| e as &dyn Display)
    }
}

impl Display for LoggedEntityEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use LoggedEntityDecision::*;
        use LoggedEntityEvent::*;
        match self {
            Equipped(e) => write!(f, "equipped {}", E(*e)),
            Eaten(e) => write!(f, "ate {}", E(*e)),
            PickedUp(e) => write!(f, "picked up {}", E(*e)),

            AiDecision(decision) => {
                write!(f, "decided to ")?;
                match decision {
                    GoPickup(what) => write!(f, "pickup nearby {}", what),
                    EatHeldItem(e) => write!(f, "eat held {}", E(*e)),
                    Wander => write!(f, "wander around"),
                    Goto { target, reason } => write!(f, "go to {} because {}", target, *reason),
                    GoBreakBlock(pos) => write!(f, "break the block at {}", pos),
                    Follow(e) => write!(f, "follow {}", E(*e)),
                    Haul { item, dest } => write!(f, "haul {} to {}", E(*item), dest),
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
    use super::*;
    use common::Itertools;

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
