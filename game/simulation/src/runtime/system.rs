use crate::activity::{
    ActivityComponent2, BlockingActivityComponent, EventUnblockResult, EventUnsubscribeResult,
};
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
        WriteStorage<'a, ActivityComponent2>,
        Read<'a, LazyUpdate>,
    );

    fn run(
        &mut self,
        (mut events, mut timers, runtime, mut logging, mut activities, updates): Self::SystemData,
    ) {
        // consume timers
        for (task_handle, fut) in timers.maintain(Tick::fetch()) {
            trace!("timer elapsed"; "task" => ?task_handle);
            // TODO could avoid the need for allocations in ManualFutures here and lookup the task in the runtime by id instead?
            fut.trigger(());
        }

        // log events
        for (subject, events) in events.events().group_by(|evt| evt.subject).into_iter() {
            let logging = match logging.get_mut(subject.into()) {
                Some(comp) => comp,
                None => continue,
            };

            logging.log_events(events.map(|e| &e.payload));
        }

        // consume events
        events.consume_events(|subscriber, evt| {
            let subscriber = Entity::from(subscriber);
            let task = match activities.get_mut(subscriber.into()) {
                Some(comp) => {
                    if let Some(task) = comp.task() {
                        task
                    } else {
                        warn!("no current task?"; "subscriber" => subscriber); // TODO wut do? task is finished?
                        return EventUnsubscribeResult::UnsubscribeAll;
                    }
                }
                None => {
                    warn!("subscriber is missing activity component"; "event" => ?evt, "subscriber" => subscriber);
                    return EventUnsubscribeResult::UnsubscribeAll;
                }
            };

            // event has arrived for task, push it onto task event queue
            task.push_event(evt);

            // mark task as ready now to be polled next tick
            runtime.mark_ready(task);

            // task has not yet responded to event, can't return anything useful here TODO
            EventUnsubscribeResult::StaySubscribed
        });
    }
}
