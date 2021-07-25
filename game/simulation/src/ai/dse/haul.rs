use crate::activity::HaulTarget;
use crate::ai::consideration::{HasExtraHandsForHaulingConsideration, MyProximityToConsideration};
use crate::ai::{AiAction, AiContext};
use crate::ecs::Entity;
use ai::{AiBox, Consideration, Context, DecisionWeightType, Dse};
use unit::world::WorldPoint;

pub struct HaulDse {
    pub thing: Entity,
    pub src_tgt: (HaulTarget, HaulTarget),
    pub extra_hands_needed: u16,

    /// Position of destination haul target
    pub destination: WorldPoint,
}

impl Dse<AiContext> for HaulDse {
    fn name(&self) -> &'static str {
        "Haul"
    }

    fn considerations(&self) -> Vec<AiBox<dyn Consideration<AiContext>>> {
        vec![
            AiBox::new(HasExtraHandsForHaulingConsideration(
                self.extra_hands_needed,
                self.thing,
            )),
            AiBox::new(MyProximityToConsideration {
                target: self.destination.floor(),
                max_distance: 5000.0, // squared
            }),
            // TODO consider distance to source too
        ]
    }

    fn weight_type(&self) -> DecisionWeightType {
        DecisionWeightType::Normal
    }

    fn action(&self, _: &mut <AiContext as Context>::Blackboard) -> <AiContext as Context>::Action {
        let (src, tgt) = self.src_tgt;
        AiAction::Haul(self.thing, src, tgt)
    }
}
