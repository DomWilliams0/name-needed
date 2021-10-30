use crate::ecs::ComponentGetError;

use crate::activity::context::ActivityContext;
use crate::queued_update::QueuedUpdates;
use crate::{ComponentWorld, TerrainUpdatesRes, WorldPositionRange, WorldTerrainUpdate};
use crate::{TransformComponent, WorldPosition};
use common::*;
use unit::world::WorldPoint;
use world::block::{Block, BlockType};

#[derive(Debug, Error)]
pub enum BuildBlockError {
    #[error("Bad entity with no transform")]
    MissingTransform(#[from] ComponentGetError),

    #[error("Too far from block {target} to break it from {current}")]
    TooFar {
        current: WorldPoint,
        target: WorldPosition,
    },

    #[error("Block is invalid or non-air")]
    BadBlock,
}

#[derive(Default)]
pub struct BuildBlockSubactivity;

impl BuildBlockSubactivity {
    pub async fn build_block(
        &self,
        ctx: &ActivityContext,
        block: WorldPosition,
        bt: BlockType,
    ) -> Result<(), BuildBlockError> {
        // check we are close enough
        let pos = ctx
            .world()
            .component::<TransformComponent>(ctx.entity())
            .map_err(BuildBlockError::MissingTransform)?
            .position;

        if pos.distance2(block) > 5.0 {
            return Err(BuildBlockError::TooFar {
                current: pos,
                target: block,
            });
        }

        // simulate work lol
        ctx.wait(3).await;

        let world = ctx.world().voxel_world();
        let world = world.borrow();

        match world.block(block).map(|b| b.block_type()) {
            Some(BlockType::Air) => { /* nice */ }
            Some(current) if current == bt => {
                trace!("block is already build target type"; "block" => %block, "type" => ?current);
            }
            Some(_) | None => {
                trace!("cannot build non-air or invalid block"; "block" => %block);
                return Err(BuildBlockError::BadBlock);
            }
        };

        // TODO start a build process in the world that has progress, takes ingredients, and takes time
        let terrain_updates = ctx.world().resource_mut::<TerrainUpdatesRes>();
        terrain_updates.push(WorldTerrainUpdate::new(
            WorldPositionRange::with_single(block),
            bt,
        ));

        Ok(())
    }
}
