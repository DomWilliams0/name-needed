use crate::ecs::*;
use crate::event::EntityEvent;
use std::collections::VecDeque;

struct RingBuffer<T>(VecDeque<T>, usize);

#[derive(Component, EcsComponent)]
#[storage(HashMapStorage)]
#[name("entity-logs")]
pub struct EntityLoggingComponent {
    // TODO use enums instead of strings
    logs: RingBuffer<String>,
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
    pub fn log_event(&mut self, event: &EntityEvent) {
        // TODO dont allocate string here
        // TODO pass in an impl LogEvent instead
        // TODO optimise for the multiple case
        self.logs.push(format!("{:?}", event.payload));
    }

    pub fn iter_logs(&self) -> impl Iterator<Item = &str> + '_ {
        self.logs.iter().map(|s| s.as_str())
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
