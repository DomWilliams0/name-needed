use crate::activity::activity::{
    Activity, ActivityContext, ActivityEventContext, ActivityResult, Finish,
};
use crate::ai::{AiAction, AiComponent};
use crate::ecs::*;
use crate::event::EntityEventQueue;
use crate::queued_update::QueuedUpdates;
use common::*;

use crate::activity::EventUnblockResult;

use crate::activity::activities::NopActivity;

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
        Read<'a, QueuedUpdates>,
        Read<'a, LazyUpdate>,
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
                if let Err(e) = activity.current.finish(Finish::Interrupted, &mut ctx) {
                    error!("error interrupting activity '{}': {}", activity.current, e);
                }

                // replace current with new activity, dropping the old one
                new_action.into_activity(&mut activity.current);
            }

            match activity.current.on_tick(&mut ctx) {
                ActivityResult::Blocked => {
                    // subscribe to requested events if any. if no subscriptions are added, the only
                    // way to unblock will be on activity end
                    for sub in &subscriptions {
                        debug!(
                            "subscribing {} to events: {:?}",
                            crate::entity_pretty!(entity),
                            sub
                        );
                    }
                    event_queue.subscribe(entity, subscriptions.drain(..));

                    // mark activity as blocked
                    comp_updates.insert(entity, BlockingActivityComponent::default());
                    debug!("blocking activity for {}", crate::entity_pretty!(entity));
                }

                ActivityResult::Ongoing => {
                    // go again next tick
                }
                ActivityResult::Finished(finish) => {
                    debug!(
                        "finished activity with '{:?}': '{}'. reverting to nop activity",
                        finish, activity.current
                    );

                    // finish current and replace with nop
                    if let Err(e) = activity.current.finish(finish, &mut ctx) {
                        error!("error finishing activity '{}': {}", activity.current, e);
                    }
                    activity.current = Box::new(NopActivity);

                    // ensure unblocked
                    comp_updates.remove::<BlockingActivityComponent>(entity);

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
        Write<'a, EntityEventQueue>,
        WriteStorage<'a, ActivityComponent>,
        Read<'a, LazyUpdate>,
    );

    fn run(&mut self, (mut events, mut activities, updates): Self::SystemData) {
        events.handle_events(|subscriber, event| {
            let activity = activities
                .get_mut(subscriber)
                .expect("subscriber must have activity component");

            debug!("passing event to {:?} ({:?})", subscriber, event);

            let ctx = ActivityEventContext { subscriber };

            let (unblock, unsubscribe) = activity.current.on_event(event, &ctx);

            if let EventUnblockResult::Unblock = unblock {
                debug!(
                    "unblocking activity of {:?} ({})",
                    subscriber, activity.current
                );
                updates.remove::<BlockingActivityComponent>(subscriber);
            }

            unsubscribe
        });
    }
}

impl Default for ActivityComponent {
    fn default() -> Self {
        Self {
            current: Box::new(NopActivity),
            new_activity: None,
        }
    }
}

impl ActivityComponent {
    pub fn exertion(&self) -> f32 {
        self.current.current_subactivity().exertion()
    }
}
