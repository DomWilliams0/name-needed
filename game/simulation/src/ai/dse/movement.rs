use ai::{AiBox, Consideration, Context, DecisionWeightType, Dse};

use crate::ai::consideration::ConstantConsideration;
use crate::ai::{AiAction, AiContext};

pub struct WanderDse;

impl Dse<AiContext> for WanderDse {
    fn name(&self) -> &'static str {
        "Wander"
    }

    fn considerations(&self) -> Vec<AiBox<dyn Consideration<AiContext>>> {
        vec![AiBox::new(ConstantConsideration(0.2))]
    }

    fn weight_type(&self) -> DecisionWeightType {
        DecisionWeightType::Idle
    }

    fn action(&self, _: &mut <AiContext as Context>::Blackboard) -> <AiContext as Context>::Action {
        AiAction::Wander
    }
}
