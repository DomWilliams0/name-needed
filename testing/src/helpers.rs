use crate::HookContext;
use common::BoxedResult;
use simulation::{ComponentWorld, Entity, WorldPosition};
use std::error::Error;

pub enum EntityPosition {
    Origin,
    Far,
    Custom((i32, i32)),
}

impl HookContext<'_> {
    pub fn new_human(&self, pos: EntityPosition) -> BoxedResult<Entity> {
        self.new_entity("core_living_human", pos)
    }

    pub fn new_entity(&self, def: &str, pos: EntityPosition) -> BoxedResult<Entity> {
        let pos = match pos {
            EntityPosition::Origin => (0, 0),
            EntityPosition::Far => (8, 8),
            EntityPosition::Custom(pos) => pos,
        };

        let e = self
            .simulation
            .ecs
            .build_entity(def)?
            .with_position(pos)
            .spawn()?;
        Ok(e)
    }
}
