use crate::ecs::EcsWorld;
use common::*;
use std::cell::RefCell;
use std::error::Error;

type Update = dyn FnOnce(&mut EcsWorld) -> Result<(), Box<dyn Error>>;
type Entry = (&'static str, Box<Update>);

pub struct QueuedUpdates {
    updates: RefCell<Vec<Entry>>,
}

impl Default for QueuedUpdates {
    fn default() -> Self {
        Self {
            updates: RefCell::new(Vec::with_capacity(256)),
        }
    }
}

impl QueuedUpdates {
    pub fn queue<F: 'static + FnOnce(&mut EcsWorld) -> Result<(), Box<dyn Error>>>(
        &self,
        name: &'static str,
        update: F,
    ) {
        // TODO pool/reuse these boxes
        let update = Box::new(update);
        self.updates.borrow_mut().push((name, update))
    }

    pub fn execute(&mut self, world: &mut EcsWorld) {
        let mut vec = self.updates.borrow_mut();
        if !vec.is_empty() {
            debug!("running {} queued updates", vec.len());

            for (name, update) in vec.drain(..) {
                match update(world) {
                    Err(e) => warn!("queued update '{}' failed: {}", name, e),
                    Ok(_) => trace!("queued update '{}' was successful", name),
                }
            }
        }
    }
}
