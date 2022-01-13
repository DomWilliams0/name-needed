use ai::{AiBox, Consideration, Considerations, Context, DecisionWeightType, Dse};

use crate::ai::consideration::ConstantConsideration;
use crate::ai::{AiAction, AiContext};

pub struct WanderDse;

impl Dse<AiContext> for WanderDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        out.add(ConstantConsideration(0.2));
    }

    fn weight_type(&self) -> DecisionWeightType {
        DecisionWeightType::Idle
    }

    fn action(&self, _: &mut <AiContext as Context>::Blackboard) -> <AiContext as Context>::Action {
        AiAction::Wander
    }
}
