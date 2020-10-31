use crate::activity::HaulTarget;
use crate::ai::dse::{BreakBlockDse, HaulDse};
use crate::ai::AiContext;
use crate::ecs::{EcsWorld, Entity};
use crate::item::HaulableItemComponent;
use crate::ComponentWorld;
use ai::Dse;
use unit::world::WorldPosition;

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum Task {
    BreakBlock(WorldPosition),
    Haul(Entity, HaulTarget, HaulTarget),
    // TODO PlaceBlocks(block type, at position)
}

impl Task {
    // TODO temporary box allocation is gross, use dynstack for dses
    pub fn as_dse(&self, world: &EcsWorld) -> Option<Box<dyn Dse<AiContext>>> {
        match self {
            Task::BreakBlock(range) => Some(Box::new(BreakBlockDse(*range))),
            Task::Haul(e, src, tgt) => {
                let pos = tgt.target_position(world)?;
                let extra_hands = world
                    .component::<HaulableItemComponent>(*e)
                    .ok()
                    .map(|comp| comp.extra_hands)?;
                Some(Box::new(HaulDse {
                    thing: *e,
                    src_tgt: (src.clone(), tgt.clone()),
                    extra_hands_needed: extra_hands,
                    destination: pos,
                }))
            }
        }
    }
}
