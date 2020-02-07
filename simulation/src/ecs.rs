use legion;
pub use legion::prelude::{Entity, IntoQuery, Read, TryRead, TryWrite, Write};

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
