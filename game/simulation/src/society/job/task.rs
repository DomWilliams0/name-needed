use ai::WeightedDse;

use common::*;
use std::num::NonZeroU16;
use unit::world::WorldPosition;

use crate::activity::HaulTarget;
use crate::ai::dse::{BreakBlockDse, BuildDse, GatherMaterialsDse, HaulDse};
use crate::ai::AiContext;
use crate::build::BuildMaterial;
use crate::ecs::{EcsWorld, Entity};
use crate::item::HaulableItemComponent;
use crate::job::{BuildDetails, SocietyJobHandle};
use crate::{ComponentWorld, HaulSource};

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct HaulSocietyTask {
    pub item: Entity,
    pub src: HaulSource,
    pub dst: HaulTarget,
}

/// Lightweight, atomic, reservable, agnostic of the owning [SocietyJob]. These map to DSEs that
/// are each considered separately by the AI
#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum SocietyTask {
    /// Break the given block
    BreakBlock(WorldPosition),

    /// Work on a build job
    Build(SocietyJobHandle, BuildDetails),

    /// Gather materials for a build at the given position
    GatherMaterials {
        target: WorldPosition,
        material: BuildMaterial,
        job: SocietyJobHandle,
        extra_hands_needed_for_haul: u16,
    },

    /// Haul something.
    /// Boxed as this variant is much larger than the rest
    Haul(Box<HaulSocietyTask>),
}

impl SocietyTask {
    pub fn haul(item: Entity, src: HaulSource, dst: HaulTarget) -> Self {
        Self::Haul(Box::new(HaulSocietyTask { item, src, dst }))
    }

    // TODO temporary box allocation is gross, use dynstack for dses
    /// More reservations = lower weight
    #[allow(unused_variables)] // used in macro
    pub fn as_dse(
        &self,
        world: &EcsWorld,
        existing_reservations: u16,
    ) -> Option<WeightedDse<AiContext>> {
        use SocietyTask::*;

        let weighting = match existing_reservations {
            0 => 1.0,
            1 => 0.9,
            2 => 0.8,
            3 => 0.75,
            _ => return None, // don't even bother
        };

        // macro_rules! dse_as_boxed_trait {
        //     ($dse:expr) => {{
        //         let boxed = BumpBox::new_in($dse, alloc);
        //         // safety: normal safe cast allowed with Box but not custom BumpBox
        //         unsafe { BumpBox::from_raw(BumpBox::into_raw(boxed) as *mut dyn Dse<AiContext>) }
        //     }};
        // }

        macro_rules! dse {
            ($dse:expr) => {
                Some(WeightedDse::new($dse, weighting))
            };
        }

        match self {
            BreakBlock(range) => dse!(BreakBlockDse(*range)),
            Build(job, details) => {
                // TODO distinct build actions e.g. sawing, wood building, stone building etc
                dse!(BuildDse {
                    job: *job,
                    details: details.clone(),
                })
            }
            GatherMaterials {
                target,
                material,
                job,
                extra_hands_needed_for_haul,
            } => dse!(GatherMaterialsDse {
                target: *target,
                material: material.clone(),
                job: *job,
                extra_hands_for_haul: *extra_hands_needed_for_haul
            }),
            Haul(haul) => {
                let pos = haul.dst.location(world)?;
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
        }
    }

    pub fn max_workers(&self) -> NonZeroU16 {
        use SocietyTask::*;
        let n = match self {
            BreakBlock(_) => 1,
            Build(_, _) => 1,
            // TODO some types of hauling will be shareable
            // TODO depends on work item
            Haul(_) => 1,
            GatherMaterials { .. } => 3,
        };

        NonZeroU16::new(n).unwrap()
    }
}

impl Display for SocietyTask {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use SocietyTask::*;
        match self {
            BreakBlock(b) => write!(f, "Break block at {}", b),
            Build(_, details) => write!(f, "Build {:?} at {}", details.target, details.pos),
            Haul(haul) => Display::fmt(haul, f),
            // TODO include a description field for proper description e.g. "cutting log", "building wall"
            GatherMaterials { material, .. } => write!(f, "Gather {:?}", material),
        }
    }
}

impl Display for HaulSocietyTask {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Haul {} to {}", self.item, self.dst)
    }
}
