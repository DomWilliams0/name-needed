use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::World;

/// Reference counted reference to the world
pub struct WorldRef<D>(Arc<RwLock<World<D>>>);

// safety: contains an Arc and Mutex
unsafe impl<D> Send for WorldRef<D> {}
unsafe impl<D> Sync for WorldRef<D> {}

pub type InnerWorldRef<'a, D> = RwLockReadGuard<'a, World<D>>;
pub type InnerWorldRefMut<'a, D> = RwLockWriteGuard<'a, World<D>>;

impl<D> WorldRef<D> {
    pub fn new(world: World<D>) -> Self {
        Self(Arc::new(RwLock::new(world)))
    }

    // TODO don't unwrap()

    pub fn borrow(&self) -> InnerWorldRef<'_, D> {
        (*self.0).read().unwrap()
    }

    pub fn borrow_mut(&self) -> InnerWorldRefMut<'_, D> {
        (*self.0).write().unwrap()
    }

    #[cfg(test)]
    pub fn into_inner(self) -> World<D> {
        let mutex = Arc::try_unwrap(self.0).unwrap_or_else(|arc| {
            panic!(
                "exclusive world reference needed but there are {}",
                Arc::strong_count(&arc)
            )
        });
        mutex.into_inner().expect("world lock is poisoned")
    }
}

impl<D> Default for WorldRef<D> {
    fn default() -> Self {
        WorldRef(Arc::new(RwLock::new(World::default())))
    }
}
impl<D> Clone for WorldRef<D> {
    fn clone(&self) -> Self {
        WorldRef(Arc::clone(&self.0))
    }
}
