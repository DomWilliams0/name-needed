use crate::ai::consideration::MyProximityToConsideration;

use crate::ai::{AiAction, AiContext};

use crate::job::{BuildDetails, SocietyJobHandle};

use ai::{Considerations, Context, DecisionWeight, Dse};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BuildDse {
    pub job: SocietyJobHandle,
    pub details: BuildDetails,
}

impl Dse<AiContext> for BuildDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        // TODO wants to work, can work
        // TODO has tool
        out.add(MyProximityToConsideration(self.details.pos.centred()));
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::Normal
    }

    fn action(&self, _blackboard: &mut <AiContext as Context>::Blackboard) -> AiAction {
        AiAction::GoBuild {
            job: self.job,
            details: self.details.clone(),
        }
    }
}
