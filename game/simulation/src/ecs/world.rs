use specs::prelude::*;
use specs::storage::InsertResult;

use common::*;

use crate::ecs::{ComponentRegistry, InteractiveComponent, SpecsWorld, E};
use crate::event::{EntityEvent, EntityEventQueue};

use crate::definitions::{DefinitionBuilder, DefinitionErrorKind};
use crate::item::{ContainerComponent, ContainerResolver};
use crate::{definitions, WorldRef};
use specs::world::EntitiesRes;
use specs::LazyUpdate;
use std::ops::{Deref, DerefMut};

pub struct EcsWorld {
    world: SpecsWorld,
    component_registry: ComponentRegistry,
}

/// World reference for the current frame only - very unsafe, don't store!
pub struct EcsWorldFrameRef(&'static EcsWorld);

#[macro_export]
macro_rules! entity_pretty {
    ($e:expr) => {
        format_args!("{}:{}", $e.gen().id(), $e.id())
    };
}

#[derive(Debug, Error)]
pub enum ComponentGetError {
    #[error("The entity {} doesn't exist", E(*.0))]
    NoSuchEntity(Entity),

    #[error("The entity {} doesn't have the given component '{1}'", E(*.0))]
    NoSuchComponent(Entity, &'static str),
}

pub trait ComponentWorld: ContainerResolver + Sized {
    fn component<T: Component>(&self, entity: Entity) -> Result<&T, ComponentGetError>;
    fn component_mut<T: Component>(&self, entity: Entity) -> Result<&mut T, ComponentGetError>;
    fn has_component<T: Component>(&self, entity: Entity) -> bool;
    fn has_component_by_name(&self, comp: &str, entity: Entity) -> bool;
    fn components<J: Join>(&self, entity: Entity, storages: J) -> Option<J::Type>;

    fn resource<T: Resource>(&self) -> &T;
    #[allow(clippy::mut_from_ref)]
    fn resource_mut<T: Resource>(&self) -> &mut T;

    fn add_now<T: Component>(&mut self, entity: Entity, component: T) -> InsertResult<T>;
    fn remove_now<T: Component>(&mut self, entity: Entity) -> Option<T>;

    fn add_lazy<T: Component>(&self, entity: Entity, component: T);

    fn remove_lazy<T: Component>(&self, entity: Entity);

    fn voxel_world(&self) -> WorldRef;

    fn build_entity(
        &mut self,
        definition_uid: &str,
    ) -> Result<DefinitionBuilder<Self>, DefinitionErrorKind>;

    /// From specs:
    /// > You have to make sure that no component storage is borrowed during the building!
    fn create_entity(&self) -> EntityBuilder;

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

impl<W: ComponentWorld> ContainerResolver for W {
    fn container(&self, e: Entity) -> Option<&ContainerComponent> {
        self.component(e).ok()
    }

    fn container_mut(&self, e: Entity) -> Option<&mut ContainerComponent> {
        self.component_mut(e).ok()
    }
}

impl Deref for EcsWorld {
    type Target = SpecsWorld;

    fn deref(&self) -> &Self::Target {
        &self.world
    }
}

impl DerefMut for EcsWorld {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.world
    }
}

impl EcsWorld {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let mut world = SpecsWorld::new();
        let reg = ComponentRegistry::new(&mut world);

        EcsWorld {
            world,
            component_registry: reg,
        }
    }

    /// Iterates through all known component types and checks each one
    pub fn all_components_for(
        &self,
        entity: Entity,
    ) -> impl Iterator<Item = (&'static str, Option<&dyn InteractiveComponent>)> + '_ {
        self.component_registry.all_components_for(self, entity)
    }
}

impl ComponentWorld for EcsWorld {
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

    fn has_component<T: Component>(&self, entity: Entity) -> bool {
        let storage = self.read_storage::<T>();
        storage.contains(entity)
    }

    fn has_component_by_name(&self, comp: &str, entity: Entity) -> bool {
        self.component_registry.has_component(comp, self, entity)
    }

    fn components<J: Join>(&self, entity: Entity, storages: J) -> Option<J::Type> {
        let entities = self.read_resource::<EntitiesRes>();
        storages.join().get(entity, &entities.into())
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

    // TODO specs lazy updates allocs a Box for each action - when our QueuedUpdates uses an arena swap this out to use that instead
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

    fn build_entity(
        &mut self,
        definition_uid: &str,
    ) -> Result<DefinitionBuilder<Self>, DefinitionErrorKind> {
        let definitions = self.resource::<definitions::Registry>();
        definitions.instantiate(&*definition_uid, self)
    }

    fn create_entity(&self) -> EntityBuilder {
        WorldExt::create_entity_unchecked(&self.world)
    }

    fn kill_entity(&self, entity: Entity) {
        let entities = self.read_resource::<EntitiesRes>();
        if let Err(e) = entities.delete(entity) {
            warn!("failed to delete entity"; E(entity), "error" => %e);
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
