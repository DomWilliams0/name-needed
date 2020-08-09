use crate::activity::activity::{
    Activity, ActivityContext, ActivityResult, Finish, GotoThenNop, NopActivity,
};
use crate::ai::{AiAction, AiComponent};
use crate::ecs::*;
use crate::event::EntityEventQueue;
use crate::queued_update::QueuedUpdates;
use common::*;

use crate::activity::EventUnblockResult;
use unit::world::WorldPoint;

pub struct ActivitySystem;

pub struct ActivityEventSystem;

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct ActivityComponent {
    pub current: Box<dyn Activity<EcsWorld>>,
    pub new_activity: Option<AiAction>,
}

#[derive(Component, Default)]
#[storage(NullStorage)]
pub struct BlockingActivityComponent;

impl<'a> System<'a> for ActivitySystem {
    type SystemData = (
        WriteStorage<'a, ActivityComponent>,
        WriteStorage<'a, AiComponent>,
        ReadStorage<'a, BlockingActivityComponent>,
        Read<'a, EntitiesRes>,
        Read<'a, EcsWorldFrameRef>,
        Write<'a, QueuedUpdates>,
        Write<'a, LazyUpdate>,
        Write<'a, EntityEventQueue>,
    );

    fn run(
        &mut self,
        (mut activities, mut ai, blocking, entities, world, updates, comp_updates, mut event_queue): Self::SystemData,
    ) {
        let mut subscriptions = Vec::new(); // TODO reuse allocation in system
        for (entity, ai, activity, _) in (&entities, &mut ai, &mut activities, !&blocking).join() {
            debug_assert!(subscriptions.is_empty());
            let mut ctx = ActivityContext::<EcsWorld> {
                entity,
                world: &*world,
                updates: &*updates,
                subscriptions: &mut subscriptions,
            };

            if let Some(new_action) = activity.new_activity.take() {
                // interrupt current activity with new
                activity.current.on_finish(Finish::Interrupted, &mut ctx);

                // replace current with new activity, dropping the old one
                new_action.into_activity(&mut activity.current);
            }

            match activity.current.on_tick(&mut ctx) {
                ActivityResult::Blocked(subscriptions) => {
                    // subscribe to requested events
                    event_queue.subscribe(entity, subscriptions.drain(..));

                    // mark activity as blocked
                    comp_updates.insert(entity, BlockingActivityComponent::default());
                }

                ActivityResult::Ongoing => {
                    // go again next tick
                }
                ActivityResult::Finished(finish) => {
                    debug!(
                        "finished activity with finish {:?}: '{}'. reverting to nop activity",
                        finish, activity.current
                    );

                    // finish current and replace with nop
                    activity.current.on_finish(finish, &mut ctx);
                    activity.current = Box::new(NopActivity);

                    // next tick ai should return a new decision rather than unchanged to avoid
                    // infinite Nop loops
                    ai.clear_last_action();
                }
            }
        }
    }
}

impl<'a> System<'a> for ActivityEventSystem {
    type SystemData = (
        Read<'a, LazyUpdate>,
        Write<'a, EntityEventQueue>,
        WriteStorage<'a, ActivityComponent>,
        ReadStorage<'a, BlockingActivityComponent>,
    );

    fn run(&mut self, (updates, mut events, mut activities, blocking): Self::SystemData) {
        events.handle_events(|subscriber, event| {
            // TODO use fancy bitmask magic to get both at once
            let activity = activities
                .get_mut(subscriber)
                .expect("subscriber must have activity component");
            assert!(
                blocking.get(subscriber).is_some(),
                "subscriber must be in a blocked state"
            );

            debug!("passing event to {:?} ({:?})", subscriber, event);

            let (unblock, unsubscribe) = activity.current.on_event(event);

            if let EventUnblockResult::Unblock = unblock {
                updates.remove::<BlockingActivityComponent>(subscriber);
                debug!("unblocking activity of {:?}", subscriber);
            }

            unsubscribe
        });
    }
}

impl Default for ActivityComponent {
    fn default() -> Self {
        Self {
            current: Box::new(GotoThenNop::new(WorldPoint(8.0, 8.0, 3.0))),
            new_activity: None,
        }
    }
}
