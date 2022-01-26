use ai::{Considerations, Context, DecisionWeight, Dse};

use crate::ai::consideration::ConstantConsideration;
use crate::ai::{AiAction, AiContext};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct WanderDse;

impl Dse<AiContext> for WanderDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        out.add(ConstantConsideration(0.2));
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Idle
    }

    fn action(&self, _: &mut <AiContext as Context>::Blackboard) -> <AiContext as Context>::Action {
        AiAction::Wander
    }
}
