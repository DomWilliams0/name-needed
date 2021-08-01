use crate::activity::activity::{
    ActivityEventContext, ActivityFinish, ActivityResult, SubActivity,
};
use crate::activity::subactivities::GoToSubActivity;
use crate::activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
use crate::event::{EntityEvent, EntityEventPayload};
use crate::{nop_subactivity, unexpected_event, ComponentWorld};
use common::*;
use unit::world::WorldPosition;

use world::SearchGoal;

#[derive(Debug)]
enum BreakBlockState {
    Going(GoToSubActivity),
    Breaking,
}

#[derive(Debug)]
pub struct GoBreakBlockActivity {
    block: WorldPosition,
    state: BreakBlockState,
    finished: Option<BoxedResult<()>>,
}

impl Activity for GoBreakBlockActivity {
    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_>) -> ActivityResult {
        if let Some(res) = self.finished.take() {
            return ActivityResult::from(res);
        }

        match &self.state {
            BreakBlockState::Going(sub) => sub.init(ctx),

            BreakBlockState::Breaking => {
                // TODO block breaking/world interacting should be done in a system
                let world_ref = ctx.world.voxel_world();
                let world = world_ref.borrow();
                match world.block(self.block) {
                    None => {
                        // block no longer exists, sounds bad
                        trace!("block no longer exists??");
                        ActivityResult::Finished(ActivityFinish::Interrupted)
                    }
                    Some(block) if block.is_destroyed() => {
                        // destroyed, congratulations on your efforts
                        trace!("block has been destroyed");
                        ActivityResult::Finished(ActivityFinish::Success)
                    }
                    Some(_) => {
                        // there remains destruction to be done
                        // TODO get current held tool to determine how fast the block can be broken
                        // TODO breaking blocks with your hand hurts!
                        // TODO define proper scale/enum/consts for block and tool durability
                        let break_rate = 6; // lets assume this is with a hand and terribly slow
                        trace!("damaging block"; "damage" => break_rate);
                        ctx.updates.queue_block_damage(self.block, break_rate);
                        ActivityResult::Ongoing
                    }
                }
            }
        }
    }

    fn on_event(
        &mut self,
        event: &EntityEvent,
        _: &ActivityEventContext,
    ) -> (EventUnblockResult, EventUnsubscribeResult) {
        match &event.payload {
            EntityEventPayload::Arrived(token, result) => {
                match &self.state {
                    BreakBlockState::Going(sub) if *token == sub.token() => {}
                    _ => return unexpected_event!(event),
                };

                if let Err(e) = result {
                    debug!("failed to navigate to block"; "error" => %e);
                    self.finished = Some(Err(Box::new(e.to_owned())));
                } else {
                    trace!("arrived at block, switching to breaking state");
                    self.state = BreakBlockState::Breaking;
                }

                (
                    EventUnblockResult::Unblock,
                    EventUnsubscribeResult::UnsubscribeAll,
                )
            }

            e => unexpected_event!(e),
        }
    }

    fn on_finish(&mut self, _: &ActivityFinish, ctx: &mut ActivityContext) -> BoxedResult<()> {
        ctx.clear_path();
        Ok(())
    }

    fn current_subactivity(&self) -> &dyn SubActivity {
        match &self.state {
            BreakBlockState::Going(sub) => sub,
            BreakBlockState::Breaking => nop_subactivity!("Breaking block", 1.2),
        }
    }
}

impl GoBreakBlockActivity {
    pub fn new(block: WorldPosition) -> Self {
        Self {
            block,
            state: BreakBlockState::Going(GoToSubActivity::with_goal(
                block.centred(),
                NormalizedFloat::new(0.8),
                SearchGoal::Adjacent,
            )),
            finished: None,
        }
    }
}

impl Display for GoBreakBlockActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Breaking block at {}", self.block)
    }
}
