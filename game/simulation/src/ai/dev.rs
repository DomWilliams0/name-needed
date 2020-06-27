use crate::ai::activity::AiAction;
use crate::ai::AiComponent;
use crate::ecs::*;
use common::*;

/// Divine command issued by dev mode, to be obeyed immediately
#[derive(Component)]
#[storage(HashMapStorage)]
pub struct DivineCommandComponent(pub AiAction);

/// Removes the divine command component when it has been completed
pub struct DivineCommandCompletionSystem;

impl<'a> System<'a> for DivineCommandCompletionSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, LazyUpdate>,
        ReadStorage<'a, AiComponent>,
        ReadStorage<'a, DivineCommandComponent>,
    );

    fn run(&mut self, (entities, lazy_update, ai, divine): Self::SystemData) {
        for (e, ai, divine) in (&entities, &ai, &divine).join() {
            let last = ai.last_completed_action.as_ref();
            if Some(&divine.0) == last {
                debug!("divine command has finished, removing component");
                lazy_update.remove::<DivineCommandComponent>(e);
            }
        }
    }
}
