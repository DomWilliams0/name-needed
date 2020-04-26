use common::struclog;

use specs::prelude::*;
pub use specs::{
    world::EntitiesRes, Component, Entity, Join, Read, ReadExpect, ReadStorage, System, SystemData,
    VecStorage, Write, WriteExpect, WriteStorage,
};

pub type EcsWorld = World;

pub fn create_ecs_world() -> EcsWorld {
    World::new()
}

pub fn entity_id(e: Entity) -> struclog::EntityId {
    ((e.gen().id() as u64) << 32) | e.id() as u64
}
