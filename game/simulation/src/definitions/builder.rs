use common::*;
use unit::world::{WorldPoint, WorldPosition};

use crate::definitions::loader::Definition;
use crate::definitions::DefinitionNameComponent;
use crate::ecs::*;
use crate::string::{CachedStr, StringCache};
use crate::{ComponentWorld, InnerWorldRef, TransformComponent};

pub trait EntityPosition {
    /// (position, optional rotation)
    fn resolve(&self, world: &InnerWorldRef) -> Result<(WorldPoint, Option<Deg>), BuilderError>;
}

#[derive(Debug, Error, Clone)]
pub enum BuilderError {
    #[error("No position specified for entity that requires a transform")]
    MissingPosition,

    #[error("Column is inaccessible: {0:?}")]
    InaccessibleColumn((i32, i32)),

    #[error("Position is not walkable: {0}")]
    PositionNotWalkable(WorldPosition),
}

#[must_use = "Use spawn() to create the entity"]
pub struct DefinitionBuilder<'d, W: ComponentWorld> {
    definition: &'d Definition,
    /// Unnecessary when the temporary DefinitionNameComponent is removed
    uid: CachedStr,
    world: &'d W,

    position: Option<Box<dyn EntityPosition>>,
    accessibility_required: bool,
}

impl<'d, W: ComponentWorld> DefinitionBuilder<'d, W> {
    pub fn new_with_cached(definition: &'d Definition, world: &'d W, uid: CachedStr) -> Self {
        Self {
            definition,
            world,
            uid,
            position: None,
            accessibility_required: true,
        }
    }

    pub fn new(definition: &'d Definition, world: &'d W, uid: &str) -> Self {
        Self::new_with_cached(definition, world, world.resource::<StringCache>().get(uid))
    }

    pub fn with_position<P: EntityPosition + 'static>(self, pos: P) -> Self {
        // TODO avoid box by resolving here and storing result
        Self {
            position: Some(Box::new(pos)),
            ..self
        }
    }

    pub fn doesnt_need_to_be_accessible(self) -> Self {
        Self {
            accessibility_required: false,
            ..self
        }
    }

    pub fn spawn(self) -> Result<Entity, BuilderError> {
        // resolve position if given
        let world_ref = self.world.voxel_world();
        let world = world_ref.borrow();
        let (pos, rot) = match self.position {
            Some(pos) => {
                let (point, rot) = pos.resolve(&world)?;
                let pos = point.floor();
                if !self.accessibility_required || world.area(pos).ok().is_some() {
                    (Some(point), rot)
                } else {
                    return Err(BuilderError::PositionNotWalkable(pos));
                }
            }
            None => (None, None),
        };

        let mut builder = self
            .world
            .create_entity()
            .with(DefinitionNameComponent(self.uid));

        for comp in self.definition.components() {
            builder = comp.instantiate(builder);
        }

        let entity = builder.build().into();

        // notify world
        self.world.on_new_entity_creation(entity);

        // set position in transform if present
        if let Ok(mut transform) = self.world.component_mut::<TransformComponent>(entity) {
            if let Some(pos) = pos {
                transform.reset_position(pos)
            } else {
                return Err(BuilderError::MissingPosition);
            }

            if let Some(rot) = rot {
                transform.rotate_to(rot.into());
            }
        }

        Ok(entity)
    }
}

impl EntityPosition for WorldPosition {
    fn resolve(&self, _: &InnerWorldRef) -> Result<(WorldPoint, Option<Deg>), BuilderError> {
        Ok((self.centred(), None))
    }
}

impl EntityPosition for WorldPoint {
    fn resolve(&self, _: &InnerWorldRef) -> Result<(WorldPoint, Option<Deg>), BuilderError> {
        Ok((*self, None))
    }
}

impl EntityPosition for (i32, i32) {
    fn resolve(&self, world: &InnerWorldRef) -> Result<(WorldPoint, Option<Deg>), BuilderError> {
        let (x, y) = *self;
        world
            .find_accessible_block_in_column(x, y)
            .ok_or(BuilderError::InaccessibleColumn((x, y)))
            .map(|pos| (pos.centred(), None))
    }
}

impl EntityPosition for (WorldPosition, f32) {
    fn resolve(&self, world: &InnerWorldRef) -> Result<(WorldPoint, Option<Deg>), BuilderError> {
        let (pos, _) = EntityPosition::resolve(&self.0, world)?;
        let rot = deg(self.1);
        Ok((pos, Some(rot)))
    }
}
