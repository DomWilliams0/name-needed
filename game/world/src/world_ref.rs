use crate::World;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Reference counted reference to the world
#[derive(Default, Clone)]
pub struct WorldRef(Arc<RwLock<World>>);

// safety: contains an Arc and Mutex
unsafe impl Send for WorldRef {}
unsafe impl Sync for WorldRef {}

pub type InnerWorldRef<'a> = RwLockReadGuard<'a, World>;
pub type InnerWorldRefMut<'a> = RwLockWriteGuard<'a, World>;

impl WorldRef {
    pub fn new(world: World) -> Self {
        Self(Arc::new(RwLock::new(world)))
    }

    // TODO don't unwrap()

    pub fn borrow(&self) -> InnerWorldRef<'_> {
        (*self.0).read().unwrap()
    }

    pub fn borrow_mut(&self) -> InnerWorldRefMut<'_> {
        (*self.0).write().unwrap()
    }

    #[cfg(test)]
    pub fn into_inner(self) -> World {
        self.borrow().clone()
    }
}
