use crate::ai::activity::AiAction;
use crate::ai::AiComponent;
use crate::ecs::*;
use crate::queued_update::QueuedUpdates;
use crate::TransformComponent;
use common::*;
use world::block::BlockType;
use world::WorldRef;

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
        Read<'a, QueuedUpdates>,
        Read<'a, WorldRef>,
        ReadStorage<'a, DivineCommandComponent>,
        ReadStorage<'a, TransformComponent>,
    );

    fn run(&mut self, (entities, lazy_update, update, world, divine, transform): Self::SystemData) {
        for (e, divine, transform) in (&entities, &divine, transform.maybe()).join() {
            if divine.is_complete(transform, &world) {
                debug!("divine command complete ({:?})", divine.0);

                // remove component
                lazy_update.remove::<DivineCommandComponent>(e);

                // remove dse
                update.queue("remove divine command dse", move |world| {
                    if let Ok(ai) = world.component_mut::<AiComponent>(e) {
                        ai.remove_divine_command();
                    }

                    Ok(())
                });
            }
        }
    }
}

impl DivineCommandComponent {
    fn is_complete(&self, transform: Option<&TransformComponent>, world: &Read<WorldRef>) -> bool {
        match &self.0 {
            AiAction::Goto(target) => {
                let my_pos = match transform {
                    None => {
                        warn!("missing transform for goto divine command");
                        return true;
                    }
                    Some(t) => t.position,
                };
                my_pos.is_almost(target, 1.5)
            }
            AiAction::GoBreakBlock(block) => {
                let w = world.borrow();
                w.block(*block)
                    .map(|b| b.block_type() == BlockType::Air)
                    .unwrap_or(false)
            }

            AiAction::GoPickUp(_) => todo!("pickup not implemented as a divine command"),
            a => {
                warn!("command not available as a divine command: {:?}", a);
                true
            }
        }
    }
}
