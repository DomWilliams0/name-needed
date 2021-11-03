use std::pin::Pin;

use common::*;

use crate::activity::context::ActivityContext;
use crate::activity::NopActivity;
use crate::ai::{AiAction, AiComponent};
use crate::ecs::*;

use crate::activity::status::{StatusReceiver, StatusRef};
use crate::event::EntityEventQueue;
use crate::job::{SocietyJobRef, SocietyTask};
use crate::runtime::{Runtime, TaskRef, TaskResult};
use crate::{Societies, SocietyComponent};

use std::mem::transmute;
use std::rc::Rc;

#[derive(Component, EcsComponent, Default)]
#[storage(DenseVecStorage)]
#[name("activity")]
#[clone(disallow)]
pub struct ActivityComponent {
    current_society_task: Option<(SocietyJobRef, SocietyTask)>,
    /// Set by AI to trigger a new activity
    new_activity: Option<(AiAction, Option<(SocietyJobRef, SocietyTask)>)>,
    current: Option<ActiveTask>,
    /// Reused between tasks
    status: StatusReceiver,
}

struct ActiveTask {
    task: TaskRef,
    description: Box<dyn Display>,
}

/// Interrupts current with new activities
pub struct ActivitySystem<'a>(pub Pin<&'a EcsWorld>);

impl<'a> System<'a> for ActivitySystem<'a> {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, Runtime>,
        Write<'a, EntityEventQueue>,
        Read<'a, Societies>,
        WriteStorage<'a, ActivityComponent>,
        WriteStorage<'a, AiComponent>,
        ReadStorage<'a, SocietyComponent>,
    );

    fn run(
        &mut self,
        (
            entities,
            runtime,
            mut event_queue,
            societies_res,
            mut activities,
            mut ais,
            societies,
        ): Self::SystemData,
    ) {
        for (e, activity, ai) in (&entities, &mut activities, &mut ais).join() {
            let e = Entity::from(e);
            let mut new_activity = None;

            if let Some((new_action, new_society_task)) = activity.new_activity.take() {
                // TODO handle society task
                debug!("interrupting activity with new"; e, "action" => ?new_action);

                // cancel current
                if let Some(task) = activity.current.take() {
                    task.task.cancel();

                    // unsubscribe from all events from previous activity
                    event_queue.unsubscribe_all(e);
                }

                // replace current with new activity, dropping the old one
                new_activity = Some(new_action.into_activity());
                activity.current_society_task = new_society_task;

            // not necessary to manually cancel society reservation here, as the ai interruption
            // already did
            } else {
                let (finished, result) = match activity.current.as_ref() {
                    None => (true, None),
                    Some(task) => {
                        let result = task.task.result();
                        (result.is_some(), result)
                    }
                };

                if finished {
                    // current task has finished
                    if let Some(res) = result.as_ref() {
                        debug!("activity finished, reverting to nop"; e, "result" => ?res);

                        // post debug event with activity result
                        #[cfg(feature = "testing")]
                        {
                            use crate::event::EntityEventDebugPayload;
                            use crate::{EntityEvent, EntityEventPayload};
                            if let Some(current) = activity.current.as_ref() {
                                event_queue.post(EntityEvent {
                                    subject: e,
                                    payload: EntityEventPayload::Debug(
                                        EntityEventDebugPayload::FinishedActivity {
                                            description: current.description.to_string(),
                                            result: res.into(),
                                        },
                                    ),
                                })
                            }
                        }
                    } else {
                        debug!("activity finished, reverting to nop"; e);
                    }
                    new_activity = Some(Rc::new(NopActivity::default()));

                    // interrupt ai and unreserve society task
                    let society = e
                        .get(&societies)
                        .and_then(|soc| societies_res.society_by_handle(soc.handle));
                    ai.interrupt_current_action(e, None, society);

                    // next tick ai should return a new decision rather than unchanged to avoid
                    // infinite Nop loops
                    ai.clear_last_action();

                    // notify society job of completion
                    if let Some((job, task)) = activity.current_society_task.take() {
                        if let Some(TaskResult::Finished(finish)) = result {
                            job.write().notify_completion(task, finish.into());
                        }
                    }
                }
            }

            // spawn task for new activity
            if let Some(new_activity) = new_activity {
                // safety: ecs world is pinned and guaranteed to be valid as long as this system
                // is being ticked
                let world = unsafe { transmute::<Pin<&EcsWorld>, Pin<&'static EcsWorld>>(self.0) };

                let description = new_activity.description();

                let status_tx = activity.status.updater();
                let (taskref_tx, taskref_rx) = futures::channel::oneshot::channel();
                let task = runtime.spawn(taskref_tx, async move {
                    // recv task ref from runtime
                    let task = taskref_rx.await.unwrap(); // will not be cancelled

                    // create context
                    let ctx = ActivityContext::new(
                        e,
                        world,
                        task,
                        status_tx,
                        new_activity.clone(),
                    );

                    let result = new_activity.dew_it(&ctx).await;
                    match result.as_ref() {
                        Ok(_) => {
                            debug!("activity finished"; e, "activity" => ?new_activity);
                        }
                        Err(err) => {
                            debug!("activity failed"; e, "activity" => ?new_activity, "err" => %err);
                        }
                    };

                    result
                });

                activity.current = Some(ActiveTask { task, description });
            }
        }
    }
}

impl ActivityComponent {
    pub fn interrupt_with_new_activity(
        &mut self,
        action: AiAction,
        society_task: Option<(SocietyJobRef, SocietyTask)>,
    ) {
        self.new_activity = Some((action, society_task));
    }

    pub fn task(&self) -> Option<&TaskRef> {
        self.current.as_ref().map(|t| &t.task)
    }

    /// (activity description, current status)
    pub fn status(&self) -> Option<(&dyn Display, StatusRef)> {
        self.current
            .as_ref()
            .map(|t| (&*t.description, self.status.current()))
    }

    /// Exertion of current subactivity
    pub fn exertion(&self) -> f32 {
        self.status.current().exertion()
    }
}
