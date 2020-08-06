mod component;
mod template;
mod world;

pub use specs::{
    world::EntitiesRes, Component, DenseVecStorage, Entity, HashMapStorage, Join, LazyUpdate,
    NullStorage, Read, ReadExpect, ReadStorage, System, SystemData, VecStorage, WorldExt, Write,
    WriteExpect, WriteStorage,
};
pub use specs_derive::Component;

pub use self::world::{entity_id, ComponentGetError, ComponentWorld, EcsWorld, EcsWorldFrameRef};
pub use crate::register_component_template;
pub use component::{ComponentBuildError, Map, Value};
pub use template::{ComponentTemplate, ComponentTemplateEntry, ValueImpl};
