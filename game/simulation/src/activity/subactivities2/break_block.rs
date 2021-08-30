use crate::activity::activity2::ActivityContext2;
use crate::activity::EventUnsubscribeResult;
use crate::ecs::ComponentGetError;
use crate::event::prelude::*;
use crate::queued_update::QueuedUpdates;
use crate::{unexpected_event2, TransformComponent, WorldPosition};
use crate::{ComponentWorld, FollowPathComponent};
use common::*;
use unit::world::WorldPoint;
use world::{NavigationError, SearchGoal};

#[derive(Debug, Error)]
pub enum BreakBlockError {
    #[error("Bad entity with no transform")]
    MissingTransform(#[from] ComponentGetError),

    #[error("Too far from block {target} to break it from {current}")]
    TooFar {
        current: WorldPoint,
        target: WorldPosition,
    },
}

#[derive(Default)]
pub struct BreakBlockSubactivity;

impl BreakBlockSubactivity {
    pub async fn break_block(
        &mut self,
        ctx: &ActivityContext2<'_>,
        block: WorldPosition,
    ) -> Result<(), BreakBlockError> {
        // check we are close enough to break it
        let pos = ctx
            .world()
            .component::<TransformComponent>(ctx.entity())
            .map_err(BreakBlockError::MissingTransform)?
            .position;

        if pos.distance2(block) > 5.0 {
            return Err(BreakBlockError::TooFar {
                current: pos,
                target: block,
            });
        }

        let world = ctx.world().voxel_world();
        loop {
            {
                // can't hold world ref across ticks
                let world = world.borrow();

                if world.block(block).map(|b| b.is_destroyed()).unwrap_or(true) {
                    // we're done
                    trace!("block has been destroyed"; "block" => %block);
                    break;
                }

                // apply damage to block
                // TODO get current held tool to determine how fast the block can be broken
                // TODO breaking blocks with your hand hurts!
                // TODO define proper scale/enum/consts for block and tool durability
                let break_rate = 6;
                // lets assume this is with a hand and terribly slow
                trace!("damaging block"; "damage" => break_rate, "block" => %block);
                ctx.world()
                    .resource::<QueuedUpdates>()
                    .queue_block_damage(block, break_rate);
            }

            // check again next tick
            ctx.yield_now().await;
        }

        Ok(())
    }
}
