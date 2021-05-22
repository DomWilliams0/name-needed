use crate::activity::activity::{
    Activity, ActivityContext, ActivityEventContext, ActivityFinish, ActivityResult,
};
use crate::ai::{AiAction, AiComponent};
use crate::ecs::*;
use crate::event::{EntityEvent, EntityEventPayload, EntityEventQueue, EntityTimers};
use crate::queued_update::QueuedUpdates;
use common::*;

use crate::activity::{EventUnblockResult, EventUnsubscribeResult};

use crate::activity::activities::NopActivity;
use crate::activity::event_logging::EntityLoggingComponent;
use crate::simulation::Tick;
use crate::{Societies, SocietyComponent};

pub struct ActivitySystem;

pub struct ActivityEventSystem;

#[derive(Component, EcsComponent)]
#[storage(DenseVecStorage)]
#[name("activity")]
pub struct ActivityComponent {
    pub current: Box<dyn Activity<EcsWorld>>,
    new_activity: Option<AiAction>,
}

#[derive(Component, EcsComponent, Default)]
#[storage(NullStorage)]
#[name("blocking-activity")]
pub struct BlockingActivityComponent;

impl<'a> System<'a> for ActivitySystem {
    type SystemData = (
        WriteStorage<'a, ActivityComponent>,
        WriteStorage<'a, AiComponent>,
        ReadStorage<'a, BlockingActivityComponent>,
        WriteStorage<'a, SocietyComponent>,
        Read<'a, EntitiesRes>,
        Read<'a, EcsWorldFrameRef>,
        Read<'a, QueuedUpdates>,
        Read<'a, LazyUpdate>,
        Write<'a, Societies>,
        Write<'a, EntityEventQueue>,
    );

    fn run(
        &mut self,
        (
            mut activities,
            mut ai,
            blocking,
            society,
            entities,
            world,
            updates,
            comp_updates,
            mut societies,
            mut event_queue,
        ): Self::SystemData,
    ) {
        let mut subscriptions = Vec::new(); // TODO reuse allocation in system
        for (entity, ai, activity, _) in (&entities, &mut ai, &mut activities, !&blocking).join() {
            log_scope!(o!("system" => "activity", E(entity)));
            debug!("current activity"; "activity" => &activity.current);

            debug_assert!(subscriptions.is_empty());
            let mut ctx = ActivityContext::<EcsWorld> {
                entity,
                world: &*world,
                updates: &*updates,
                subscriptions: &mut subscriptions,
            };

            if let Some(new_action) = activity.new_activity.take() {
                debug!("interrupting activity with new"; "action" => ?new_action);

                if let Err(e) = activity
                    .current
                    .finish(ActivityFinish::Interrupted, &mut ctx)
                {
                    error!("error interrupting current activity"; "activity" => &activity.current, "error" => %e);
                }

                // unsubscribe from all events from previous activity
                event_queue.unsubscribe_all(entity);
                comp_updates.remove::<BlockingActivityComponent>(entity);

                // replace current with new activity, dropping the old one
                new_action.into_activity(&mut activity.current);
            }

            // TODO consider allowing consideration of a new activity while doing one, then swapping immediately with no pause

            match activity.current.on_tick(&mut ctx) {
                ActivityResult::Blocked => {
                    // subscribe to requested events if any. if no subscriptions are added, the only
                    // way to unblock will be on activity end
                    debug!("subscribing to {count} events", count = subscriptions.len());
                    for sub in &subscriptions {
                        trace!("subscribing to event"; "subscription" => ?sub);
                    }
                    event_queue.subscribe(entity, subscriptions.drain(..));

                    // mark activity as blocked
                    comp_updates.insert(entity, BlockingActivityComponent::default());
                    debug!("blocking activity");
                }

                ActivityResult::Ongoing => {
                    // go again next tick
                    trace!("activity is ongoing")
                }
                ActivityResult::Finished(finish) => {
                    debug!("finished activity, reverting to nop"; "activity" => &activity.current, "finish" => ?finish);

                    // finish current and replace with nop
                    if let Err(e) = activity.current.finish(finish, &mut ctx) {
                        error!("error finishing activity"; "activity" => &activity.current, "error" => %e);
                    }

                    activity.current = Box::new(NopActivity::default());

                    // ensure unblocked and unsubscribed
                    event_queue.unsubscribe_all(entity);
                    comp_updates.remove::<BlockingActivityComponent>(entity);

                    ai.interrupt_current_action(entity, || {
                        society
                            .get(entity)
                            .and_then(|soc| societies.society_by_handle_mut(soc.handle))
                            .expect("should have society")
                    });

                    // next tick ai should return a new decision rather than unchanged to avoid
                    // infinite Nop loops
                    ai.clear_last_action();
                }
            }
        }

        event_queue.log();
    }
}

impl<'a> System<'a> for ActivityEventSystem {
    type SystemData = (
        Write<'a, EntityEventQueue>,
        Write<'a, EntityTimers>,
        WriteStorage<'a, EntityLoggingComponent>,
        WriteStorage<'a, ActivityComponent>,
        Read<'a, LazyUpdate>,
    );

    fn run(
        &mut self,
        (mut events, mut timers, mut logging, mut activities, updates): Self::SystemData,
    ) {
        // post events for elapsed timers
        for (token, subject) in timers.maintain(Tick::fetch()) {
            events.post(EntityEvent {
                subject,
                payload: EntityEventPayload::TimerElapsed(token),
            });

            trace!("entity timer elapsed"; "subject" => E(subject), "token" => ?token);
        }

        events.consume_events(|subscriber, event| {
            let activity = match activities
                .get_mut(subscriber) {
                Some(comp) => comp,
                None => {
                    warn!("subscriber is missing activity component"; "subscriber" => E(subscriber), "event" => ?event);
                    return EventUnsubscribeResult::UnsubscribeAll;
                }
            };

            log_scope!(o!("subscriber" => E(subscriber)));

            let ctx = ActivityEventContext { subscriber };
            let (unblock, unsubscribe) = activity.current.on_event(event, &ctx);
            debug!("event handler result"; "unblock" => ?unblock, "unsubscribe" => ?unsubscribe);

            if let EventUnblockResult::Unblock = unblock {
                // entity is now unblocked
                updates.remove::<BlockingActivityComponent>(subscriber);
            }

            unsubscribe
        }, |events| {

            // log all events per subject
            for (subject, events) in events.iter().group_by(|evt| evt.subject).into_iter() {
                let logging = match logging
                    .get_mut(subject) {
                    Some(comp) => comp,
                    None => continue,
                };

                for event in events {
                    logging.log_event(&event.payload);
                }

            }
        });
    }
}

impl ActivityComponent {
    pub fn exertion(&self) -> f32 {
        self.current.current_subactivity().exertion()
    }

    pub fn interrupt_with_new_activity(
        &mut self,
        action: AiAction,
        me: Entity,
        world: &impl ComponentWorld,
    ) {
        self.new_activity = Some(action);

        // ensure unblocked
        world.remove_lazy::<BlockingActivityComponent>(me);
    }
}

impl Default for ActivityComponent {
    fn default() -> Self {
        Self {
            current: Box::new(NopActivity::default()),
            new_activity: None,
        }
    }
}
