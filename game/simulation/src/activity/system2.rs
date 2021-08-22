use std::pin::Pin;

use async_trait::async_trait;
use futures::Future;

use common::*;

use crate::activity::activity2::{Activity2, ActivityContext2};
use crate::activity::{ActivityContext, NopActivity2};
use crate::ai::AiAction;
use crate::ecs::*;
use crate::event::RuntimeTimers;
use crate::job::{SocietyJobRef, SocietyTask};
use crate::runtime::{ManualFuture, Runtime, TaskHandle, TaskRef, TimerFuture};

// TODO rename
#[derive(Component, EcsComponent)]
#[storage(DenseVecStorage)]
#[name("activity2")]
pub struct ActivityComponent2 {
    // current: Box<dyn Activity>,
    // current_society_task: Option<(SocietyJobRef, SocietyTask)>,
    /// Set by AI to trigger a new activity
    new_activity: Option<(AiAction, Option<(SocietyJobRef, SocietyTask)>)>,

    current_task: Option<TaskRef>,
}

/// Interrupts current with new activities
pub struct ActivitySystem2<'a>(pub Pin<&'a EcsWorld>);

impl Default for ActivityComponent2 {
    fn default() -> Self {
        Self {
            new_activity: None,
            // current: Box::new(NopActivity2::default()),
            current_task: None,
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
        for (e, activity) in (&entities, &mut activities).join() {
            let e = Entity::from(e);
            let mut new_activity = None;

            if let Some((new_action, new_society_task)) = activity.new_activity.take() {
                debug!("interrupting activity with new"; e, "action" => ?new_action);

                // cancel current
                if let Some(task) = activity.current_task.take() {
                    task.cancel();
                }
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
                new_activity = Some(new_action.into_activity2());
            // activity.current = new_action.into_activity2();
            // activity.current_society_task = new_society_task;

            // not necessary to manually cancel society reservation here, as the ai interruption
            // already did
            } else if activity
                .current_task
                .as_ref()
                .map(|t| t.is_finished())
                .unwrap_or(true)
            {
                // current task has finished
                debug!("no activity, reverting to nop"; e);
                new_activity = Some(Box::new(NopActivity2::default()));
            }

            // spawn task for new activity
            if let Some(new_activity) = new_activity {
                let ctx = ActivityContext2 {
                    entity: e.into(),
                    world: self.0,
                };

                activity.kick_off(&*runtime, new_activity, ctx);
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

    fn kick_off(
        &mut self,
        runtime: &Runtime,
        mut activity: Box<dyn Activity2>,
        ctx: ActivityContext2<'_>,
    ) {
        // safety: ecs world is pinned and guaranteed to be valid as long as this system
        // is being ticked
        let ctx =
            unsafe { std::mem::transmute::<ActivityContext2, ActivityContext2<'static>>(ctx) };

        self.current_task = Some(runtime.spawn(async move {
            let e = ctx.entity;
            match activity.dew_it(ctx).await {
                Ok(_) => {
                    debug!("activity finished"; e, "activity" => %activity);
                }
                Err(err) => {
                    debug!("activity failed"; e, "activity" => %activity, "err" => %err);
                }
            };
        }));
    }
}
