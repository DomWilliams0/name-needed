use ai::{Considerations, DecisionWeight, Dse};

use crate::ai::consideration::IsFarFromHerdLeaderConsideration;
use crate::ai::{AiBlackboard, AiContext, AiTarget};
use crate::AiAction;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct StayCloseToHerdDse;

impl Dse<AiContext> for StayCloseToHerdDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        out.add(IsFarFromHerdLeaderConsideration);
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Idle
    }

    fn action(&self, _: &mut AiBlackboard, _: Option<AiTarget>) -> AiAction {
        AiAction::ReturnToHerd
    }
}
