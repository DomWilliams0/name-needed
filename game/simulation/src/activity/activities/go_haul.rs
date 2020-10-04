use crate::activity::activity::{ActivityEventContext, ActivityResult, Finish, SubActivity};
use crate::activity::subactivities::{GoToSubActivity, HaulError, HaulSubActivity};
use crate::activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
use crate::ecs::{Entity, E};
use crate::event::prelude::*;
use crate::{nop_subactivity, unexpected_event, ComponentWorld, TransformComponent};
use common::*;
use unit::world::WorldPosition;
use world::SearchGoal;

// TODO support for hauling multiple things at once to the same loc, if the necessary amount of hands are available
// TODO support hauling multiple things to multiple locations
// TODO haul target should hold pos+item radius, assigned once on creation

#[derive(Debug)]
pub struct HaulActivity {
    thing: Entity,
    target: WorldPosition,
    state: HaulState,
    /// Kept separate from state so we can always run its on_finish() regardless
    haul_sub: Option<HaulSubActivity>,
}

#[derive(Debug)]
enum HaulState {
    Start,
    Going(GoToSubActivity),
    StartHauling,
    Hauling(GoToSubActivity),
    Finished(BoxedResult<()>),
    Dummy,
}

impl HaulActivity {
    pub fn new(entity: Entity, target: WorldPosition) -> Self {
        // TODO pass current position of item, store in Start state
        HaulActivity {
            thing: entity,
            target,
            state: HaulState::Start,
            haul_sub: None,
        }
    }
}

impl<W: ComponentWorld> Activity<W> for HaulActivity {
    fn on_tick<'a>(&mut self, ctx: &'a mut ActivityContext<'_, W>) -> ActivityResult {
        match std::mem::replace(&mut self.state, HaulState::Dummy) {
            HaulState::Start => {
                // find item pos
                let pos = match ctx.world.component::<TransformComponent>(self.thing) {
                    Ok(transform) => transform.position,
                    Err(e) => return ActivityResult::errored(e),
                };

                // go to it
                // TODO arrival radius depends on the size of the item
                // TODO could the item ever move while we're going to it? only by gravity?
                let goto = GoToSubActivity::with_goal(
                    pos,
                    NormalizedFloat::new(1.0),
                    SearchGoal::Nearby(1),
                );

                let result = goto.init(ctx);

                // subscribe to ar anything happening to the item
                ctx.subscribe_to(self.thing, EventSubscription::All);

                self.state = HaulState::Going(goto);
                result
            }
            HaulState::Going(_) => unreachable!(),
            HaulState::StartHauling => {
                let haul = self.haul_sub.as_ref().expect("haul should be initialised");

                // init hauling first
                match haul.init(ctx) {
                    ActivityResult::Blocked => {
                        // success
                    }
                    finish @ ActivityResult::Finished(_) => return finish,
                    ActivityResult::Ongoing => unreachable!(),
                }

                // if it succeeded, off we go to the haul target
                let goto = GoToSubActivity::with_goal(
                    self.target.centred(),
                    NormalizedFloat::new(0.8),
                    SearchGoal::Nearby(1),
                );
                let result = goto.init(ctx);

                // still subscribed to item events, no need to resubscribe

                self.state = HaulState::Hauling(goto);
                result
            }
            HaulState::Hauling(_) => unreachable!(),
            HaulState::Finished(result) => result.into(),
            HaulState::Dummy => unreachable!(),
        }
    }

    fn on_event(
        &mut self,
        event: &EntityEvent,
        ctx: &ActivityEventContext,
    ) -> (EventUnblockResult, EventUnsubscribeResult) {
        match &event.payload {
            EntityEventPayload::Arrived(token, result) => {
                match &self.state {
                    HaulState::Going(sub) if *token == sub.token() => {
                        // arrived at item empty handed, start hauling
                        if let Err(e) = result {
                            debug!("failed to navigate to haul item"; "error" => %e);
                            self.state = HaulState::Finished(Err(Box::new(e.to_owned())));
                        } else {
                            trace!("arrived at block, switching to hauling state");
                            self.haul_sub = Some(HaulSubActivity::new(self.thing));
                            self.state = HaulState::StartHauling;
                        }

                        // unsubscribe from arrival/self events but stay subscribed to all item events
                        (
                            EventUnblockResult::Unblock,
                            EventUnsubscribeResult::Unsubscribe(EntityEventSubscription(
                                ctx.subscriber,
                                EventSubscription::All,
                            )),
                        )
                    }

                    HaulState::Hauling(goto) if *token == goto.token() => {
                        // arrived at haul target, stop hauling
                        trace!("arrived at haul target, finishing haul");
                        self.state = HaulState::Finished(Ok(()));

                        (
                            EventUnblockResult::Unblock,
                            EventUnsubscribeResult::UnsubscribeAll,
                        )
                    }

                    _ => unexpected_event!(event),
                }
            }
            EntityEventPayload::Hauled(Ok((_, hauler))) if *hauler == ctx.subscriber => {
                // this is the one thing we *wanted* to happen!
                (
                    EventUnblockResult::KeepBlocking,
                    EventUnsubscribeResult::StaySubscribed,
                )
            }
            e if event.subject == self.thing && e.is_destructive() => {
                trace!("item to haul has been destroyed"; "reason" => ?e);
                self.state = HaulState::Finished(Err(Box::new(HaulError::Interrupted)));

                (
                    EventUnblockResult::Unblock,
                    EventUnsubscribeResult::UnsubscribeAll,
                )
            }

            e => unexpected_event!(e),
        }
    }

    fn on_finish(&mut self, _: Finish, ctx: &mut ActivityContext<W>) -> BoxedResult<()> {
        // cancel haul if it has been initialised, regardless of state
        if let Some(haul) = self.haul_sub.take() {
            haul.on_finish(ctx)
        } else {
            Ok(())
        }
    }

    fn current_subactivity(&self) -> &dyn SubActivity<W> {
        match &self.state {
            HaulState::Start | HaulState::Finished(_) | HaulState::Dummy => nop_subactivity!(),
            HaulState::Going(goto) | HaulState::Hauling(goto) => goto,
            HaulState::StartHauling => self.haul_sub.as_ref().expect("haul should be initialised"),
        }
    }
}

impl Display for HaulActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        // TODO format the other entity better e.g. get item name. or do this in the ui layer?
        write!(f, "Hauling {} to {}", E(self.thing), self.target)
    }
}
