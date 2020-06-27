use std::fmt::Formatter;

use common::derive_more::Display;
use common::*;
use unit::world::{WorldPoint, WorldPosition};
use world::block::BlockType;
use world::{SearchGoal, WorldRef};

use crate::ai::activity::movement::{GoingToActivity, GotoThen};
use crate::ai::activity::{Activity, ActivityContext, ActivityResult, Finish};
use crate::ComponentWorld;

#[derive(Display)]
#[display(fmt = "Breaking block at {}", pos)]
pub struct BreakBlockActivity {
    pos: WorldPosition,
}

pub type GoBreakBlockActivity<W> = GotoThen<W, BreakBlockActivity>;

impl<W: ComponentWorld> Activity<W> for BreakBlockActivity {
    fn on_start(&mut self, _: &ActivityContext<W>) {
        // TODO get block type we're about to break, and equip the best tool for it
    }

    fn on_tick(&mut self, ctx: &ActivityContext<W>) -> ActivityResult {
        let world_ref = ctx.world.resource::<WorldRef>();
        let world = world_ref.borrow();
        match world.block(self.pos) {
            None => {
                // block no longer exists, sounds bad
                ActivityResult::Finished(Finish::Interrupted)
            }
            Some(block) if block.block_type() == BlockType::Air => {
                // destroyed, congratulations on your efforts
                ActivityResult::Finished(Finish::Succeeded)
            }
            Some(_) => {
                // theres destruction to be done
                // TODO get current held tool to determine how fast the block can be broken
                // TODO breaking blocks with your hand hurts!
                // TODO define proper scale/enum/consts for block and tool durability
                let break_rate = 6; // lets assume this is with a hand and terribly slow
                ctx.updates.queue_block_damage(self.pos, break_rate);
                ActivityResult::Ongoing
            }
        }
    }

    fn on_finish(&mut self, _: Finish, _: &ActivityContext<W>) {}

    fn exertion(&self) -> f32 {
        // TODO exertion depends on the tool and block
        2.0
    }
}

impl BreakBlockActivity {
    pub fn new<W: ComponentWorld>(block: WorldPosition) -> GoBreakBlockActivity<W> {
        let break_block = Self { pos: block };
        GotoThen::new(
            block.centred(),
            SearchGoal::Adjacent,
            NormalizedFloat::one(),
            break_block,
        )
    }
}

impl GoingToActivity for BreakBlockActivity {
    fn display_with_target(&self, f: &mut Formatter<'_>, _: WorldPoint) -> std::fmt::Result {
        write!(f, "Breaking block at {}", self.pos)
    }
}
