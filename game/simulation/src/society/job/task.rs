use crate::activity::HaulTarget;
use ai::{Dse, WeightedDse};
use common::*;
use unit::world::WorldPosition;

use crate::ai::dse::{BreakBlockDse, HaulDse};
use crate::ai::AiContext;
use crate::ecs::{EcsWorld, Entity};
use crate::ComponentWorld;

use crate::item::HaulableItemComponent;

/// Lightweight, atomic, reservable, agnostic of the owning [SocietyJob].
#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum SocietyTask {
    BreakBlock(WorldPosition),
    Haul(Entity, HaulTarget, HaulTarget),
    // TODO PlaceBlocks(block type, at position)
}

impl SocietyTask {
    // TODO temporary box allocation is gross, use dynstack for dses
    /// More reservations = lower weight
    pub fn as_dse(
        &self,
        world: &EcsWorld,
        existing_reservations: u16,
    ) -> Option<Box<dyn Dse<AiContext>>> {
        use SocietyTask::*;

        // TODO use an equation you unmathematical twat
        let weighting = match existing_reservations {
            0 => 1.0,
            1 => 0.7,
            2 => 0.4,
            3 => 0.2,
            _ => 0.0,
        };

        macro_rules! dse {
            ($dse:expr) => {
                Some(Box::new(WeightedDse::new($dse, weighting)))
            };
        }

        match self {
            BreakBlock(range) => dse!(BreakBlockDse(*range)),
            Haul(e, src, tgt) => {
                let pos = tgt.target_position(world)?;
                let extra_hands = world
                    .component::<HaulableItemComponent>(*e)
                    .ok()
                    .map(|comp| comp.extra_hands)?;

                dse!(HaulDse {
                    thing: *e,
                    src_tgt: (*src, *tgt),
                    extra_hands_needed: extra_hands,
                    destination: pos,
                })
            }
        }
    }

    pub fn is_shareable(&self) -> bool {
        use SocietyTask::*;
        match self {
            BreakBlock(_) => true,
            // TODO some types of hauling will be shareable
            Haul(_, _, _) => false,
        }
    }
}

impl Display for SocietyTask {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use SocietyTask::*;
        match self {
            BreakBlock(b) => write!(f, "Break block at {}", b),
            Haul(e, _, tgt) => write!(f, "Haul {} to {}", e, tgt),
        }
    }
}
