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
use std::mem::transmute;

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
            if let Some(mut new_activity) = new_activity {
                // safety: ecs world is pinned and guaranteed to be valid as long as this system
                // is being ticked
                let world = unsafe { transmute::<Pin<&EcsWorld>, Pin<&'static EcsWorld>>(self.0) };

                let (tx, rx) = futures::channel::oneshot::channel();
                let task = runtime.spawn(tx, async move {
                    // recv task ref from runtime
                    let task = rx.await.unwrap(); // will not be cancelled

                    // create context
                    let entity = e.into();
                    let ctx = ActivityContext2 {
                        entity,
                        world,
                        task,
                    };

                    // safety: ecs world is pinned and guaranteed to be valid as long as this system
                    // is being ticked
                    // let ctx =
                    //     unsafe { std::mem::transmute::<ActivityContext2, ActivityContext2<'static>>(ctx) };

                    match new_activity.dew_it(ctx).await {
                        Ok(_) => {
                            debug!("activity finished"; entity, "activity" => %new_activity);
                        }
                        Err(err) => {
                            debug!("activity failed"; entity, "activity" => %new_activity, "err" => %err);
                        }
                    };
                });

                activity.current_task = Some(task);
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

    pub fn task(&self) -> Option<&TaskRef> {
        self.current_task.as_ref()
    }
}
