use common::*;
use unit::world::{WorldPoint, WorldPosition};
use world::SearchGoal;

use crate::activity::activity::{
    ActivityEventContext, ActivityFinish, ActivityResult, SubActivity,
};
use crate::activity::subactivities::{GoToSubActivity, HaulError, HaulSubActivity};
use crate::activity::{Activity, ActivityContext, EventUnblockResult, EventUnsubscribeResult};
use crate::ecs::{Entity, E};
use crate::event::prelude::*;
use crate::item::{ContainedInComponent, ContainerComponent};
use crate::{
    nop_subactivity, unexpected_event, ComponentWorld, PhysicalComponent, TransformComponent,
};

// TODO support for hauling multiple things at once to the same loc, if the necessary amount of hands are available
// TODO support hauling multiple things to multiple locations
// TODO haul target should hold pos+item radius, assigned once on creation
// TODO events for items entering/exiting containers

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum HaulTarget {
    /// Put in/take from an accessible position
    Position(WorldPosition),

    /// Put in/take out of a container
    Container(Entity),
}

#[derive(Debug)]
pub struct HaulActivity {
    thing: Entity,
    source: HaulTarget,
    target: HaulTarget,

    state: HaulState,
    /// Kept separate from state so we can always run its on_finish() regardless
    haul_sub: Option<HaulSubActivity>,
}

#[derive(Debug)]
enum HaulState {
    Start,
    Going(GoToSubActivity),
    /// Arrived at the source container, need to remove the item from it
    PrepareHauling,
    StartHauling,
    Hauling(GoToSubActivity),
    /// Arrived at the target
    EndHauling,
    Finished(BoxedResult<()>),
    Dummy,
}

impl HaulActivity {
    pub fn new(entity: Entity, source: HaulTarget, target: HaulTarget) -> Self {
        HaulActivity {
            thing: entity,
            source,
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
                // find source position
                let pos = match self.source.source_position(ctx.world, self.thing) {
                    Ok(pos) => pos,
                    Err(e) => return ActivityResult::errored(e),
                };

                // go to it
                // TODO arrival radius depends on the size of the item
                // TODO could the item ever move while we're going to it? only by gravity?
                let goto = GoToSubActivity::with_goal(
                    pos,
                    NormalizedFloat::new(0.8),
                    SearchGoal::Adjacent,
                );

                let result = goto.init(ctx);

                // subscribe to anything happening to the item
                ctx.subscribe_to(self.thing, EventSubscription::All);

                self.state = HaulState::Going(goto);
                result
            }
            HaulState::Going(_) => unreachable!(),
            HaulState::PrepareHauling => {
                match self.source {
                    HaulTarget::Position(_) => {
                        // nothing to do, event already checked this
                        unreachable!()
                    }
                    HaulTarget::Container(container) => {
                        // get item out of container
                        let item = self.thing;
                        ctx.updates
                            .queue("remove item from container", move |world| {
                                let mut do_remove = || -> Result<(), HaulError> {
                                    let container = world
                                        .component_mut::<ContainerComponent>(container)
                                        .map_err(|_| HaulError::BadContainer)?;

                                    // remove from container
                                    container.container.remove(item)?;

                                    // remove contained component
                                    world.helpers_comps().remove_from_container(item);

                                    // the item needs a transform to be added back, leave this until
                                    // the initialisation of the haul subactivity to avoid jerkiness
                                    Ok(())
                                };

                                let result = do_remove().map(|_| container);
                                world.post_event(EntityEvent {
                                    subject: item,
                                    payload: EntityEventPayload::ExitedContainer(result),
                                });

                                Ok(())
                            });
                    }
                }

                // wait for item to exit container
                ctx.subscribe_to(
                    self.thing,
                    EventSubscription::Specific(EntityEventType::ExitedContainer),
                );
                self.state = HaulState::StartHauling;
                ActivityResult::Blocked
            }

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
                let target_pos = match self.target.target_position(ctx.world) {
                    Some(pos) => pos,
                    None => return ActivityResult::errored(HaulError::BadContainer),
                };

                let goto = GoToSubActivity::with_goal(
                    target_pos,
                    NormalizedFloat::new(0.8),
                    SearchGoal::Adjacent,
                );
                let result = goto.init(ctx);

                // still subscribed to item events, no need to resubscribe

                self.state = HaulState::Hauling(goto);
                result
            }
            HaulState::Hauling(_) => unreachable!(),
            HaulState::EndHauling => {
                // arrived at the target, finish haul
                let haul = self.haul_sub.take().expect("should be hauling");

                // cancel haul by hauler first
                let result = haul.on_finish(&ActivityFinish::Success, ctx);

                // then place the haulee in the world
                // TODO this should be in the/a subactivity
                let item = self.thing;
                match self.target {
                    HaulTarget::Position(pos) => {
                        // drop the item in place
                        ctx.updates.queue("drop hauled item", move |world| {
                            world
                                .component_mut::<TransformComponent>(item)
                                .map(|transform| {
                                    // TODO don't always drop item in centre
                                    transform.reset_position(pos.centred());
                                })?;

                            Ok(())
                        });
                    }
                    HaulTarget::Container(container_entity) => {
                        // put the item in the container
                        let hauler = ctx.entity;
                        ctx.updates
                            .queue("put hauled item into container", move |world| {
                                let mut do_put = || -> Result<Entity, HaulError> {
                                    let item_physical = world
                                        .component::<PhysicalComponent>(item)
                                        .map_err(|_| HaulError::BadItem)?;

                                    let container = world
                                        .component_mut::<ContainerComponent>(container_entity)
                                        .map_err(|_| HaulError::BadContainer)?;

                                    container.container.add_with(
                                        item,
                                        item_physical.volume,
                                        item_physical.size,
                                    )?;

                                    // added to container successfully, do component dance
                                    world.helpers_comps().add_to_container(
                                        item,
                                        ContainedInComponent::Container(container_entity),
                                    );

                                    trace!("put item into container"; "item" => E(item),
                                        "container" => E(container_entity), "hauler" => E(hauler)
                                    );

                                    Ok(container_entity)
                                };

                                let payload = do_put();
                                world.post_event(EntityEvent {
                                    subject: item,
                                    payload: EntityEventPayload::EnteredContainer(payload),
                                });

                                Ok(())
                            });

                        if result.is_err() {
                            // bail out if we're in an errored state
                            return result.into();
                        }

                        // wait for success
                        ctx.subscribe_to(
                            item,
                            EventSubscription::Specific(EntityEventType::EnteredContainer),
                        );
                        return ActivityResult::Blocked;
                    }
                };

                result.into()
            }
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
                        match result {
                            Err(e) => {
                                debug!("failed to navigate to haul item"; "error" => %e);
                                self.state = HaulState::Finished(Err(Box::new(e.to_owned())));
                            }
                            Ok(_) => {
                                trace!("arrived at block, switching to hauling state");
                                self.haul_sub = Some(HaulSubActivity::new(self.thing));
                                self.state = match self.source {
                                    HaulTarget::Position(_) => {
                                        // can just pick up item
                                        HaulState::StartHauling
                                    }
                                    HaulTarget::Container(_) => {
                                        // must remove item from container first
                                        HaulState::PrepareHauling
                                    }
                                }
                            }
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
                        self.state = HaulState::EndHauling;

                        (
                            EventUnblockResult::Unblock,
                            EventUnsubscribeResult::UnsubscribeAll,
                        )
                    }

                    _ => unexpected_event!(event),
                }
            }
            EntityEventPayload::Hauled(Ok(hauler)) if *hauler == ctx.subscriber => {
                // this is the one thing we *wanted* to happen!
                (
                    EventUnblockResult::KeepBlocking,
                    EventUnsubscribeResult::StaySubscribed,
                )
            }
            EntityEventPayload::ExitedContainer(result) => {
                if let HaulState::StartHauling = self.state {
                    if let Err(err) = result {
                        debug!("removing item from container failed"; "error" => %err);
                        self.state = HaulState::Finished(Err(Box::new(err.to_owned())));
                    } else {
                        // nice, the item has been removed from the container successfully
                    }

                    // unblock either way
                    (
                        EventUnblockResult::Unblock,
                        EventUnsubscribeResult::UnsubscribeAll,
                    )
                } else {
                    unexpected_event!(&event.payload)
                }
            }
            EntityEventPayload::EnteredContainer(result) if event.subject == self.thing => {
                let result = match result {
                    Err(err) => {
                        debug!("failed to put item in container"; "error" => %err);
                        Err(err.to_owned().into())
                    }
                    Ok(container) => {
                        trace!("put item in container successfully"; "container" => E(*container));
                        Ok(())
                    }
                };

                self.state = HaulState::Finished(result);
                (
                    EventUnblockResult::Unblock,
                    EventUnsubscribeResult::UnsubscribeAll,
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

    fn on_finish(
        &mut self,
        finish: ActivityFinish,
        ctx: &mut ActivityContext<W>,
    ) -> BoxedResult<()> {
        // cancel haul if it has been initialised, regardless of state
        if let Some(haul) = self.haul_sub.take() {
            haul.on_finish(&finish, ctx)
        } else {
            Ok(())
        }
    }

    fn current_subactivity(&self) -> &dyn SubActivity<W> {
        match &self.state {
            HaulState::Start
            | HaulState::Finished(_)
            | HaulState::Dummy
            | HaulState::EndHauling => nop_subactivity!(),
            HaulState::Going(goto) | HaulState::Hauling(goto) => goto,
            HaulState::PrepareHauling | HaulState::StartHauling => {
                self.haul_sub.as_ref().expect("haul should be initialised")
            }
        }
    }
}

impl HaulTarget {
    /// Creates from entity's current position or the container it is currently contained in
    pub fn with_entity(entity: Entity, world: &impl ComponentWorld) -> Option<Self> {
        if let Ok(ContainedInComponent::Container(container)) = world.component(entity) {
            Some(Self::Container(*container))
        } else if let Ok(transform) = world.component::<TransformComponent>(entity) {
            Some(Self::Position(transform.accessible_position()))
        } else {
            None
        }
    }

    pub fn target_position(&self, world: &impl ComponentWorld) -> Option<WorldPoint> {
        match self {
            HaulTarget::Position(pos) => Some(pos.centred()),
            HaulTarget::Container(container) => Self::position_of(world, *container),
        }
    }

    pub fn source_position(
        &self,
        world: &impl ComponentWorld,
        item: Entity,
    ) -> Result<WorldPoint, HaulError> {
        match self {
            HaulTarget::Position(_) => {
                // get current position instead of possibly outdated source position
                Self::position_of(world, item).ok_or(HaulError::BadItem)
            }
            HaulTarget::Container(container) => {
                Self::position_of(world, *container).ok_or(HaulError::BadContainer)
            }
        }
    }

    // TODO explicit access side for container, e.g. front of chest
    fn position_of(world: &impl ComponentWorld, entity: Entity) -> Option<WorldPoint> {
        world
            .component::<TransformComponent>(entity)
            .ok()
            .map(|transform| transform.position)
    }
}

impl Display for HaulActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        // TODO format the other entity better e.g. get item name. or do this in the ui layer?
        write!(f, "Hauling {} to {}", E(self.thing), self.target)
    }
}

impl Display for HaulTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            HaulTarget::Position(pos) => write!(f, "position {}", pos),
            HaulTarget::Container(container) => write!(f, "container {}", E(*container)),
        }
    }
}
