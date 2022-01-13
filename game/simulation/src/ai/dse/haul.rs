use crate::activity::{HaulPurpose, HaulSource, HaulTarget};
use crate::ai::consideration::{
    HasExtraHandsForHaulingConsideration, MyProximityToConsideration, Proximity,
};
use crate::ai::{AiAction, AiContext};
use crate::ecs::Entity;
use crate::item::ItemFilter;
use ai::{Considerations, Context, DecisionWeightType, Dse};
use unit::world::WorldPoint;

pub struct HaulDse {
    pub thing: Entity,
    pub src_tgt: (HaulSource, HaulTarget),
    pub extra_hands_needed: u16,

    /// Position of destination haul target
    pub destination: WorldPoint,
}

impl Dse<AiContext> for HaulDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        out.add(HasExtraHandsForHaulingConsideration(
            self.extra_hands_needed,
            Some(ItemFilter::SpecificEntity(self.thing)),
        ));
        out.add(MyProximityToConsideration {
            target: self.destination,
            proximity: Proximity::Walkable,
        });
        // TODO consider distance to source too
    }

    fn weight_type(&self) -> DecisionWeightType {
        DecisionWeightType::Normal
    }

    fn action(&self, _: &mut <AiContext as Context>::Blackboard) -> <AiContext as Context>::Action {
        let (src, tgt) = self.src_tgt;
        AiAction::Haul(self.thing, src, tgt, HaulPurpose::JustBecause)
    }
}
