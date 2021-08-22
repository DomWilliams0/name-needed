use async_trait::async_trait;
use futures::Future;

use common::*;

use crate::activity::activity2::{Activity2, NopActivity2};
use crate::activity::ActivityContext;
use crate::ai::AiAction;
use crate::ecs::*;
use crate::job::{SocietyJobRef, SocietyTask};
use crate::runtime::{ManualFuture, Runtime, TaskHandle};
use std::pin::Pin;

// TODO rename
#[derive(Component, EcsComponent)]
#[storage(DenseVecStorage)]
#[name("activity2")]
pub struct ActivityComponent2 {
    // current: Box<dyn Activity>,
    // current_society_task: Option<(SocietyJobRef, SocietyTask)>,
    /// Set by AI to trigger a new activity
    new_activity: Option<(AiAction, Option<(SocietyJobRef, SocietyTask)>)>,

    current: Box<dyn Activity2>,
}

pub struct ActivityContext2<'a> {
    pub entity: Entity,
    pub world: Pin<&'a EcsWorld>,
}

// only used on the main thread
unsafe impl Sync for ActivityContext2<'_> {}
unsafe impl Send for ActivityContext2<'_> {}

/// Interrupts current with new activities
pub struct ActivitySystem2<'a>(pub Pin<&'a EcsWorld>);

impl Default for ActivityComponent2 {
    fn default() -> Self {
        Self {
            new_activity: None,
            current: Box::new(NopActivity2::default()),
        }
    }
}

impl<'a> System<'a> for ActivitySystem2<'a> {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, Runtime>,
        WriteStorage<'a, ActivityComponent2>,
    );

    fn run(&mut self, (entities, runtime, mut activities): Self::SystemData) {
        for (entity, activity) in (&entities, &mut activities).join() {
            if let Some((new_action, new_society_task)) = activity.new_activity.take() {
                debug!("interrupting activity with new"; "action" => ?new_action);

                // TODO cancel current
                // if let Err(e) = activity
                //     .current
                //     .finish(&ActivityFinish::Interrupted, &mut ctx)
                // {
                //     error!("error interrupting current activity"; "activity" => &activity.current, "error" => %e);
                // }

                // TODO unsubscribe from all events from previous activity
                // event_queue.unsubscribe_all(entity);
                // comp_updates.remove::<BlockingActivityComponent>(entity);

                // replace current with new activity, dropping the old one
                let mut activity = new_action.into_activity2();
                // activity.current = new_action.into_activity2();
                // activity.current_society_task = new_society_task;

                // not necessary to manually cancel society reservation here, as the ai interruption
                // already did

                let ctx = ActivityContext2 {
                    entity,
                    world: self.0,
                };

                // safety: ecs world is pinned and guaranteed to be valid as long as this system
                // is being ticked
                let ctx = unsafe {
                    std::mem::transmute::<ActivityContext2, ActivityContext2<'static>>(ctx)
                };

                // TODO store task
                let task = runtime.spawn(async move {
                    if let Err(err) = activity.dew_it(ctx).await {
                        warn!("activity failed"; "activity" => %activity, "error" => %err);
                    }
                });
            }
        }
    }
}

impl ActivityComponent2 {
    pub fn interrupt_with_new_activity(
        &mut self,
        action: AiAction,
        society_task: Option<(SocietyJobRef, SocietyTask)>,
        me: Entity,
        world: &impl ComponentWorld,
    ) {
        self.new_activity = Some((action, society_task));
        // // ensure unblocked
        // world.remove_lazy::<BlockingActivityComponent>(me);
    }
}
