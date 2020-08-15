use specs::prelude::*;
use specs::storage::InsertResult;

use common::*;

use crate::event::{EntityEvent, EntityEventQueue};
use ::world::WorldRef;
use common::struclog;
use specs::world::EntitiesRes;
use specs::LazyUpdate;
use std::ops::{Deref, DerefMut};

pub type EcsWorld = World;

/// World reference for the current frame only - very unsafe, don't store!
pub struct EcsWorldFrameRef(&'static EcsWorld);

#[allow(dead_code)]
pub fn entity_id(e: Entity) -> struclog::EntityId {
    ((e.gen().id() as u64) << 32) | e.id() as u64
}

#[macro_export]
macro_rules! entity_pretty {
    ($e:expr) => {
        format_args!("{}:{}", $e.gen().id(), $e.id())
    };
}

#[derive(Debug, Error)]
pub enum ComponentGetError {
    #[error("The entity {:?} doesn't exist", _0)]
    NoSuchEntity(Entity),
    #[error("The entity {:?} doesn't have the given component '{}'", _0, _1)]
    NoSuchComponent(Entity, &'static str),
}

pub trait ComponentWorld {
    #[cfg(test)]
    fn test_new() -> Self;

    fn component<T: Component>(&self, entity: Entity) -> Result<&T, ComponentGetError>;
    fn component_mut<T: Component>(&self, entity: Entity) -> Result<&mut T, ComponentGetError>;

    fn resource<T: Resource>(&self) -> &T;
    #[allow(clippy::mut_from_ref)]
    fn resource_mut<T: Resource>(&self) -> &mut T;

    fn add_now<T: Component>(&mut self, entity: Entity, component: T) -> InsertResult<T>;
    fn remove_now<T: Component>(&mut self, entity: Entity) -> Option<T>;

    fn add_lazy<T: Component>(&self, entity: Entity, component: T);

    fn remove_lazy<T: Component>(&self, entity: Entity);

    fn voxel_world(&self) -> WorldRef;
    fn create_entity(&mut self) -> EntityBuilder;
    fn kill_entity(&self, entity: Entity);
    fn is_entity_alive(&self, entity: Entity) -> bool;

    // ---
    fn mk_component_error<T: Component>(&self, entity: Entity) -> ComponentGetError {
        if self.is_entity_alive(entity) {
            ComponentGetError::no_such_component::<T>(entity)
        } else {
            ComponentGetError::NoSuchEntity(entity)
        }
    }

    fn post_event(&mut self, event: EntityEvent) {
        let queue = self.resource_mut::<EntityEventQueue>();
        queue.post(event)
    }
}

impl ComponentWorld for EcsWorld {
    #[cfg(test)]
    fn test_new() -> Self {
        let mut w = Self::new();
        crate::simulation::register_components(&mut w);
        w.insert(WorldRef::default());
        w
    }

    fn component<T: Component>(&self, entity: Entity) -> Result<&T, ComponentGetError> {
        let storage = self.read_storage::<T>();
        // safety: storage has the same lifetime as self, so its ok to "upcast" the components
        // lifetime from that of the storage to that of self
        let result: Option<&T> = unsafe { std::mem::transmute(storage.get(entity)) };
        result.ok_or_else(|| self.mk_component_error::<T>(entity))
    }

    fn component_mut<T: Component>(&self, entity: Entity) -> Result<&mut T, ComponentGetError> {
        let mut storage = self.write_storage::<T>();
        // safety: storage has the same lifetime as self, so its ok to "upcast" the components
        // lifetime from that of the storage to that of self
        let result: Option<&mut T> = unsafe { std::mem::transmute(storage.get_mut(entity)) };
        result.ok_or_else(|| self.mk_component_error::<T>(entity))
    }

    fn resource<T: Resource>(&self) -> &T {
        let res = self.read_resource::<T>();
        // safety: storage has the same lifetime as self, so its ok to "upcast" the resource's
        // lifetime from that of the storage to that of self
        unsafe { std::mem::transmute(res.deref()) }
    }

    fn resource_mut<T: Resource>(&self) -> &mut T {
        let mut res = self.write_resource::<T>();
        // safety: storage has the same lifetime as self, so its ok to "upcast" the components
        // lifetime from that of the storage to that of self
        let res: &mut T = unsafe { std::mem::transmute(res.deref_mut()) };
        res
    }

    fn add_now<T: Component>(&mut self, entity: Entity, component: T) -> InsertResult<T> {
        let mut storage = self.write_storage::<T>();
        storage.insert(entity, component)
    }

    fn remove_now<T: Component>(&mut self, entity: Entity) -> Option<T> {
        let mut storage = self.write_storage::<T>();
        storage.remove(entity)
    }

    fn add_lazy<T: Component>(&self, entity: Entity, component: T) {
        let lazy = self.read_resource::<LazyUpdate>();
        lazy.insert(entity, component);
    }

    fn remove_lazy<T: Component>(&self, entity: Entity) {
        let lazy = self.read_resource::<LazyUpdate>();
        lazy.remove::<T>(entity);
    }

    fn voxel_world(&self) -> WorldRef {
        (*self.read_resource::<WorldRef>()).clone()
    }

    fn create_entity(&mut self) -> EntityBuilder {
        WorldExt::create_entity(self)
    }

    fn kill_entity(&self, entity: Entity) {
        let entities = self.read_resource::<EntitiesRes>();
        if let Err(e) = entities.delete(entity) {
            warn!("failed to delete entity {:?}: {}", entity, e);
        }
    }

    fn is_entity_alive(&self, entity: Entity) -> bool {
        // must check if generation is alive first to avoid panic
        entity.gen().is_alive() && self.is_alive(entity)
    }
}

impl EcsWorldFrameRef {
    pub unsafe fn init(world_ref: &EcsWorld) -> Self {
        Self(std::mem::transmute(world_ref))
    }
}

impl Default for EcsWorldFrameRef {
    fn default() -> Self {
        unreachable!("ecs world ref missing")
    }
}
impl Deref for EcsWorldFrameRef {
    type Target = EcsWorld;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl ComponentGetError {
    fn no_such_component<T>(entity: Entity) -> Self {
        Self::NoSuchComponent(entity, std::any::type_name::<T>())
    }
}
