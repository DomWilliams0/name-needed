use ai::{Dse, WeightedDse};
use common::*;
use unit::world::WorldPosition;

use crate::activity::HaulTarget;
use crate::ai::dse::{BreakBlockDse, HaulDse};
use crate::ai::AiContext;
use crate::ecs::{EcsWorld, Entity};
use crate::ComponentWorld;

use crate::item::HaulableItemComponent;
use crate::society::work_item::WorkItemRef;

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct HaulSocietyTask {
    pub item: Entity,
    pub src: HaulTarget,
    pub dst: HaulTarget,
}

/// Lightweight, atomic, reservable, agnostic of the owning [SocietyJob].
#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum SocietyTask {
    BreakBlock(WorldPosition),
    /// Boxed as this variant is much larger than the rest
    Haul(Box<HaulSocietyTask>),
    WorkOnWorkItem(WorkItemRef),
}

impl SocietyTask {
    pub fn haul(item: Entity, src: HaulTarget, dst: HaulTarget) -> Self {
        Self::Haul(Box::new(HaulSocietyTask { item, src, dst }))
    }

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
            Haul(haul) => {
                let pos = haul.dst.target_position(world)?;
                let extra_hands = world
                    .component::<HaulableItemComponent>(haul.item)
                    .ok()
                    .map(|comp| comp.extra_hands)?;

                dse!(HaulDse {
                    thing: haul.item,
                    src_tgt: (haul.src, haul.dst),
                    extra_hands_needed: extra_hands,
                    destination: pos,
                })
            }
            WorkOnWorkItem(h) => todo!(), // TODO
        }
    }

    /// TODO add limit on number of shares
    pub fn is_shareable(&self) -> bool {
        use SocietyTask::*;
        match self {
            BreakBlock(_) => true,
            // TODO some types of hauling will be shareable
            // TODO depends on work item
            Haul(_) | WorkOnWorkItem(_) => false,
        }
    }
}

impl Display for SocietyTask {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use SocietyTask::*;
        match self {
            BreakBlock(b) => write!(f, "Break block at {}", b),
            Haul(haul) => Display::fmt(haul, f),
            WorkOnWorkItem(wi) => write!(f, "Work on {}", wi),
        }
    }
}

impl Display for HaulSocietyTask {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Haul {} to {}", self.item, self.dst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_task_struct() {
        let haul_size = dbg!(std::mem::size_of::<HaulSocietyTask>());
        let task_size = dbg!(std::mem::size_of::<SocietyTask>());

        assert!(task_size < 32);

        let sz_diff = haul_size as f64 / task_size as f64;
        assert!(sz_diff > 1.5); // yuge!
    }
}
