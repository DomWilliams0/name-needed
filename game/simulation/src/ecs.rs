use specs::prelude::*;
use specs::storage::InsertResult;
pub use specs::{
    world::EntitiesRes, Component, DenseVecStorage, Entity, HashMapStorage, Join, NullStorage,
    Read, ReadExpect, ReadStorage, System, SystemData, VecStorage, WorldExt, Write, WriteExpect,
    WriteStorage,
};
pub use specs_derive::Component;

use common::*;
#[cfg(test)]
pub use dummy::DummyComponentReceptacle;
use smallvec::alloc::fmt::Formatter;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::ops::Deref;
use world::WorldRef;

pub type EcsWorld = World;

/// World reference for the current frame only - very unsafe, don't store!
pub struct EcsWorldFrameRef(&'static EcsWorld);

pub fn entity_id(e: Entity) -> struclog::EntityId {
    ((e.gen().id() as u64) << 32) | e.id() as u64
}

#[macro_export]
macro_rules! entity_pretty {
    ($e:expr) => {
        format_args!("{}:{}", $e.gen().id(), $e.id())
    };
}

#[derive(Debug)]
pub struct NoSuchComponent(Entity, &'static str);

pub trait ComponentWorld {
    type Builder: ComponentBuilder;
    fn component<T: Component>(&self, entity: Entity) -> Result<&T, NoSuchComponent>;
    fn component_mut<T: Component>(&self, entity: Entity) -> Result<&mut T, NoSuchComponent>;

    fn resource<T: Resource>(&self) -> &T;
    fn resource_mut<T: Resource, F: FnOnce(&mut T) -> R, R>(&self, f: F) -> R;

    fn add_now<T: Component>(&mut self, entity: Entity, component: T) -> InsertResult<T>;
    fn remove_now<T: Component>(&mut self, entity: Entity) -> Option<T>;

    fn add_lazy<T: Component>(&self, entity: Entity, component: T);

    fn remove_lazy<T: Component>(&self, entity: Entity);

    fn voxel_world(&self) -> WorldRef;
    fn create_entity(&mut self) -> Self::Builder;
    fn kill_entity(&self, entity: Entity);
}

pub trait ComponentBuilder {
    fn with_<T: Component>(self, c: T) -> Self;
    fn build_(self) -> Entity;
}

impl ComponentWorld for EcsWorld {
    type Builder = EntityBuilder<'static>; // not really static OwO sorry

    fn component<T: Component>(&self, entity: Entity) -> Result<&T, NoSuchComponent> {
        let storage = self.read_storage::<T>();
        // safety: storage has the same lifetime as self, so its ok to "upcast" the components
        // lifetime from that of the storage to that of self
        let result: Option<&T> = unsafe { std::mem::transmute(storage.get(entity)) };
        result.ok_or_else(|| NoSuchComponent::new::<T>(entity))
    }

    fn component_mut<T: Component>(&self, entity: Entity) -> Result<&mut T, NoSuchComponent> {
        let mut storage = self.write_storage::<T>();
        // safety: storage has the same lifetime as self, so its ok to "upcast" the components
        // lifetime from that of the storage to that of self
        let result: Option<&mut T> = unsafe { std::mem::transmute(storage.get_mut(entity)) };
        result.ok_or_else(|| NoSuchComponent::new::<T>(entity))
    }

    fn resource<T: Resource>(&self) -> &T {
        let res = self.read_resource::<T>();
        // safety: storage has the same lifetime as self, so its ok to "upcast" the resource's
        // lifetime from that of the storage to that of self
        unsafe { std::mem::transmute(res.deref()) }
    }

    fn resource_mut<T: Resource, F: FnOnce(&mut T) -> R, R>(&self, f: F) -> R {
        // TODO transmute magic to remove closure
        let mut res = self.write_resource::<T>();
        f(&mut *res)
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

    fn create_entity(&mut self) -> EntityBuilder<'static> {
        // safety: builder's lifetime is self but we need GATs for that, lets pretend
        // it's static for now
        unsafe { std::mem::transmute(WorldExt::create_entity(self)) }
    }

    fn kill_entity(&self, entity: Entity) {
        let entities = self.read_resource::<EntitiesRes>();
        if let Err(e) = entities.delete(entity) {
            warn!("failed to delete entity {:?}: {}", entity, e);
        }
    }
}

impl<'a> ComponentBuilder for EntityBuilder<'a> {
    fn with_<T: Component>(self, c: T) -> Self {
        Builder::with(self, c)
    }

    fn build_(self) -> Entity {
        Builder::build(self)
    }
}

#[cfg(test)]
mod dummy {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use polymap::TypeMap;
    use specs::storage::InsertResult;
    use specs::Builder;

    use world::WorldRef;

    use crate::ecs::{
        Component, ComponentBuilder, ComponentWorld, EcsWorld, Entity, NoSuchComponent, WorldExt,
    };
    use specs::prelude::Resource;

    pub struct DummyComponentReceptacle {
        world: WorldRef,
        entity_allocs_only: EcsWorld,
        components: RefCell<HashMap<Entity, TypeMap>>,
    }

    impl DummyComponentReceptacle {
        pub fn new() -> Self {
            Self {
                entity_allocs_only: EcsWorld::new(),
                world: Default::default(),
                components: Default::default(),
            }
        }
    }

    impl ComponentWorld for DummyComponentReceptacle {
        type Builder = DummyEntityBuilder<'static>;

        fn component<T: Component>(&self, entity: Entity) -> Result<&T, NoSuchComponent> {
            let comps = self.components.borrow();
            let comp: &T = comps
                .get(&entity)
                .and_then(|comps| comps.get::<T>())
                .ok_or_else(|| NoSuchComponent::new::<T>(entity))?;

            let ok = Result::<&T, NoSuchComponent>::Ok(comp);
            // safety: transmute lifetime to outlive the `comps` borrow
            unsafe { std::mem::transmute(ok) }
        }

        fn component_mut<T: Component>(&self, _entity: Entity) -> Result<&mut T, NoSuchComponent> {
            unimplemented!()
        }

        fn resource<T: Resource>(&self) -> &T {
            unimplemented!()
        }

        fn resource_mut<T: Resource, F: FnOnce(&mut T) -> R, R>(&self, _f: F) -> R {
            unimplemented!()
        }

        fn add_now<T: Component>(&mut self, entity: Entity, component: T) -> InsertResult<T> {
            let mut comps = self.components.borrow_mut();
            Ok(comps.entry(entity).or_default().insert(component))
        }

        fn remove_now<T: Component>(&mut self, _entity: Entity) -> Option<T> {
            unimplemented!()
        }

        fn add_lazy<T: Component>(&self, _entity: Entity, _component: T) {
            unimplemented!()
        }

        fn remove_lazy<T: Component>(&self, _entity: Entity) {
            unimplemented!()
        }

        fn voxel_world(&self) -> WorldRef {
            self.world.clone()
        }

        fn create_entity(&mut self) -> DummyEntityBuilder<'static> {
            let entity = WorldExt::create_entity(&mut self.entity_allocs_only).build();
            // safety: see EcsWorld implementation
            unsafe {
                std::mem::transmute(DummyEntityBuilder {
                    entity,
                    world: self,
                })
            }
        }

        fn kill_entity(&self, _entity: Entity) {
            unimplemented!()
        }
    }

    pub struct DummyEntityBuilder<'a> {
        entity: Entity,
        world: &'a mut DummyComponentReceptacle,
    }

    impl<'a> ComponentBuilder for DummyEntityBuilder<'a> {
        fn with_<T: Component>(self, c: T) -> Self {
            self.world
                .add_now(self.entity, c)
                .expect("failed to add component");
            self
        }

        fn build_(self) -> Entity {
            self.entity
        }
    }
}
impl Display for NoSuchComponent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Either entity {:?} is dead or has no such component {}",
            self.0, self.1
        )
    }
}

impl Error for NoSuchComponent {}

impl NoSuchComponent {
    fn new<T>(entity: Entity) -> Self {
        Self(entity, std::any::type_name::<T>())
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
