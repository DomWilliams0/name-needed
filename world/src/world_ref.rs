#![allow(clippy::non_send_fields_in_send_ty)]
use misc::parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{World, WorldContext};
use std::sync::Arc;

/// Reference counted reference to the world
#[repr(transparent)]
pub struct WorldRef<C: WorldContext>(Arc<RwLock<World<C>>>);

pub type InnerWorldRef<'a, C> = RwLockReadGuard<'a, World<C>>;
pub type InnerWorldRefMut<'a, C> = RwLockWriteGuard<'a, World<C>>;

impl<C: WorldContext> WorldRef<C> {
    pub fn new(world: World<C>) -> Self {
        Self(Arc::new(RwLock::new(world)))
    }

    pub fn borrow(&self) -> InnerWorldRef<'_, C> {
        (*self.0).read()
    }

    pub fn borrow_mut(&self) -> InnerWorldRefMut<'_, C> {
        (*self.0).write()
    }

    #[cfg(test)]
    pub fn into_inner(self) -> World<C> {
        let mutex = Arc::try_unwrap(self.0).unwrap_or_else(|arc| {
            panic!(
                "exclusive world reference needed but there are {}",
                Arc::strong_count(&arc)
            )
        });
        mutex.into_inner()
    }
}

impl<C: WorldContext> Default for WorldRef<C> {
    fn default() -> Self {
        WorldRef(Arc::new(RwLock::new(World::default())))
    }
}
impl<C: WorldContext> Clone for WorldRef<C> {
    fn clone(&self) -> Self {
        WorldRef(Arc::clone(&self.0))
    }
}
