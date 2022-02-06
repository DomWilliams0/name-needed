use ai::{Considerations, DecisionWeight, Dse};

use crate::ai::consideration::ConstantConsideration;
use crate::ai::{AiAction, AiBlackboard, AiContext, AiTarget};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct WanderDse;

impl Dse<AiContext> for WanderDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        out.add(ConstantConsideration(0.01));
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Idle
    }

    fn action(&self, _: &mut AiBlackboard, _: Option<AiTarget>) -> AiAction {
        AiAction::Wander
    }
}
