use std::collections::HashMap;

use ai::{AiBox, Intelligence, IntelligentDecision, Smarts};
use common::*;

use crate::ai::activity::{Activity, ActivityContext, ActivityResult, Finish, NopActivity};
use crate::ai::dse::human_dses;
use crate::ai::{AiAction, AiContext, Blackboard, SharedBlackboard};
use crate::ecs::*;
use crate::item::InventoryComponent;
use crate::needs::HungerComponent;
use crate::queued_update::QueuedUpdates;
use crate::simulation::Tick;
use crate::TransformComponent;

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct AiComponent {
    pub intelligence: ai::Intelligence<AiContext>,
    pub last_completed_action: Option<AiAction>,
    current_action: Option<AiAction>,
}

impl AiComponent {
    pub fn human() -> Self {
        Self {
            intelligence: Intelligence::new(Smarts::new(human_dses()).expect("empty human DSEs")),
            last_completed_action: None,
            current_action: None,
        }
    }
}

pub struct AiSystem;

impl<'a> System<'a> for AiSystem {
    type SystemData = (
        Read<'a, Tick>,
        Read<'a, EntitiesRes>,
        Read<'a, EcsWorldFrameRef>,
        Write<'a, QueuedUpdates>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, HungerComponent>,
        ReadStorage<'a, InventoryComponent>,
        WriteStorage<'a, AiComponent>,
        WriteStorage<'a, ActivityComponent>,
    );

    fn run(
        &mut self,
        (tick, entities, ecs_world, updates, transform, hunger, inventory, mut ai, mut activity): Self::SystemData,
    ) {
        // TODO only run occasionally - FIXME TERRIBLE HACK
        if tick.0 % 10 != 0 {
            return;
        }

        let ecs_world: &EcsWorld = &*ecs_world;

        let mut shared_bb = SharedBlackboard {
            area_link_cache: HashMap::new(),
        };

        for (e, transform, hunger, ai, activity) in
            (&entities, &transform, &hunger, &mut ai, &mut activity).join()
        {
            // initialize blackboard
            // TODO use arena/bump allocator and share instance between entities
            let mut bb = Blackboard {
                entity: e,
                position: transform.position,
                hunger: hunger.hunger(),
                inventory_search_cache: HashMap::new(),
                local_area_search_cache: HashMap::new(),
                inventory: inventory.get(e),
                world: ecs_world,
                shared: &mut shared_bb,
            };

            // Safety: can't use true lifetime on Blackboard so using 'static and transmuting until
            // we get our GATs
            let bb_ref: &mut Blackboard = unsafe { std::mem::transmute(&mut bb) };
            let ctx = ActivityContext {
                entity: e,
                world: ecs_world,
                updates: &updates,
            };

            // choose best action
            match ai.intelligence.choose(bb_ref) {
                IntelligentDecision::New { dse, action } => {
                    debug!("{:?}: new activity: {}", e, dse.name());
                    trace!("activity: {:?}", action);

                    let (mut old, new) = {
                        let new_activity = action.clone().into();
                        let old_activity = std::mem::replace(&mut activity.current, new_activity);
                        (old_activity, &mut activity.current)
                    };

                    ai.current_action = Some(action);
                    ai.last_completed_action = None; // interrupted

                    old.on_finish(Finish::Interrupted, &ctx);
                    new.on_start(&ctx)
                }
                IntelligentDecision::Unchanged => {
                    let result = activity.current.on_tick(&ctx);

                    if let ActivityResult::Finished(finish) = result {
                        let new = AiBox::new(NopActivity);
                        let mut old = std::mem::replace(&mut activity.current, new);

                        old.on_finish(finish, &ctx);
                        // no need to nop.on_start()

                        ai.last_completed_action = std::mem::take(&mut ai.current_action);
                    }
                }
            }
        }
    }
}

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct ActivityComponent {
    pub current: AiBox<dyn Activity<EcsWorld>>,
}

impl Default for ActivityComponent {
    fn default() -> Self {
        Self {
            current: AiBox::new(NopActivity),
        }
    }
}
