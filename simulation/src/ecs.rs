use std::fmt::{Display, Error, Formatter};
use std::num::Wrapping;

pub use pyro::{All, Read, Write};
use pyro::{ComponentId, Entity, StorageId, Version};

use world::WorldRef;

pub type EcsWorld = pyro::World<pyro::SoaStorage>;

pub struct TickData<'a> {
    pub voxel_world: WorldRef,
    pub ecs_world: &'a mut EcsWorld,
}

pub trait System {
    fn tick_system(&mut self, data: &TickData);
}

/// Marker for components
pub trait Component {}

#[derive(Copy, Clone)]
pub struct NiceEntity(pub Entity);

/// Identical to pyro's Entity so we can transmute to it for nice formatting....
struct PyroEntity {
    /// Removing entities will increment the versioning. Accessing an [`Entity`] with an
    /// outdated version will result in a `panic`. `version` does wrap on overflow.
    version: Wrapping<Version>,
    /// The id of the storage where the [`Entity`] lives in
    _storage_id: StorageId,
    /// The actual id inside a storage
    id: ComponentId,
}

impl Display for NiceEntity {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let copy: PyroEntity = unsafe { std::mem::transmute(self.0) };
        write!(f, "Entity[{}:{}]", copy.version, copy.id)
    }
}
