use crate::activity::{HaulPurpose, HaulSource, HaulTarget};
use crate::ai::consideration::{HasExtraHandsForHaulingConsideration, MyProximityToConsideration};
use crate::ai::{AiAction, AiBlackboard, AiContext, AiTarget};
use crate::ecs::Entity;

use ai::{Considerations, DecisionWeight, Dse};
use unit::world::WorldPoint;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct HaulDse {
    pub thing: Entity,
    pub src_tgt: (HaulSource, HaulTarget),
    pub extra_hands_needed: u16,

    /// Position of destination haul target
    pub destination: WorldPoint,
}

impl Dse<AiContext> for HaulDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        out.add(HasExtraHandsForHaulingConsideration {
            extra_hands: self.extra_hands_needed,
            target: Some(self.thing),
        });
        out.add(MyProximityToConsideration(AiTarget::Point(
            self.destination,
        )));
        // TODO consider distance to source too
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Normal
    }

    fn action(&self, _: &mut AiBlackboard, _: Option<AiTarget>) -> AiAction {
        let (src, tgt) = self.src_tgt;
        AiAction::Haul(self.thing, src, tgt, HaulPurpose::JustBecause)
    }
}
