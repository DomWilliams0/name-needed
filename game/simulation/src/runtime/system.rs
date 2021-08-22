use crate::activity::{BlockingActivityComponent, EventUnblockResult, EventUnsubscribeResult};
use crate::ecs::*;
use crate::event::{EntityEvent, EntityEventPayload, EntityEventQueue, RuntimeTimers};
use crate::runtime::Runtime;
use crate::{ActivityComponent, EntityLoggingComponent, Tick};
use common::*;

/// Consumes events, does not run/poll any tasks
pub struct RuntimeSystem;

impl<'a> System<'a> for RuntimeSystem {
    type SystemData = (
        Write<'a, EntityEventQueue>,
        Write<'a, RuntimeTimers>,
        Read<'a, Runtime>,
        WriteStorage<'a, EntityLoggingComponent>,
        WriteStorage<'a, ActivityComponent>,
        Read<'a, LazyUpdate>,
    );

    fn run(
        &mut self,
        (mut events, mut timers, runtime, mut logging, mut activities, updates): Self::SystemData,
    ) {
        // consume timers
        for (task_handle, fut) in timers.maintain(Tick::fetch()) {
            trace!("timer elapsed"; "task" => ?task_handle);
            fut.trigger(());
        }
        /*
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

                logging.log_events(events.map(|e| &e.payload));
            }
        });*/
    }
}
