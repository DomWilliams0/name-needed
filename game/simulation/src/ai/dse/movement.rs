use ai::{AiBox, Consideration, Context, DecisionWeight, Dse};

use crate::ai::activity::AiAction;
use crate::ai::consideration::ConstantConsideration;
use crate::ai::AiContext;

pub struct WanderDse;

impl Dse<AiContext> for WanderDse {
    fn name(&self) -> &str {
        "Wander"
    }

    fn considerations(&self) -> Vec<AiBox<dyn Consideration<AiContext>>> {
        vec![AiBox::new(ConstantConsideration(0.2))]
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Idle
    }

    fn action(&self, _: &mut <AiContext as Context>::Blackboard) -> <AiContext as Context>::Action {
        AiAction::Wander
    }
}
