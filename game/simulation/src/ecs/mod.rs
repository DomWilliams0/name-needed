pub use specs::{
    world::EntitiesRes, Builder, Component, DenseVecStorage, EntityBuilder, HashMapStorage, Join,
    LazyUpdate, NullStorage, Read, ReadExpect, ReadStorage, RunNow, System, SystemData, VecStorage,
    WorldExt, Write, WriteExpect, WriteStorage,
};
pub use specs_derive::Component;

pub use component::{ComponentBuildError, ComponentEntry, InteractiveComponent, Map, Value};
pub use ecs_derive::EcsComponent;
pub use entity::{Entity, EntityWrapper};
pub use template::{ComponentTemplate, ComponentTemplateEntry, ValueImpl};

pub use crate::register_component_template;

pub use self::world::{
    ComponentGetError, ComponentRef, ComponentRefErased, ComponentRefMut, ComponentWorld, EcsWorld,
    SpecsWorld,
};

mod component;
mod entity;
mod tag;
mod template;
mod world;
mod world_ext;
