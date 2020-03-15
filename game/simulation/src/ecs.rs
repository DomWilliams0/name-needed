use common::struclog;
pub use legion::prelude::{Entity, IntoQuery, Read, TryRead, TryWrite, Write};
use std::mem;

use world::WorldRef;

pub type EcsWorld = legion::world::World;

pub fn create_ecs_world() -> EcsWorld {
    let universe = legion::world::Universe::new();
    universe.create_world()
}

pub struct TickData<'a> {
    pub voxel_world: WorldRef,
    pub ecs_world: &'a mut EcsWorld,
}

pub trait System {
    fn tick_system(&mut self, data: &mut TickData);
}

pub fn entity_id(e: Entity) -> struclog::EntityId {
    debug_assert_eq!(
        mem::size_of::<Entity>(),
        mem::size_of::<struclog::EntityId>()
    );
    unsafe { mem::transmute(e) }
}
